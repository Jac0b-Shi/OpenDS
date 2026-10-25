[CmdletBinding()]
param([string]$Executable = (Join-Path $PSScriptRoot '../target/release/opends.exe'), [switch]$Expanded)
$ErrorActionPreference = 'Stop'
$exe = [IO.Path]::GetFullPath($Executable)
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('OpenDS-battery-validation-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$names = @('LOCALAPPDATA','OPENDS_UI_SMOKE_LANGUAGE','OPENDS_UI_SMOKE_DPI','OPENDS_UI_SMOKE_SKIP_DIALOGS','OPENDS_UI_SMOKE_THEME','OPENDS_UI_SMOKE_EXPAND','OPENDS_UI_SMOKE_SYSTEM_THEME')
$original = @{}
foreach ($name in $names) { $original[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:LOCALAPPDATA = $fixture
    $env:OPENDS_UI_SMOKE_SKIP_DIALOGS = '1'
    Remove-Item Env:OPENDS_UI_SMOKE_EXPAND -ErrorAction SilentlyContinue
    foreach ($locale in @('zh-CN','en-US')) {
        $env:OPENDS_UI_SMOKE_LANGUAGE = $locale
        if ($Expanded -and $locale -eq 'en-US') { $env:OPENDS_UI_SMOKE_EXPAND='1' }
        else { Remove-Item Env:OPENDS_UI_SMOKE_EXPAND -ErrorAction SilentlyContinue }
        foreach ($theme in @('light','dark','contrast')) {
            $env:OPENDS_UI_SMOKE_THEME = $theme
            if ($theme -eq 'contrast') {
                $env:OPENDS_UI_SMOKE_THEME='system'
                $env:OPENDS_UI_SMOKE_SYSTEM_THEME='contrast'
            } else { Remove-Item Env:OPENDS_UI_SMOKE_SYSTEM_THEME -ErrorAction SilentlyContinue }
            foreach ($dpi in @(96,144,192)) {
                $env:OPENDS_UI_SMOKE_DPI = $dpi.ToString()
                $case = $locale + '-' + $theme + '-' + $dpi
                if ($Expanded -and $locale -eq 'en-US') { $case += '-expanded' }
                $out = Join-Path $fixture ($case + '.out')
                $err = Join-Path $fixture ($case + '.err')
                $process = Start-Process -FilePath $exe -ArgumentList @('ui-smoke-test','--smoke-battery') -WindowStyle Hidden -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
                if (!$process.WaitForExit(120000)) { throw ('Battery test timed out; isolated PID=' + $process.Id) }
                if ($process.ExitCode -ne 0) { throw ($case + ': ' + (Get-Content -LiteralPath $err -Raw)) }
                Write-Output ('Passed: ' + $case)
            }
        }
    }
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $original[$name]) }
    Write-Output ('TEST_LOG_DIRECTORY=' + $fixture)
}
