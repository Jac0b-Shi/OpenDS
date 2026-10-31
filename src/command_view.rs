//! Shared event table and per-pixel-alpha overlay; all HWND state belongs to the UI thread.
use super::*;
use crate::commands::{self, Event, Kind, Settings};
use std::sync::atomic::Ordering::Relaxed;
use winapi::shared::dxgiformat::DXGI_FORMAT_B8G8R8A8_UNORM;
use winapi::um::dcommon::D2D1_ALPHA_MODE_PREMULTIPLIED;
const TIMER: usize = 0x4344;
const REFRESH_INTERVAL_MS: u32 = 50;
const SHOW: u32 = 101;
const LOCK: u32 = 102;
const RESET: u32 = 103;
const SETTINGS: u32 = 104;

/// Cached Direct2D target for one HWND.
///
/// Rebuilt on resize, DPI change and device loss rather than per frame, and
/// released with the window (`Drop`) so a closed overlay frees its surface.
struct Surface {
    renderer: Renderer,
    dc: HDC,
    bitmap: HBITMAP,
    pixels: *mut u32,
    old: HGDIOBJ,
}
impl Surface {
    unsafe fn new(_hwnd: HWND, w: i32, h: i32, dpi: f32) -> Result<Self, String> {
        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            return Err("CreateCompatibleDC failed".into());
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bits = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if bitmap.is_null() {
            DeleteDC(dc);
            return Err("CreateDIBSection failed".into());
        }
        let old = SelectObject(dc, bitmap as _);
        let make = (|| -> Result<Renderer, String> {
            let mut factory: Com<ID2D1Factory> = Com::empty();
            hr(D2D1CreateFactory(
                D2D1_FACTORY_TYPE_SINGLE_THREADED,
                &ID2D1Factory::uuidof(),
                null(),
                factory.out() as _,
            ))?;
            let mut dw: Com<IDWriteFactory> = Com::empty();
            hr(DWriteCreateFactory(
                DWRITE_FACTORY_TYPE_SHARED,
                &IDWriteFactory::uuidof(),
                dw.out() as _,
            ))?;
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                _type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: dpi,
                dpiY: dpi,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let mut target: Com<ID2D1DCRenderTarget> = Com::empty();
            hr((*factory.0).CreateDCRenderTarget(&props, target.out()))?;
            let rect = RECT {
                left: 0,
                top: 0,
                right: w,
                bottom: h,
            };
            hr((*target.0).BindDC(dc, &rect))?;
            let ptr = target.0;
            target.0 = null_mut();
            Renderer::from_target(factory, dw, Com(ptr as *mut ID2D1RenderTarget))
        })();
        match make {
            Ok(renderer) => Ok(Self {
                renderer,
                dc,
                bitmap,
                pixels: bits as *mut u32,
                old,
            }),
            Err(e) => {
                SelectObject(dc, old);
                DeleteObject(bitmap as _);
                DeleteDC(dc);
                Err(e)
            }
        }
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            self.renderer.brush = Com::empty();
            self.renderer.target = Com::empty();
            SelectObject(self.dc, self.old);
            DeleteObject(self.bitmap as _);
            DeleteDC(self.dc);
        }
    }
}

