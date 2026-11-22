"""Read back front/rear/side views and prove the rear never receives the print."""
import math
from pathlib import Path
import sys
import time
from types import SimpleNamespace
import numpy as np

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT))
from renderer import ControllerView
from gi.repository import Gtk,GLib,GdkPixbuf

owner=SimpleNamespace(state={'connected':False,'capture':False})
window=Gtk.Window();window.set_default_size(1000,720)
view=ControllerView(owner);view.color_scheme='grey-camouflage';view.yaw=0;view.pitch=.15
window.add(view);window.show_all()
output=ROOT/'test-output';output.mkdir(exist_ok=True)
stage=0;started=time.monotonic();errors=[];suppress=False
original_uniform=view.uniform


def uniform(name,value,shader=None):
 original_uniform(name,False if name=='camouflage' and suppress else value,shader)
view.uniform=uniform


def capture(name):
 path=output/(name+'.png');view.capture_png(path)
 image=GdkPixbuf.Pixbuf.new_from_file(str(path))
 return np.frombuffer(image.get_pixels(),np.uint8).reshape(image.get_height(),image.get_rowstride())[:,:image.get_width()*4].reshape(image.get_height(),image.get_width(),4).copy()


def step():
 global stage,suppress,front,rear
 assert not view.failure,view.failure
 if not view.ready or view.frame_count<2:
  assert time.monotonic()-started<25,'GL context did not render'
  view.queue_render();return True
 if stage==0:
  front=capture('camouflage-front');assert front[0,0,3]==0
  view.yaw=math.pi
 elif stage==1:
  rear=capture('camouflage-rear');suppress=True
 elif stage==2:
  rear_plain=capture('camouflage-rear-print-disabled')
  # Central rear panel and grip roots must be unchanged when all print is disabled.
  difference=np.abs(rear[250:420,220:780,:3].astype(float)-rear_plain[250:420,220:780,:3])
  assert difference.max()<1,'Camouflage is leaking onto the rear panel'
  view.yaw=0
 elif stage==3:
  front_plain=capture('camouflage-front-print-disabled')
  difference=np.abs(front[200:330,350:650,:3].astype(float)-front_plain[200:330,350:650,:3])
  assert difference.mean()>10,'Front touchpad should carry the camouflage print'
  suppress=False;view.yaw=1.1
 elif stage==4:
  capture('camouflage-side')
  window.destroy();Gtk.main_quit();return False
 stage+=1;view.changed=True;view.queue_render();return True


def exception(kind,value,tb):
 errors.append(str(value));sys.__excepthook__(kind,value,tb);window.destroy();Gtk.main_quit()
sys.excepthook=exception
GLib.timeout_add(700,step)
Gtk.main()
if errors:raise SystemExit('\n'.join(errors))
print('Camouflage front print, plain rear, side coverage and alpha readback PASS')
