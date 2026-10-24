[CmdletBinding()]
param([string]$Executable = (Join-Path $PSScriptRoot '../target/release/opends.exe'))
$ErrorActionPreference = 'Stop'
$exe = [IO.Path]::GetFullPath($Executable)
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('OpenDS-responsive-validation-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$names = @('LOCALAPPDATA','OPENDS_UI_SMOKE_LANGUAGE','OPENDS_UI_SMOKE_DPI','OPENDS_UI_SMOKE_SKIP_DIALOGS')
$original = @{}
foreach ($name in $names) { $original[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:LOCALAPPDATA = $fixture
    $env:OPENDS_UI_SMOKE_SKIP_DIALOGS = '1'
    foreach ($locale in @('zh-CN','en-US')) {
        $env:OPENDS_UI_SMOKE_LANGUAGE = $locale
        foreach ($dpi in @(96,144,192)) {
            $env:OPENDS_UI_SMOKE_DPI = $dpi.ToString()
            $out = Join-Path $fixture ($locale + '-' + $dpi + '.out')
            $err = Join-Path $fixture ($locale + '-' + $dpi + '.err')
            $process = Start-Process -FilePath $exe -ArgumentList @('ui-smoke-test','--smoke-responsiveness') -WindowStyle Hidden -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
            if (!$process.WaitForExit(25000)) { throw ('Responsiveness test timed out; isolated test PID=' + $process.Id) }
            if ($process.ExitCode -ne 0) { throw (Get-Content -LiteralPath $err -Raw) }
            Write-Output ($locale + '-' + $dpi + ': ' + ((Get-Content -LiteralPath $out) -match 'UI responsiveness:'))
        }
    }
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $original[$name]) }
    Write-Output ('TEST_LOG_DIRECTORY=' + $fixture)
}
