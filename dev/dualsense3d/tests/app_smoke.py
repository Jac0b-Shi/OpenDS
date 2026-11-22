"""Exercise the full dev UI, native GL context reparenting and bridge ownership guard."""
import fcntl
import os
from pathlib import Path
import runpy
import sys
import tempfile
import time

ROOT=Path(__file__).resolve().parents[1]
temporary=tempfile.TemporaryDirectory(prefix='opends-3d-app-test-')
os.environ['OPENDS_3D_PROFILE']=str(Path(temporary.name)/'profile')
os.environ['OPENDS_3D_STATE']=str(Path(temporary.name)/'state')
sys.argv=[str(ROOT/'app.py')]
module=runpy.run_path(str(ROOT/'app.py'),run_name='opends_3d_test')
Gtk,GLib,Gio=module['Gtk'],module['GLib'],module['Gio']
application=Gtk.Application(application_id='io.github.OpenDS.Dev3D.Verification',flags=Gio.ApplicationFlags.NON_UNIQUE)
application.register(None)
window=module['DevWindow'](application,str(module['BACKEND']),str(module['CONFIG']))
output=ROOT/'test-output';output.mkdir(exist_ok=True)
errors=[];loop=GLib.MainLoop();stage=0;started=time.monotonic()

def exception(kind,value,tb):
 errors.append(str(value));sys.__excepthook__(kind,value,tb);window.shutdown();loop.quit()
sys.excepthook=exception

def step():
 global stage
 if stage==4:
  if window.service.poll() is None:return True
  loop.quit();return False
 if not window.built:return True
 view=window.monitor_canvas.view
 if view.failure:raise AssertionError(view.failure)
 if not view.ready or not view.frame_count:
  assert time.monotonic()-started<25,'Native GL context did not start'
  return True
 if stage==0:
  assert module['CONFIG'].is_file()
  assert window.get_title()=='OpenDS · 3D Dev'
  window.monitor_canvas.demo.set_active(True)
  monitor=window.monitor_canvas
  assert monitor.view.transparent_background
  monitor.color_combo.set_active_id('grey-camouflage')
  monitor.transparent_check.set_active(False)
  from appearance import load
  assert load(monitor.appearance_path)=={'color':'grey-camouflage','transparent':False}
  stage=1
 elif stage==1:
  assert view.demo and abs(view.animation.values['left-stick-x'])>.05
  view.capture_png(output/'dev-model.png')
  from gi.repository import Gdk
  pixbuf=Gdk.pixbuf_get_from_window(window.get_window(),0,0,window.get_allocated_width(),window.get_allocated_height())
  pixbuf.savev(str(output/'dev-window.png'),'png',[],[])
  window.toggle_detach();assert window.detached is not None
  stage=2
 elif stage==2:
  assert view.ready and not view.failure
  assert view.color_scheme=='grey-camouflage' and not view.transparent_background
  view.capture_png(output/'detached-model.png')
  window.toggle_detach();assert window.detached is None
  stage=3
 elif stage==3:
  assert view.ready and not view.failure
  assert view.color_scheme=='grey-camouflage' and not view.transparent_background
  # Use a temporary stand-in for the stable lock; no real controller is touched.
  original=window.call.__func__.__globals__['ORIGINAL_STATE']
  standin=Path(temporary.name)/'stable-state'
  lock=standin/'opends/controller.lock';lock.parent.mkdir(parents=True)
  with lock.open('a+b') as owned:
   fcntl.flock(owned.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
   window.call.__func__.__globals__['ORIGINAL_STATE']=standin
   rejected=[];show_error=window.error;window.error=rejected.append
   window.call('bridge_start')
   window.error=show_error;window.call.__func__.__globals__['ORIGINAL_STATE']=original
   assert rejected and 'stable OpenDS' in rejected[0]
   assert window.stable_guard is None
  assert not window.state.get('running')
  window.shutdown();stage=4
 else:
  if window.service.poll() is None:return True
  loop.quit();return False
 return True

GLib.timeout_add(1200,step)
GLib.timeout_add(30000,lambda:(errors.append('Dev application test timed out'),window.shutdown(),loop.quit(),False)[-1])
loop.run()
if errors:raise SystemExit('\n'.join(errors))
assert window.service.poll()==0,window.service.returncode
temporary.cleanup()
print('Development app, 3D demo, detach/dock, bridge exclusion and shutdown PASS')
