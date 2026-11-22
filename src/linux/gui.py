"""Native GTK application matching the Windows OpenDS pages and graphics."""
import configparser
import ctypes
import datetime
import io
import json
import os
from pathlib import Path
import queue
import shlex
import signal
import subprocess
import sys
import threading
import time
import wave

import gi
gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, Gio, GLib, Gtk
import cairo

EXE = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[2] / "target/release/opends"
sys.path.insert(0, str(EXE.parent / "ui"))
from graphics import HistoryCanvas, MonitorCanvas

APP_ID = "io.github.OpenDS"
TRANSLATIONS = {}

def translate(value):
    return TRANSLATIONS.get(str(value), str(value))

def check_button(value):
    return Gtk.CheckButton(label=translate(value))


def label(value="", css=None, wrap=False):
    obj = Gtk.Label(label=translate(value), xalign=0)
    obj.set_line_wrap(wrap)
    if css:
        obj.get_style_context().add_class(css)
    return obj


def button(value, callback, css=None):
    obj = Gtk.Button(label=translate(value))
    obj.connect("clicked", lambda *_: callback())
    if css:
        obj.get_style_context().add_class(css)
    return obj


def box(horizontal=False, spacing=10):
    return Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL if horizontal else Gtk.Orientation.VERTICAL, spacing=spacing)


def pack(parent, child, expand=False):
    parent.pack_start(child, expand, expand, 0)
    return child


def desktop_quote(value):
    return '"' + str(value).replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$').replace('%', '%%') + '"'


