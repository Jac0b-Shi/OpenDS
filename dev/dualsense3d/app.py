"""Isolated 3D development front end; stable application files are never updated."""
import configparser
import fcntl
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import time

HERE=Path(__file__).resolve().parent
BACKEND=HERE/'backend/opends'
ORIGINAL_CONFIG=Path(os.environ.get('XDG_CONFIG_HOME',str(Path.home()/'.config')))
ORIGINAL_STATE=Path(os.environ.get('XDG_STATE_HOME',str(Path.home()/'.local/state')))
arguments=list(sys.argv[1:])
PROFILE=Path(os.environ.get('OPENDS_3D_PROFILE',str(ORIGINAL_CONFIG/'opends-3d-dev')))
STATE=Path(os.environ.get('OPENDS_3D_STATE',str(ORIGINAL_STATE/'opends-3d-dev')))
CONFIG=PROFILE/'opends/opends.ini'


def prepare_profile():
    CONFIG.parent.mkdir(parents=True,exist_ok=True)
    created=not CONFIG.exists()
    if not CONFIG.exists():
        parser=configparser.ConfigParser(interpolation=None)
        stable=ORIGINAL_CONFIG/'opends/opends.ini'
        if stable.is_file():parser.read(stable,encoding='utf-8')
        if not parser.has_section('opends'):parser.add_section('opends')
        parser['opends']['auto_start']='false'
        parser['opends']['port']='3242'
        parser['opends']['close_action']='ask'
        with CONFIG.open('w',encoding='utf-8') as output:parser.write(output)
    os.environ['XDG_CONFIG_HOME']=str(PROFILE)
    os.environ['XDG_STATE_HOME']=str(STATE)
    if created:
        subprocess.run([str(BACKEND),'config','import',str(CONFIG)],check=True,capture_output=True)
    if '--smoke' in arguments:os.environ['OPENDS_UI_SMOKE']='1'


if not BACKEND.is_file():raise SystemExit('Prepare the development bundle first: ./dev/dualsense3d/prepare.sh')
prepare_profile()
sys.path.insert(0,str(HERE))
sys.argv=[str(HERE/'app.py'),str(BACKEND),str(CONFIG)]+(['--tray'] if '--tray' in arguments else [])
spec=importlib.util.spec_from_file_location('opends_stable_ui',HERE/'base/gui.py')
ui=importlib.util.module_from_spec(spec);spec.loader.exec_module(ui)
from renderer import ControllerView
import appearance
from theme import Theme
from gi.repository import Gtk,Gio,GLib
import cairo

APP_ID='io.github.OpenDS.Dev3D'
ui.APP_ID=APP_ID
OriginalWindow=ui.Window
OriginalMonitor=ui.MonitorCanvas


class Waveforms(Gtk.DrawingArea):
    def __init__(self,owner):
        super().__init__();self.owner=owner
        self.set_size_request(310,410)
        self.connect('draw',self.draw)

    def draw(self,widget,cr):
        width,height=self.get_allocated_width(),self.get_allocated_height()
        virtual=(width+32)/.53;left=min(virtual*.47,470)
        class Viewport:
            owner=self.owner
            def get_allocated_width(self):return virtual
            def get_allocated_height(self):return height
        cr.save();cr.rectangle(0,0,width,height);cr.clip();cr.translate(-left-16,0)
        OriginalMonitor.draw(Viewport(),widget,cr)
        cr.restore();return False


