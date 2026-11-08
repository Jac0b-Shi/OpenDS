# OpenDS · 3D Dev

An independent development version of the Ubuntu app. This experiment replaces the 2D live-monitor handset
with an actual 81-mesh DualSense model and native OpenGL 3.3 rendering.

## Start

From the repository root:

```bash
./scripts/build-linux.sh
# Extra 3D runtime/build dependencies on Ubuntu:
sudo apt-get install python3-numpy python3-pip libgl1-mesa-dri
./dev/dualsense3d/prepare.sh
./dev/dualsense3d/launch.sh
```

`prepare.sh` copies the built Linux backend into the private development bundle,
building it first if necessary, and downloads checksum-pinned PyOpenGL into that
bundle. It works from a fresh checkout and does not require an installed 2D app
or a local backup. Existing private backend copies are kept unchanged.

For an animated visual demo without a controller:

```bash
./dev/dualsense3d/launch.sh --demo
```

Install a separate **OpenDS 3D Dev** entry in Show Apps:

```bash
python3 dev/dualsense3d/install.py
```

The dev installer writes `~/.local/lib/opends-3d-dev` and
`~/.local/share/applications/io.github.OpenDS.Dev3D.desktop`. It never invokes
the stable installer or writes to `~/.local/lib/opends`.

## Model interaction

- Drag the body/background to orbit; scroll to zoom.
- Click the face buttons, D-pad, bumpers, Create, Options, PS, mute and touchpad
  to preview their travel. Drag sticks to tilt them, or drag triggers to vary
  their pull. Releasing the mouse returns controls to rest with spring smoothing.
- Hold Shift while dragging to orbit even when the pointer is over a control.
- **Front** selects the face-on camera; **Reset view**, R or double-click restores
  the studio camera and recenters optional tilt following.
- **Demo animation** exercises the model without sending input to the pad, game,
  recording or bridge. Its badge explicitly says `DEMO · VISUAL ONLY`.
- **Follow tilt** is experimental, bounded, gravity-relative accelerometer tilt.
  It is not calibrated absolute orientation and does not integrate yaw.
- **Appearance** selects White, Midnight Black, Cosmic Red, Nova Pink, Starlight
  Blue, Galactic Purple, Volcanic Red, Cobalt Blue, Sterling Silver or Grey
  Camouflage shell finishes. Grey Camouflage follows Sony's region placement:
  patterned front panels and touchpad, a plain grey rear, dark face buttons and
  D-pad, and unchanged light strips. Its subdued print includes small geometric
  motifs and remains attached to the model during animation. Placement and
  colors use [Sony's official product views](https://www.playstation.com/en-gb/accessories/dualsense-wireless-controller/)
  as references. The procedural print is an approximation, not a scan of the
  manufacturer's artwork. Other shell finishes retain their original controls.
- The viewport background is transparent by default. Disable **Transparent
  background** in Appearance to restore the studio backdrop and ground shadow.
  Appearance is saved in `~/.config/opends-3d-dev/opends/view3d.json` and survives
  app restarts and monitor detach/dock. Exported viewport PNGs retain alpha.
- **General settings → Appearance** applies Light, Dark or Follow system to the
  whole interface, including unchecked/checked boxes, page backgrounds, menus,
  dialogs, monitor plots and detached windows. Follow system responds to Ubuntu
  appearance changes while the app is open. Use **Save settings** to retain the
  selected app theme across restarts.

Live buttons, stick travel, trigger pull and both touch contacts come from the
same Rust desktop service as stable OpenDS. Start the bridge and **Start capture**
to see live input. The existing waveform plots, recording, history, overlay and
detachable monitor remain available. Pausing capture retains its last pose.

The model has studio lighting, a shadow map, textured materials, clear face-button
caps and named moving parts. Model and dependency credits are available in the
**Credits** button and [CREDITS.md](CREDITS.md). All runtime assets are local.

## Separate installation and profiles

The 3D installer registers its own application ID and never removes another
installation or overwrites its preferences. Machine-local migration archives
and settings are not distributed with the project.

The dev app runs a frozen copy of the stable service/UI, with a separate ID and
profile. Default locations:

- Settings: `~/.config/opends-3d-dev/opends/opends.ini`
- Logs/state: `~/.local/state/opends-3d-dev/opends/`
- Default virtual USB port: 3242

On first launch it copies settings into the dev profile, disables automatic
bridging there and selects port 3242. The former INI is retained. Only one app
can own the physical bridge; the shared ownership guard also protects against
a manually restored older app.

For disposable test profiles set `OPENDS_3D_PROFILE` and `OPENDS_3D_STATE` before
launch. The graphics use Ubuntu's Python, NumPy, GTK and Cairo, plus a private
checksum-pinned PyOpenGL package. No system Python packages are modified.

## Verification

```bash
/usr/bin/python3 dev/dualsense3d/tests/test_scene.py
LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a /usr/bin/python3 dev/dualsense3d/tests/render_smoke.py
LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a /usr/bin/python3 dev/dualsense3d/tests/app_smoke.py
LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a /usr/bin/python3 dev/dualsense3d/tests/theme_smoke.py
LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a /usr/bin/python3 dev/dualsense3d/tests/camouflage_smoke.py
LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a /usr/bin/python3 dev/dualsense3d/tests/test_install.py
```

The render test creates a real OpenGL context and reads back neutral/active
frames to `test-output/`. Mesa software rendering is supported for testing;
the desktop app uses the available graphics driver. This remains a development
prototype, with platform/GPU and physical-controller acceptance testing ongoing.

The full application test also verifies camera/control interaction, GPU context
recreation during detach/dock, profile separation, bridge ownership exclusion and
clean shutdown. Visual previews never write controller input or feedback.

The theme test checks rendered page and unchecked-indicator pixels in both
palettes, live system-theme changes and explicit overrides. Its settings are
isolated in a disposable profile and in-memory GNOME settings backend.

The camouflage test renders front, rear and side views, then compares the
rear panel with the print disabled to catch accidental camouflage on the back.
