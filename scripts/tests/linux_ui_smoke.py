"""Exercise all native desktop pages and auxiliary windows under xvfb-run."""
import os
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import time

root = Path(__file__).resolve().parents[2]
temporary = tempfile.TemporaryDirectory(prefix="opends-gtk-qa-")
os.environ["XDG_CONFIG_HOME"] = str(Path(temporary.name) / "config")
os.environ["XDG_STATE_HOME"] = str(Path(temporary.name) / "state")
exe = root / "target/release/opends"
config = subprocess.check_output([str(exe), "config", "init"], text=True).strip()
if os.environ.get("OPENDS_UI_TEST_LANGUAGE"):
    path=Path(config)
    path.write_text(path.read_text().replace("language=auto\n", "language="+os.environ["OPENDS_UI_TEST_LANGUAGE"]+"\n"))
sys.argv = ["gui.py", str(exe), config]
ui = runpy.run_path(str(root / "src/linux/gui.py"), run_name="opends_test")
Gtk, GLib, Gio = ui["Gtk"], ui["GLib"], ui["Gio"]
app = Gtk.Application(application_id="io.github.OpenDS.Verification", flags=Gio.ApplicationFlags.NON_UNIQUE)
app.register(None)
window = ui["Window"](app, str(exe), config)
errors = []
loop = GLib.MainLoop()


def exception(kind, value, tb):
    errors.append(str(value))
    sys.__excepthook__(kind, value, tb)
    window.shutdown()
    loop.quit()


sys.excepthook = exception
steps = iter(["overview", "monitor", "connection", "hiding", "diagnostics", "general", "render", "dark", "light", "overlay", "detach", "dock", "history", "microphone", "shutdown"])
started = time.monotonic()


def screenshot(name):
    output = os.environ.get("OPENDS_UI_SCREENSHOTS")
    if output:
        from gi.repository import Gdk
        path = Path(output)
        path.mkdir(parents=True, exist_ok=True)
        pixbuf = Gdk.pixbuf_get_from_window(window.get_window(), 0, 0, window.get_allocated_width(), window.get_allocated_height())
        pixbuf.savev(str(path / (name + ".png")), "png", [], [])
    return False


def step():
    if not window.built:
        assert time.monotonic()-started < 15, "GUI startup timed out"
        return True
    action = next(steps, "end")
    print("UI check:", action, flush=True)
    if action in window.nav:
        window.navigate(action)
        GLib.timeout_add(150, lambda: screenshot(action))
    elif action == "render":
        import cairo
        previous = window.state
        window.state = {"connected": True, "capture": True, "edge": True, "source": True, "forwarded": True,
                        "microphone": True, "pressed": [0,0,0], "wave_states": ["Active"]*3,
                        "input": {"sticks": [200,50,20,190], "triggers": [170,220], "buttons": [0xf0,0xff,0xff],
                                  "touch": [[1,400,300],[1,1500,900]], "gyro": [13,-9,7], "accel": [10,20,8000]},
                        "waves": [[[-1000*(c+1) for c in range(10)],[1000*(c+1) for c in range(10)],1023,0] for _ in range(240)],
                        "events": [{"kind":"input","combination":"LS ↗ + RS ↓ + DP ↑ + △ + L1 + L2:2 + R2:3", "strength":[4,1,2,3],"duration":"1.23 s"},
                                   {"kind":"connected","combination":"","duration":"300 ms"}]}
        canvas=window.monitor_canvas
        surface=cairo.ImageSurface(cairo.FORMAT_ARGB32,canvas.get_allocated_width(),canvas.get_allocated_height())
        canvas.draw(canvas,cairo.Context(surface))
        if os.environ.get("OPENDS_UI_SCREENSHOTS"):surface.write_to_png(str(Path(os.environ["OPENDS_UI_SCREENSHOTS"])/"monitor-active.png"))
        histories=[]
        original_source=window.get("command_source","2")
        for mode in range(3):
            window.set("command_source",mode)
            canvas=window.history_canvas
            surface=cairo.ImageSurface(cairo.FORMAT_ARGB32,canvas.get_allocated_width(),canvas.get_allocated_height())
            canvas.draw(canvas,cairo.Context(surface));surface.flush();histories.append(bytes(surface.get_data()))
        assert len(set(histories))==3,"Text, icon and hidden source modes should render differently"
        window.set("command_source",original_source)
        window.state=previous
    elif action in ("dark", "light"):
        window.set("theme", action)
        window.apply_theme()
        window.navigate("monitor")
        GLib.timeout_add(150, lambda: screenshot(action))
    elif action == "overlay":
        window.show_overlay()
        window.lock_overlay()
        assert window.overlay_locked
        window.restore_overlay()
        assert not window.overlay_locked
        import cairo
        old_alpha=window.get("command_bg_alpha","85")
        window.set("command_bg_alpha",20)
        canvas=window.overlay_canvas
        w,h=canvas.get_allocated_width(),canvas.get_allocated_height()
        surface=cairo.ImageSurface(cairo.FORMAT_ARGB32,w,h)
        canvas.draw(canvas,cairo.Context(surface));surface.flush()
        assert abs(bytes(surface.get_data())[(h-1)*surface.get_stride()+(w-1)*4+3]-51)<=1,"Overlay background alpha should remain independent"
        window.set("command_bg_alpha",old_alpha)
    elif action == "detach":
        window.toggle_detach()
        assert window.detached is not None
    elif action == "dock":
        window.toggle_detach()
        assert window.detached is None
        assert window.monitor_body.get_parent() == window.monitor_host
    elif action == "history":
        def dismiss_dialog():
            for item in Gtk.Window.list_toplevels():
                if isinstance(item, Gtk.Dialog):
                    item.response(Gtk.ResponseType.CANCEL)
            return False
        GLib.timeout_add(150, dismiss_dialog)
        window.history_settings()
    elif action == "microphone":
        window.microphone_test()
        assert window.microphone_window is not None
    elif action == "shutdown":
        window.shutdown()
    elif action == "end":
        if window.service.poll() is None:
            return True
        loop.quit()
        return False
    return True


GLib.timeout_add(400, step)
GLib.timeout_add(20000, lambda: (errors.append("UI test timed out"), window.shutdown(), loop.quit(), False)[-1])
loop.run()
if errors:
    raise SystemExit("UI verification failed: " + repr(errors))
assert window.service.poll() == 0, window.service.returncode
temporary.cleanup()
print("All pages, themes, overlay, monitor docking, settings, microphone window and shutdown passed.")
