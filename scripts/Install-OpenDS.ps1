[CmdletBinding()]
param([string]$Destination = (Join-Path $env:LOCALAPPDATA 'Programs\OpenDS'), [switch]$NoShortcut, [switch]$Startup)
$ErrorActionPreference = 'Stop'
$source = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$destinationPath = [IO.Path]::GetFullPath($Destination)
if ($source.TrimEnd('\') -ieq $destinationPath.TrimEnd('\')) { throw 'Choose an installation folder different from the extracted package.' }
if (Get-Process opends -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($destinationPath + '\',[StringComparison]::OrdinalIgnoreCase) }) {
    throw 'Exit the installed OpenDS before updating.'
}
New-Item -ItemType Directory -Force -Path $destinationPath | Out-Null
foreach ($file in @('opends.exe','libVIIPER.dll','libopus-0.dll','README.md','README.zh-CN.md','LICENSE','THIRD-PARTY.md','SHA256SUMS.txt')) {
    Copy-Item -LiteralPath (Join-Path $source $file) -Destination $destinationPath -Force
}
foreach ($folder in @('scripts','licenses','docs')) {
    Copy-Item -LiteralPath (Join-Path $source $folder) -Destination $destinationPath -Recurse -Force
}
if (!$NoShortcut) {
    $menu = Join-Path ([Environment]::GetFolderPath('Programs')) 'OpenDS.lnk'
    $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($menu)
    $shortcut.TargetPath = Join-Path $destinationPath 'opends.exe'
    $shortcut.WorkingDirectory = $destinationPath
    $shortcut.IconLocation = $shortcut.TargetPath
    $shortcut.Save()
}
Write-Host "Installed: $destinationPath. Open OpenDS from the Start menu."
Write-Host 'Drivers are installed separately from the Diagnostics page. Existing configuration is preserved.'
if ($Startup) {
    $startupProcess = Start-Process -FilePath (Join-Path $destinationPath 'opends.exe') -ArgumentList 'startup','enable' -WindowStyle Hidden -Wait -PassThru
    if ($startupProcess.ExitCode -ne 0) { throw 'Installation succeeded, but enabling logon startup failed.' }
}
