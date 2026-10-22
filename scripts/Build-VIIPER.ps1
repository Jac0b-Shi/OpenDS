[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Compiler,
    [string]$Proxy,
    [switch]$Offline
)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$compilerPath = (Resolve-Path -LiteralPath $Compiler).Path
$names = @('CC','CGO_ENABLED','GOTOOLCHAIN','GOPROXY','HTTP_PROXY','HTTPS_PROXY')
$saved = @{}
foreach ($name in $names) { $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
Push-Location (Join-Path $root 'vendor/viiper-source')
try {
    $env:CC = $compilerPath
    $env:CGO_ENABLED = '1'
    $env:GOTOOLCHAIN = 'auto'
    if ($Offline) {
        $env:GOPROXY = 'off'
    } else {
        if (!$Proxy) { throw 'Use -Proxy for downloads, or -Offline with cached Go dependencies.' }
        $env:HTTP_PROXY = $Proxy
        $env:HTTPS_PROXY = $Proxy
        $env:GOPROXY = 'https://goproxy.cn'
    }
    & go test ./device/dualsense ./usb ./usbip
    if ($LASTEXITCODE -ne 0) { throw 'VIIPER profile tests failed.' }
    $output = Join-Path $root 'target/viiper-build/libVIIPER.dll'
    New-Item -ItemType Directory -Force (Split-Path $output) | Out-Null
    & go build -buildmode=c-shared -trimpath -ldflags '-s -w' -o $output ./lib/viiper
    if ($LASTEXITCODE -ne 0) { throw 'VIIPER build failed.' }
    Copy-Item -LiteralPath $output -Destination (Join-Path $root 'vendor/viiper/libVIIPER.dll') -Force
    Get-FileHash -LiteralPath $output -Algorithm SHA256
} finally {
    Pop-Location
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
}
