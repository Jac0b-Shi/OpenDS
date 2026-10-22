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
# Use this host's built-ins even when a parent PS7 process passes its module path
# to Windows PowerShell 5.1 (the runtime used by the GUI).
Import-Module (Join-Path $PSHOME 'Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1')
Import-Module (Join-Path $PSHOME 'Modules/Microsoft.PowerShell.Security/Microsoft.PowerShell.Security.psd1')
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
# PS5 writes a JSON array as one pipeline object; wrapping it in @() nests it.
$assets = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'drivers.json') -Raw | ConvertFrom-Json
. (Join-Path $PSScriptRoot 'Driver-Download.ps1')

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
        Save-DriverAsset $asset $destination $Proxy
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
