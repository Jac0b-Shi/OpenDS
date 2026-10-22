[CmdletBinding()]
param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $root
try {
    & (Join-Path $PSScriptRoot 'Test-VIIPERBuildInfo.ps1')
    if (!$SkipTests) {
        & cargo test --locked --offline
        if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
    }
    & cargo build --release --locked --offline
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    # Exercise the real GUI initialization and event loop, without starting a bridge.
    $smokeOut = Join-Path $root 'target\gui-smoke-output.txt'
    $smokeErr = Join-Path $root 'target\gui-smoke-error.txt'
    $guiCheck = Start-Process -FilePath (Join-Path $root 'target\release\opends.exe') -ArgumentList 'ui-smoke-test' -PassThru -RedirectStandardOutput $smokeOut -RedirectStandardError $smokeErr
    if (!$guiCheck.WaitForExit(15000)) { throw 'GUI smoke check did not exit within 15 seconds; inspect the app before packaging.' }
    if ($guiCheck.ExitCode -ne 0) { throw ('GUI startup failed: ' + (Get-Content -LiteralPath $smokeErr -Raw)) }
    $version = ((Get-Content Cargo.toml | Select-String '^version = "([^"]+)"' | Select-Object -First 1).Matches.Groups[1].Value)
    $name = 'OpenDS-' + $version + '-windows-x64-' + (Get-Date -Format 'yyyyMMdd-HHmmss')
    $dist = Join-Path $root 'target\dist'
    $stage = Join-Path $dist $name
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    Copy-Item target/release/opends.exe $stage
    Copy-Item vendor/viiper/libVIIPER.dll $stage
    Copy-Item vendor/viiper/build-info.json (Join-Path $stage 'viiper-build-info.json')
    Copy-Item vendor/opus/libopus-0.dll $stage
    Copy-Item README.md,README.zh-CN.md,LICENSE,THIRD-PARTY.md $stage
    Copy-Item licenses $stage -Recurse
    $packageScripts = Join-Path $stage 'scripts'
    New-Item -ItemType Directory -Path $packageScripts | Out-Null
    foreach ($file in @('Install-OpenDS.ps1','Install-Drivers.ps1','Restore-Visibility.ps1','Check-AudioIdentity.ps1','Measure-Resident.ps1','drivers.json')) {
        Copy-Item (Join-Path scripts $file) $packageScripts
    }
    $packageDocs = Join-Path $stage 'docs'
    New-Item -ItemType Directory -Path $packageDocs | Out-Null
    Copy-Item docs/guides (Join-Path $packageDocs 'guides') -Recurse
    # Collect notices for every resolved crate, including optional build dependencies.
    $metadataText = & cargo metadata --locked --offline --all-features --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed.' }
    $metadata = $metadataText | ConvertFrom-Json
    $notices = Join-Path $stage 'licenses\rust'
    New-Item -ItemType Directory -Path $notices | Out-Null
    $dependencies = @()
    foreach ($package in $metadata.packages) {
        if (!$package.source) { continue }
        $dependencies += [pscustomobject]@{name=$package.name;version=$package.version;license=$package.license;source=$package.source;repository=$package.repository}
        $crate = Split-Path $package.manifest_path
        foreach ($license in (Get-ChildItem -LiteralPath $crate -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)' })) {
            Copy-Item -LiteralPath $license.FullName -Destination (Join-Path $notices ($package.name + '-' + $package.version + '-' + $license.Name))
        }
    }
    $dependencies | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 (Join-Path $stage 'licenses\rust-dependencies.json')
    $sourceStage = Join-Path $dist ($name + '-source')
    New-Item -ItemType Directory -Path $sourceStage | Out-Null
    # Include the actual dirty working tree, never just HEAD or local configuration.
    $sourceFiles = @(& git ls-files --cached --others --exclude-standard)
    if ($LASTEXITCODE -ne 0) { throw 'Git source inventory failed.' }
    foreach ($relative in $sourceFiles) {
        if (!(Test-Path -LiteralPath $relative -PathType Leaf)) { continue }
        $targetFile = Join-Path $sourceStage $relative
        New-Item -ItemType Directory -Path (Split-Path $targetFile) -Force | Out-Null
        Copy-Item -LiteralPath $relative -Destination $targetFile
    }
    $viiperSource = Join-Path $sourceStage 'third-party\VIIPER-v1.0.1.zip'
    New-Item -ItemType Directory -Path (Split-Path $viiperSource) -Force | Out-Null
    if (Test-Path third-party/VIIPER-v1.0.1.zip) {
        Copy-Item third-party/VIIPER-v1.0.1.zip $viiperSource
    } else {
        # vendor/viiper-source 即 GPLv3 对应源码，直接打包给使用者
        Compress-Archive -Path 'vendor/viiper-source/*' -DestinationPath $viiperSource -Force
        if (-not (Test-Path $viiperSource)) { throw 'VIIPER source archive failed.' }
    }
    New-Item -ItemType Directory -Path (Join-Path $sourceStage 'vendor\viiper') -Force | Out-Null
    Copy-Item vendor/viiper/libVIIPER.dll (Join-Path $sourceStage 'vendor\viiper')
    Copy-Item (Join-Path $stage 'licenses') $sourceStage -Recurse -Force
    $hashes = foreach ($file in (Get-ChildItem -LiteralPath $stage -Recurse -File | Sort-Object FullName)) {
        (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + $file.FullName.Substring($stage.Length+1).Replace('\','/')
    }
    $hashes | Set-Content -Encoding ascii (Join-Path $stage 'SHA256SUMS.txt')
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $binaryZip = $stage + '.zip'
    $sourceZip = $sourceStage + '.zip'
    [IO.Compression.ZipFile]::CreateFromDirectory($stage,$binaryZip)
    [IO.Compression.ZipFile]::CreateFromDirectory($sourceStage,$sourceZip)
    @($binaryZip,$sourceZip) | ForEach-Object {
        $hash = Get-FileHash -LiteralPath $_ -Algorithm SHA256
        [pscustomobject]@{Path=$_;Bytes=(Get-Item -LiteralPath $_).Length;SHA256=$hash.Hash}
    } | Format-List
    Write-Output ('PACKAGE_DIRECTORY=' + $stage)
} finally { Pop-Location }
