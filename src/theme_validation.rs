//! UI smoke acceptance checks; never called by GUI/tray/bridge commands.
use super::*;
use crate::{
    appearance::{self, Kind},
    theme_dialog::{Request, TestAction},
};
impl App {
    pub(super) fn test_responsiveness(&self) -> Result<(), String> {
        use winapi::um::wingdi::*;
        unsafe {
            let hwnd = theme::hwnd(&self.window.handle);
            let original_page = self.page.get();
            let original_kind = appearance::palette().kind;
            self.page.set(2);
            self.layout();
            for kind in [Kind::Light, Kind::Dark, Kind::Contrast] {
                appearance::smoke_palette(kind);
                for control in [
                    &self.speaker.handle,
                    &self.haptic_gain.handle,
                    &self.port.handle,
                    &self.buffer.handle,
                ] {
                    let edit = theme::hwnd(control);
                    let mut rect: RECT = std::mem::zeroed();
                    GetWindowRect(edit, &mut rect);
                    let mut origin = POINT { x: 0, y: 0 };
                    ClientToScreen(edit, &mut origin);
                    let margin = origin.y - rect.top;
                    let width = rect.right - rect.left;
                    let height = rect.bottom - rect.top;
                    let screen = GetDC(hwnd);
                    let dc = CreateCompatibleDC(screen);
                    let bitmap = CreateCompatibleBitmap(screen, width, height);
                    let previous = SelectObject(dc, bitmap as _);
                    SendMessageW(
                        edit,
                        WM_PRINT,
                        dc as usize,
                        (PRF_NONCLIENT | PRF_CLIENT | PRF_ERASEBKGND) as isize,
                    );
                    let actual = GetPixel(dc, width / 2, (margin / 2).max(1));
                    let expected = appearance::palette().surface;
                    SelectObject(dc, previous);
                    DeleteObject(bitmap as _);
                    DeleteDC(dc);
                    ReleaseDC(hwnd, screen);
                    if margin > 2 && actual != expected {
                        return Err(format!("Edit margin color differs from surface: {kind:?}, actual={actual:06x}, expected={expected:06x}"));
                    }
                }
            }
            appearance::smoke_palette(original_kind);
            // Exercise the real button command path, including layout, repaint
            // and cached status updates. Device access is excluded in smoke mode.
            let mut samples = Vec::new();
            let mut commands = Vec::new();
            for _ in 0..10 {
                for button in &self.nav {
                    let started = std::time::Instant::now();
                    SendMessageW(
                        hwnd,
                        WM_COMMAND,
                        (BN_CLICKED as usize) << 16,
                        theme::hwnd(&button.handle) as isize,
                    );
                    commands.push(started.elapsed().as_micros());
                    RedrawWindow(
                        hwnd,
                        std::ptr::null(),
                        std::ptr::null_mut(),
                        RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW,
                    );
                    samples.push(started.elapsed().as_micros());
                }
            }
            samples.sort_unstable();
            commands.sort_unstable();
            let command_p95 = commands[commands.len() * 95 / 100];
            let p95 = samples[samples.len() * 95 / 100];
            let maximum = samples[samples.len() - 1];
            println!("UI responsiveness: command_p95_us={command_p95}, page_switch_p95_us={p95}, max_us={maximum}, native_edit_backgrounds=passed");
            // Gate event handling independently of the runner's GPU/DWM speed;
            // total frame latency remains reported for local comparison.
            if command_p95 > 250_000 {
                return Err("Page commands exceeded the 250 ms latency budget".into());
            }
            self.page.set(original_page);
            self.layout();
            Ok(())
        }
    }
    pub(super) fn test_themes(&self) -> Result<(), String> {
        unsafe {
            #[link(name = "user32")]
            extern "system" {
                fn GetGuiResources(process: *mut std::ffi::c_void, flags: u32) -> u32;
            }
            #[link(name = "kernel32")]
            extern "system" {
                fn GetCurrentProcess() -> *mut std::ffi::c_void;
            }
            fn check(value: bool, detail: &str) -> Result<(), String> {
                if value {
                    Ok(())
                } else {
                    Err(format!("Theme validation: {detail}"))
                }
            }
            unsafe fn pump() {
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(
                    &mut msg,
                    std::ptr::null_mut(),
                    crate::appearance::APPLY,
                    crate::appearance::APPLY + 1,
                    PM_REMOVE,
                ) != 0
                {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            println!("Theme validation: persistence and rollback");
            #[repr(C)]
            struct Memory {
                cb: u32,
                faults: u32,
                values: [usize; 9],
            }
            #[link(name = "psapi")]
            extern "system" {
                fn GetProcessMemoryInfo(
                    process: *mut std::ffi::c_void,
                    info: *mut Memory,
                    size: u32,
                ) -> i32;
            }
            unsafe fn private_bytes() -> usize {
                let mut memory: Memory = std::mem::zeroed();
                memory.cb = std::mem::size_of::<Memory>() as u32;
                if GetProcessMemoryInfo(
                    GetCurrentProcess(),
                    &mut memory,
                    std::mem::size_of::<Memory>() as u32,
                ) == 0
                {
                    0
                } else {
                    memory.values[8]
                }
            }
            let hwnd = theme::hwnd(&self.window.handle);
            let original = self.config.borrow().theme;
            let disk = Config::load(&self.path)?;
            let input = self.port.text();
            self.port.set_text("4321");
            let initial_check = checked(&self.auto_start);
            let state = self.controller.borrow().state.clone();
            let running = state.running.load(Ordering::Acquire);
            for mode in [ThemeMode::Light, ThemeMode::Dark, ThemeMode::System] {
                self.theme_choice.set_selection(Some(mode.index()));
                self.select_theme()?;
                pump();
                let saved = Config::load(&self.path)?;
                check(
                    saved.theme == mode
                        && saved.port == disk.port
                        && saved.close_action == disk.close_action,
                    "theme-only persistence",
                )?;
                check(
                    self.port.text() == "4321" && checked(&self.auto_start) == initial_check,
                    "unsaved controls changed",
                )?;
            }
            let previous = self.config.borrow().theme;
            let previous_palette = appearance::palette();
            let blocker = self
                .path
                .with_extension(format!("tmp-{}", std::process::id()));
            std::fs::write(&blocker, "owned theme validation fixture")
                .map_err(|e| e.to_string())?;
            self.theme_choice
                .set_selection(Some(ThemeMode::Dark.index()));
            let failed = self.select_theme();
            let _ = std::fs::remove_file(&blocker);
            check(
                failed.is_err()
                    && self.config.borrow().theme == previous
                    && appearance::palette() == previous_palette
                    && self.theme_choice.selection() == Some(previous.index()),
                "save failure rollback",
            )?;
            self.port.set_text(&input);
            // Real monitor/CSV and microphone diagnostic buffers, using synthetic samples only.
            let hub = state.monitor.clone();
            hub.source.store(true, Ordering::Release);
            hub.start(100)?;
            let csv = self.path.with_extension("theme-validation.csv");
            hub.start_recording(csv.clone())?;
            let microphone = hub.mic_test_capture.begin();
            hub.mic_test_capture.push(&[101; 48]);
            let pcm = vec![0x11u8; 3840];
            hub.bytes(&pcm);
            hub.input(crate::monitor::neutral());
            std::thread::sleep(std::time::Duration::from_millis(40));
            let fixed_time = hub.now();
            let retained = hub
                .history
                .lock()
                .map_err(|e| e.to_string())?
                .plot(30, 100, fixed_time);
            check(
                retained.iter().any(|b| b.valid != 0),
                "synthetic source did not enter history",
            )?;
            let audio = crate::audio_test_view::Dialog::new(hwnd, hub.clone())?;
            // Warm both palettes and all retained drawing resources before sampling handles.
            for kind in [Kind::Dark, Kind::Light] {
                appearance::smoke_palette(kind);
                pump();
                RedrawWindow(
                    hwnd,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW,
                );
            }
            crate::gui_close::smoke_test(hwnd)?;
            crate::gui_language::smoke(hwnd)?;
            if let Some(view) = self.monitor_view.borrow_mut().as_mut() {
                view.smoke();
            }
            pump();
            RedrawWindow(
                hwnd,
                std::ptr::null(),
                std::ptr::null_mut(),
                RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW,
            );
            println!("Theme validation: starting 500 palette switches");
            let gdi_before = GetGuiResources(GetCurrentProcess(), 0);
            let user_before = GetGuiResources(GetCurrentProcess(), 1);
            let registrations = appearance::registered_windows();
            let private_before = private_bytes();
            for i in 0..500 {
                appearance::smoke_palette(if i % 2 == 0 { Kind::Dark } else { Kind::Light });
                pump();
                RedrawWindow(
                    hwnd,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_UPDATENOW,
                );
                if i % 50 == 0 {
                    if let Some(view) = self.monitor_view.borrow_mut().as_mut() {
                        view.smoke();
                    }
                    let dialog = crate::audio_test_view::Dialog::new(hwnd, hub.clone())?;
                    drop(dialog);
                    crate::gui_close::smoke_test(hwnd)?;
                    crate::gui_language::smoke(hwnd)?;
                    hub.mic_test_capture.push(&[202; 48]);
                    println!("Theme validation checkpoint {}: GDI {}, USER {}, private {} bytes, history {} bytes",i,GetGuiResources(GetCurrentProcess(),0),GetGuiResources(GetCurrentProcess(),1),private_bytes(),hub.allocated_bytes());
                }
                hub.bytes(&pcm);
                hub.input(crate::monitor::neutral());
            }
            println!("Theme validation: palette switches finished");
            pump();
            let gdi_after = GetGuiResources(GetCurrentProcess(), 0);
            let user_after = GetGuiResources(GetCurrentProcess(), 1);
            println!("Theme validation handles: GDI {gdi_before}->{gdi_after}, USER {user_before}->{user_after}");
            check(
                gdi_after <= gdi_before + 2 && user_after <= user_before + 2,
                "GDI/USER handle growth",
            )?;
            check(
                appearance::registered_windows() == registrations,
                "window registrations leaked",
            )?;
            println!(
                "Theme validation private memory: {private_before}->{} bytes",
                private_bytes()
            );
            let kept = hub
                .history
                .lock()
                .map_err(|e| e.to_string())?
                .plot(30, 100, fixed_time);
            check(
                retained
                    .iter()
                    .zip(&kept)
                    .all(|(a, b)| a.valid == b.valid && a.min == b.min && a.max == b.max),
                "history lost on theme change",
            )?;
            check(
                hub.active.load(Ordering::Acquire) && hub.recording.active(),
                "capture/CSV stopped",
            )?;
            let mic = microphone.finish();
            check(mic.pcm.len() > 96, "microphone capture interrupted")?;
            hub.stop();
            let csv_text = std::fs::read_to_string(&csv).map_err(|e| e.to_string())?;
            check(csv_text.lines().count() > 2, "CSV rows missing")?;
            let _ = std::fs::remove_file(csv);
            drop(audio);
            check(
                state.running.load(Ordering::Acquire) == running,
                "bridge state changed",
            )?;
            for (action, expected) in [
                (TestAction::Key(VK_ESCAPE as u32), IDCANCEL),
                (TestAction::Key(VK_RETURN as u32), 100),
                (TestAction::Accelerator('l'), IDCANCEL),
                (TestAction::Accelerator('r'), 100),
            ] {
                let r = crate::theme_dialog::show(
                    hwnd,
                    Request {
                        title: "OpenDS · UI test",
                        message: "Keyboard validation",
                        buttons: &[(IDCANCEL, "&Later"), (100, "&Restart now")],
                        default: 100,
                        cancel: IDCANCEL,
                        verification: None,
                        test: Some(action),
                    },
                )?;
                check(r.button == expected, "dialog keyboard result")?;
            }
            appearance::set_mode(ThemeMode::System);
            for (setting, expected) in [
                ("dark", Kind::Dark),
                ("light", Kind::Light),
                ("contrast", Kind::Contrast),
                ("missing", Kind::Light),
            ] {
                std::env::set_var("OPENDS_UI_SMOKE_SYSTEM_THEME", setting);
                SendMessageW(hwnd, WM_SETTINGCHANGE, 0, 0);
                pump();
                check(
                    appearance::palette().kind == expected,
                    "system setting change/fallback",
                )?;
            }
            appearance::set_mode(ThemeMode::Light);
            std::env::set_var("OPENDS_UI_SMOKE_SYSTEM_THEME", "dark");
            SendMessageW(hwnd, WM_THEMECHANGED, 0, 0);
            pump();
            check(
                appearance::palette().kind == Kind::Light,
                "manual override changed with system",
            )?;
            std::env::set_var("OPENDS_UI_SMOKE_SYSTEM_THEME", "contrast");
            SendMessageW(hwnd, WM_SETTINGCHANGE, 0, 0);
            pump();
            check(
                appearance::palette().kind == Kind::Contrast,
                "high contrast must override manual choice",
            )?;
            std::env::remove_var("OPENDS_UI_SMOKE_SYSTEM_THEME");
            self.theme_choice.set_selection(Some(original.index()));
            self.select_theme()?;
            appearance::set_mode(original);
            // Exercise the same message path as Windows without modifying system settings.
            SendMessageW(hwnd, WM_SETTINGCHANGE, 0, 0);
            SendMessageW(hwnd, WM_THEMECHANGED, 0, 0);
            pump();
            println!("Theme validation: 500 switches, GDI {gdi_before}->{gdi_after}, USER {user_before}->{user_after}; history, CSV, microphone, controls and keyboard passed");
            self.layout();
            Ok(())
        }
    }
}
