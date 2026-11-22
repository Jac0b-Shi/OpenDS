import hashlib,json,math,sys,unittest
from pathlib import Path
import numpy as np
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT))
from scene import Scene,Animator,targets,CONTROLS
from appearance import camouflage_surface

class SceneTests(unittest.TestCase):
 @classmethod
 def setUpClass(cls):cls.scene=Scene(ROOT/'assets/dualsense.glb')
 def test_pinned_model_contains_all_independent_controls(self):
  source=json.loads((ROOT/'assets/SOURCE.json').read_text())
  self.assertEqual(hashlib.sha256((ROOT/'assets/dualsense.glb').read_bytes()).hexdigest(),source['sha256'])
  self.assertEqual(set(self.scene.controls),CONTROLS)
  self.assertGreater(len(self.scene.parts),60)
  self.assertTrue(np.all(np.isfinite(self.scene.minimum)))
 def test_raw_input_buttons_diagonals_stick_travel_and_full_trigger_range(self):
  result=targets({'sticks':[0,255,128,128],'triggers':[0,255],'buttons':[0x11,0x41,0x07]},[0x20,0,0])
  self.assertEqual((result['left-stick-x'],result['left-stick-y']),(-1,1))
  self.assertEqual((result['up'],result['right']),(1,1))
  self.assertEqual((result['square'],result['cross']),(1,1))
  self.assertEqual((result['left-stick'],result['ps'],result['touchpad'],result['mute']),(1,1,1,1))
  self.assertEqual((result['l2'],result['r2']),(0,1))
 def test_trigger_rotates_about_hinge_and_buttons_move_inward(self):
  scene=self.scene;pivot=scene.pivots['l2']
  pose=scene.pose('l2',{'l2':1})
  self.assertTrue(np.allclose(pose@np.append(pivot,1),np.append(pivot,1),atol=1e-6))
  self.assertLess(scene.pose('cross',{'cross':1})[2,3],0)
  self.assertTrue(np.allclose(scene.pose('cross',{}),np.eye(4)))
 def test_animation_returns_to_neutral_without_sticking(self):
  animation=Animator();goal=targets({'sticks':[255,0,128,128],'triggers':[255,0],'buttons':[0x28,0,0]})
  for _ in range(120):animation.step(goal,1/60)
  self.assertGreater(animation.values['l2'],.99)
  for _ in range(180):animation.step(targets({}),1/60)
  self.assertLess(max(abs(v) for v in animation.values.values()),.0001)
 def test_touch_markers_follow_curved_touchpad_surface(self):
  for x,y in [(-.6,1.3),(.6,.9),(0,1.1)]:
   height=self.scene.touch_height(x,y)
   self.assertTrue(.49<height<.84)
 def test_camouflage_has_plain_rear_and_printed_front_with_dark_controls(self):
  parts=self.scene.parts
  rear=next(p for p in parts if p['name'].startswith('back-shell__'))
  self.assertEqual(camouflage_surface(rear),'rear')
  printed=[p for p in parts if camouflage_surface(p)=='pattern']
  self.assertEqual({p['name'].split('__')[0] for p in printed},{'white-shell-left','white-shell-right','touchpad'})
  for p in parts:
   if p['control'] in ('up','right','down','left') and '__cap__' in p['name']:
    self.assertEqual(camouflage_surface(p),'dpad-cap')
   if p['control'] in ('triangle','square','cross','circle') and '__cap__' in p['name']:
    self.assertEqual(camouflage_surface(p),'face-cap')
   if p['name'].startswith(('lights__','l1__','r1__','l2__','r2__')):
    self.assertIsNone(camouflage_surface(p))

if __name__=='__main__':unittest.main()
