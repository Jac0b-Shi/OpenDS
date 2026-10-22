<# Read-only bounded resource sampling; duration is not a stability guarantee. #>
[CmdletBinding()]
param(
    [int]$ProcessId,
    [ValidateRange(0.01,720)][double]$Hours = 168,
    [ValidateRange(1,3600)][int]$IntervalSeconds = 60,
    [string]$OutputDirectory = (Join-Path $env:LOCALAPPDATA 'OpenDS\measurements')
)
$ErrorActionPreference = 'Stop'
if ($ProcessId) { $process = Get-Process -Id $ProcessId } else {
    $candidates = @(Get-Process opends -ErrorAction SilentlyContinue)
    if ($candidates.Count -ne 1) { throw 'Specify -ProcessId when there is not exactly one OpenDS process' }
    $process = $candidates[0]
}
$identityTime = $process.StartTime
$identityId = $process.Id
$path = $process.Path
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$stem = Join-Path $OutputDirectory ('resident-'+(Get-Date -Format 'yyyyMMdd-HHmmss')+'-'+$identityId)
$csv = $stem+'.csv'
if (Test-Path -LiteralPath $csv) { throw 'Measurement already exists' }
$clock = [Diagnostics.Stopwatch]::StartNew()
$deadline = $Hours*3600
$previousCpu = $process.CPU
$previousSeconds = 0.0
$first = $null
$last = $null
$samples = 0
$peakPrivate = 0L
$peakHandles = 0
$completed = $false
try {
    while ($true) {
        $process = Get-Process -Id $identityId
        if ($process.StartTime -ne $identityTime) { throw 'Process ID reused; original process exited' }
        $elapsed = $clock.Elapsed.TotalSeconds
        $span = $elapsed-$previousSeconds
        $row = [pscustomobject]@{
            Time = (Get-Date).ToString('o')
            Seconds = [math]::Round($elapsed,3)
            CpuCorePercent = if ($span -gt 0.1) { [math]::Round(100*($process.CPU-$previousCpu)/$span,3) } else { 0 }
            PrivateBytes = $process.PrivateMemorySize64
            WorkingSetBytes = $process.WorkingSet64
            Handles = $process.HandleCount
            Threads = $process.Threads.Count
        }
        $row | Export-Csv -LiteralPath $csv -Append -NoTypeInformation -Encoding utf8
        if ($null -eq $first) { $first = $row }
        $last = $row
        $samples++
        $peakPrivate = [math]::Max($peakPrivate,$row.PrivateBytes)
        $peakHandles = [math]::Max($peakHandles,$row.Handles)
        $previousCpu = $process.CPU
        $previousSeconds = $elapsed
        if ($elapsed -ge $deadline) { $completed = $true; break }
        Start-Sleep -Milliseconds ([int](1000*[math]::Min($IntervalSeconds,$deadline-$elapsed)))
    }
} finally {
    [pscustomobject]@{
        ProcessId=$identityId;Executable=$path;StartTime=$identityTime.ToString('o')
        RequestedHours=$Hours;ObservedSeconds=$clock.Elapsed.TotalSeconds;Completed=$completed
        Samples=$samples;First=$first;Last=$last;PeakPrivateBytes=$peakPrivate;PeakHandles=$peakHandles
        Csv=$csv
    } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath ($stem+'.json') -Encoding utf8
    Write-Host ('Report: '+$stem+'.json')
}
