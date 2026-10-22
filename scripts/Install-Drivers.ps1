# Requires Windows PowerShell 5.1 or PowerShell 7. Download-only unless -Install is given.
[CmdletBinding()]
param(
    [ValidateSet('all','usbip','hidhide')][string]$Component = 'all',
    [switch]$Install,
    [string]$Proxy,
    [string]$Cache = (Join-Path $env:LOCALAPPDATA 'OpenDS\Downloads'),
    [string]$ManualDirectory
)
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$assets = @(Get-Content -LiteralPath (Join-Path $PSScriptRoot 'drivers.json') -Raw | ConvertFrom-Json)

# Best-effort PAC parsing: pull the first usable PROXY/HTTPS directive out of a
# PAC file. Real PAC scripts can branch per URL, which this cannot evaluate; the
# download path always falls back to a direct attempt, so a miss is harmless.
function Resolve-PacProxy([string]$ConfigUrl) {
    try {
        $text = if ($ConfigUrl -like 'file://*') {
            Get-Content -LiteralPath ([uri]$ConfigUrl).LocalPath -Raw -ErrorAction Stop
        } else {
            (Invoke-WebRequest -UseBasicParsing -Uri $ConfigUrl -TimeoutSec 15).Content
        }
        if (-not $text) { return $null }
        $found = [regex]::Matches($text, '(?i)\b(?:PROXY|HTTPS)\s+([A-Za-z0-9\.\-]+:\d+)')
        foreach ($m in $found) {
            $candidate = $m.Groups[1].Value
            if ($candidate) { return 'http://' + $candidate }
        }
    } catch { }
    return $null
}

# Discover a proxy when the caller did not pass one. A missing proxy is NOT an
# error: many networks reach the release hosts directly, so detection only
# decides which attempt runs first.
if ([string]::IsNullOrWhiteSpace($Proxy)) {
    foreach ($key in @('HTTPS_PROXY','HTTP_PROXY','ALL_PROXY')) {
        $candidate = [Environment]::GetEnvironmentVariable($key)
        if ($candidate) { $Proxy = $candidate; break }
    }
}
if ([string]::IsNullOrWhiteSpace($Proxy)) {
    $settings = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' -ErrorAction SilentlyContinue
    if ($settings.ProxyEnable -and $settings.ProxyServer) {
        $server = [string]$settings.ProxyServer
        if ($server.Contains('=')) {
            $entry = @($server.Split(';') | Where-Object { $_ -like 'https=*' } | Select-Object -First 1)
            if (!$entry.Count) { $entry = @($server.Split(';') | Where-Object { $_ -like 'http=*' } | Select-Object -First 1) }
            $server = if ($entry.Count) { $entry[0].Split('=',2)[1] } else { '' }
        }
        if ($server) { $Proxy = $server }
    } elseif ($settings.AutoConfigURL) {
        $Proxy = Resolve-PacProxy ([string]$settings.AutoConfigURL)
    }
}
# Normalize once, so -Proxy 127.0.0.1:7890 and environment variables behave the
# same as the registry value. Invoke-WebRequest requires a scheme.
if ($Proxy -and $Proxy -notmatch '^\w+://') { $Proxy = 'http://' + $Proxy }

function Test-Asset($Item, [string]$Path) {
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -ine $Item.sha256) {
        throw "SHA256 mismatch: $Path. Remove the file and download the pinned release again."
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notlike ('*' + $Item.publisher + '*')) {
        throw "Signature or publisher verification failed: $Path ($($signature.Status)). Installer was NOT started."
    }
}

# Try the configured proxy first, then a direct connection. Only when every
# attempt fails does the script ask the user to download by hand.
function Save-Asset($Item, [string]$Destination) {
    $attempts = @()
    if ($Proxy) { $attempts += @{ Label = "proxy $Proxy"; Proxy = $Proxy } }
    $attempts += @{ Label = 'direct connection'; Proxy = $null }

    $partial = $Destination + '.partial-' + [guid]::NewGuid().ToString('N')
    $failures = @()
    foreach ($attempt in $attempts) {
        Write-Host "Downloading $($Item.file) via $($attempt.Label)..."
        $saved = [Net.WebRequest]::DefaultWebProxy
        try {
            [Net.WebRequest]::DefaultWebProxy = if ($attempt.Proxy) { New-Object Net.WebProxy($attempt.Proxy) } else { $null }
            Invoke-WebRequest -UseBasicParsing -Uri $Item.url -OutFile $partial -TimeoutSec 180
            Test-Asset $Item $partial
            Move-Item -LiteralPath $partial -Destination $Destination
            return
        } catch {
            $failures += "  $($attempt.Label): $($_.Exception.Message)"
        } finally {
            [Net.WebRequest]::DefaultWebProxy = $saved
            if (Test-Path -LiteralPath $partial) { Remove-Item -LiteralPath $partial -Force -ErrorAction SilentlyContinue }
        }
    }

    throw ("Could not download $($Item.file).`n" + ($failures -join "`n") +
        "`n`nDownload it manually: $($Item.url)`nSave as: $Destination`n" +
        "Then rerun with -ManualDirectory <folder>, or pass -Proxy http://127.0.0.1:PORT.")
}

New-Item -ItemType Directory -Force -Path $Cache | Out-Null
foreach ($asset in $assets) {
    if ($Component -ne 'all' -and $Component -ne $asset.id) { continue }
    $destination = Join-Path $Cache $asset.file
    if ($ManualDirectory) {
        $destination = Join-Path (Resolve-Path -LiteralPath $ManualDirectory).Path $asset.file
        if (!(Test-Path -LiteralPath $destination -PathType Leaf)) { throw "Download manually: $($asset.url)`nSave as: $destination" }
    } elseif (!(Test-Path -LiteralPath $destination -PathType Leaf)) {
        Save-Asset $asset $destination
    }
    Test-Asset $asset $destination
    Write-Host "Verified SHA256 and publisher: $($asset.file)"
    if ($Install) {
        Write-Host 'Opening the official installer. Follow its prompts and restart Windows if requested.'
        $options = @{ FilePath=$destination; Verb='RunAs'; Wait=$true; PassThru=$true }
        if ($asset.arguments) { $options.ArgumentList = $asset.arguments }
        $process = Start-Process @options
        if ($process.ExitCode -notin @(0,1641,3010)) { throw "Installer exited with code $($process.ExitCode)." }
        Write-Host "Installer exit code: $($process.ExitCode)"
    }
}
Write-Host 'Done. Recheck dependencies in OpenDS; restart Windows if the installer requests it.'