struct WindowState {
    hub: Arc<Monitor>,
    cfg: Settings,
    path: PathBuf,
    main: HWND,
    main_geometry: (i32, i32, i32, i32, u32),
    main_shown: bool,
    overlay: HWND,
    settings: HWND,
    list: HWND,
    edit: HWND,
    choices: HWND,
    status: HWND,
    preview: HWND,
    keys: Vec<String>,
    locked: bool,
    show_main: bool,
    main_surface: Option<Surface>,
    preview_surface: Option<Surface>,
    surface: Option<Surface>,
    overlay_size: (i32, i32),
    layered_ok: bool,
    last_pulse: Instant,
    last_main: String,
    last_overlay: String,
    name_revisions: [u64; 2],
}
/// One rendered instance of the table.
///
/// The main panel and the overlay are separate `View`s over the same
/// [`commands::Commands`] data; neither owns the history, so opening or closing
/// one leaves the other's contents intact.
pub struct View {
    state: Box<RefCell<WindowState>>,
}
struct NativeFont(HFONT);
impl Drop for NativeFont {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.0 as _);
        }
    }
}
thread_local! { static FONTS: RefCell<std::collections::HashMap<u32,NativeFont>> = RefCell::new(std::collections::HashMap::new()); }
unsafe fn native_font(dpi: u32) -> HFONT {
    FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        fonts
            .entry(dpi)
            .or_insert_with(|| {
                let mut lf: LOGFONTW = std::mem::zeroed();
                lf.lfHeight = -(13 * dpi as i32 / 96);
                lf.lfWeight = 400;
                lf.lfCharSet = DEFAULT_CHARSET as u8;
                for (dst, src) in lf
                    .lfFaceName
                    .iter_mut()
                    .zip(wide(crate::gui_theme::font_family()))
                {
                    *dst = src;
                }
                NativeFont(CreateFontIndirectW(&lf))
            })
            .0
    })
}
unsafe fn control(
    parent: HWND,
    class: &str,
    text: &str,
    style: u32,
    id: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> HWND {
    let dpi = crate::gui_theme::window_dpi(parent).max(96) as i32;
    let (x, y, w, h) = (x * dpi / 96, y * dpi / 96, w * dpi / 96, h * dpi / 96);
    let hwnd = CreateWindowExW(
        0,
        wide(class).as_ptr(),
        wide(text).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        w,
        h,
        parent,
        id as _,
        GetModuleHandleW(null()),
        null_mut(),
    );
    SendMessageW(hwnd, WM_SETFONT, native_font(dpi as u32) as usize, 0);
    hwnd
}
impl View {
    pub unsafe fn validate(&mut self) -> Result<(), String> {
        let ptr = self.state.as_ref() as *const RefCell<WindowState>;
        let mut s = self.state.borrow_mut();
        if s.overlay.is_null() {
            return Err("Overlay was not created".into());
        }
        let original = s.cfg.clone();
        s.locked = true;
        s.styles();
        if IsWindowVisible(s.overlay) == 0 {
            return Err("Locking hid the overlay".into());
        }
        let style = GetWindowLongPtrW(s.overlay, GWL_EXSTYLE) as u32;
        if style & (WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE)
            != (WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE)
        {
            return Err("Mouse passthrough styles missing".into());
        }
        s.locked = false;
        s.styles();
        if IsWindowVisible(s.overlay) == 0 {
            return Err("Unlocking hid the overlay".into());
        }
        if GetWindowLongPtrW(s.overlay, GWL_EXSTYLE) as u32 & WS_EX_TRANSPARENT != 0 {
            return Err("Overlay did not unlock".into());
        }
        let mut state = commands::State::default();
        state.sticks[0] = commands::Axis {
            direction: 2,
            strength: 3,
        };
        state.buttons = 16;
        let e = Event {
            session: 1,
            sequence: 1,
            time: 0,
            delta: Some(1_250_000_000),
            kind: Kind::Input,
            state,
            raw: [0; 9],
        };
        for (bg, fg) in [(0, 0), (55, 100), (100, 0)] {
            s.cfg.bg_alpha = bg;
            s.cfg.fg_alpha = fg;
            let hwnd = s.overlay;
            s.draw_overlay(hwnd, 560, 430, 96., &[e.clone()], Some(&e));
            if !s.layered_ok {
                return Err("UpdateLayeredWindow failed".into());
            }
            let surface = s
                .surface
                .as_ref()
                .ok_or("Layered drawing surface unavailable")?;
            let pixels = std::slice::from_raw_parts(surface.pixels, 560 * 430);
            let (min, max) = pixels
                .iter()
                .map(|p| p >> 24)
                .fold((255, 0), |(min, max), a| (min.min(a), max.max(a)));
            if (bg == 0 && fg == 0 && max != 0)
                || (bg == 55 && fg == 100 && (min < 139 || max < 250))
                || (bg == 100 && fg == 0 && (min != 255 || max != 255))
            {
                return Err(format!(
                    "Unexpected alpha: bg={bg} fg={fg} min={min} max={max}"
                ));
            }
        }
        if crate::appearance::palette().kind != crate::appearance::Kind::Contrast {
            s.cfg.auto_colors = false;
            s.cfg.theme = 2;
            s.cfg.bg_alpha = 100;
            s.cfg.fg_alpha = 100;
            s.cfg.background[1] = 0;
            s.cfg.dark_button = 0x00ff00;
            s.cfg.dark_connection = 0x00ff00;
            for kind in [Kind::Input, Kind::Connected] {
                let mut sample = e.clone();
                sample.kind = kind;
                let hwnd = s.overlay;
                s.draw_overlay(hwnd, 560, 430, 96., &[sample.clone()], Some(&sample));
                let surface = s
                    .surface
                    .as_ref()
                    .ok_or("Manual palette surface unavailable")?;
                let pixels = std::slice::from_raw_parts(surface.pixels, 560 * 430);
                if !pixels.iter().any(|p| {
                    let (r, g, b) = ((p >> 16) & 255, (p >> 8) & 255, p & 255);
                    g > r + 80 && g > b + 80
                }) {
                    return Err(
                        "Manual dark palette was not used for buttons or connection icons".into(),
                    );
                }
            }
        }
        s.cfg = original;
        s.open_settings(ptr);
        for color in [false, true, false] {
            let fields = s.cfg.fields();
            let index = fields
                .iter()
                .position(|(_, v)| v.starts_with('#') == color)
                .ok_or("No matching setting for label check")?;
            SendMessageW(s.list, LB_SETCURSEL, index, 0);
            s.select_setting();
            let mut text = [0u16; 128];
            let length = GetWindowTextW(GetDlgItem(s.settings, 204), text.as_mut_ptr(), 128);
            let actual = String::from_utf16_lossy(&text[..length.max(0) as usize]);
            let expected = if color {
                crate::tr!(command_setting_color_value)
            } else {
                crate::tr!(command_setting_value)
            };
            if actual != expected {
                return Err(format!("Unexpected setting value label: {actual}"));
            }
            let swatch = GetDlgItem(s.settings, 210);
            if (GetWindowLongPtrW(swatch, GWL_STYLE) & WS_VISIBLE as isize != 0) != color {
                return Err("Color swatch visibility does not match selected setting".into());
            }
            if color {
                for value in ["#123456", "#FF0000", "#invalid"] {
                    SetWindowTextW(s.edit, wide(value).as_ptr());
                    let expected = match value {
                        "#123456" => Some(0x563412),
                        "#FF0000" => Some(0x0000ff),
                        _ => None,
                    };
                    if s.swatch_color() != expected {
                        return Err("Color swatch did not follow the edited RGB value".into());
                    }
                }
                s.select_setting();
            }
        }
        for id in [202, 203, 205, 206, 207] {
            crate::theme_controls::validate_palette(GetDlgItem(s.settings, id))?;
        }
        ShowWindow(s.settings, SW_HIDE);
        s.restore_bounds();
        s.styles();
        s.last_overlay.clear();
        eprintln!("Command overlay alpha, lock/unlock and layered surface checks passed");
        Ok(())
    }
    pub unsafe fn new(
        parent: HWND,
        hub: Arc<Monitor>,
        cfg: Settings,
        path: PathBuf,
    ) -> Result<Self, String> {
        register("OpenDSCommandTable", Some(proc));
        register("OpenDSCommandOverlay", Some(proc));
        register("OpenDSCommandSettings", Some(proc));
        register("OpenDSCommandSwatch", Some(proc));
        register("OpenDSCommandPreview", Some(proc));
        let state = Box::new(RefCell::new(WindowState {
            hub,
            cfg,
            path,
            main: null_mut(),
            main_geometry: (0, 0, 0, 0, 0),
            main_shown: false,
            overlay: null_mut(),
            settings: null_mut(),
            list: null_mut(),
            edit: null_mut(),
            choices: null_mut(),
            status: null_mut(),
            preview: null_mut(),
            keys: vec![],
            locked: false,
            show_main: !(std::env::args().any(|a| a == "--smoke-command-benchmark")
                && matches!(
                    std::env::var("OPENDS_COMMAND_BENCH_MODE").as_deref(),
                    Ok("disabled" | "overlay")
                )),
            main_surface: None,
            preview_surface: None,
            surface: None,
            overlay_size: (0, 0),
            layered_ok: false,
            last_pulse: Instant::now(),
            last_main: String::new(),
            last_overlay: String::new(),
            name_revisions: [u64::MAX; 2],
        }));
        let ptr = &*state as *const RefCell<WindowState>;
        let h = CreateWindowExW(
            0,
            wide("OpenDSCommandTable").as_ptr(),
            wide(crate::tr!(command_title)).as_ptr(),
            WS_CHILD | WS_CLIPCHILDREN,
            0,
            0,
            360,
            320,
            parent,
            null_mut(),
            GetModuleHandleW(null()),
            null_mut(),
        );
        if h.is_null() {
            return Err("Could not create command table".into());
        }
        state.borrow_mut().main = h;
        SetWindowLongPtrW(h, GWLP_USERDATA, ptr as isize);
        for (text, id, x, w) in [
            (crate::tr!(command_overlay), SHOW, 4, 54),
            (crate::tr!(command_lock), LOCK, 60, 62),
            (crate::tr!(command_short_settings), SETTINGS, 124, 48),
            (crate::tr!(command_short_reset), RESET, 174, 48),
            (crate::tr!(command_clear), 105, 224, 48),
        ] {
            control(h, "BUTTON", text, BS_PUSHBUTTON, id as usize, x, 28, w, 25);
        }
        SetTimer(h, TIMER, REFRESH_INTERVAL_MS, None);
        crate::appearance::install(h);
        Ok(Self { state })
    }
    pub unsafe fn place(
        &mut self,
        parent: HWND,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        dpi: f32,
        visible: bool,
    ) {
        let mut s = self.state.borrow_mut();
        let visible = visible && s.show_main;
        if GetParent(s.main) != parent {
            SetParent(s.main, parent);
            SetWindowLongPtrW(
                parent,
                GWL_STYLE,
                GetWindowLongPtrW(parent, GWL_STYLE) | WS_CLIPCHILDREN as isize,
            );
        }
        let scale = dpi / 96.;
        let mut r: RECT = std::mem::zeroed();
        GetWindowRect(s.main, &mut r);
        let width = (w * scale) as i32;
        let mut bounds: RECT = std::mem::zeroed();
        GetClientRect(parent, &mut bounds);
        let requested_y = (y * scale) as i32;
        let py = requested_y.max(0);
        // Keep the newest row visible while this block is on-screen, without covering
        // the wave plots that follow it in the narrow, single-column layout.
        let height = ((h * scale) as i32 + requested_y.min(0))
            .max(0)
            .min((bounds.bottom - py).max(0));
        let visible = visible && height > 60;
        if r.right - r.left != width || r.bottom - r.top != height {
            s.main_surface = None;
        }
        let geometry = (
            (x * scale) as i32,
            py,
            width.max(1),
            height.max(1),
            dpi.to_bits(),
        );
        if s.main_geometry != geometry {
            s.main_geometry = geometry;
            SetWindowPos(
                s.main,
                null_mut(),
                (x * scale) as i32,
                py,
                width.max(1),
                height.max(1),
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
            for (id, x, width) in [
                (101, 4, 54),
                (102, 60, 62),
                (104, 124, 48),
                (103, 174, 48),
                (105, 224, 48),
            ] {
                let c = GetDlgItem(s.main, id);
                SetWindowPos(
                    c,
                    null_mut(),
                    (x as f32 * scale) as i32,
                    (28. * scale) as i32,
                    (width as f32 * scale) as i32,
                    (25. * scale) as i32,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                SendMessageW(c, WM_SETFONT, native_font(dpi as u32) as usize, 1);
            }
        }
        if s.main_shown != visible {
            s.main_shown = visible;
            ShowWindow(s.main, if visible { SW_SHOWNA } else { SW_HIDE });
        }
    }
    pub unsafe fn pulse(&mut self) {
        self.state.borrow_mut().pulse();
    }
    pub unsafe fn action(&mut self, action: u32) {
        let ptr = &*self.state as *const RefCell<WindowState>;
        let mut s = self.state.borrow_mut();
        s.action(action, ptr);
    }
}
impl Drop for View {
    fn drop(&mut self) {
        let s = self.state.get_mut();
        s.hub.commands.main_capacity.store(0, Relaxed);
        s.hub.commands.visible_capacity(true, 0);
        unsafe {
            for h in [s.settings, s.overlay, s.main] {
                if !h.is_null() {
                    SetWindowLongPtrW(h, GWLP_USERDATA, 0);
                    DestroyWindow(h);
                }
            }
        }
    }
}

impl WindowState {
    fn dark(&self, overlay: bool) -> bool {
        if overlay {
            match self.cfg.theme {
                1 => false,
                2 => true,
                _ => crate::appearance::palette().kind != crate::appearance::Kind::Light,
            }
        } else {
            crate::appearance::palette().kind != crate::appearance::Kind::Light
        }
    }
    unsafe fn persist(&mut self) {
        if let Ok(mut cfg) = Config::load(&self.path) {
            cfg.commands = self.cfg.clone();
            if let Err(e) = cfg.save(&self.path) {
                self.message(&e);
            }
        }
        self.hub
            .commands
            .configure(self.cfg.clone(), self.hub.now());
        self.last_main.clear();
        self.last_overlay.clear();
        InvalidateRect(self.main, null(), 0);
    }
    unsafe fn message(&self, s: &str) {
        if !self.status.is_null() {
            SetWindowTextW(self.status, wide(s).as_ptr());
        } else {
            SetWindowTextW(self.main, wide(s).as_ptr());
        }
    }
    unsafe fn action(&mut self, action: u32, ptr: *const RefCell<Self>) {
        match action {
            SHOW => {
                if self.overlay.is_null() {
                    self.overlay = CreateWindowExW(
                        WS_EX_LAYERED | WS_EX_TOOLWINDOW,
                        wide("OpenDSCommandOverlay").as_ptr(),
                        wide(crate::tr!(command_overlay)).as_ptr(),
                        WS_POPUP,
                        self.cfg.x,
                        self.cfg.y,
                        self.cfg.width,
                        self.cfg.height,
                        null_mut(),
                        null_mut(),
                        GetModuleHandleW(null()),
                        null_mut(),
                    );
                    if !self.overlay.is_null() {
                        SetWindowLongPtrW(self.overlay, GWLP_USERDATA, ptr as isize);
                        SetTimer(self.overlay, TIMER, REFRESH_INTERVAL_MS, None);
                        crate::appearance::install(self.overlay);
                        self.locked = false;
                        self.restore_bounds();
                        self.styles();
                        ShowWindow(self.overlay, SW_SHOWNOACTIVATE);
                    }
                } else {
                    let visible = IsWindowVisible(self.overlay) != 0;
                    ShowWindow(
                        self.overlay,
                        if visible { SW_HIDE } else { SW_SHOWNOACTIVATE },
                    );
                    if visible {
                        self.hub.commands.visible_capacity(true, 0);
                    }
                }
            }
            LOCK => {
                if !self.overlay.is_null() {
                    self.locked = !self.locked;
                    self.styles();
                }
            }
            RESET => {
                self.cfg.x = 100;
                self.cfg.y = 100;
                self.cfg.width = 560;
                self.cfg.height = 430;
                self.cfg.bg_alpha = 55;
                self.cfg.fg_alpha = 100;
                self.locked = false;
                if !self.overlay.is_null() {
                    self.restore_bounds();
                    self.styles();
                    ShowWindow(self.overlay, SW_SHOWNOACTIVATE);
                }
                self.persist();
            }
            SETTINGS => self.open_settings(ptr),
            105 => {
                self.hub.commands.clear(self.hub.now());
                self.last_main.clear();
                self.last_overlay.clear();
            }
            _ => {}
        }
    }
    unsafe fn restore_bounds(&mut self) {
        let mon = MonitorFromPoint(
            POINT {
                x: self.cfg.x,
                y: self.cfg.y,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) != 0 {
            let r = mi.rcWork;
            let dpi = crate::gui_theme::window_dpi(self.overlay).max(96) as i32;
            let w = (self.cfg.width * dpi / 96).min(r.right - r.left);
            let h = (self.cfg.height * dpi / 96).min(r.bottom - r.top);
            let x = self.cfg.x.clamp(r.left, (r.right - w).max(r.left));
            let y = self.cfg.y.clamp(r.top, (r.bottom - h).max(r.top));
            SetWindowPos(
                self.overlay,
                null_mut(),
                x,
                y,
                w,
                h,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
    unsafe fn styles(&mut self) {
        SetWindowTextW(
            GetDlgItem(self.main, LOCK as i32),
            wide(if self.locked {
                crate::tr!(command_unlock)
            } else {
                crate::tr!(command_lock)
            })
            .as_ptr(),
        );
        let h = self.overlay;
        if h.is_null() {
            return;
        }
        let ex = WS_EX_LAYERED
            | WS_EX_TOOLWINDOW
            | if self.locked {
                WS_EX_TRANSPARENT | WS_EX_NOACTIVATE
            } else {
                0
            };
        SetWindowLongPtrW(h, GWL_EXSTYLE, ex as isize);
        SetWindowPos(
            h,
            if self.cfg.topmost {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            },
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
        self.surface = None;
        self.last_overlay.clear();
    }
    unsafe fn pulse(&mut self) {
        if self.last_pulse.elapsed().as_millis() < REFRESH_INTERVAL_MS as u128 {
            return;
        }
        self.last_pulse = Instant::now();
        self.tick(self.main);
        if !self.overlay.is_null() {
            self.tick(self.overlay);
        }
    }
    unsafe fn paint_table(&mut self, hwnd: HWND, dc: HDC) {
        if hwnd == self.main || hwnd == self.preview {
            let preview = hwnd == self.preview;
            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            let dpi = crate::gui_theme::window_dpi(hwnd).max(96) as f32;
            let width = rect.right as f32 * 96. / dpi;
            let height = rect.bottom as f32 * 96. / dpi;
            let dark = self.dark(preview);
            let mut cfg = self.cfg.clone();
            let contrast = crate::appearance::palette().kind == crate::appearance::Kind::Contrast;
            let (events, latest) = self
                .hub
                .commands
                .history
                .lock()
                .map(|h| {
                    (
                        if h.events.is_empty() {
                            h.latest.iter().cloned().collect::<Vec<_>>()
                        } else {
                            h.events.iter().cloned().collect::<Vec<_>>()
                        },
                        h.latest.clone(),
                    )
                })
                .unwrap_or_default();
            if preview && self.preview_surface.is_none() {
                self.preview_surface = Surface::new(hwnd, rect.right, rect.bottom, dpi).ok();
            } else if !preview && self.main_surface.is_none() {
                self.main_surface = Surface::new(hwnd, rect.right, rect.bottom, dpi).ok();
            }
            if let Some(surface) = if preview {
                self.preview_surface.as_ref()
            } else {
                self.main_surface.as_ref()
            } {
                let r = &surface.renderer;
                (*r.target.0).BeginDraw();
                let (bg, _) = cfg.palette(dark);
                (*r.target.0).Clear(&if contrast {
                    palette_color(crate::appearance::palette().bg)
                } else {
                    ink(bg, 1.)
                });
                let start = if preview {
                    6.
                } else {
                    r.text(
                        8.,
                        2.,
                        width - 16.,
                        24.,
                        crate::tr!(command_title),
                        1,
                        if contrast {
                            palette_color(crate::appearance::palette().text)
                        } else {
                            ink(cfg.palette(dark).1, 1.)
                        },
                    );
                    60.
                };
                if preview {
                    (*r.target.0).Clear(&ink(0xb0b0b0, 1.));
                    for row in 0..((height / 16.) as usize + 1) {
                        for col in 0..((width / 16.) as usize + 1) {
                            if (row + col) % 2 == 0 {
                                r.rect(
                                    col as f32 * 16.,
                                    row as f32 * 16.,
                                    16.,
                                    16.,
                                    0.,
                                    ink(0xe8e8e8, 1.),
                                );
                            }
                        }
                    }
                    r.rect(0., 0., width, height, 0., {
                        let mut c = if contrast {
                            palette_color(crate::appearance::palette().bg)
                        } else {
                            ink(cfg.palette(dark).0, 1.)
                        };
                        c.a = cfg.bg_alpha as f32 / 100.;
                        c
                    });
                    cfg.right = true;
                    let samples = (1..=3)
                        .map(|level| {
                            let mut state = commands::State::default();
                            state.sticks = [
                                commands::Axis {
                                    direction: 2,
                                    strength: level,
                                },
                                commands::Axis {
                                    direction: 7,
                                    strength: level,
                                },
                            ];
                            state.triggers = [level; 2];
                            state.buttons = 16;
                            Event {
                                session: 0,
                                sequence: level as u64,
                                time: (3 - level as u64) * 1_250_000_000,
                                delta: Some(1_250_000_000),
                                kind: Kind::Input,
                                state,
                                raw: [0; 9],
                            }
                        })
                        .collect::<Vec<_>>();
                    draw(
                        r,
                        &cfg,
                        &samples,
                        samples.first(),
                        4_900_000_000,
                        width,
                        height,
                        start,
                        dark,
                        cfg.fg_alpha as f32 / 100.,
                        3,
                    );
                } else {
                    draw(
                        r,
                        &cfg,
                        &events,
                        latest.as_ref(),
                        self.hub.display_time(),
                        width,
                        height,
                        start,
                        dark,
                        1.,
                        cfg.rows,
                    );
                }
                if (*r.target.0).EndDraw(null_mut(), null_mut()) < 0 {
                    if preview {
                        self.preview_surface = None;
                    } else {
                        self.main_surface = None;
                    }
                } else {
                    BitBlt(dc, 0, 0, rect.right, rect.bottom, surface.dc, 0, 0, SRCCOPY);
                }
            }
        }
    }
    unsafe fn tick(&mut self, hwnd: HWND) {
        let overlay = hwnd == self.overlay;
        let visible = IsWindowVisible(hwnd) != 0 && IsIconic(GetAncestor(hwnd, GA_ROOT)) == 0;
        if !visible {
            self.hub.commands.visible_capacity(overlay, 0);
            if overlay {
                self.last_overlay.clear();
            } else {
                self.last_main.clear();
            }
            return;
        }
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rect);
        let dpi = crate::gui_theme::window_dpi(hwnd).max(96) as f32;
        let width = rect.right as f32 * 96. / dpi;
        let height = rect.bottom as f32 * 96. / dpi;
        let (events, revision, latest) = if let Ok(h) = self.hub.commands.history.try_lock() {
            (
                if h.events.is_empty() {
                    h.latest.iter().cloned().collect::<Vec<_>>()
                } else {
                    h.events.iter().cloned().collect::<Vec<_>>()
                },
                h.revision,
                h.latest.clone(),
            )
        } else {
            return;
        };
        let rows = if overlay {
            self.cfg.overlay_rows
        } else {
            self.cfg.rows
        };
        let mut remaining = height - if overlay { 28. } else { 60. };
        let mut capacity = 0;
        for e in events.iter().take(rows) {
            let h = event_height(e, &self.cfg, width);
            if remaining < h {
                break;
            }
            remaining -= h;
            capacity += 1;
        }
        if capacity == events.len() && events.len() < rows {
            capacity +=
                ((remaining / row_height(width)).floor().max(0.) as usize).min(rows - events.len());
        }
        self.hub.commands.visible_capacity(overlay, capacity);
        let wait = latest
            .as_ref()
            .map(|e| commands::duration(self.hub.display_time().saturating_sub(e.time)))
            .unwrap_or_default();
        let dark = self.dark(overlay);
        let key = format!(
            "{revision}/{wait}/{dark}/{:?}/{:?}/{:08X}/{:08X}",
            crate::appearance::palette().kind,
            (rect.right, rect.bottom),
            crate::appearance::palette().bg,
            crate::appearance::palette().text
        );
        let last = if overlay {
            &mut self.last_overlay
        } else {
            &mut self.last_main
        };
        if *last != key {
            *last = key;
            if overlay {
                self.draw_overlay(hwnd, rect.right, rect.bottom, dpi, &events, latest.as_ref());
            } else {
                let dc = GetDC(hwnd);
                if !dc.is_null() {
                    self.paint_table(hwnd, dc);
                    ReleaseDC(hwnd, dc);
                    ValidateRect(hwnd, null());
                }
            }
            if self.name_revisions[overlay as usize] != revision {
                self.name_revisions[overlay as usize] = revision;
                let names = events
                    .iter()
                    .take(capacity)
                    .map(|e| match e.kind {
                        Kind::Input => commands::combination(e.state, self.cfg.right),
                        _ => event_label(e.kind).into(),
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                SetWindowTextW(
                    hwnd,
                    wide(&format!("{}: {names}", crate::tr!(command_title))).as_ptr(),
                );
            }
        }
        if let Some(e) = self.hub.commands.recording.error() {
            self.message(&e);
        }
    }
    unsafe fn draw_overlay(
        &mut self,
        hwnd: HWND,
        w: i32,
        h: i32,
        dpi: f32,
        events: &[Event],
        latest: Option<&Event>,
    ) {
        if w <= 0 || h <= 0 {
            return;
        }
        if self.surface.is_none() || self.overlay_size != (w, h) {
            self.surface = None;
            match Surface::new(hwnd, w, h, dpi) {
                Ok(s) => {
                    self.surface = Some(s);
                    self.overlay_size = (w, h)
                }
                Err(e) => {
                    self.message(&e);
                    return;
                }
            }
        }
        let dark = self.dark(true);
        let Some(s) = self.surface.as_ref() else {
            return;
        };
        let r = &s.renderer;
        (*r.target.0).BeginDraw();
        (*r.target.0).SetDpi(dpi, dpi);
        let (bg, _) = self.cfg.palette(dark);
        let mut background =
            if crate::appearance::palette().kind == crate::appearance::Kind::Contrast {
                palette_color(crate::appearance::palette().bg)
            } else {
                ink(bg, 1.)
            };
        background.a = self.cfg.bg_alpha as f32 / 100.;
        (*r.target.0).Clear(&background);
        r.text(
            8.,
            2.,
            w as f32 * 96. / dpi - 16.,
            24.,
            crate::tr!(command_title),
            0,
            if crate::appearance::palette().kind == crate::appearance::Kind::Contrast {
                let mut c = palette_color(crate::appearance::palette().text);
                c.a = self.cfg.fg_alpha as f32 / 100.;
                c
            } else {
                ink(self.cfg.palette(dark).1, self.cfg.fg_alpha as f32 / 100.)
            },
        );
        draw(
            r,
            &self.cfg,
            events,
            latest,
            self.hub.display_time(),
            w as f32 * 96. / dpi,
            h as f32 * 96. / dpi,
            28.,
            dark,
            self.cfg.fg_alpha as f32 / 100.,
            self.cfg.overlay_rows,
        );
        if (*r.target.0).EndDraw(null_mut(), null_mut()) < 0 {
            self.surface = None;
            return;
        }
        let mut wr: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut wr);
        let mut dest = POINT {
            x: wr.left,
            y: wr.top,
        };
        let mut size = SIZE { cx: w, cy: h };
        let mut source = POINT { x: 0, y: 0 };
        let mut blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA,
        };
        let updated = UpdateLayeredWindow(
            hwnd,
            null_mut(),
            &mut dest,
            &mut size,
            s.dc,
            &mut source,
            0,
            &mut blend,
            ULW_ALPHA,
        );
        self.layered_ok = updated != 0;
        if updated == 0 && std::env::args().nth(1).as_deref() == Some("ui-smoke-test") {
            eprintln!(
                "command_overlay_update_failed={}",
                std::io::Error::last_os_error()
            );
        }
    }
    unsafe fn open_settings(&mut self, ptr: *const RefCell<Self>) {
        if !self.settings.is_null() {
            ShowWindow(self.settings, SW_SHOW);
            SetForegroundWindow(self.settings);
            return;
        }
        let dpi = crate::gui_theme::window_dpi(self.main).max(96) as i32;
        let h = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            wide("OpenDSCommandSettings").as_ptr(),
            wide(crate::tr!(command_settings)).as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            100,
            100,
            820 * dpi / 96,
            540 * dpi / 96,
            self.main,
            null_mut(),
            GetModuleHandleW(null()),
            null_mut(),
        );
        self.settings = h;
        SetWindowLongPtrW(h, GWLP_USERDATA, ptr as isize);
        crate::appearance::install(h);
        self.list = control(
            h,
            "LISTBOX",
            "",
            LBS_NOTIFY | WS_VSCROLL | WS_BORDER,
            201,
            12,
            12,
            370,
            390,
        );
        self.edit = control(
            h,
            "EDIT",
            "",
            WS_BORDER | ES_AUTOHSCROLL,
            202,
            400,
            45,
            220,
            28,
        );
        self.choices = control(
            h,
            "COMBOBOX",
            "",
            CBS_DROPDOWNLIST | WS_VSCROLL,
            203,
            400,
            45,
            260,
            200,
        );
        control(
            h,
            "STATIC",
            crate::tr!(command_setting_value),
            0,
            204,
            400,
            15,
            360,
            25,
        );
        control(
            h,
            "BUTTON",
            crate::tr!(command_apply),
            BS_PUSHBUTTON,
            205,
            400,
            86,
            100,
            28,
        );
        control(
            h,
            "BUTTON",
            crate::tr!(command_color),
            BS_PUSHBUTTON,
            206,
            510,
            86,
            100,
            28,
        );
        control(
            h,
            "BUTTON",
            crate::tr!(command_defaults),
            BS_PUSHBUTTON,
            207,
            620,
            86,
            150,
            28,
        );
        self.status = control(
            h,
            "STATIC",
            crate::tr!(command_settings_hint),
            SS_LEFT,
            208,
            12,
            420,
            770,
            65,
        );
        self.preview = control(
            h,
            "OpenDSCommandPreview",
            crate::tr!(command_preview),
            0,
            209,
            400,
            130,
            370,
            270,
        );
        SetWindowLongPtrW(self.preview, GWLP_USERDATA, ptr as isize);
        let swatch = control(
            h,
            "OpenDSCommandSwatch",
            crate::tr!(command_color_preview),
            0,
            210,
            628,
            45,
            28,
            28,
        );
        SetWindowLongPtrW(swatch, GWLP_USERDATA, ptr as isize);
        // The controls must exist before the theme adapters enumerate children.
        crate::appearance::install(h);
        self.fill_settings();
        SetWindowPos(h, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        ShowWindow(h, SW_SHOW);
    }
    unsafe fn settings_layout(&mut self) {
        let dpi = crate::gui_theme::window_dpi(self.settings).max(96) as i32;
        for (id, x, y, w, h) in [
            (201, 12, 12, 370, 390),
            (202, 400, 45, 220, 28),
            (203, 400, 45, 260, 200),
            (204, 400, 15, 360, 25),
            (205, 400, 86, 100, 28),
            (206, 510, 86, 100, 28),
            (207, 620, 86, 150, 28),
            (208, 12, 420, 770, 65),
            (209, 400, 130, 370, 270),
            (210, 628, 45, 28, 28),
        ] {
            let c = GetDlgItem(self.settings, id);
            SetWindowPos(
                c,
                null_mut(),
                x * dpi / 96,
                y * dpi / 96,
                w * dpi / 96,
                h * dpi / 96,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            SendMessageW(c, WM_SETFONT, native_font(dpi as u32) as usize, 1);
        }
        self.preview_surface = None;
    }
    unsafe fn notify_layout(&self) {
        let canvas = GetParent(self.main);
        let mut name = [0u16; 64];
        let n = GetClassNameW(canvas, name.as_mut_ptr(), 64);
        if String::from_utf16_lossy(&name[..n.max(0) as usize]) == "OpenDSMonitorCanvas" {
            PostMessageW(GetParent(canvas), WM_SIZE, 0, 0);
        }
    }
    unsafe fn fill_settings(&mut self) {
        SendMessageW(self.list, LB_RESETCONTENT, 0, 0);
        self.keys.clear();
        for (k, _) in self.cfg.fields() {
            let label = setting_label(&k);
            SendMessageW(self.list, LB_ADDSTRING, 0, wide(&label).as_ptr() as isize);
            self.keys.push(k);
        }
        SendMessageW(self.list, LB_SETCURSEL, 0, 0);
        self.select_setting();
    }
    unsafe fn selected(&self) -> Option<String> {
        let i = SendMessageW(self.list, LB_GETCURSEL, 0, 0);
        self.keys.get(i as usize).cloned()
    }
    unsafe fn select_setting(&mut self) {
        let Some(key) = self.selected() else {
            return;
        };
        let value = self
            .cfg
            .fields()
            .into_iter()
            .find(|(k, _)| *k == key)
            .unwrap()
            .1;
        let choices = options(&key);
        SetWindowTextW(
            GetDlgItem(self.settings, 204),
            wide(if value.starts_with('#') {
                crate::tr!(command_setting_color_value)
            } else {
                crate::tr!(command_setting_value)
            })
            .as_ptr(),
        );
        let swatch = GetDlgItem(self.settings, 210);
        ShowWindow(
            swatch,
            if value.starts_with('#') {
                SW_SHOW
            } else {
                SW_HIDE
            },
        );
        InvalidateRect(swatch, null(), 0);
        EnableWindow(
            GetDlgItem(self.settings, 206),
            value.starts_with('#') as i32,
        );
        ShowWindow(
            self.edit,
            if choices.is_empty() { SW_SHOW } else { SW_HIDE },
        );
        ShowWindow(
            self.choices,
            if choices.is_empty() { SW_HIDE } else { SW_SHOW },
        );
        SetWindowTextW(self.edit, wide(&value).as_ptr());
        SendMessageW(self.choices, CB_RESETCONTENT, 0, 0);
        if !choices.is_empty() {
            for label in &choices {
                SendMessageW(self.choices, CB_ADDSTRING, 0, wide(label).as_ptr() as isize);
            }
            let index = if value == "true" {
                1
            } else if value == "false" {
                0
            } else {
                value.parse::<usize>().unwrap_or(0)
            };
            SendMessageW(self.choices, CB_SETCURSEL, index, 0);
        }
        InvalidateRect(self.preview, null(), 0);
    }
    unsafe fn swatch_color(&self) -> Option<u32> {
        let mut buffer = [0u16; 128];
        let length = GetWindowTextW(self.edit, buffer.as_mut_ptr(), 128);
        let text = String::from_utf16_lossy(&buffer[..length.max(0) as usize]);
        let hex = text.trim().strip_prefix('#')?;
        if hex.len() != 6 {
            return None;
        }
        let rgb = u32::from_str_radix(hex, 16).ok()?;
        Some(((rgb & 255) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 255))
    }
    unsafe fn apply_setting(&mut self) {
        let Some(key) = self.selected() else {
            return;
        };
        let choices = options(&key);
        let value = if choices.is_empty() {
            let mut b = [0u16; 128];
            let n = GetWindowTextW(self.edit, b.as_mut_ptr(), 128);
            String::from_utf16_lossy(&b[..n.max(0) as usize])
        } else {
            let i = SendMessageW(self.choices, CB_GETCURSEL, 0, 0);
            if ["enabled", "right", "auto_colors", "topmost"].contains(&key.as_str()) {
                (i == 1).to_string()
            } else {
                i.to_string()
            }
        };
        let mut cfg = self.cfg.clone();
        if let Err(e) = cfg.set(&key, &value).and_then(|_| cfg.validate()) {
            self.message(&format!("{}: {e}", crate::tr!(command_invalid)));
            return;
        }
        self.cfg = cfg;
        self.persist();
        if key == "rows" {
            self.notify_layout();
        }
        if !self.overlay.is_null() {
            if ["x", "y", "width", "height"].contains(&key.as_str()) {
                self.restore_bounds();
            }
            self.styles();
        }
        self.select_setting();
        self.message(crate::tr!(command_saved));
    }
}

fn event_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Connected => crate::tr!(command_connected),
        Kind::Disconnected => crate::tr!(command_disconnected),
        Kind::Gap => crate::tr!(command_gap),
        Kind::Settings => crate::tr!(command_boundary),
        Kind::Clear => crate::tr!(command_clear),
        Kind::Input => "",
    }
}
fn ink(rgb: u32, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((rgb >> 16) & 255) as f32 / 255.,
        g: ((rgb >> 8) & 255) as f32 / 255.,
        b: (rgb & 255) as f32 / 255.,
        a: alpha,
    }
}
fn options(key: &str) -> Vec<String> {
    match key {
        "source" => vec![
            crate::tr!(command_source_text),
            crate::tr!(command_source_icon),
            crate::tr!(command_source_hidden),
        ],
        "theme" => vec![
            crate::tr!(command_follow),
            crate::tr!(command_light),
            crate::tr!(command_dark),
        ],
        "enabled" | "right" | "auto_colors" | "topmost" => {
            vec![crate::tr!(command_no), crate::tr!(command_yes)]
        }
        _ => vec![],
    }
    .into_iter()
    .map(str::to_string)
    .collect()
}
fn setting_label(k: &str) -> String {
    let (key, mode) = if let Some(k) = k.strip_prefix("dark_") {
        (k, crate::tr!(command_dark))
    } else {
        (k, crate::tr!(command_light))
    };
    if let Some(label) = match key {
        "neutral" => Some(crate::tr!(command_neutral)),
        "dpad" => Some(crate::tr!(command_dpad)),
        "button" => Some(crate::tr!(command_button)),
        "connection" => Some(crate::tr!(command_connection)),
        _ => None,
    } {
        return format!("{mode} · {label}");
    }

    let direct = match k {
        "enabled" => crate::tr!(command_enabled),
        "right" => crate::tr!(command_right),
        "source" => crate::tr!(command_source),
        "rows" => crate::tr!(command_rows),
        "overlay_rows" => crate::tr!(command_overlay_rows),
        "auto_colors" => crate::tr!(command_auto_colors),
        "theme" => crate::tr!(command_theme),
        "bg_alpha" => crate::tr!(command_bg_alpha),
        "fg_alpha" => crate::tr!(command_fg_alpha),
        "topmost" => crate::tr!(command_topmost),
        "x" => crate::tr!(command_x),
        "y" => crate::tr!(command_y),
        "width" => crate::tr!(command_width),
        "height" => crate::tr!(command_height),
        "neutral" => crate::tr!(command_neutral),
        "dpad" => crate::tr!(command_dpad),
        "button" => crate::tr!(command_button),
        "connection" => crate::tr!(command_connection),
        _ => "",
    };
    if !direct.is_empty() {
        return direct.into();
    }
    let a: Vec<_> = k.split('_').collect();
    let i = a.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    let j = a.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
    match a[0] {
        "stick" => format!(
            "{} · {} (%)",
            ["LS", "RS"][i],
            [
                crate::tr!(command_deadzone),
                crate::tr!(command_medium),
                crate::tr!(command_full)
            ][j]
        ),
        "trigger" => format!(
            "{} · {} (1–255)",
            ["L2", "R2"][i],
            [crate::tr!(command_medium), crate::tr!(command_full)][j]
        ),
        "color" | "darkcolor" => format!(
            "{} · {} · {}",
            if a[0] == "color" {
                crate::tr!(command_light)
            } else {
                crate::tr!(command_dark)
            },
            ["LS", "RS", "L2", "R2"][i],
            [
                crate::tr!(command_gentle),
                crate::tr!(command_medium),
                crate::tr!(command_full)
            ][j]
        ),
        "background" => format!(
            "{} · {}",
            [crate::tr!(command_light), crate::tr!(command_dark)][i],
            crate::tr!(command_background)
        ),
        "text" => format!(
            "{} · {}",
            [crate::tr!(command_light), crate::tr!(command_dark)][i],
            crate::tr!(command_text)
        ),
        _ => k.into(),
    }
}
struct Token {
    label: String,
    color: u32,
    dark_color: u32,
    source: Option<usize>,
    semantic: bool,
}
fn tokens(e: &Event, cfg: &Settings) -> Vec<Token> {
    if e.kind != Kind::Input {
        return vec![Token {
            label: event_label(e.kind).into(),
            color: cfg.connection,
            dark_color: cfg.dark_connection,
            source: None,
            semantic: false,
        }];
    }
    let mut out = vec![];
    let s = e.state;
    for i in 0..if cfg.right { 2 } else { 1 } {
        let a = s.sticks[i];
        let prefix = if cfg.source == 0
            || crate::appearance::palette().kind == crate::appearance::Kind::Contrast
        {
            if i == 0 {
                "LS "
            } else {
                "RS "
            }
        } else {
            ""
        };
        out.push(Token {
            label: format!(
                "{prefix}{}{}",
                commands::ARROWS[a.direction as usize],
                if crate::appearance::palette().kind == crate::appearance::Kind::Contrast {
                    ["", "¹", "²", "³"][a.strength as usize]
                } else {
                    ""
                }
            ),
            color: if a.strength == 0 {
                cfg.neutral
            } else {
                cfg.colors[i][a.strength as usize - 1]
            },
            dark_color: if a.strength == 0 {
                cfg.dark_neutral
            } else {
                cfg.dark_colors[i][a.strength as usize - 1]
            },
            source: if cfg.source == 1 { Some(i) } else { None },
            semantic: a.strength > 0,
        });
    }
    for (i, a) in ["↑", "→", "↓", "←"].iter().enumerate() {
        if s.dpad & (1 << i) != 0 {
            out.push(Token {
                label: format!(
                    "{}{a}",
                    if cfg.source == 0
                        || crate::appearance::palette().kind == crate::appearance::Kind::Contrast
                    {
                        crate::tr!(command_dpad_prefix)
                    } else {
                        ""
                    }
                ),
                color: cfg.dpad,
                dark_color: cfg.dark_dpad,
                source: if cfg.source == 1 { Some(2) } else { None },
                semantic: false,
            });
        }
    }
    let text = commands::combination(
        commands::State {
            sticks: [commands::Axis::default(); 2],
            triggers: [0; 2],
            dpad: 0,
            buttons: s.buttons,
        },
        false,
    );
    for b in text.split(" + ").skip(1) {
        out.push(Token {
            label: b.into(),
            color: cfg.button,
            dark_color: cfg.dark_button,
            source: None,
            semantic: false,
        });
    }
    for (i, label) in ["L2", "R2"].iter().enumerate() {
        if s.triggers[i] > 0 {
            out.push(Token {
                label: format!(
                    "{label}{}",
                    if crate::appearance::palette().kind == crate::appearance::Kind::Contrast {
                        ["", "¹", "²", "³"][s.triggers[i] as usize]
                    } else {
                        ""
                    }
                ),
                color: cfg.colors[i + 2][s.triggers[i] as usize - 1],
                dark_color: cfg.dark_colors[i + 2][s.triggers[i] as usize - 1],
                source: None,
                semantic: true,
            });
        }
    }
    out
}
fn token_width(t: &Token) -> f32 {
    (t.label.chars().count() as f32 * 10. + if t.source.is_some() { 20. } else { 0. } + 16.)
        .max(38.)
}
fn row_height(_w: f32) -> f32 {
    30.
}
fn event_height(e: &Event, cfg: &Settings, w: f32) -> f32 {
    let available = (w - 180.).max(50.);
    let mut x = if matches!(e.kind, Kind::Connected | Kind::Disconnected) {
        38.
    } else {
        0.
    };
    let mut lines = 1;
    for t in tokens(e, cfg) {
        let tw = token_width(&t);
        if x > 0. && x + tw > available {
            lines += 1;
            x = 0.;
        }
        x += tw;
    }
    (lines as f32 * 24. + 6.).max(row_height(w))
}
unsafe fn source_icon(r: &Renderer, i: usize, x: f32, y: f32, c: D2D1_COLOR_F) {
    if i == 2 {
        r.line((x + 8., y + 1.), (x + 8., y + 15.), 3., c);
        r.line((x + 1., y + 8.), (x + 15., y + 8.), 3., c);
    } else {
        r.ellipse(x + 8., y + 8., 6., c);
        r.line(
            (x + 8., y + 8.),
            (x + if i == 0 { 3. } else { 13. }, y + 2.),
            2.,
            c,
        );
    }
}
unsafe fn plug(r: &Renderer, connected: bool, x: f32, y: f32, c: D2D1_COLOR_F) {
    let gap = if connected { 0. } else { 5. };
    r.rect(x, y + 5., 9., 10., 1., c);
    r.line((x + 9., y + 8.), (x + 14., y + 8.), 2., c);
    r.line((x + 9., y + 12.), (x + 14., y + 12.), 2., c);
    r.rect(x + 14. + gap, y + 4., 8., 12., 1., c);
    r.line((x - 4., y + 10.), (x, y + 10.), 2., c);
    r.line((x + 22. + gap, y + 10.), (x + 27. + gap, y + 10.), 2., c);
}
unsafe fn draw(
    r: &Renderer,
    cfg: &Settings,
    events: &[Event],
    _latest: Option<&Event>,
    now: u64,
    w: f32,
    h: f32,
    start: f32,
    dark: bool,
    alpha: f32,
    rows: usize,
) {
    let (_, text) = cfg.palette(dark);
    let contrast = crate::appearance::palette().kind == crate::appearance::Kind::Contrast;
    let color_for = |c: u32, semantic: bool| {
        if contrast {
            {
                let mut c = palette_color(crate::appearance::palette().text);
                c.a = alpha;
                c
            }
        } else {
            ink(cfg.color(c, dark, semantic), alpha)
        }
    };
    let mut y = start;
    if events.is_empty() {
        r.text(
            10.,
            y,
            w - 20.,
            40.,
            crate::tr!(command_empty),
            1,
            color_for(text, true),
        );
        return;
    }
    for (index, e) in events.iter().take(rows).enumerate() {
        let height = event_height(e, cfg, w);
        if y + height > h {
            break;
        }
        let mut x = 10.;
        let mut yy = y;
        let available = (w - 170.).max(60.);
        if matches!(e.kind, Kind::Connected | Kind::Disconnected) {
            plug(
                r,
                e.kind == Kind::Connected,
                x,
                yy + 1.,
                color_for(
                    if dark && !cfg.auto_colors {
                        cfg.dark_connection
                    } else {
                        cfg.connection
                    },
                    false,
                ),
            );
            x += 38.;
        }
        for t in tokens(e, cfg) {
            let tw = token_width(&t);
            if x > 10. && x + tw > available {
                x = 10.;
                yy += 24.;
            }
            let mut tx = x;
            let semantic = t.semantic;
            let token_color = if dark && !cfg.auto_colors {
                t.dark_color
            } else {
                t.color
            };
            if let Some(i) = t.source {
                source_icon(r, i, tx, yy + 4., color_for(token_color, semantic));
                tx += 20.;
            }
            r.text(
                tx,
                yy,
                (tw - (tx - x)).min((available - tx).max(1.)),
                24.,
                &t.label,
                1,
                color_for(token_color, semantic),
            );
            x += tw;
        }
        let duration = commands::duration(commands::state_duration(events, index, now));
        r.text(w - 156., y, 146., 22., &duration, 0, color_for(text, true));
        y += height;
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<WindowState>;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if !matches!(
        msg,
        WM_TIMER
            | WM_PAINT
            | WM_ERASEBKGND
            | WM_COMMAND
            | WM_CLOSE
            | WM_SIZE
            | WM_EXITSIZEMOVE
            | WM_DPICHANGED
            | WM_GETMINMAXINFO
            | WM_NCHITTEST
            | crate::appearance::APPLY
    ) {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if msg == WM_NCHITTEST {
        let Ok(s) = (*ptr).try_borrow() else {
            return DefWindowProcW(hwnd, msg, w, l);
        };
        if hwnd == s.overlay {
            if s.locked {
                return HTTRANSPARENT as isize;
            }
            let mut r: RECT = std::mem::zeroed();
            GetWindowRect(hwnd, &mut r);
            let x = (l as i16) as i32 - r.left;
            let y = ((l >> 16) as i16) as i32 - r.top;
            let right = x >= r.right - r.left - 7;
            let bottom = y >= r.bottom - r.top - 7;
            let hit = if y < 7 {
                if x < 7 {
                    HTTOPLEFT
                } else if right {
                    HTTOPRIGHT
                } else {
                    HTTOP
                }
            } else if bottom {
                if x < 7 {
                    HTBOTTOMLEFT
                } else if right {
                    HTBOTTOMRIGHT
                } else {
                    HTBOTTOM
                }
            } else if x < 7 {
                HTLEFT
            } else if right {
                HTRIGHT
            } else if y < 26 {
                HTCAPTION
            } else {
                HTCLIENT
            };
            return hit as isize;
        }
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let Ok(mut s) = (*ptr).try_borrow_mut() else {
        return DefWindowProcW(hwnd, msg, w, l);
    };
    match msg {
        WM_GETMINMAXINFO => {
            if hwnd == s.overlay {
                let mm = &mut *(l as *mut MINMAXINFO);
                let dpi = crate::gui_theme::window_dpi(hwnd).max(96) as i32;
                mm.ptMinTrackSize = POINT {
                    x: 320 * dpi / 96,
                    y: 100 * dpi / 96,
                };
                mm.ptMaxTrackSize = POINT {
                    x: 2048 * dpi / 96,
                    y: 2048 * dpi / 96,
                };
            }
            0
        }
        WM_TIMER => {
            s.pulse();
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            if hwnd == GetDlgItem(s.settings, 210) {
                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);
                FillRect(dc, &rect, crate::appearance::surface_brush());
                if let Some(rgb) = s.swatch_color() {
                    let brush = CreateSolidBrush(rgb);
                    FillRect(dc, &rect, brush);
                    DeleteObject(brush as _);
                }
                let border = CreateSolidBrush(crate::appearance::palette().line);
                FrameRect(dc, &rect, border);
                DeleteObject(border as _);
            } else {
                if hwnd == s.settings {
                    FillRect(dc, &ps.rcPaint, crate::appearance::surface_brush());
                }
                s.paint_table(hwnd, dc);
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_COMMAND => {
            let id = (w & 65535) as u32;
            match id {
                101..=105 => s.action(id, ptr),
                201 => s.select_setting(),
                202 if (w >> 16) as u16 == EN_CHANGE => {
                    InvalidateRect(GetDlgItem(s.settings, 210), null(), 0);
                }
                205 => s.apply_setting(),
                206 => {
                    let Some(key) = s.selected() else { return 0 };
                    let value = s
                        .cfg
                        .fields()
                        .into_iter()
                        .find(|(k, _)| *k == key)
                        .map(|(_, v)| v)
                        .unwrap_or_default();
                    if !value.starts_with('#') {
                        return 0;
                    }
                    let rgb = u32::from_str_radix(&value[1..], 16).unwrap_or(0);
                    let mut custom = [0u32; 16];
                    let mut cc: winapi::um::commdlg::CHOOSECOLORW = std::mem::zeroed();
                    cc.lStructSize = std::mem::size_of_val(&cc) as u32;
                    cc.hwndOwner = hwnd;
                    cc.rgbResult = ((rgb & 255) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 255);
                    cc.lpCustColors = custom.as_mut_ptr();
                    cc.Flags = winapi::um::commdlg::CC_FULLOPEN | winapi::um::commdlg::CC_RGBINIT;
                    // The native dialog pumps messages: release the state borrow.
                    drop(s);
                    if winapi::um::commdlg::ChooseColorW(&mut cc) != 0 {
                        if let Ok(s) = (*ptr).try_borrow() {
                            let rgb = ((cc.rgbResult & 255) << 16)
                                | (cc.rgbResult & 0xff00)
                                | ((cc.rgbResult >> 16) & 255);
                            SetWindowTextW(s.edit, wide(&format!("#{rgb:06X}")).as_ptr());
                            InvalidateRect(GetDlgItem(s.settings, 210), null(), 0);
                        }
                    }
                    return 0;
                }
                207 => {
                    s.cfg = Settings::default();
                    s.locked = false;
                    s.persist();
                    if !s.overlay.is_null() {
                        s.restore_bounds();
                        s.styles();
                    }
                    s.fill_settings();
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            if hwnd == s.overlay {
                s.hub.commands.visible_capacity(true, 0);
            }
            0
        }
        WM_SIZE => {
            if hwnd == s.main {
                s.main_surface = None;
                s.last_main.clear();
            } else if hwnd == s.overlay {
                s.surface = None;
                s.last_overlay.clear();
            }
            0
        }
        WM_EXITSIZEMOVE => {
            if hwnd == s.overlay && !s.locked {
                let mut r: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut r);
                let dpi = crate::gui_theme::window_dpi(hwnd).max(96) as i32;
                s.cfg.x = r.left;
                s.cfg.y = r.top;
                s.cfg.width = ((r.right - r.left) * 96 / dpi).clamp(320, 2048);
                s.cfg.height = ((r.bottom - r.top) * 96 / dpi).clamp(100, 2048);
                s.persist();
            }
            0
        }
        WM_DPICHANGED => {
            let r = &*(l as *const RECT);
            SetWindowPos(
                hwnd,
                null_mut(),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
            if hwnd == s.settings {
                s.settings_layout();
            }
            s.surface = None;
            s.preview_surface = None;
            s.main_surface = None;
            s.last_main.clear();
            s.last_overlay.clear();
            0
        }
        crate::appearance::APPLY => {
            s.surface = None;
            s.preview_surface = None;
            s.main_surface = None;
            s.last_main.clear();
            s.last_overlay.clear();
            InvalidateRect(s.main, null(), 0);
            if !s.preview.is_null() {
                InvalidateRect(s.preview, null(), 0);
            }
            if !s.settings.is_null() {
                InvalidateRect(GetDlgItem(s.settings, 210), null(), 0);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
