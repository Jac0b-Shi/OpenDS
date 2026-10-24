[CmdletBinding()]
param([string]$Executable = (Join-Path $PSScriptRoot '../target/release/opends.exe'))
$ErrorActionPreference = 'Stop'
$exe = [IO.Path]::GetFullPath($Executable)
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('OpenDS-shutdown-validation-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$names = @('LOCALAPPDATA','OPENDS_UI_SMOKE_LANGUAGE','OPENDS_UI_SMOKE_SKIP_DIALOGS','OPENDS_UI_SMOKE_RESTART_PARENT')
$original = @{}
foreach ($name in $names) { $original[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:LOCALAPPDATA = $fixture
    $env:OPENDS_UI_SMOKE_SKIP_DIALOGS = '1'
    Remove-Item Env:OPENDS_UI_SMOKE_RESTART_PARENT -ErrorAction SilentlyContinue
    foreach ($locale in @('zh-CN','en-US')) {
        $env:OPENDS_UI_SMOKE_LANGUAGE = $locale
        foreach ($stalled in @($false,$true)) {
            $arguments = @('ui-smoke-test','--smoke-shutdown')
            if ($stalled) { $arguments += '--smoke-shutdown-stalled' }
            $out = Join-Path $fixture ($locale + '-' + $stalled + '.out')
            $err = Join-Path $fixture ($locale + '-' + $stalled + '.err')
            $timer = [Diagnostics.Stopwatch]::StartNew()
            $process = Start-Process -FilePath $exe -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
            # Startup includes DPI/font and window fixtures; measure the shutdown
            # handler separately so a cold or busy CI runner cannot mask it.
            if (!$process.WaitForExit(25000)) { throw ('Shutdown test timed out; isolated test PID=' + $process.Id) }
            if ($process.ExitCode -ne 0) { throw (Get-Content -LiteralPath $err -Raw) }
            $timing = [regex]::Match((Get-Content -LiteralPath $err -Raw), 'shutdown_cleanup_ms=(\d+)')
            if (!$timing.Success -or [long]$timing.Groups[1].Value -ge 4500) { throw 'The shutdown handler exceeded its cleanup budget.' }
            if ($stalled -and [long]$timing.Groups[1].Value -lt 3400) { throw 'The stalled-worker case did not exercise the cleanup budget.' }
            Write-Output ('Passed: locale=' + $locale + ', stalled=' + $stalled + ', cleanup_ms=' + $timing.Groups[1].Value + ', process_ms=' + $timer.ElapsedMilliseconds)
        }
    }
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $original[$name]) }
    # Retain the small isolated fixture and logs on failure for inspection.
    Write-Output ('TEST_LOG_DIRECTORY=' + $fixture)
}
