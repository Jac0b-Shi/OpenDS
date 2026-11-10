"""Real OpenGL render/readback check; no controller, bridge or stable-app writes."""
import os,sys,time
import copy
from pathlib import Path
root=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(root))
import gi
gi.require_version('Gtk','3.0')
from gi.repository import Gtk,GLib
from types import SimpleNamespace
from renderer import ControllerView
import numpy as np
from gi.repository import GdkPixbuf

owner=SimpleNamespace(state={'connected':False,'capture':False})
window=Gtk.Window(title='OpenDS 3D render verification');window.set_default_size(1000,720)
view=ControllerView(owner);window.add(view);window.show_all()
errors=[];started=time.monotonic();stage=0
def exception(kind,value,tb):
 errors.append(str(value));sys.__excepthook__(kind,value,tb);Gtk.main_quit()
sys.excepthook=exception
output=root/'test-output';output.mkdir(exist_ok=True)
def step():
 global stage
 if view.failure:
  errors.append(view.failure);Gtk.main_quit();return False
 if not view.ready or view.frame_count<2:
  if time.monotonic()-started>25:errors.append('GL renderer did not produce frames');Gtk.main_quit();return False
  view.queue_render();return True
 if stage==0:
  view.capture_png(output/'neutral.png')
  def pixels(name):
   image=GdkPixbuf.Pixbuf.new_from_file(str(output/name))
   return np.frombuffer(image.get_pixels(),np.uint8).reshape(image.get_height(),image.get_rowstride())[:,:image.get_width()*4].reshape(image.get_height(),image.get_width(),4)
  neutral=pixels('neutral.png')
  assert neutral[0,0,3]==0 and neutral[-1,-1,3]==0,'Background must be transparent'
  assert np.count_nonzero(neutral[:,:,3]==255)>10000,'Controller body must remain opaque'
  view.color_scheme='galactic-purple';view.changed=True
  view.neutral=neutral.copy()
  print('OpenGL:',view.gl_version,'Renderer:',view.gl_renderer,flush=True)
  # Pick the actual front cap through the camera, rather than a 2D hit region.
  cap=next(p for p in view.scene.parts if 'cross__cap__' in p['name'])
  point=(cap['min']+cap['max'])/2;point[2]=cap['max'][2]
  clip=view.projection@view.view@view.transforms[cap['name']]@__import__('numpy').append(point,1)
  ndc=clip[:3]/clip[3]
  x=(ndc[0]+1)*view.get_allocated_width()/2;y=(1-ndc[1])*view.get_allocated_height()/2
  assert view.hit(x,y)=='cross','3D ray picking did not select the cross button'
  before=copy.deepcopy(owner.state)
  from gi.repository import Gdk
  event=SimpleNamespace(x=x,y=y,button=1,state=0,type=Gdk.EventType.BUTTON_PRESS)
  view.press(view,event)
  assert view.preview.get('cross')==1
  view.release()
  assert not view.preview and owner.state==before,'Visual preview must never change live input'
  yaw=view.yaw
  view.press(view,SimpleNamespace(x=20,y=20,button=1,state=0,type=Gdk.EventType.BUTTON_PRESS))
  view.motion(view,SimpleNamespace(x=80,y=40));view.release()
  assert view.yaw!=yaw
  view.reset_camera()
  owner.state={'connected':True,'capture':True,'input':{'sticks':[230,24,48,210],'triggers':[220,160],'buttons':[0xf0,0xc3,7],'touch':[[1,450,300],[1,1500,800]]},'pressed':[0,0,0]}
  stage=1;return True
 if stage==1:
  view.capture_png(output/'active.png')
  assert view.animation.values['l2']>.5
  assert abs(view.animation.values['left-stick-x'])>.3
  image=GdkPixbuf.Pixbuf.new_from_file(str(output/'active.png'))
  rgba=np.frombuffer(image.get_pixels(),np.uint8).reshape(image.get_height(),image.get_rowstride())[:,:image.get_width()*4].reshape(image.get_height(),image.get_width(),4)
  # Colored shell remains visible; black trim and independently animated controls survive.
  assert np.count_nonzero((rgba[:,:,2].astype(int)-rgba[:,:,1])>25)>5000,'Purple shell was not rendered'
  view.transparent_background=False;view.changed=True
  stage=2;return True
 if stage==2:
  view.capture_png(output/'studio.png')
  image=GdkPixbuf.Pixbuf.new_from_file(str(output/'studio.png'))
  assert image.get_pixels()[3]==255,'Studio backdrop should be opaque'
  view.color_scheme='grey-camouflage';view.transparent_background=True;view.changed=True
  stage=3;return True
 if stage==3:
  view.capture_png(output/'camouflage.png')
  print('Model parts:',len(view.scene.parts),'controls:',len(view.scene.controls),'frames:',view.frame_count,flush=True)
  window.destroy();Gtk.main_quit();return False
 return True
GLib.timeout_add(1000,step)
Gtk.main()
if errors:raise SystemExit('\n'.join(errors))
print('3D GPU render and live animation readback PASS')
