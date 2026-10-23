[CmdletBinding()]
param([string]$Executable = (Join-Path $PSScriptRoot '../target/release/opends.exe'))
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$exe = [IO.Path]::GetFullPath($Executable)
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
$fixture = [IO.Path]::GetFullPath((Join-Path $tempRoot ('OpenDS-theme-validation-' + [Guid]::NewGuid().ToString('N'))))
if (!$fixture.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid fixture location.' }
New-Item -ItemType Directory -Path $fixture | Out-Null
$names = @('LOCALAPPDATA','OPENDS_UI_SMOKE_THEME','OPENDS_UI_SMOKE_LANGUAGE','OPENDS_UI_SMOKE_DPI','OPENDS_UI_SMOKE_SECONDS','OPENDS_UI_SMOKE_PAGE','OPENDS_UI_SMOKE_SKIP_DIALOGS','OPENDS_UI_SMOKE_THEME_STRESS','OPENDS_UI_SMOKE_SYSTEM_THEME','OPENDS_UI_SMOKE_EXPAND')
$original = @{}
foreach ($name in $names) { $original[$name] = [Environment]::GetEnvironmentVariable($name) }
$results = [Collections.Generic.List[object]]::new()
function Invoke-ThemeCheck([string]$Locale, [string]$Theme, [int]$Dpi, [bool]$Stress = $false) {
    $env:OPENDS_UI_SMOKE_LANGUAGE = $Locale
    $env:OPENDS_UI_SMOKE_THEME = $Theme
    $env:OPENDS_UI_SMOKE_DPI = $Dpi.ToString()
    $env:OPENDS_UI_SMOKE_SECONDS = '2'
    $env:OPENDS_UI_SMOKE_PAGE = '5'
    if ($Dpi -eq 96) { Remove-Item Env:OPENDS_UI_SMOKE_SKIP_DIALOGS -ErrorAction SilentlyContinue } else { $env:OPENDS_UI_SMOKE_SKIP_DIALOGS = '1' }
    if ($Stress) { $env:OPENDS_UI_SMOKE_THEME_STRESS = '1' } else { Remove-Item Env:OPENDS_UI_SMOKE_THEME_STRESS -ErrorAction SilentlyContinue }
    $case = "$Locale-$Theme-$Dpi-$Stress"
    $stdout = Join-Path $fixture ($case + '-out.txt')
    $stderr = Join-Path $fixture ($case + '-err.txt')
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath $exe -ArgumentList 'ui-smoke-test' -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $timeout = if ($Stress) { 90000 } else { 25000 }
    if (!$process.WaitForExit($timeout)) { $process.Kill(); $process.WaitForExit(); throw ('Theme check timed out: ' + $case) }
    $output = Get-Content -LiteralPath $stdout -Raw
    if ($process.ExitCode -ne 0) { throw ($case + ': ' + (Get-Content -LiteralPath $stderr -Raw) + "`n" + $output) }
    $results.Add([pscustomobject]@{locale=$Locale; theme=$Theme; dpi=$Dpi; stress=$Stress; elapsed_ms=$timer.ElapsedMilliseconds; exit_code=$process.ExitCode; output=$output})
    Write-Host ('Passed: ' + $case)
}
try {
    $env:LOCALAPPDATA = $fixture
    Remove-Item Env:OPENDS_UI_SMOKE_EXPAND -ErrorAction SilentlyContinue
    Remove-Item Env:OPENDS_UI_SMOKE_SYSTEM_THEME -ErrorAction SilentlyContinue
    foreach ($locale in @('zh-CN','en-US')) {
        foreach ($theme in @('system','light','dark')) {
            foreach ($dpi in @(96,144,192)) { Invoke-ThemeCheck $locale $theme $dpi }
        }
    }
    Invoke-ThemeCheck 'en-US' 'dark' 96 $true
    $reportDir = Join-Path $root 'target/review'
    New-Item -ItemType Directory -Path $reportDir -Force | Out-Null
    $results | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $reportDir 'theme-validation.json') -Encoding utf8
} finally {
    foreach ($name in $names) { if ($null -eq $original[$name]) { Remove-Item -LiteralPath ('Env:'+$name) -ErrorAction SilentlyContinue } else { [Environment]::SetEnvironmentVariable($name, $original[$name]) } }
    if ($fixture.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $fixture)) { Remove-Item -LiteralPath $fixture -Recurse -Force }
}
