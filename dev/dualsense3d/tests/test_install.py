"""Verify the standalone 3D install from a prefix containing spaces and Unicode."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]


class Installation(unittest.TestCase):
 def test_standalone_launcher_and_smoke_startup(self):
  with tempfile.TemporaryDirectory(prefix='opends 3D 安装 ') as temporary:
   prefix=Path(temporary)/'installed app'
   result=subprocess.run(['/usr/bin/python3',str(ROOT/'install.py'),'--prefix',str(prefix)],capture_output=True,text=True,timeout=30)
   self.assertEqual(result.returncode,0,result.stderr)
   menu=prefix/'share/applications/io.github.OpenDS.Dev3D.desktop'
   subprocess.run(['desktop-file-validate',str(menu)],check=True)
   self.assertIn('Icon=opends-3d',menu.read_text())
   app=prefix/'lib/opends-3d-dev'
   for name in ('theme.py','appearance.py','renderer.py','assets/dualsense.glb','backend/opends','backend/libVIIPER.so'):
    self.assertTrue((app/name).is_file(),name)
   # --smoke uses Gio NON_UNIQUE, so it cannot activate an already running app.
   env={**os.environ,'GTK_USE_PORTAL':'0','GIO_USE_VFS':'local','GSETTINGS_BACKEND':'memory',
        'XDG_CONFIG_HOME':str(Path(temporary)/'config'),
        'XDG_STATE_HOME':str(Path(temporary)/'state'),
        'OPENDS_3D_PROFILE':str(Path(temporary)/'3d-profile'),
        'OPENDS_3D_STATE':str(Path(temporary)/'3d-state')}
   result=subprocess.run([str(app/'launch.sh'),'--smoke'],cwd='/tmp',env=env,capture_output=True,text=True,timeout=20)
   self.assertEqual(result.returncode,0,result.stderr)
   self.assertIn('OpenDS 3D renderer:',result.stdout)
   self.assertNotIn('Traceback',result.stderr)


if __name__=='__main__':unittest.main()
