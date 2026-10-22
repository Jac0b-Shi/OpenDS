$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot '..\opends.exe'
$process = Start-Process -FilePath $exe -ArgumentList 'restore' -Wait -PassThru -NoNewWindow
if ($process.ExitCode -ne 0) { throw 'Restore failed. Exit every OpenDS instance and run opends restore from a terminal to see details. Do not delete the recovery journal.' }