class Window(Gtk.ApplicationWindow):
    def __init__(self, application, executable, path):
        super().__init__(application=application, title="OpenDS")
        self.executable = str(executable)
        self.path = Path(path)
        self.set_default_size(1100, 790)
        self.set_size_request(980, 710)
        self.set_wmclass(APP_ID, APP_ID)
        self.set_icon_name("opends")
        self.icon = Path(executable).parent / "opends.png"
        if self.icon.exists():
            self.set_icon_from_file(str(self.icon))
        self.connect("delete-event", self.on_close)
        self.state = {}
        self.pending_state = None
        self.messages = queue.Queue(maxsize=256)
        self.callbacks = {}
        self.next_id = 0
        self.strings = {}
        self.fields = {}
        self.config = configparser.ConfigParser(interpolation=None)
        self.config.read(self.path, encoding="utf-8")
        if not self.config.has_section("opends"):
            if self.config.has_section("ds5"):
                self.config["opends"] = dict(self.config["ds5"])
                self.config.remove_section("ds5")
            else:
                self.config.add_section("opends")
        self.section = self.config["opends"]
        self.dark = False
        self.built = False
        self.closing = False
        self.restart_pending = False
        self.overlay = None
        self.overlay_locked = False
        self.detached = None
        self.tray = None
        self.last_logs = None
        self.microphone_window = None
        self.microphone_recording = False
        self.microphone_generation = 0
        self.microphone_done = set()
        self.microphone_cancel = threading.Event()
        self.playback = None
        self.child = subprocess.Popen([self.executable, "service"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        self.service = self.child
        threading.Thread(target=self.read_service, daemon=True).start()
        threading.Thread(target=self.read_errors, daemon=True).start()
        self.splash = label("Starting OpenDS…", "page-title")
        self.splash.set_margin_start(30)
        self.add(self.splash)
        self.show_all()
        GLib.timeout_add(16, self.tick)

    def T(self, key, fallback):
        return self.strings.get(key, fallback)

    def get(self, key, fallback=""):
        return self.section.get(key, fallback)

    def flag(self, key, fallback=False):
        return self.get(key, str(fallback)).lower() in ("true", "1", "yes")

    def set(self, key, value):
        self.section[key] = str(value).lower() if isinstance(value, bool) else str(value)

    def read_service(self):
        try:
            for line in self.service.stdout:
                message = json.loads(line)
                if message.get("type") == "state":
                    previous = self.pending_state
                    if previous and "pressed" in previous and "pressed" in message:
                        message["pressed"] = [a | b for a, b in zip(previous["pressed"], message["pressed"])]
                    self.pending_state = message
                else:
                    self.messages.put(message)
        except (OSError, ValueError) as error:
            self.messages.put({"type": "failure", "error": str(error)})

    def read_errors(self):
        for line in self.service.stderr:
            try:
                self.messages.put_nowait({"type": "log", "text": line.rstrip()})
            except queue.Full:
                pass

    def call(self, operation, success=None, failure=None, **values):
        if self.closing and operation != "quit":
            return
        self.next_id += 1
        self.callbacks[self.next_id] = (success, failure)
        try:
            self.service.stdin.write(json.dumps({"id": self.next_id, "op": operation, **values}, ensure_ascii=False) + "\n")
            self.service.stdin.flush()
        except (BrokenPipeError, OSError):
            self.callbacks.pop(self.next_id, None)
            self.error("The OpenDS background service stopped. Reopen OpenDS to reconnect.")

    def error(self, message):
        if self.closing:
            return
        dialog = Gtk.MessageDialog(transient_for=self, modal=True, message_type=Gtk.MessageType.ERROR, buttons=Gtk.ButtonsType.CLOSE, text="OpenDS")
        dialog.format_secondary_text(str(message))
        dialog.run()
        dialog.destroy()

    def notice(self, message):
        self.notice_label.set_text(str(message))

    def read_fields(self):
        for key, getter in self.fields.items():
            self.set(key, getter())

    def persist(self, notice=True, fields=False, done=None):
        if fields:
            self.read_fields()
        output = io.StringIO()
        self.config.write(output)
        def saved(_):
            if notice:
                self.notice(self.T("gui_settings_saved_restart_the_bridge_to_apply","Settings saved. Restart the bridge to apply connection changes."))
            self.apply_theme()
            self.apply_overlay()
            if done:
                done()
        self.call("configure", success=saved, text=output.getvalue())

    def page(self, key, title):
        scroll = Gtk.ScrolledWindow()
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        content = box(spacing=16)
        for method in (content.set_margin_start, content.set_margin_end, content.set_margin_top, content.set_margin_bottom):
            method(24)
        scroll.add(content)
        self.stack.add_named(scroll, key)
        return content

    def card(self, parent, title=None):
        frame = box(spacing=12)
        frame.get_style_context().add_class("card")
        for method in (frame.set_margin_start, frame.set_margin_end):
            method(0)
        pack(parent, frame, parent.get_orientation() == Gtk.Orientation.HORIZONTAL)
        if title:
            pack(frame, label(title, "section-title"))
        return frame

    def combo(self, key, entries, parent=None, changed=None):
        obj = Gtk.ComboBoxText()
        for value, title in entries:
            obj.append(str(value), translate(title))
        obj.set_active_id(self.get(key, str(entries[0][0])))
        if obj.get_active() < 0:
            obj.set_active(0)
        self.fields[key] = obj.get_active_id
        if changed:
            obj.connect("changed", lambda *_: changed(obj.get_active_id()))
        if parent:
            pack(parent, obj)
        return obj

    def check(self, key, title, parent, changed=None):
        obj = check_button(title)
        obj.set_active(self.flag(key))
        self.fields[key] = obj.get_active
        if changed:
            obj.connect("toggled", lambda *_: changed(obj.get_active()))
        pack(parent, obj)
        return obj

    def spin(self, key, low, high, parent, title, step=1):
        row = box(True)
        pack(row, label(title), True)
        adjustment = Gtk.Adjustment(float(self.get(key, str(low))), low, high, step, step*10, 0)
        obj = Gtk.SpinButton(adjustment=adjustment, climb_rate=1, digits=2 if step < 1 else 0)
        obj.set_width_chars(7)
        pack(row, obj)
        pack(parent, row)
        self.fields[key] = obj.get_value if step < 1 else obj.get_value_as_int
        return obj

    def build(self, ready):
        self.strings = ready.get("strings", {})
        TRANSLATIONS.update({value: self.strings.get(key,value) for key,value in ready.get("english_strings",{}).items()})
        self.languages = ready.get("languages", [])
        self.remove(self.splash)
        self.layout = box(True, 0)
        self.add(self.layout)
        sidebar = box(spacing=8)
        sidebar.set_size_request(205, -1)
        sidebar.get_style_context().add_class("sidebar")
        brand = box(True)
        image = Gtk.Image.new_from_file(str(self.icon)) if self.icon.exists() else Gtk.Image.new_from_icon_name("input-gaming-symbolic", Gtk.IconSize.DIALOG)
        image.set_pixel_size(36)
        if self.icon.exists():
            from gi.repository import GdkPixbuf
            image.set_from_pixbuf(GdkPixbuf.Pixbuf.new_from_file_at_scale(str(self.icon),36,36,True))
        pack(brand,image)
        pack(brand,label("OpenDS","brand"))
        pack(sidebar,brand)
        pack(sidebar,label("DualSense · " + ready.get("version", ""), "muted"))
        spacer=Gtk.Box();spacer.set_size_request(-1,26);pack(sidebar,spacer)
        self.stack = Gtk.Stack()
        self.stack.set_transition_type(Gtk.StackTransitionType.NONE)
        self.nav = {}
        self.titles = {}
        definitions=[("overview","gui-overview","Overview"),("monitor","gui-live-monitor","Live monitor"),("connection","gui-connection-feedback","Connection & feedback"),("hiding","gui-device-hiding","Device hiding"),("diagnostics","gui-diagnostics-logs","Diagnostics & logs"),("general","language-settings-title","General settings")]
        for key, message, fallback in definitions:
            title=self.T(message,fallback)
            self.titles[key]=title
            obj=button(title,lambda name=key:self.navigate(name))
            obj.set_halign(Gtk.Align.FILL)
            obj.get_style_context().add_class("navigation")
            pack(sidebar,obj)
            self.nav[key]=obj
        pack(sidebar,Gtk.Box(),True)
        pack(sidebar,button(self.T("gui-minimize-to-tray","Minimize to tray"),self.hide_to_tray))
        pack(sidebar,button(self.T("gui-exit-opends","Exit OpenDS"),self.shutdown))
        pack(self.layout,sidebar)
        main=box(spacing=0)
        header=box(spacing=4)
        for method in (header.set_margin_start,header.set_margin_end):method(24)
        header.set_margin_top(24);header.set_margin_bottom(8)
        self.page_title=label("Overview","page-title");pack(header,self.page_title)
        self.notice_label=label("Bluetooth input · USB virtualization · bidirectional feedback","muted",True)
        pack(header,self.notice_label)
        pack(main,header)
        pack(main,self.stack,True)
        pack(self.layout,main,True)
        self.overview_page()
        self.monitor_page()
        self.connection_page()
        self.hiding_page()
        self.diagnostics_page()
        self.general_page()
        self.setup_tray()
        self.built=True
        self.apply_theme()
        self.navigate("overview")
        self.show_all()
        self.call("devices",self.devices_received)
        self.diagnose()
        if self.flag("auto_start") and not os.environ.get("OPENDS_UI_SMOKE"):
            self.call("bridge_start")
        if "--tray" in sys.argv:
            self.hide_to_tray()
        if os.environ.get("OPENDS_UI_SMOKE") == "1":
            GLib.timeout_add(1500,lambda:(self.shutdown(),False)[1])
        self.smoke_ready=True

    def navigate(self,key):
        self.stack.set_visible_child_name(key)
        self.page_title.set_text(self.titles[key])
        for name,obj in self.nav.items():
            context=obj.get_style_context()
            context.add_class("selected") if name==key else context.remove_class("selected")
        self.update_visibility()

    def overview_page(self):
        page=self.page("overview","Overview")
        hero=self.card(page)
        row=box(True,22)
        left=box()
        self.phase=label(self.T("gui-bridge-not-started","Bridge not started"),"phase",True)
        pack(left,self.phase)
        actions=box(True)
        self.start_button=button(self.T("gui-start-bridge","Start bridge"),lambda:self.call("bridge_start"),"primary")
        self.stop_button=button(self.T("gui-stop","Stop"),lambda:self.call("bridge_stop"))
        pack(actions,self.start_button);pack(actions,self.stop_button)
        pack(actions,button(self.T("gui-connection-details","Connection details"),self.connection_details))
        pack(left,actions)
        pack(row,left,True)
        battery=box(spacing=3)
        self.battery_number=label("--%","battery-number")
        self.battery_caption=label("Disconnected","muted")
        pack(battery,self.battery_number);pack(battery,self.battery_caption)
        pack(row,battery);pack(hero,row)
        stats=box(True,16);pack(page,stats)
        self.counters=[]
        for title in ["Input reports","USB reports","Audio blocks"]:
            card=self.card(stats,title)
            number=label("0","counter");pack(card,number);self.counters.append(number)
            card.set_hexpand(True)
        logs=self.card(page,self.T("gui-diagnostics-logs","Recent activity"))
        self.overview_log=self.text_view(logs,240)
        self.log_buffers=[self.overview_log.get_buffer()]

    def text_view(self,parent,height=170):
        view=Gtk.TextView();view.set_editable(False);view.set_cursor_visible(False)
        view.set_monospace(True);view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        scroll=Gtk.ScrolledWindow();scroll.set_min_content_height(height)
        scroll.add(view);pack(parent,scroll,True)
        return view

    def connection_details(self):
        lines=[self.phase.get_text(),self.state.get("error") or "",self.state.get("logs","").splitlines()[-1] if self.state.get("logs") else "",f"Input {self.state.get('counts',[0]*3)[0]} · drops {self.state.get('drops',0)}"]
        dialog=Gtk.MessageDialog(transient_for=self,modal=True,buttons=Gtk.ButtonsType.CLOSE,text="Connection details")
        dialog.format_secondary_text("\n".join(v for v in lines if v));dialog.run();dialog.destroy()

    def monitor_page(self):
        page=self.page("monitor","Live monitor")
        self.monitor_host=page
        self.monitor_body=box(spacing=10)
        pack(page,self.monitor_body)
        controls=box(True,8)
        self.capture_button=button(self.T("monitor-view-start-capture","Start capture"),self.toggle_capture,"primary")
        pack(controls,self.capture_button)
        pack(controls,button(self.T("monitor-view-clear","Clear"),lambda:self.call("clear")))
        self.detach_button=button(self.T("monitor-view-separate-window","Separate window"),self.toggle_detach)
        pack(controls,self.detach_button)
        self.record_button=button(self.T("monitor-view-start-file-recording","Start file recording"),self.toggle_record)
        pack(controls,self.record_button)
        pack(self.monitor_body,controls)
        signals=box(True,9)
        for forwarded,title in [(False,self.T("monitor-view-source","Source")),(True,self.T("monitor-view-forwarded","Forwarded"))]:
            obj=check_button(title);obj.set_active(not forwarded)
            obj.connect("toggled",lambda o,f=forwarded:self.call("signal",forwarded=f,enabled=o.get_active()))
            pack(signals,obj)
        for key,title,values,suffix in [("monitor_sample_hz","Sample",[20,50,100,200]," Hz"),("monitor_fps","Refresh",[15,30,60]," FPS"),("monitor_seconds","History",[30,60,120,300,600,900]," s")]:
            pack(signals,label(title,"muted"))
            obj=self.combo(key,[(v,str(v)+suffix) for v in values],changed=lambda value,k=key:self.monitor_preference(k,value))
            pack(signals,obj)
        pack(self.monitor_body,signals)
        self.monitor_canvas=MonitorCanvas(self)
        pack(self.monitor_body,self.monitor_canvas,True)
        historybar=box(True)
        self.history_enabled=check_button("Input history");self.history_enabled.set_active(self.flag("command_enabled",True))
        self.history_enabled.connect("toggled",lambda o:self.command_preference("enabled",o.get_active()))
        pack(historybar,self.history_enabled,True)
        pack(historybar,button("Input history settings…",self.history_settings))
        pack(historybar,button("Overlay",self.show_overlay))
        self.overlay_lock_button=button("Lock overlay",self.lock_overlay)
        pack(historybar,self.overlay_lock_button)
        pack(historybar,button("Restore overlay",self.restore_overlay))
        pack(self.monitor_body,historybar)
        self.history_canvas=HistoryCanvas(self)
        pack(self.monitor_body,self.history_canvas)
        self.monitor_footer=label("Waiting for manual start","muted",True)
        pack(self.monitor_body,self.monitor_footer)
        self.detached_hint=button("Bring the separate monitor back",self.toggle_detach)
        pack(page,self.detached_hint)
        self.detached_hint.set_no_show_all(True)

    def toggle_capture(self):
        self.call("capture_stop" if self.state.get("capture") else "capture_start")

    def monitor_preference(self,key,value):
        self.set(key,value);self.persist(notice=False)

    def command_preference(self,key,value):
        self.set("command_"+key,value);self.persist(notice=False)

    def toggle_record(self):
        if self.state.get("recording",{}).get("active"):
            self.call("record_stop");return
        path=self.file_dialog("Choose a new CSV recording",save=True,filename="opends-"+datetime.datetime.now().strftime("%Y%m%d-%H%M%S")+".csv")
        if path:self.call("record_start",path=path)

    def toggle_detach(self):
        if self.detached:
            self.monitor_body.get_parent().remove(self.monitor_body);self.monitor_host.pack_start(self.monitor_body,True,True,0)
            self.detached.destroy();self.detached=None;self.detached_hint.hide()
            self.detach_button.set_label(self.T("monitor-view-separate-window","Separate window"))
        else:
            self.monitor_host.remove(self.monitor_body)
            self.detached=Gtk.Window(title=self.T("monitor-view-opends-live-monitor","OpenDS · Live monitor"))
            self.detached.set_default_size(1100,740);self.detached.set_keep_above(True)
            container=box();top=box(True)
            keep=check_button(self.T("monitor-view-always-on-top","Always on top"));keep.set_active(True)
            keep.connect("toggled",lambda o:self.detached.set_keep_above(o.get_active()) if self.detached else None)
            pack(top,keep);pack(container,top);pack(container,self.monitor_body,True)
            self.detached.add(container)
            self.detached.connect("delete-event",lambda *_:(self.toggle_detach(),True)[1])
            self.detached.connect("window-state-event",lambda *_:self.update_visibility())
            self.detached.show_all();self.detached_hint.show()
            self.detach_button.set_label(self.T("monitor-view-dock-window","Dock window"))
        self.monitor_body.show_all();self.update_visibility()

    def connection_page(self):
        page=self.page("connection","Connection & feedback")
        device=self.card(page,"Physical controller")
        row=box(True)
        self.device_combo=Gtk.ComboBoxText();self.device_combo.append("","Auto-select Bluetooth DualSense");self.device_combo.set_active_id("")
        self.fields["device_path"]=lambda:self.device_combo.get_active_id() or ""
        pack(row,self.device_combo,True);pack(row,button(self.T("gui-refresh-devices","Refresh devices"),lambda:self.call("devices",self.devices_received)))
        pack(device,row)
        options=self.card(page,"Feedback")
        self.bridge_controls=[]
        for key,message,fallback in [("audio_enabled","gui-forward-game-audio","Forward game audio"),("haptics_enabled","gui-forward-native-stereo-haptics","Forward native stereo haptics"),("microphone_enabled","gui-forward-controller-microphone","Forward controller microphone")]:
            self.bridge_controls.append(self.check(key,self.T(message,fallback),options))
        pack(options,button(self.T("gui-microphone-test","Microphone test…"),self.microphone_test))
        row=box(True);pack(row,label("Audio route"),True)
        route=self.combo("audio_route",[(0,"Automatic · game setting / headphone connection"),(1,"Controller speaker"),(2,"Headphones")]);pack(row,route);pack(options,row);self.bridge_controls.append(route)
        values=self.card(page,"Volume, gain & buffering")
        grid=box(True,22);pack(values,grid)
        for fields in [[("speaker_volume",0,255,"Speaker volume"),("headphone_volume",0,127,"Headphone volume"),("haptics_gain",10,20,"Haptic gain (10 = 1×)")],[("microphone_gain_db",0,50,"Microphone gain (dB)"),("microphone_buffer_ms",16,512,"Microphone buffer (ms)"),("rumble_gain",10,20,"Rumble gain (10 = 1×)")],[("port",1,65535,"USB/IP port"),("buffer",1,128,"Audio buffer blocks"),("opus_complexity",0,10,"Opus complexity")]]:
            column=box();column.set_hexpand(True);pack(grid,column,True)
            for key,low,high,title in fields:
                widget=self.spin(key,low,high,column,title)
                if key not in ("microphone_gain_db","microphone_buffer_ms"):self.bridge_controls.append(widget)
        advanced=self.card(page)
        self.check("reconnect",self.T("gui-reconnect-automatically","Reconnect automatically"),advanced)
        self.hide_check=self.check("hide_device",self.T("gui-hide-the-physical-controller-to-avoid-duplicate","Hide the physical controller to avoid duplicate input"),advanced)
        self.check("edge_as_ds","Present DualSense Edge as a standard DualSense",advanced)
        pack(page,button(self.T("gui-save-settings","Save settings"),lambda:self.persist(fields=True),"primary"))

    def devices_received(self,devices):
        self.device_combo.remove_all();self.device_combo.append("","Auto-select Bluetooth DualSense")
        for device in devices:self.device_combo.append(device["path"],device["path"]+("" if device["accessible"] else " · permission denied"))
        selected=self.get("device_path")
        if selected and selected not in [d["path"] for d in devices]:self.device_combo.append(selected,selected+" · disconnected")
        self.device_combo.set_active_id(selected)

    def hiding_page(self):
        page=self.page("hiding","Device hiding")
        card=self.card(page,"Physical controller visibility")
        self.hiding_status=label("No active input grab","phase",True);pack(card,self.hiding_status)
        self.hiding_enabled=check_button("Grab physical input while bridging")
        self.hiding_enabled.set_active(self.flag("hide_device",True))
        self.hiding_enabled.connect("toggled",lambda o:(self.set("hide_device",o.get_active()),self.hide_check.set_active(o.get_active()),self.persist()))
        pack(card,self.hiding_enabled)
        pack(card,label("OpenDS uses exclusive Linux input grabs. Stopping the bridge restores controller visibility. A crash also releases the grab automatically.",wrap=True))
        pack(card,button(self.T("gui-restore-controller-visibility","Restore controller visibility"),lambda:self.call("bridge_stop")))
        apps=self.card(page,"Games using direct controller access")
        pack(apps,label("Disable Steam Input. For SDL games, launch with SDL_JOYSTICK_HIDAPI=0 so the game uses the virtual USB controller. Linux has no HidHide application allowlist; OpenDS always retains access to the physical hidraw device.",wrap=True))
        pack(apps,button("Open Ubuntu Bluetooth settings",lambda:self.launch(["gnome-control-center","bluetooth"])))
        pack(apps,button("Copy Steam launch option",lambda:Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).set_text("SDL_JOYSTICK_HIDAPI=0 %command%",-1)))

    def diagnostics_page(self):
        page=self.page("diagnostics","Diagnostics & logs")
        card=self.card(page,"Dependencies & controller permissions")
        self.diagnostics_view=self.text_view(card,170)
        actions=box(True)
        pack(actions,button(self.T("gui-check-again","Check again"),self.diagnose))
        pack(actions,button("Set up Ubuntu dependencies…",self.setup_dependencies))
        pack(actions,button("Open log folder",lambda:self.open_folder(self.state_directory()/"logs")))
        pack(card,actions)
        row=box(True);pack(row,label("USB/IP directory"),True)
        self.usbip_directory=Gtk.Entry();self.usbip_directory.set_text(self.get("usbip_directory"));pack(row,self.usbip_directory,True)
        self.fields["usbip_directory"]=self.usbip_directory.get_text
        pack(row,button("Choose…",self.choose_usbip));pack(row,button("Auto",lambda:self.usbip_directory.set_text("")));pack(card,row)
        pack(card,button("Save tool directory",lambda:self.persist(fields=True)))
        logs=self.card(page,"Bridge log")
        self.diagnostics_log=self.text_view(logs,200)
        self.log_buffers.append(self.diagnostics_log.get_buffer())

    def diagnose(self):
        self.call("doctor",lambda value:self.diagnostics_view.get_buffer().set_text(value))

    def choose_usbip(self):
        path=self.file_dialog("USB/IP installation directory",folder=True)
        if path:self.usbip_directory.set_text(path)

    def setup_dependencies(self):
        script=Path(self.executable).parent/"setup-ubuntu.sh"
        if not script.exists():script=Path(__file__).resolve().parents[2]/"scripts/setup-ubuntu.sh" if "__file__" in globals() else Path.cwd()/"scripts/setup-ubuntu.sh"
        self.launch([str(script)],finished=lambda code:self.diagnose())

    def general_page(self):
        page=self.page("general","General settings")
        startup=self.card(page,"Startup & window behavior")
        self.check("auto_start",self.T("gui-start-bridging-when-opends-opens","Start bridging when OpenDS opens"),startup)
        self.login=check_button("Run in the tray when Ubuntu starts")
        self.login.set_active(self.autostart_path().is_file());pack(startup,self.login)
        self.combo("close_action",[("ask",self.T("gui-ask","Ask")),("tray",self.T("gui-minimize-to-tray","Minimize to tray")),("exit",self.T("gui-exit-application","Exit application"))],startup)
        appearance=self.card(page,"Appearance")
        self.combo("theme",[("system",self.T("appearance-system","Follow system")),("light",self.T("appearance-light","Light")),("dark",self.T("appearance-dark","Dark"))],appearance,lambda value:self.theme_changed(value))
        languages=self.card(page,"Language")
        entries=[("auto",self.T("language-follow-system","Follow system language"))]+[(l["code"],f"{l['name']} · {'local' if l['local'] else 'built-in'} ({l['translated']*100//max(1,l['total'])}%)") for l in self.languages]
        self.language_combo=self.combo("language",entries,languages)
        pack(languages,label("Language changes apply after restarting OpenDS. Missing translations fall back to English.","muted",True))
        actions=box(True);pack(actions,button(self.T("language-import","Import language file…"),self.import_language));pack(actions,button(self.T("language-open-folder","Open language folder"),lambda:self.open_folder(self.path.parent/"locales")));pack(languages,actions)
        pack(page,button(self.T("gui-save-settings","Save settings"),self.save_general,"primary"))
        advanced=self.card(page,"Advanced configuration")
        pack(advanced,button("Edit all settings…",self.advanced_settings))

    def theme_changed(self,value):
        self.set("theme",value);self.apply_theme()

    def apply_theme(self):
        requested=self.get("theme","system")
        prefer_dark=False
        schema=Gio.SettingsSchemaSource.get_default().lookup("org.gnome.desktop.interface",True)
        if schema:
            settings=Gio.Settings.new_full(schema,None,None)
            prefer_dark=settings.get_string("color-scheme")=="prefer-dark" if schema.has_key("color-scheme") else False
        self.dark=requested=="dark" or requested=="system" and prefer_dark
        Gtk.Settings.get_default().set_property("gtk-application-prefer-dark-theme",self.dark)
        bg="#10151e" if self.dark else "#f4f6fa";card="#1a2230" if self.dark else "#ffffff";fg="#e6ecf5" if self.dark else "#202b3d";muted="#99a8bf" if self.dark else "#6b778c";border="#2c384a" if self.dark else "#e5eaf2"
        css=f'''window {{background:{bg};color:{fg};}} .sidebar {{background:{card};padding:24px 14px;border-right:1px solid {border};}} .brand {{font-size:25px;font-weight:700;}} .page-title {{font-size:25px;font-weight:700;}} .section-title {{font-size:13px;font-weight:700;}} .muted {{color:{muted};font-size:11px;}} .card {{background:{card};border:1px solid {border};border-radius:10px;padding:18px;}} .navigation {{border:0;box-shadow:none;background:transparent;min-height:36px;padding:4px 12px;}} .navigation.selected {{background:#e8f0ff;color:#2363ce;font-weight:700;}} button {{border-radius:6px;padding:7px 11px;background-image:none;background-color:{card};color:{fg};border:1px solid {border};}} button:disabled {{color:{muted};background-color:{bg};}} entry, spinbutton, combobox button {{background-color:{card};color:{fg};}} checkbutton check:checked {{background-image:none;background-color:#2868d8;border-color:#2868d8;color:white;}} button.primary {{background:#2868d8;color:white;border-color:#2868d8;}} .phase {{font-size:17px;font-weight:600;}} .counter {{font-size:30px;font-weight:700;}} .battery-number {{font-size:29px;font-weight:700;color:#38a96b;}} textview, textview text {{background:{card};color:{fg};}} .error {{color:#df665d;}} window.input-overlay {{background-color:transparent;background-image:none;}}'''
        provider=Gtk.CssProvider();provider.load_from_data(css.encode())
        if hasattr(self,"css_provider"):Gtk.StyleContext.remove_provider_for_screen(Gdk.Screen.get_default(),self.css_provider)
        self.css_provider=provider
        Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(),provider,Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        if self.built:
            self.monitor_canvas.queue_draw();self.history_canvas.queue_draw()

    def save_general(self):
        previous=self.get("language","auto")
        self.read_fields()
        try:self.write_autostart(self.login.get_active())
        except OSError as error:self.error(error);return
        def saved():
            if previous!=self.get("language"):
                dialog=Gtk.MessageDialog(transient_for=self,modal=True,buttons=Gtk.ButtonsType.YES_NO,text="Restart OpenDS now?")
                dialog.format_secondary_text("Language changes require a restart. Bridging and recording will stop.")
                response=dialog.run();dialog.destroy()
                if response==Gtk.ResponseType.YES:self.restart_pending=True;self.shutdown()
        self.persist(done=saved)

    def autostart_path(self):
        return Path(os.environ.get("XDG_CONFIG_HOME",str(Path.home()/".config")))/"autostart"/(APP_ID+".desktop")

    def write_autostart(self,enabled):
        path=self.autostart_path()
        if enabled:
            path.parent.mkdir(parents=True,exist_ok=True)
            path.write_text("[Desktop Entry]\nType=Application\nName=OpenDS\nExec="+desktop_quote(self.executable)+" tray\nIcon=opends\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",encoding="utf-8")
        else:path.unlink(missing_ok=True)

    def import_language(self):
        path=self.file_dialog("Import Fluent language pack")
        if path:
            def imported(value):
                self.language_combo.append(value["code"],value["name"]+" · local")
                self.language_combo.set_active_id(value["code"])
                self.notice(f"Imported {value['name']}. Save settings and restart to use it.")
            self.call("language_import",imported,path=path)

    def advanced_settings(self):
        dialog=Gtk.Dialog(title="Advanced configuration",transient_for=self,modal=True)
        dialog.set_default_size(700,600);dialog.add_button("Cancel",Gtk.ResponseType.CANCEL);dialog.add_button("Validate & save",Gtk.ResponseType.OK)
        view=Gtk.TextView();view.set_monospace(True)
        output=io.StringIO();self.config.write(output);view.get_buffer().set_text(output.getvalue())
        scroll=Gtk.ScrolledWindow();scroll.add(view);dialog.get_content_area().pack_start(scroll,True,True,0);dialog.show_all()
        if dialog.run()==Gtk.ResponseType.OK:
            buffer=view.get_buffer();value=buffer.get_text(buffer.get_start_iter(),buffer.get_end_iter(),True)
            def saved(_):
                self.config.read_string(value);self.notice("Configuration saved. Reopen OpenDS to refresh all controls.")
            self.call("configure",saved,text=value)
        dialog.destroy()

    def history_settings(self):
        dialog=Gtk.Dialog(title="Input history & overlay settings",transient_for=self,modal=True)
        dialog.set_default_size(720,600);dialog.add_button("Cancel",Gtk.ResponseType.CANCEL);dialog.add_button("Apply",Gtk.ResponseType.OK)
        notebook=Gtk.Notebook();dialog.get_content_area().pack_start(notebook,True,True,0)
        getters={}
        groups={}
        for name in ["Display","Thresholds","Light colors","Dark colors","Overlay"]:
            scroll=Gtk.ScrolledWindow();content=box(spacing=10)
            for setter in (content.set_margin_top,content.set_margin_bottom,content.set_margin_start,content.set_margin_end):setter(18)
            scroll.add(content);notebook.append_page(scroll,label(name));groups[name]=content
        labels={"right":"Include right stick","enabled":"Show input history","source":"Source label","rows":"Main history rows","content_size":"Content size (%)","display_min_ms":"Display minimum duration (ms)","record_min_ms":"CSV minimum duration (ms)","overlay_rows":"Overlay row count","overlay_auto_rows":"Fit complete rows to overlay height","separators":"Show record separators","separator_alpha":"Separator opacity (%)","separator_style":"Separator style","auto_colors":"Automatic colors","bg_alpha":"Background opacity (%)","fg_alpha":"Content opacity (%)","topmost":"Always on top","theme":"Overlay theme","width":"Width","height":"Height","x":"Screen X","y":"Screen Y"}
        ranges={"rows":(1,64,1),"overlay_rows":(1,64,1),"content_size":(50,500,10),"display_min_ms":(0,60000,.01),"record_min_ms":(0,60000,.01),"bg_alpha":(0,100,1),"fg_alpha":(0,100,1),"separator_alpha":(0,100,1),"width":(320,2048,1),"height":(100,2048,1),"x":(-10000,10000,1),"y":(-10000,10000,1)}
        booleans={"enabled","right","overlay_auto_rows","separators","auto_colors","topmost"}
        tier_widgets={};ignore_widgets={}
        for key,value in self.section.items():
            if not key.startswith("command_"):continue
            short=key[len("command_"):]
            color=value.startswith("#")
            if color:group="Dark colors" if short.startswith("dark") or short.endswith("_1") and short.startswith(("background","text")) else "Light colors"
            elif short.startswith(("stick_","trigger_","triggerdead_","ignore_")):group="Thresholds"
            elif short in {"overlay_rows","overlay_auto_rows","bg_alpha","fg_alpha","topmost","theme","x","y","width","height"}:group="Overlay"
            else:group="Display"
            parent=groups[group];row=box(True);pack(row,label(labels.get(short,short.replace("_"," ").title())),True)
            if color:
                widget=Gtk.ColorButton();rgba=Gdk.RGBA();rgba.parse(value);widget.set_rgba(rgba)
                getters[key]=lambda w=widget:"#%02X%02X%02X"%tuple(round(v*255) for v in (w.get_rgba().red,w.get_rgba().green,w.get_rgba().blue))
            elif short in booleans or short.startswith("ignore_"):
                widget=Gtk.Switch();widget.set_active(value.lower()=="true")
                if short.startswith("ignore_"):
                    axis=int(short[-1]);row.get_children()[0].set_text(["LS","RS","L2","R2"][axis]+" · distinguish strength")
                    widget.set_active(value.lower()!="true");getters[key]=lambda w=widget:not w.get_active();ignore_widgets[axis]=widget
                else:getters[key]=widget.get_active
            elif short in ("source","theme","separator_style"):
                widget=Gtk.ComboBoxText()
                choices={"source":["Text","Icon","Hidden"],"theme":["Follow main window","Light","Dark"],"separator_style":["Flat","Fading","Tapered"]}[short]
                for i,title in enumerate(choices):widget.append(str(i),title)
                widget.set_active_id(value);getters[key]=widget.get_active_id
            else:
                low,high,step=ranges.get(short,(0,100,.1) if short.startswith("stick_") else (0,255,1))
                widget=Gtk.SpinButton.new_with_range(low,high,step);widget.set_value(float(value));widget.set_digits(2 if step<1 else 0)
                getters[key]=widget.get_value if step<1 else widget.get_value_as_int
                if short.startswith(("stick_","trigger_")):
                    bits=short.split("_");axis=int(bits[1])+(2 if short.startswith("trigger_") else 0)
                    if short.startswith("trigger_") or bits[-1]!="0":tier_widgets.setdefault(axis,[]).append(widget)
            pack(row,widget);pack(parent,row)
        def sync_tiers(*_):
            for axis,widgets in tier_widgets.items():
                for widget in widgets:widget.set_sensitive(ignore_widgets[axis].get_active())
        for widget in ignore_widgets.values():widget.connect("notify::active",sync_tiers)
        sync_tiers();dialog.show_all()
        if dialog.run()==Gtk.ResponseType.OK:
            before=dict(self.section)
            for key,getter in getters.items():self.set(key,getter())
            output=io.StringIO();self.config.write(output)
            def applied(_):
                self.history_enabled.set_active(self.flag("command_enabled",True));self.apply_overlay(reset=True);self.history_canvas.queue_draw()
            def failed():
                self.section.clear();self.section.update(before)
            self.call("configure",applied,failure=failed,text=output.getvalue())
        dialog.destroy()

    def show_overlay(self):
        if self.overlay:
            self.overlay.show_all();self.overlay.present();return
        self.overlay=Gtk.Window(title="OpenDS · Input history overlay")
        self.overlay.set_type_hint(Gdk.WindowTypeHint.UTILITY);self.overlay.set_app_paintable(True)
        self.overlay.get_style_context().add_class("input-overlay")
        visual=self.overlay.get_screen().get_rgba_visual()
        if visual:self.overlay.set_visual(visual)
        self.overlay_canvas=HistoryCanvas(self,True);self.overlay.add(self.overlay_canvas)
        self.overlay.connect("delete-event",lambda *_:(self.hide_overlay(),True)[1])
        self.overlay.connect("configure-event",self.overlay_geometry)
        self.overlay.connect("realize",lambda *_:self.apply_overlay())
        self.overlay.show_all();self.apply_overlay(reset=True);self.update_visibility()

    def hide_overlay(self):
        if self.overlay:self.overlay.hide()
        self.update_visibility()

    def overlay_geometry(self,widget,event):
        if not self.overlay_locked:
            self.set("command_x",event.x);self.set("command_y",event.y)
            self.set("command_width",max(320,event.width));self.set("command_height",max(100,event.height))
        return False

    def apply_overlay(self,reset=False):
        if not self.overlay:return
        self.overlay.set_keep_above(self.flag("command_topmost",True))
        if reset:
            self.overlay.resize(int(self.get("command_width","560")),int(self.get("command_height","430")))
            self.overlay.move(int(self.get("command_x","100")),int(self.get("command_y","100")))
        self.overlay.set_decorated(not self.overlay_locked);self.overlay.set_accept_focus(not self.overlay_locked)
        if self.overlay.get_realized():self.overlay.input_shape_combine_region(cairo.Region() if self.overlay_locked else None)
        self.overlay_canvas.queue_draw()

    def lock_overlay(self):
        if not self.overlay:self.show_overlay()
        self.overlay_locked=not self.overlay_locked;self.apply_overlay()
        self.overlay_lock_button.set_label("Unlock overlay" if self.overlay_locked else "Lock overlay")

    def restore_overlay(self):
        if not self.overlay:self.show_overlay()
        self.overlay_locked=False
        self.overlay_lock_button.set_label("Lock overlay")
        self.set("command_x",100);self.set("command_y",100);self.set("command_width",560);self.set("command_height",430)
        self.apply_overlay(reset=True);self.overlay.show_all();self.overlay.present()
        self.persist(notice=False)

    def setup_tray(self):
        try:
            gi.require_version("AyatanaAppIndicator3","0.1")
            from gi.repository import AyatanaAppIndicator3 as Indicator
            self.tray=Indicator.Indicator.new(APP_ID,str(self.icon) if self.icon.exists() else "input-gaming-symbolic",Indicator.IndicatorCategory.APPLICATION_STATUS)
            menu=Gtk.Menu()
            actions=[("Start bridge",lambda:self.call("bridge_start")),("Stop bridge",lambda:self.call("bridge_stop")),("Open settings",self.present_window),("Show input overlay",self.show_overlay),("Lock / unlock overlay",self.lock_overlay),("Restore overlay",self.restore_overlay),("Exit OpenDS",self.shutdown)]
            for title,action in actions:
                item=Gtk.MenuItem(label=translate(title));item.connect("activate",lambda _,fn=action:fn());menu.append(item)
            menu.show_all();self.tray.set_menu(menu);self.tray.set_status(Indicator.IndicatorStatus.ACTIVE)
        except (ImportError,ValueError):self.tray=None

    def hide_to_tray(self):
        if not self.tray:
            self.error("Install gir1.2-ayatanaappindicator3-0.1 to enable tray operation.")
            return
        self.hide();self.update_visibility()

    def present_window(self):
        self.show_all();self.present();self.update_visibility()

    def update_visibility(self):
        if not self.built:return False
        visible=(self.get_visible() and self.stack.get_visible_child_name()=="monitor") or bool(self.detached and self.detached.get_visible()) or bool(self.overlay and self.overlay.get_visible())
        if getattr(self,"graphics_visible",None)!=visible:
            self.graphics_visible=visible;self.call("visible",value=visible)
        return False

    def on_close(self,*_):
        action=self.get("close_action","ask")
        if action=="tray":self.hide_to_tray();return True
        if action=="ask":
            dialog=Gtk.MessageDialog(transient_for=self,modal=True,text="Close OpenDS?")
            dialog.format_secondary_text("Keep bridging in the tray, or stop bridging and exit.")
            dialog.add_button("Cancel",Gtk.ResponseType.CANCEL);dialog.add_button("Minimize to tray",1);dialog.add_button("Exit",2)
            response=dialog.run();dialog.destroy()
            if response==1:self.hide_to_tray()
            elif response==2:self.shutdown()
        else:self.shutdown()
        return True

    def shutdown(self):
        if self.closing:return
        self.microphone_cancel.set()
        if self.playback and self.playback.poll() is None:self.playback.terminate()
        self.call("quit");self.closing=True
        if self.built:self.phase.set_text("Stopping · restoring controller visibility…")
        GLib.timeout_add(100,self.finish_shutdown)

    def finish_shutdown(self):
        if self.service.poll() is None:return True
        if self.overlay:self.overlay.destroy()
        if self.detached:self.detached.destroy()
        if self.microphone_window:self.microphone_window.destroy()
        application=self.get_application();self.destroy();application.quit()
        if self.restart_pending:os.execv(self.executable,[self.executable,"gui"])
        return False

    def tick(self):
        for _ in range(100):
            try:message=self.messages.get_nowait()
            except queue.Empty:break
            kind=message.get("type")
            if kind=="ready":self.build(message)
            elif kind=="reply":
                callback,failed=self.callbacks.pop(message.get("id"),(None,None))
                if not message.get("ok"):
                    if failed:failed()
                    self.error(message.get("error","Operation failed"))
                elif callback:callback(message.get("value"))
            elif kind=="microphone":self.microphone_source_done(message)
            elif kind=="usb_recording":self.microphone_usb_done(message)
            elif kind=="failure":self.error(message.get("error"))
        state=self.pending_state;self.pending_state=None
        if state and self.built and not self.closing:
            self.state=state
            running=state.get("running");connected=state.get("connected")
            phase=self.T("gui-connected-forwarding","Connected · forwarding") if connected else "Starting · waiting for controller or administrator authentication" if running else self.T("gui-bridge-not-started","Bridge not started")
            if state.get("error"):phase=state["error"]
            self.phase.set_text(phase)
            self.start_button.set_sensitive(not running);self.stop_button.set_sensitive(running)
            for widget in self.bridge_controls:widget.set_sensitive(not running)
            for widget,value in zip(self.counters,state.get("counts",[0]*3)):widget.set_text(f"{value:,}")
            battery=state.get("battery",{})
            self.battery_number.set_text("--%" if battery.get("percent") is None else str(battery["percent"])+"%")
            self.battery_caption.set_text(battery.get("caption",""));self.battery_number.set_tooltip_text(battery.get("tooltip",""))
            self.hiding_status.set_text("Physical evdev input is hidden" if state.get("hidden") else "Physical evdev input is visible")
            self.capture_button.set_label(self.T("monitor-view-stop-capture","Stop capture") if state.get("capture") else self.T("monitor-view-start-capture","Start capture"))
            recording=state.get("recording",{})
            self.record_button.set_label(self.T("monitor-view-stop-file-recording","Stop file recording") if recording.get("active") else self.T("monitor-view-start-file-recording","Start file recording"))
            self.record_button.set_sensitive(state.get("capture",False))
            note=("Capturing" if state.get("capture") else "Stopped / history retained")+f" · dropped {state.get('drops',0)} · decode errors {state.get('decode_errors',0)} · resets {state.get('resets',0)} · cache {state.get('cache',0)/1048576:.1f} MiB"
            if recording.get("active"):note+=f" · recording {recording.get('rows',0)} rows"
            if recording.get("error"):note+=" · CSV error: "+recording["error"]
            self.monitor_footer.set_text(note)
            logs=state.get("logs","").replace("\r\n","\n")
            if logs!=self.last_logs:
                self.last_logs=logs
                for buffer in self.log_buffers:buffer.set_text(logs)
            if self.graphics_visible:
                self.monitor_canvas.queue_draw();self.history_canvas.queue_draw()
                if self.overlay and self.overlay.get_visible():self.overlay_canvas.queue_draw()
        if self.service.poll() is not None and not self.closing:
            self.error("OpenDS service stopped. Another OpenDS instance may already be running.")
            self.closing=True
            application=self.get_application();self.destroy();application.quit();return False
        return not self.closing

    def state_directory(self):
        return Path(os.environ.get("XDG_STATE_HOME",str(Path.home()/".local/state")))/"opends"

    def file_dialog(self,title,save=False,folder=False,filename=None):
        action=Gtk.FileChooserAction.SELECT_FOLDER if folder else Gtk.FileChooserAction.SAVE if save else Gtk.FileChooserAction.OPEN
        dialog=Gtk.FileChooserDialog(title=title,transient_for=self,action=action)
        dialog.add_button("Cancel",Gtk.ResponseType.CANCEL);dialog.add_button("Save" if save else "Choose",Gtk.ResponseType.OK)
        if filename:dialog.set_current_name(filename)
        if save:dialog.set_do_overwrite_confirmation(True)
        result=dialog.get_filename() if dialog.run()==Gtk.ResponseType.OK else None
        dialog.destroy();return result

    def open_folder(self,path):
        path.mkdir(parents=True,exist_ok=True)
        Gio.AppInfo.launch_default_for_uri(path.as_uri(),None)

    def launch(self,args,finished=None):
        try:child=subprocess.Popen(args)
        except OSError as error:self.error(error);return
        if finished:
            def poll():
                if child.poll() is None:return True
                finished(child.returncode);return False
            GLib.timeout_add(200,poll)

    def microphone_test(self):
        if self.microphone_window:self.microphone_window.present();return
        window=Gtk.Window(title="OpenDS · Microphone test",transient_for=self)
        window.set_default_size(620,390)
        content=box(spacing=16)
        for method in (content.set_margin_start,content.set_margin_end,content.set_margin_top,content.set_margin_bottom):method(24)
        window.add(content)
        pack(content,label("Microphone test","page-title"))
        pack(content,label("Record five seconds after a three-second countdown. Compare the audio received by the virtual USB microphone with the decoded Bluetooth source.",wrap=True))
        self.mic_status=label("Enable microphone forwarding and start the bridge first.","muted",True);pack(content,self.mic_status)
        self.mic_source=Gtk.ComboBoxText()
        try:
            sources=json.loads(subprocess.check_output(["pactl","-f","json","list","sources"],text=True,timeout=4))
            for source in sources:
                if source["name"].endswith(".monitor"):continue
                self.mic_source.append(source["name"],source.get("description",source["name"]))
            preferred=next((s["name"] for s in sources if any(term in s["name"].lower() for term in ("sony","dualsense","wireless_controller")) and not s["name"].endswith(".monitor")),None)
            self.mic_source.set_active_id(preferred) if preferred else self.mic_source.set_active(-1)
        except (OSError,ValueError,subprocess.SubprocessError):pass
        pack(content,label("Virtual USB microphone capture source"));pack(content,self.mic_source)
        actions=box(True)
        pack(actions,button("Record 5 seconds",self.record_microphone,"primary"))
        pack(actions,button("Replay USB side",lambda:self.play_microphone("usb")))
        pack(actions,button("Replay Bluetooth side",lambda:self.play_microphone("bluetooth")))
        pack(content,actions)
        pack(content,button("Clear recording",self.clear_microphone))
        def close(*_):
            self.clear_microphone();window.destroy();self.microphone_window=None;return True
        window.connect("delete-event",close)
        self.microphone_window=window;window.show_all()

    def record_microphone(self):
        if self.microphone_recording:self.error("Cancel or finish the current recording first.");return
        if not self.state.get("connected") or not self.state.get("microphone"):
            self.error("Start the bridge with microphone forwarding enabled first.");return
        source=self.mic_source.get_active_id()
        if not source:self.error("Select the virtual USB DualSense microphone source first.");return
        self.microphone_cancel=threading.Event();self.microphone_recording=True
        self.microphone_generation+=1;self.microphone_done=set()
        self.mic_status.set_text("Starting in 3…")
        self.call("mic_capture",success=lambda _:self.start_usb_microphone_capture(source),failure=lambda:setattr(self,"microphone_recording",False),generation=self.microphone_generation)

    def start_usb_microphone_capture(self,source):
        cancel=self.microphone_cancel
        generation=self.microphone_generation
        def record():
            child=None
            try:
                for _ in range(150):
                    if cancel.wait(.02):return
                child=subprocess.Popen(["parec","--device="+source,"--format=s16le","--rate=48000","--channels=2","--raw"],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
                data=bytearray();expected=48000*2*2*5
                # A worker reads fixed chunks; the main GTK thread remains responsive.
                import selectors
                selector=selectors.DefaultSelector();selector.register(child.stdout,selectors.EVENT_READ)
                deadline=time.monotonic()+7
                while len(data)<expected and time.monotonic()<deadline and not cancel.is_set():
                    for key,_ in selector.select(.1):
                        chunk=os.read(key.fileobj.fileno(),min(8192,expected-len(data)))
                        if not chunk:raise RuntimeError("Virtual microphone capture ended before five seconds")
                        data.extend(chunk)
                selector.close()
                if cancel.is_set():return
                if len(data)<expected:raise RuntimeError("The virtual microphone did not provide five seconds of audio")
                path=self.state_directory()/"microphone-test-usb.wav";path.parent.mkdir(parents=True,exist_ok=True)
                with wave.open(str(path),"wb") as output:output.setnchannels(2);output.setsampwidth(2);output.setframerate(48000);output.writeframes(data)
                self.messages.put({"type":"usb_recording","ok":True,"bytes":len(data),"generation":generation})
            except (OSError,RuntimeError) as error:self.messages.put({"type":"usb_recording","ok":False,"error":str(error),"generation":generation})
            finally:
                if child:
                    child.terminate()
                    try:child.communicate(timeout=2)
                    except subprocess.TimeoutExpired:child.kill();child.communicate()
        threading.Thread(target=record,daemon=True).start()
        GLib.timeout_add(3000,lambda:(self.mic_status.set_text("Recording five seconds…") if self.microphone_window else None,False)[1])

    def microphone_source_done(self,message):
        if not self.microphone_window or message.get("generation")!=self.microphone_generation:return
        self.microphone_done.add("source")
        self.microphone_recording=len(self.microphone_done)<2
        if not message.get("ok"):
            self.microphone_recording=False;self.microphone_cancel.set();self.mic_status.set_text(message.get("error","Bluetooth recording failed"))
        else:
            value=message["value"];self.mic_status.set_text(f"Bluetooth source: {value['frames']} frames · longest gap {value['gap_ms']:.1f} ms · dropped {value['dropped']}. USB capture is finishing…")

    def microphone_usb_done(self,message):
        if message.get("generation")!=self.microphone_generation:return
        self.microphone_done.add("usb")
        self.microphone_recording=len(self.microphone_done)<2
        if self.microphone_window:self.mic_status.set_text("Recording complete. Replay either side to compare." if message.get("ok") else message.get("error","USB recording failed"))

    def play_microphone(self,side):
        path=self.state_directory()/f"microphone-test-{side}.wav"
        if not path.is_file():self.error("Record audio first.");return
        if self.playback and self.playback.poll() is None:self.playback.terminate()
        try:self.playback=subprocess.Popen(["paplay","--stream-name=OpenDS microphone test",str(path)])
        except OSError as error:self.error(error)

    def clear_microphone(self):
        self.microphone_generation+=1
        self.microphone_cancel.set();self.microphone_recording=False;self.call("mic_cancel")
        if self.playback and self.playback.poll() is None:self.playback.terminate()
        for side in ("usb","bluetooth"):(self.state_directory()/f"microphone-test-{side}.wav").unlink(missing_ok=True)
        if self.microphone_window:self.mic_status.set_text("Recording cleared. Ready to record again.")


def main():
    GLib.set_prgname(APP_ID)
    GLib.set_application_name("OpenDS")
    path=Path(sys.argv[2])
    flags=Gio.ApplicationFlags.NON_UNIQUE if os.environ.get("OPENDS_UI_SMOKE") else Gio.ApplicationFlags.FLAGS_NONE
    application=Gtk.Application(application_id=APP_ID,flags=flags)
    windows=[]
    def activate(app):
        if windows:windows[0].present_window()
        else:windows.append(Window(app,EXE,path))
    application.connect("activate",activate)
    for kind in (signal.SIGTERM,signal.SIGINT):
        GLib.unix_signal_add(GLib.PRIORITY_DEFAULT,kind,lambda:(windows[0].shutdown() if windows else application.quit(),False)[1])
    return application.run([sys.argv[0]])


if __name__=="__main__":
    sys.exit(main())
