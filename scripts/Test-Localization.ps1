[CmdletBinding()]
param(
    [string]$Executable = (Join-Path $PSScriptRoot '../target/release/opends.exe'),
    [string]$BaselineExecutable
)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$exe = [IO.Path]::GetFullPath($Executable)
$fixture = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) ('OpenDS-language-validation-' + [Guid]::NewGuid().ToString('N'))))
$tempPrefix = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
if (!$fixture.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid fixture location.' }
New-Item -ItemType Directory -Path (Join-Path $fixture 'OpenDS') -Force | Out-Null
$names = @('LOCALAPPDATA','OPENDS_UI_SMOKE_LANGUAGE','OPENDS_UI_SMOKE_DPI','OPENDS_UI_SMOKE_EXPAND','OPENDS_UI_SMOKE_SKIP_DIALOGS','OPENDS_UI_SMOKE_SECONDS','OPENDS_UI_SMOKE_PAGE','OPENDS_UI_SMOKE_RESTART_PARENT')
$original = @{}
foreach ($name in $names) { $original[$name] = [Environment]::GetEnvironmentVariable($name) }
$results = [Collections.Generic.List[object]]::new()
function Invoke-UiCheck([string]$Path, [string]$Locale, [int]$Dpi, [bool]$Expand, [string]$Restart = '') {
    $env:OPENDS_UI_SMOKE_LANGUAGE = $Locale
    $env:OPENDS_UI_SMOKE_DPI = $Dpi.ToString()
    $env:OPENDS_UI_SMOKE_SECONDS = '2'
    $env:OPENDS_UI_SMOKE_PAGE = '5'
    if ($Expand) { $env:OPENDS_UI_SMOKE_EXPAND = '1' } else { Remove-Item Env:OPENDS_UI_SMOKE_EXPAND -ErrorAction SilentlyContinue }
    if ($Dpi -eq 96 -and !$Expand -and !$Restart) { Remove-Item Env:OPENDS_UI_SMOKE_SKIP_DIALOGS -ErrorAction SilentlyContinue } else { $env:OPENDS_UI_SMOKE_SKIP_DIALOGS = '1' }
    $case = $Locale + '-' + $Dpi + '-' + $Expand + '-' + $Restart
    $stdout = Join-Path $fixture ($case + '-out.txt')
    $stderr = Join-Path $fixture ($case + '-err.txt')
    $arguments = @('ui-smoke-test')
    if ($Restart) { $arguments += @('--smoke-restart','--restart-state',$Restart) }
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath $Path -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    $firstWindow = $null
    $peak = 0L
    while (!$process.HasExited -and $timer.Elapsed.TotalSeconds -lt 25) {
        $process.Refresh()
        $peak = [Math]::Max($peak, $process.WorkingSet64)
        if ($null -eq $firstWindow -and $process.MainWindowHandle -ne 0) { $firstWindow = $timer.ElapsedMilliseconds }
        Start-Sleep -Milliseconds 25
    }
    if (!$process.HasExited) { $process.Kill(); throw ('UI check timed out: ' + $case) }
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) { throw ($case + ': ' + (Get-Content -LiteralPath $stderr -Raw)) }
    if ($Restart) {
        $marker = Join-Path ([IO.Path]::GetTempPath()) ('opends-restart-smoke-' + $process.Id + '.done')
        $deadline = [DateTime]::UtcNow.AddSeconds(25)
        while (!(Test-Path -LiteralPath $marker) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
        if (!(Test-Path -LiteralPath $marker)) { throw 'Restarted test process did not finish.' }
        $handoff = (Get-Content -LiteralPath $marker -Raw).Split(':')
        if ($handoff[0] -eq $process.Id.ToString() -or $handoff[1] -ne $(if ($Restart -eq 'running') { 'true' } else { 'false' })) { throw 'Restart did not preserve the bridge state.' }
        Remove-Item -LiteralPath $marker
    }
    $results.Add([pscustomobject]@{executable=$Path; locale=$Locale; dpi=$Dpi; expanded=$Expand; restart=$Restart; first_window_ms=$firstWindow; elapsed_ms=$timer.ElapsedMilliseconds; sampled_peak_working_set_bytes=$peak; exit_code=$process.ExitCode})
    Write-Host ('Passed: ' + $case)
}
try {
    $env:LOCALAPPDATA = $fixture
    Remove-Item Env:OPENDS_UI_SMOKE_RESTART_PARENT -ErrorAction SilentlyContinue
    foreach ($locale in @('zh-CN','en-US')) {
        Set-Content -LiteralPath (Join-Path $fixture 'OpenDS/opends.ini') -Value ('[opends]' + "`nlanguage=" + $locale) -Encoding utf8
        foreach ($file in (Get-ChildItem -LiteralPath (Join-Path $root 'locales') -Filter '*.ftl')) {
            $check = Start-Process -FilePath $exe -ArgumentList @('language','check',('"' + $file.FullName + '"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $fixture 'check-out.txt') -RedirectStandardError (Join-Path $fixture 'check-err.txt')
            if (!$check.WaitForExit(15000) -or $check.ExitCode -ne 0) { throw ('Language validation failed: ' + $file.Name) }
        }
        foreach ($dpi in @(96,144,192)) { Invoke-UiCheck $exe $locale $dpi $false }
    }
    foreach ($dpi in @(96,144,192)) { Invoke-UiCheck $exe 'en-US' $dpi $true }
    Invoke-UiCheck $exe 'en-US' 96 $false 'stopped'
    Invoke-UiCheck $exe 'en-US' 96 $false 'running'
    if ($BaselineExecutable) { Invoke-UiCheck ([IO.Path]::GetFullPath($BaselineExecutable)) 'zh-CN' 96 $false }
    $reportDir = Join-Path $root 'target/review'
    New-Item -ItemType Directory -Path $reportDir -Force | Out-Null
    $results | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $reportDir 'localization-validation.json') -Encoding utf8
} finally {
    foreach ($name in $names) { if ($null -eq $original[$name]) { Remove-Item -LiteralPath ('Env:'+$name) -ErrorAction SilentlyContinue } else { [Environment]::SetEnvironmentVariable($name, $original[$name]) } }
    if ($fixture.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $fixture)) { Remove-Item -LiteralPath $fixture -Recurse -Force }
}