class Monitor3D(Gtk.Box):
    def __init__(self,owner):
        super().__init__(orientation=Gtk.Orientation.VERTICAL,spacing=8)
        self.owner=owner;self.view=ControllerView(owner)
        self.appearance_path=CONFIG.parent/'view3d.json'
        self.appearance=appearance.load(self.appearance_path)
        self.view.color_scheme=self.appearance['color']
        self.view.transparent_background=self.appearance['transparent']
        self.set_size_request(850,465)
        toolbar=ui.box(True,7)
        ui.pack(toolbar,ui.label('DualSense · 3D studio','section-title'),True)
        ui.pack(toolbar,ui.button('Front',lambda:self.view.front_camera()))
        ui.pack(toolbar,ui.button('Reset view',lambda:self.view.reset_camera()))
        motion=Gtk.CheckButton(label='Follow tilt')
        motion.set_tooltip_text('Experimental gravity-relative tilt; reset view to recenter. This is not calibrated absolute orientation.')
        motion.connect('toggled',lambda checkbox:self.set_tilt(checkbox.get_active()))
        ui.pack(toolbar,motion)
        self.demo=Gtk.CheckButton(label='Demo animation')
        self.demo.connect('toggled',self.demo_changed);ui.pack(toolbar,self.demo)
        menu=Gtk.MenuButton(label='Appearance')
        popover=Gtk.Popover.new(menu);options=ui.box(False,10)
        for method in (options.set_margin_start,options.set_margin_end,options.set_margin_top,options.set_margin_bottom):method(14)
        ui.pack(options,ui.label('Shell color','section-title'))
        self.color_combo=Gtk.ComboBoxText()
        for key,(name,_,_) in appearance.COLORS.items():self.color_combo.append(key,name)
        self.color_combo.set_active_id(self.appearance['color']);ui.pack(options,self.color_combo)
        self.transparent_check=Gtk.CheckButton(label='Transparent background')
        self.transparent_check.set_active(self.appearance['transparent']);ui.pack(options,self.transparent_check)
        ui.pack(options,ui.label('Color previews · approximate finishes','muted'))
        self.color_combo.connect('changed',self.appearance_changed)
        self.transparent_check.connect('toggled',self.appearance_changed)
        popover.add(options);options.show_all();menu.set_popover(popover);ui.pack(toolbar,menu)
        ui.pack(toolbar,ui.button('Credits',self.credits));ui.pack(self,toolbar)
        row=ui.box(True,12)
        overlay=Gtk.Overlay();overlay.add(self.view);self.stage=overlay
        self.badge=ui.label('NO LIVE INPUT')
        self.badge.get_style_context().add_class('three-badge')
        self.badge.set_halign(Gtk.Align.START);self.badge.set_valign(Gtk.Align.START)
        self.badge.set_margin_start(12);self.badge.set_margin_top(12)
        overlay.add_overlay(self.badge);overlay.set_overlay_pass_through(self.badge,True)
        self.problem=ui.label('',wrap=True);self.problem.set_halign(Gtk.Align.CENTER);self.problem.set_valign(Gtk.Align.CENTER)
        self.problem.set_margin_start(25);self.problem.set_margin_end(25)
        self.problem.get_style_context().add_class('three-problem')
        self.problem.set_no_show_all(True);overlay.add_overlay(self.problem)
        ui.pack(row,overlay,True)
        self.waves=Waveforms(owner);ui.pack(row,self.waves,True);ui.pack(self,row,True)
        self.readout=ui.label('LS 128 / 128   ·   RS 128 / 128   ·   L2 0% / R2 0%','muted');ui.pack(self,self.readout)
        ui.pack(self,ui.label('Drag the body to orbit · scroll to zoom · click buttons / drag sticks and triggers to preview · R to reset','muted',True))
        self.last_signature=None

    def recreate_view(self):
        # GTK GL contexts belong to a toplevel surface. Rebuild on detach/dock
        # rather than reusing texture/FBO names from an unrealized context.
        previous=self.view
        settings={name:getattr(previous,name) for name in ['yaw','pitch','distance','demo','follow_tilt','started','color_scheme','transparent_background']}
        pose=dict(previous.animation.values)
        self.stage.remove(previous);previous.destroy()
        self.view=ControllerView(self.owner)
        for name,value in settings.items():setattr(self.view,name,value)
        self.view.animation.values.update(pose)
        self.stage.add(self.view);self.view.show()
        self.problem.hide();self.last_signature=None

    def set_tilt(self,value):
        self.view.follow_tilt=value;self.view.tilt_reference=None;self.view.changed=True

    def appearance_changed(self,*_):
        self.appearance={'color':self.color_combo.get_active_id(),'transparent':self.transparent_check.get_active()}
        self.view.color_scheme=self.appearance['color'];self.view.transparent_background=self.appearance['transparent']
        self.view.changed=True;self.view.queue_render()
        try:appearance.save(self.appearance_path,self.appearance)
        except OSError as error:self.owner.error('Could not save 3D appearance: '+str(error))

    def demo_changed(self,checkbox):
        self.view.demo=checkbox.get_active();self.view.started=time.monotonic();self.view.changed=True;self.queue_draw()

    def credits(self):
        dialog=Gtk.Dialog(title='3D model credits',transient_for=self.owner,modal=True)
        dialog.add_button('Close',Gtk.ResponseType.CLOSE)
        content=dialog.get_content_area();content.set_spacing(10)
        for method in (content.set_margin_start,content.set_margin_end,content.set_margin_top,content.set_margin_bottom):method(18)
        content.add(ui.label('PS5 Controller — Taohid Animation','section-title'))
        content.add(ui.label('CC BY 4.0 · prepared interactive mesh by Safa Elmali\nOpenDS adaptation: native OpenGL renderer, lighting, control animation and input integration.',wrap=True))
        content.add(Gtk.LinkButton.new_with_label('https://sketchfab.com/3d-models/ps5-controller-b7bb9c5102a04cb0b1966c6d02bad7d6','Original model'))
        content.add(Gtk.LinkButton.new_with_label('https://creativecommons.org/licenses/by/4.0/','Model license'))
        dialog.show_all();dialog.run();dialog.destroy()

    def queue_draw(self):
        if not hasattr(self,'view'):return
        self.waves.queue_draw()
        data=self.owner.state
        state=data.get('input',{});sticks=state.get('sticks',[128]*4);triggers=state.get('triggers',[0,0])
        self.readout.set_text(f'LS {sticks[0]:3} / {sticks[1]:3}   ·   RS {sticks[2]:3} / {sticks[3]:3}   ·   L2 {triggers[0]*100//255}% / R2 {triggers[1]*100//255}%')
        self.badge.set_text('DEMO · VISUAL ONLY' if self.view.demo else 'LOCAL PREVIEW · VISUAL ONLY' if self.view.preview else 'LIVE INPUT' if data.get('capture') and data.get('connected') else 'CAPTURE PAUSED' if data.get('connected') else 'NO LIVE INPUT')
        if self.view.failure:
            self.problem.set_text('3D renderer unavailable\n'+self.view.failure);self.problem.show()
        signature=(str(state),str(data.get('pressed')),data.get('connected'),data.get('capture'))
        if signature!=self.last_signature:self.view.changed=True;self.last_signature=signature


