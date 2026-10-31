[CmdletBinding()]
param([string]$Executable = (Join-Path $PSScriptRoot '../target/release/opends.exe'))
$ErrorActionPreference = 'Stop'
$exe = [IO.Path]::GetFullPath($Executable)
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('OpenDS-commands-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture -Force | Out-Null
$names = @('LOCALAPPDATA','OPENDS_UI_SMOKE_SKIP_DIALOGS','OPENDS_UI_SMOKE_SECONDS','OPENDS_UI_SMOKE_LANGUAGE','OPENDS_UI_SMOKE_THEME','OPENDS_UI_SMOKE_DPI')
$original = @{}
foreach ($name in $names) { $original[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:LOCALAPPDATA = $fixture
    $env:OPENDS_UI_SMOKE_SKIP_DIALOGS = '1'
    $env:OPENDS_UI_SMOKE_SECONDS = '2'
    foreach ($locale in @('zh-CN','en-US')) {
        foreach ($theme in @('light','dark')) {
            foreach ($dpi in @(96,144,192)) {
                $env:OPENDS_UI_SMOKE_LANGUAGE = $locale
                $env:OPENDS_UI_SMOKE_THEME = $theme
                $env:OPENDS_UI_SMOKE_DPI = $dpi.ToString()
                $name = "$locale-$theme-$dpi"
                $out = Join-Path $fixture "$name.out"
                $err = Join-Path $fixture "$name.err"
                $process = Start-Process $exe -ArgumentList @('ui-smoke-test','--smoke-commands') -WindowStyle Hidden -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
                if (!$process.WaitForExit(25000)) { $process.Kill(); throw "Command UI timeout: $name" }
                $errors = Get-Content -LiteralPath $err -Raw
                $output = Get-Content -LiteralPath $out -Raw
                if ($errors -notmatch 'Command overlay alpha, lock/unlock and layered surface checks passed' -or $errors -match 'command_overlay_update_failed|panicked' -or $output -notmatch '(?m)^GUI ') {
                    throw "Command UI check failed: $name`n$errors"
                }
                Write-Output "$name passed"
            }
        }
    }
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name,$original[$name]) }
    Write-Output "TEST_LOG_DIRECTORY=$fixture"
}
