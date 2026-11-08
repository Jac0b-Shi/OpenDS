"""Verify actual page/checkbox pixels and live system-theme following.

All preferences and GNOME settings are disposable and confined to this process.
"""
import os
from pathlib import Path
import runpy
import sys
import tempfile
import time

ROOT=Path(__file__).resolve().parents[1]
temporary=tempfile.TemporaryDirectory(prefix='opends-3d-theme-test-')
os.environ['OPENDS_3D_PROFILE']=str(Path(temporary.name)/'profile')
os.environ['OPENDS_3D_STATE']=str(Path(temporary.name)/'state')
os.environ['GSETTINGS_BACKEND']='memory'
sys.argv=[str(ROOT/'app.py')]
module=runpy.run_path(str(ROOT/'app.py'),run_name='opends_theme_test')
Gtk,GLib,Gio=module['Gtk'],module['GLib'],module['Gio']
from gi.repository import Gdk
application=Gtk.Application(application_id='io.github.OpenDS.ThemeVerification',flags=Gio.ApplicationFlags.NON_UNIQUE)
application.register(None)
window=module['DevWindow'](application,str(module['BACKEND']),str(module['CONFIG']))
output=ROOT/'test-output';output.mkdir(exist_ok=True)
errors=[];loop=GLib.MainLoop();stage=0;started=time.monotonic()


def descendants(widget):
 yield widget
 if isinstance(widget,Gtk.Container):
  for child in widget.get_children():yield from descendants(child)


def snapshot(name,dark):
 assert window.dark is dark
 assert Gtk.Settings.get_default().get_property('gtk-application-prefer-dark-theme') is dark
 image=Gdk.pixbuf_get_from_window(window.get_window(),0,0,window.get_allocated_width(),window.get_allocated_height())
 image.savev(str(output/name),'png',[],[])
 data=image.get_pixels();stride=image.get_rowstride();channels=image.get_n_channels()
 def pixel(x,y):return tuple(data[y*stride+x*channels:y*stride+x*channels+3])
 # The unoccupied main header and center of an unchecked indicator must use
 # the chosen app palette even when the native widget theme is the opposite.
 background=pixel(image.get_width()-15,10)
 checkbox=window.monitor_canvas.demo
 x,y=checkbox.translate_coordinates(window,0,0)
 check=pixel(x+8,y+checkbox.get_allocated_height()//2)
 assert max(background)<50 if dark else min(background)>230,('page background',background,dark)
 assert max(check)<65 if dark else min(check)>240,('unchecked indicator',check,dark)
 print(name,'page',background,'checkbox',check,flush=True)


def exception(kind,value,tb):
 errors.append(str(value));sys.__excepthook__(kind,value,tb);window.shutdown();loop.quit()
sys.excepthook=exception


def step():
 global stage,combo
 if stage==7:
  if window.service.poll() is None:return True
  loop.quit();return False
 if not window.built:
  assert time.monotonic()-started<20,'UI did not start'
  return True
 assert not window.monitor_canvas.view.failure,window.monitor_canvas.view.failure
 if stage==0:
  combo=next(widget for widget in descendants(window.stack.get_child_by_name('general')) if isinstance(widget,Gtk.ComboBoxText) and widget.get_active_id() in ('system','light','dark'))
  # Simulate a native dark widget theme, then force the opposite app theme.
  Gtk.Settings.get_default().set_property('gtk-theme-name','Adwaita-dark')
  combo.set_active_id('light');stage=1
 elif stage==1:
  snapshot('theme-light.png',False)
  combo.set_active_id('dark');stage=2
 elif stage==2:
  snapshot('theme-dark.png',True)
  window.monitor_canvas.color_combo.set_active_id('grey-camouflage')
  window.monitor_canvas.view.front_camera()
  window.monitor_canvas.transparent_check.set_active(True)
  window.theme.system.set_string('color-scheme','prefer-light')
  combo.set_active_id('system');stage=3
 elif stage==3:
  snapshot('theme-system-light.png',False)
  window.theme.system.set_string('color-scheme','prefer-dark');stage=4
 elif stage==4:
  snapshot('theme-system-dark.png',True)
  # Explicit light must remain light when the system changes behind it.
  combo.set_active_id('light')
  window.theme.system.set_string('color-scheme','prefer-light')
  window.theme.system.set_string('color-scheme','prefer-dark');stage=5
 elif stage==5:
  snapshot('theme-explicit-light.png',False)
  window.navigate('general');combo.set_active_id('dark');stage=6
 elif stage==6:
  image=Gdk.pixbuf_get_from_window(window.get_window(),0,0,window.get_allocated_width(),window.get_allocated_height())
  image.savev(str(output/'theme-general-dark.png'),'png',[],[])
  window.shutdown();stage=7
 return True


GLib.timeout_add(700,step)
GLib.timeout_add(25000,lambda:(errors.append('Theme test timed out'),window.shutdown(),loop.quit(),False)[-1])
loop.run()
if errors:raise SystemExit('\n'.join(errors))
assert window.service.poll()==0,window.service.returncode
temporary.cleanup()
print('Light/dark page and checkbox pixels, live Follow system and explicit override PASS')