class DevWindow(OriginalWindow):
    def __init__(self,*args,**kwargs):
        self.stable_guard=None;self.guard_observed_running=False;self.guard_started=0
        super().__init__(*args,**kwargs)
        self.set_title('OpenDS · 3D Dev')
        self.set_icon_name('opends-3d')
        self.set_default_size(1280,850)
        self.set_size_request(1160,760)

    def build(self,ready):
        super().build(ready)
        self.set_title('OpenDS · 3D Dev')
        self.notice_label.set_text('3D development version · separate settings · interactive controller preview')
        self.navigate('monitor')
        if '--demo' in arguments:self.monitor_canvas.demo.set_active(True)

    def apply_theme(self):
        if not hasattr(self,'theme'):self.theme=Theme(self)
        self.theme.apply()

    def toggle_detach(self):
        super().toggle_detach()
        self.monitor_canvas.recreate_view()

    def call(self,operation,success=None,failure=None,**values):
        if operation=='bridge_start':
            if self.stable_guard is not None:
                self.error('The development bridge is already starting or running.');return
            try:
                lock=ORIGINAL_STATE/'opends/controller.lock';lock.parent.mkdir(parents=True,exist_ok=True)
                self.stable_guard=lock.open('a+b')
                fcntl.flock(self.stable_guard.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
                self.guard_observed_running=False;self.guard_started=time.monotonic()
            except BlockingIOError:
                self.close_guard();self.error('Close the stable OpenDS window before starting the bridge in 3D Dev. Both interfaces can be previewed, but only one can own the controller.');return
            except OSError as error:self.close_guard();self.error(str(error));return
            old_failure=failure
            def failed():
                self.close_guard()
                if old_failure:old_failure()
            failure=failed
        return super().call(operation,success,failure,**values)

    def tick(self):
        result=super().tick()
        if self.stable_guard:
            if self.state.get('running'):self.guard_observed_running=True
            elif self.guard_observed_running or time.monotonic()-self.guard_started>15:self.close_guard()
        return result

    def close_guard(self):
        if self.stable_guard:
            self.stable_guard.close();self.stable_guard=None

    def autostart_path(self):
        return ORIGINAL_CONFIG/'autostart'/f'{APP_ID}.desktop'

    def write_autostart(self,enabled):
        path=self.autostart_path()
        if enabled:
            path.parent.mkdir(parents=True,exist_ok=True)
            path.write_text('[Desktop Entry]\nType=Application\nName=OpenDS 3D Dev\nExec=/usr/bin/python3 '+ui.desktop_quote(HERE/'app.py')+' --tray\nIcon=opends-3d\nTerminal=false\nX-GNOME-Autostart-enabled=true\n',encoding='utf-8')
        else:path.unlink(missing_ok=True)

    def finish_shutdown(self):
        if self.service.poll() is None:return True
        self.close_guard()
        restart=self.restart_pending;self.restart_pending=False
        result=super().finish_shutdown()
        if restart:os.execv('/usr/bin/python3',['/usr/bin/python3',str(HERE/'app.py')])
        return result


ui.Window=DevWindow
ui.MonitorCanvas=Monitor3D

if __name__=='__main__':
    raise SystemExit(ui.main())
