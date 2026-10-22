[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Compiler,
    [string]$Proxy,
    [switch]$Offline,
    [switch]$VerifyReproducible
)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$compilerPath = (Resolve-Path -LiteralPath $Compiler).Path
$names = @('CC','CGO_ENABLED','GOOS','GOARCH','GOTOOLCHAIN','GOPROXY','HTTP_PROXY','HTTPS_PROXY')
$saved = @{}
foreach ($name in $names) { $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
Push-Location (Join-Path $root 'vendor/viiper-source')
try {
    $env:CC = $compilerPath
    $env:CGO_ENABLED = '1'
    $env:GOOS = 'windows'
    $env:GOARCH = 'amd64'
    $env:GOTOOLCHAIN = 'auto'
    if ($Offline) {
        $env:GOPROXY = 'off'
    } else {
        if (!$Proxy) { throw 'Use -Proxy for downloads, or -Offline with cached Go dependencies.' }
        $env:HTTP_PROXY = $Proxy
        $env:HTTPS_PROXY = $Proxy
        $env:GOPROXY = 'https://goproxy.cn'
    }
    & go test ./device/dualsense ./internal/server/usb ./usb ./usbip
    if ($LASTEXITCODE -ne 0) { throw 'VIIPER profile tests failed.' }
    $output = Join-Path $root 'target/viiper-build/libVIIPER.dll'
    New-Item -ItemType Directory -Force (Split-Path $output) | Out-Null
    # Remove checkout identity, Go build ID and the PE linker timestamp.
    $buildArgs = @('build','-buildmode=c-shared','-trimpath','-buildvcs=false','-ldflags','-s -w -buildid= -extldflags=-Wl,--no-insert-timestamp')
    & go @buildArgs -o $output ./lib/viiper
    if ($LASTEXITCODE -ne 0) { throw 'VIIPER build failed.' }
    $hash = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($VerifyReproducible) {
        $checkOutput = Join-Path $root 'target/viiper-rebuild/libVIIPER.dll'
        New-Item -ItemType Directory -Force (Split-Path $checkOutput) | Out-Null
        # -a forces recompilation instead of copying the cached linked artifact.
        & go @buildArgs -a -o $checkOutput ./lib/viiper
        if ($LASTEXITCODE -ne 0) { throw 'Independent VIIPER rebuild failed.' }
        $checkHash = (Get-FileHash -LiteralPath $checkOutput -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($hash -ne $checkHash) { throw "VIIPER rebuild differs: $hash / $checkHash" }
    }
    $sourceHash = & (Join-Path $PSScriptRoot 'Get-VIIPERSourceHash.ps1')
    $goVersion = (& go version) -join ' '
    $compilerVersion = (& $compilerPath --version | Select-Object -First 1)
    $info = [ordered]@{
        schema_version = 1
        upstream_commit = '25515e1ef405f189b73d5e2707a4fbb0798e9110'
        source_sha256 = $sourceHash
        source_hash_rules = 'SHA256 of sorted UTF-8 lines: file SHA256, two spaces, relative path, LF. UTF-8 text without NUL has CRLF normalized to LF; OPENDS-PATCH.md is excluded to avoid a hash cycle.'
        go_version = $goVersion
        c_compiler = $compilerVersion
        goos = $env:GOOS
        goarch = $env:GOARCH
        cgo_enabled = $env:CGO_ENABLED
        build_args = @($buildArgs + @('-o','libVIIPER.dll','./lib/viiper'))
        dll_sha256 = $hash
        independent_rebuild_matched = [bool]$VerifyReproducible
    }
    $utf8 = New-Object Text.UTF8Encoding($false)
    [IO.File]::WriteAllText((Join-Path $root 'vendor/viiper/build-info.json'), (($info | ConvertTo-Json -Depth 4) -replace "`r`n","`n") + "`n", $utf8)
    $notesPath = Join-Path $root 'vendor/viiper-source/OPENDS-PATCH.md'
    $notes = [IO.File]::ReadAllText($notesPath)
    $record = "<!-- BUILD-INFO:BEGIN -->`nDLL SHA-256: ``$hash```n`nSource SHA-256 (rules in build-info.json): ``$sourceHash```n`nToolchain: $goVersion; $compilerVersion.`n`nIndependent forced rebuild matched: $([bool]$VerifyReproducible).`n<!-- BUILD-INFO:END -->"
    if ($notes -notmatch '(?s)<!-- BUILD-INFO:BEGIN -->.*?<!-- BUILD-INFO:END -->') { throw 'Build information markers are missing from OPENDS-PATCH.md.' }
    $notes = [regex]::Replace($notes, '(?s)<!-- BUILD-INFO:BEGIN -->.*?<!-- BUILD-INFO:END -->', [Text.RegularExpressions.MatchEvaluator]{ param($match) $record })
    [IO.File]::WriteAllText($notesPath, $notes.Replace("`r`n","`n"), $utf8)
    Copy-Item -LiteralPath $output -Destination (Join-Path $root 'vendor/viiper/libVIIPER.dll') -Force
    Write-Output 'Regenerate vendor/opends-viiper.patch after rebuilding; see OPENDS-PATCH.md.'
    Get-FileHash -LiteralPath $output -Algorithm SHA256
} finally {
    Pop-Location
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $saved[$name], 'Process') }
}
