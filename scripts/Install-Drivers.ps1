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
        if ($server) { $Proxy = if ($server -match '^\w+://') { $server } else { 'http://' + $server } }
    }
}
function Test-Asset($Item, [string]$Path) {
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -ine $Item.sha256) {
        throw "SHA256 mismatch: $Path. Remove the file and download the pinned release again."
    }
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notlike ('*' + $Item.publisher + '*')) {
        throw "Signature or publisher verification failed: $Path ($($signature.Status)). Installer was NOT started."
    }
}
New-Item -ItemType Directory -Force -Path $Cache | Out-Null
foreach ($asset in $assets) {
    if ($Component -ne 'all' -and $Component -ne $asset.id) { continue }
    $destination = Join-Path $Cache $asset.file
    if ($ManualDirectory) {
        $destination = Join-Path (Resolve-Path -LiteralPath $ManualDirectory).Path $asset.file
        if (!(Test-Path -LiteralPath $destination -PathType Leaf)) { throw "Download manually: $($asset.url)`nSave as: $destination" }
    } elseif (!(Test-Path -LiteralPath $destination -PathType Leaf)) {
        if (!$Proxy) {
            throw "No proxy found. Download manually: $($asset.url)`nThen use -ManualDirectory <folder>, or specify -Proxy http://127.0.0.1:PORT."
        }
        Write-Host "Downloading $($asset.file) through configured proxy..."
        $partial = $destination + '.partial-' + [guid]::NewGuid().ToString('N')
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $asset.url -Proxy $Proxy -OutFile $partial -TimeoutSec 120
            Test-Asset $asset $partial
            Move-Item -LiteralPath $partial -Destination $destination
        } finally {
            if (Test-Path -LiteralPath $partial) { Remove-Item -LiteralPath $partial }
        }
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
