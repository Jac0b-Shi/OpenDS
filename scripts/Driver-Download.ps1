# Shared by the installer and its network regression tests. No downloads on load.
Add-Type -AssemblyName System.Net.Http

function Get-DriverProxySetting([string]$Proxy) {
    if (![string]::IsNullOrWhiteSpace($Proxy)) { return $Proxy.Trim() }
    foreach ($key in @('HTTPS_PROXY','HTTP_PROXY','ALL_PROXY')) {
        $candidate = [Environment]::GetEnvironmentVariable($key)
        if (![string]::IsNullOrWhiteSpace($candidate)) { return $candidate.Trim() }
    }
    return $null
}

function New-DriverSystemProxy {
    # Let Windows/.NET evaluate PAC/WPAD and bypass rules for each destination,
    # including redirects. Do not scrape a proxy address from PAC JavaScript.
    return [Net.WebRequest]::GetSystemWebProxy()
}

function Receive-DriverDownload {
    param(
        [uri]$Uri,
        [string]$OutFile,
        [ValidateSet('Explicit','System','Direct')][string]$Route,
        [string]$Proxy,
        [int]$TimeoutSeconds = 180
    )
    $handler = New-Object Net.Http.HttpClientHandler
    $client = $null
    $response = $null
    $inputStream = $null
    $outputStream = $null
    $cancelResponse = $null
    $deadline = New-Object Threading.CancellationTokenSource
    try {
        if ($Route -eq 'Direct') {
            # Per-handler control works in both Windows PowerShell 5.1 and PS7;
            # neither environment variables nor the system proxy can override it.
            $handler.UseProxy = $false
        } elseif ($Route -eq 'Explicit') {
            if ($Proxy -notmatch '^\w+://') { $Proxy = 'http://' + $Proxy }
            $handler.Proxy = New-Object Net.WebProxy($Proxy)
        } else {
            $handler.Proxy = New-DriverSystemProxy
        }
        $client = New-Object Net.Http.HttpClient($handler)
        $client.Timeout = [TimeSpan]::FromSeconds($TimeoutSeconds)
        $client.DefaultRequestHeaders.UserAgent.ParseAdd('OpenDS/0.1')
        $deadline.CancelAfter([TimeSpan]::FromSeconds($TimeoutSeconds))
        $response = $client.GetAsync($Uri, [Net.Http.HttpCompletionOption]::ResponseHeadersRead, $deadline.Token).GetAwaiter().GetResult()
        # .NET Framework streams may ignore a ReadAsync cancellation token after
        # headers arrive. Closing the response aborts a stalled body read too.
        # This is a native .NET delegate: timer callbacks need no PS runspace.
        $closeResponse = [Delegate]::CreateDelegate([Action], $response, 'Dispose')
        $cancelResponse = $deadline.Token.Register([Action]$closeResponse)
        [void]$response.EnsureSuccessStatusCode()
        $inputStream = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
        $outputStream = [IO.File]::Open($OutFile, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        [void]$inputStream.CopyToAsync($outputStream, 81920, $deadline.Token).GetAwaiter().GetResult()
    } finally {
        if ($null -ne $cancelResponse) { $cancelResponse.Dispose() }
        if ($outputStream) { $outputStream.Dispose() }
        if ($inputStream) { $inputStream.Dispose() }
        if ($response) { $response.Dispose() }
        if ($client) { $client.Dispose() } else { $handler.Dispose() }
        $deadline.Dispose()
    }
}

function Save-DriverAsset($Item, [string]$Destination, [string]$Proxy) {
    $proxySetting = Get-DriverProxySetting $Proxy
    $attempts = @()
    # Labels deliberately omit proxy URLs, which can contain credentials.
    if ($proxySetting) { $attempts += @{ Label='configured proxy'; Route='Explicit'; Proxy=$proxySetting } }
    $attempts += @{ Label='Windows proxy/PAC settings (or direct)'; Route='System'; Proxy=$null }
    $attempts += @{ Label='direct connection'; Route='Direct'; Proxy=$null }
    $partial = Join-Path (Split-Path -Parent $Destination) ('.partial-' + [guid]::NewGuid().ToString('N') + '-' + [IO.Path]::GetFileName($Destination))
    $failures = @()
    foreach ($attempt in $attempts) {
        Write-Host "Downloading $($Item.file) via $($attempt.Label)..."
        try {
            Receive-DriverDownload -Uri $Item.url -OutFile $partial -Route $attempt.Route -Proxy $attempt.Proxy
            Test-Asset $Item $partial
            Move-Item -LiteralPath $partial -Destination $Destination
            return
        } catch {
            $failures += "  $($attempt.Label): $($_.Exception.Message)"
        } finally {
            if (Test-Path -LiteralPath $partial) { Remove-Item -LiteralPath $partial -ErrorAction SilentlyContinue }
        }
    }
    throw ("Could not download $($Item.file).`n" + ($failures -join "`n") +
        "`n`nDownload it manually: $($Item.url)`nSave as: $Destination`n" +
        "Then rerun with -ManualDirectory <folder>, or pass -Proxy http://127.0.0.1:PORT.")
}
