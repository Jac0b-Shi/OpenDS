[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$info = Get-Content -LiteralPath (Join-Path $root 'vendor/viiper/build-info.json') -Raw | ConvertFrom-Json
$dllHash = (Get-FileHash -LiteralPath (Join-Path $root 'vendor/viiper/libVIIPER.dll') -Algorithm SHA256).Hash.ToLowerInvariant()
if ($dllHash -ne $info.dll_sha256) { throw 'Bundled VIIPER DLL does not match build-info.json.' }
$sourceHash = & (Join-Path $PSScriptRoot 'Get-VIIPERSourceHash.ps1')
if ($sourceHash -ne $info.source_sha256) { throw 'VIIPER source has changed since the recorded DLL build.' }
$notes = Get-Content -LiteralPath (Join-Path $root 'vendor/viiper-source/OPENDS-PATCH.md') -Raw
if (!$notes.Contains($dllHash)) { throw 'OPENDS-PATCH.md does not record the current DLL hash.' }
Write-Output "VIIPER source and DLL match build-info.json: $dllHash"
