#!/usr/bin/env python3
"""Install OpenDS 3D Dev beside stable OpenDS; never calls the stable installer."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

HERE=Path(__file__).resolve().parent
APP_ID='io.github.OpenDS.Dev3D'


def quote(value):
    return '"'+str(value).replace('\\','\\\\').replace('"','\\"').replace('`','\\`').replace('$','\\$').replace('%','%%')+'"'


def install(prefix=None):
    prefix=Path(prefix).expanduser().resolve() if prefix else Path.home()/'.local'
    destination=prefix/'lib/opends-3d-dev'
    data=Path(os.environ.get('XDG_DATA_HOME',str(prefix/'share'))) if prefix==Path.home()/'.local' else prefix/'share'
    required=['backend/opends','backend/libVIIPER.so','base/gui.py','assets/dualsense.glb','vendor/python/OpenGL/__init__.py']
    for filename in required:
        if not (HERE/filename).is_file():raise SystemExit('Development bundle is incomplete. Run ./dev/dualsense3d/prepare.sh first.')
    if destination.resolve()==HERE.resolve():raise SystemExit('Installation destination must differ from the development source')
    destination.mkdir(parents=True,exist_ok=True)
    for name in ['backend','base','assets','shaders','vendor']:
        for source in (HERE/name).rglob('*'):
            if not source.is_file() or '__pycache__' in source.parts or 'wheels' in source.parts:continue
            target=destination/source.relative_to(HERE);target.parent.mkdir(parents=True,exist_ok=True)
            temporary=target.with_name(target.name+'.installing');shutil.copy2(source,temporary);temporary.replace(target)
    for name in ['app.py','renderer.py','scene.py','appearance.py','theme.py','launch.sh','README.md','CREDITS.md','requirements.txt']:
        source=HERE/name;target=destination/name;temporary=target.with_name(target.name+'.installing')
        shutil.copy2(source,temporary);temporary.replace(target)
    (destination/'launch.sh').chmod(0o755);(destination/'backend/opends').chmod(0o755)
    icon=data/'icons/hicolor/256x256/apps/opends-3d.png';icon.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(HERE/'backend/opends.png',icon)
    menu=data/'applications'/f'{APP_ID}.desktop';menu.parent.mkdir(parents=True,exist_ok=True)
    text='\n'.join(['[Desktop Entry]','Type=Application','Version=1.0','Name=OpenDS 3D Dev','Comment=Experimental realistic 3D DualSense live monitor',
        'Exec='+quote(destination/'launch.sh'),'Icon=opends-3d','Terminal=false','Categories=Game;','Keywords=DualSense;3D;controller;development;','StartupNotify=true','StartupWMClass='+APP_ID,''])
    temporary=menu.with_name('.'+APP_ID+'.installing.desktop');temporary.write_text(text,encoding='utf-8')
    if shutil.which('desktop-file-validate'):subprocess.run(['desktop-file-validate',str(temporary)],check=True)
    temporary.replace(menu)
    if shutil.which('update-desktop-database'):subprocess.run(['update-desktop-database',str(menu.parent)],check=True)
    if shutil.which('gtk-update-icon-cache'):subprocess.run(['gtk-update-icon-cache','-f','-t',str(data/'icons/hicolor')],check=True)
    print('Installed development version:',destination)
    print('Show Apps entry:',menu)
    print('Stable OpenDS installation and launcher were not changed.')
    return destination


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--prefix')
    install(parser.parse_args().prefix)
