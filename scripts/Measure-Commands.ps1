<# Isolated simulated input; never starts a controller bridge. CPU 100% = one logical core. #>
[CmdletBinding()]
param(
    [string]$Executable = (Join-Path $PSScriptRoot '../target/release/opends.exe'),
    [ValidateRange(1,10)][int]$Repeats = 3,
    [string]$OutputDirectory = (Join-Path $env:LOCALAPPDATA 'OpenDS\measurements')
)
$ErrorActionPreference = 'Stop'
$exe = [IO.Path]::GetFullPath($Executable)
$directory = Join-Path $OutputDirectory ('commands-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $directory -Force | Out-Null
$names = @('LOCALAPPDATA','OPENDS_UI_SMOKE_SKIP_DIALOGS','OPENDS_UI_SMOKE_SECONDS','OPENDS_UI_SMOKE_LANGUAGE','OPENDS_UI_SMOKE_THEME','OPENDS_UI_SMOKE_DPI','OPENDS_UI_SMOKE_DEMO','OPENDS_UI_SMOKE_THEME_STRESS','OPENDS_COMMAND_BENCH_MODE','OPENDS_COMMAND_BENCH_WORKLOAD')
$original = @{}
foreach ($name in $names) { $original[$name] = [Environment]::GetEnvironmentVariable($name) }
$samples = @()
try {
    $env:LOCALAPPDATA = $directory
    $env:OPENDS_UI_SMOKE_SKIP_DIALOGS = '1'
    $env:OPENDS_UI_SMOKE_SECONDS = '70'
    $env:OPENDS_UI_SMOKE_LANGUAGE = 'en-US'
    $env:OPENDS_UI_SMOKE_THEME = 'light'
    $env:OPENDS_UI_SMOKE_DPI = '96'
    $env:OPENDS_COMMAND_BENCH_WORKLOAD = 'mixed'
    Remove-Item Env:OPENDS_UI_SMOKE_DEMO,Env:OPENDS_UI_SMOKE_THEME_STRESS -ErrorAction SilentlyContinue
    Get-FileHash -LiteralPath $exe -Algorithm SHA256 | ConvertTo-Json | Set-Content (Join-Path $directory 'build.json') -Encoding utf8
    foreach ($round in 1..$Repeats) {
        foreach ($mode in @('disabled','main','overlay','both')) {
            $env:OPENDS_COMMAND_BENCH_MODE = $mode
            $label = "$round-$mode"
            $out = Join-Path $directory "$label.out"
            $err = Join-Path $directory "$label.err"
            $process = Start-Process $exe -ArgumentList @('ui-smoke-test','--smoke-command-benchmark') -WindowStyle Hidden -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
            try {
                $ready = [Diagnostics.Stopwatch]::StartNew()
                while ($ready.Elapsed.TotalSeconds -lt 30 -and (Get-Content $out -Raw -ErrorAction SilentlyContinue) -notmatch 'command_benchmark_ready') { Start-Sleep -Milliseconds 100 }
                if ($ready.Elapsed.TotalSeconds -ge 30) { throw "Benchmark did not initialize: $label" }
                Start-Sleep -Seconds 3
                $process.Refresh()
                $cpu = $process.TotalProcessorTime.TotalSeconds
                $memory = $process.PrivateMemorySize64
                $clock = [Diagnostics.Stopwatch]::StartNew()
                Start-Sleep -Seconds 60
                $process.Refresh()
                if ($process.HasExited) { throw "Benchmark exited early: $label" }
                $samples += [pscustomobject]@{
                    Round=$round;Mode=$mode;Seconds=$clock.Elapsed.TotalSeconds
                    CpuCorePercent=100*($process.TotalProcessorTime.TotalSeconds-$cpu)/$clock.Elapsed.TotalSeconds
                    PrivateMiBBefore=$memory/1MB;PrivateMiBAfter=$process.PrivateMemorySize64/1MB
                    Handles=$process.HandleCount;Threads=$process.Threads.Count
                }
                $samples | Export-Csv (Join-Path $directory 'samples.csv') -NoTypeInformation -Encoding utf8
                if (!$process.WaitForExit(30000)) { throw "Benchmark exit timed out: $label" }
                if ((Get-Content $out -Raw) -notmatch 'GUI.*passed' -or (Get-Content $err -Raw) -match 'command_overlay_update_failed|panicked') { throw "Benchmark failed: $label" }
                Write-Output "$label passed"
            } finally { if (!$process.HasExited) { $process.Kill(); $process.WaitForExit() } }
        }
    }
    $averages = @{}
    foreach ($group in ($samples | Group-Object Mode)) { $averages[$group.Name]=($group.Group | Measure-Object CpuCorePercent -Average).Average }
    foreach ($mode in @('main','overlay','both')) {
        [pscustomobject]@{Mode=$mode;CpuCorePercent=$averages[$mode];AddedCorePercentagePoints=$averages[$mode]-$averages['disabled'];WithinBudget=($averages[$mode]-$averages['disabled'] -le 3)}
    }
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name,$original[$name]) }
    Write-Output "MEASUREMENT_DIRECTORY=$directory"
}
