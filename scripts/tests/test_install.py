"""Installation must not register shortcuts without an explicit request."""
from pathlib import Path
import json
import os
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
HARNESS = r'''
param([string]$Mode)
$ErrorActionPreference = 'Stop'
$global:opendsTestCalls = 0
function New-Object {
    param([string]$ComObject)
    if ($ComObject -ne 'WScript.Shell') { throw 'Unexpected COM request' }
    $global:opendsTestCalls++
    $shell = [pscustomobject]@{}
    $shell | Add-Member ScriptMethod CreateShortcut {
        param($path)
        $link = [pscustomobject]@{ TargetPath=''; WorkingDirectory=''; IconLocation=''; RequestedPath=$path }
        $link | Add-Member ScriptMethod Save {
            $this | ConvertTo-Json | Set-Content -LiteralPath $env:TEST_LINK -Encoding UTF8
        }
        $link
    }
    $shell
}
$options = @{}
switch ($Mode) {
    'explicit' { $options.StartMenuShortcut = $true }
    'legacy-no' { $options.NoShortcut = $true }
    'conflict' { $options.StartMenuShortcut = $true; $options.NoShortcut = $true }
}
& $env:TEST_INSTALL -Destination $env:TEST_DEST @options
if ($global:opendsTestCalls -ne $(if ($Mode -eq 'explicit') {1} else {0})) { throw 'Shortcut consent was not respected' }
'''


def main():
    for shell in ('powershell.exe', 'pwsh.exe'):
        executable = shutil.which(shell)
        if not executable:
            raise SystemExit(f'Required test host not found: {shell}')
        for mode in ('default', 'legacy-no', 'explicit', 'conflict'):
            with tempfile.TemporaryDirectory(prefix='opends-install-') as folder:
                fixture = Path(folder)
                source = fixture / 'source'
                for sub in ('scripts', 'licenses', 'docs'):
                    (source / sub).mkdir(parents=True)
                script = source / 'scripts/Install-OpenDS.ps1'
                shutil.copyfile(ROOT / 'scripts/Install-OpenDS.ps1', script)
                for name in ('opends.exe', 'libVIIPER.dll', 'libopus-0.dll',
                             'viiper-build-info.json', 'README.md', 'README.zh-CN.md',
                             'LICENSE', 'THIRD-PARTY.md', 'SHA256SUMS.txt'):
                    (source / name).write_bytes(b'fixture')
                harness = fixture / 'check.ps1'
                harness.write_text(HARNESS, encoding='utf-8-sig')
                destination = fixture / 'installed'
                destination.mkdir()
                config = destination / 'opends.ini'
                config.write_bytes(b'preserve existing settings')
                environment = os.environ.copy()
                environment.pop('PSModulePath', None)
                # Exercise a spelling that differs from Python's path. Windows
                # may also expand 8.3 TEMP paths on hosted runners.
                requested_destination = str(destination).lower() if mode == 'explicit' else str(destination)
                environment.update(TEST_INSTALL=str(script), TEST_DEST=requested_destination,
                                   TEST_LINK=str(fixture / 'shortcut.json'))
                result = subprocess.run([executable, '-NoProfile', '-NonInteractive',
                                         '-File', str(harness), mode], env=environment,
                                        capture_output=True, timeout=30)
                if (result.returncode == 0) != (mode != 'conflict'):
                    raise AssertionError(f'{shell} {mode}: {result.stdout!r} {result.stderr!r}')
                assert config.read_bytes() == b'preserve existing settings'
                link = fixture / 'shortcut.json'
                assert link.exists() == (mode == 'explicit')
                if mode == 'explicit':
                    data = json.loads(link.read_text(encoding='utf-8-sig'))
                    assert Path(data['TargetPath']).samefile(destination / 'opends.exe'), data
                    assert Path(data['WorkingDirectory']).samefile(destination), data
                    assert Path(data['RequestedPath']).name == 'OpenDS.lnk'
                print(f'PASS {shell} {mode}')


if __name__ == '__main__':
    main()
