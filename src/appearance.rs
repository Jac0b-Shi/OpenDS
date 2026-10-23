//! GUI-thread theme state. No timers, workers, or realtime-path dependencies.
use crate::config::ThemeMode;
use std::{
    cell::RefCell,
    ptr::{null, null_mut},
};
use winapi::{
    shared::{minwindef::*, windef::*},
    um::{commctrl::*, wingdi::*, winuser::*},
};

pub const APPLY: u32 = WM_APP + 0x61;
const REFRESH: u32 = WM_APP + 0x62;
const SUBCLASS: usize = 0x4f445448;
pub const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Light,
    Dark,
    Contrast,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub kind: Kind,
    pub bg: u32,
    pub surface: u32,
    pub text: u32,
    pub muted: u32,
    pub line: u32,
    pub accent: u32,
    pub primary: u32,
    pub primary_text: u32,
    pub hover: u32,
    pub selected: u32,
    pub disabled: u32,
    pub disabled_text: u32,
    pub source: u32,
    pub forwarded: u32,
    pub grid: u32,
}
impl Palette {
    pub const fn light() -> Self {
        Self {
            kind: Kind::Light,
            bg: rgb(247, 248, 250),
            surface: rgb(255, 255, 255),
            text: rgb(23, 32, 49),
            muted: rgb(103, 114, 133),
            line: rgb(226, 231, 239),
            accent: rgb(42, 95, 225),
            primary: rgb(42, 95, 225),
            primary_text: rgb(255, 255, 255),
            hover: rgb(240, 244, 251),
            selected: rgb(232, 238, 255),
            disabled: rgb(241, 243, 247),
            disabled_text: rgb(157, 165, 178),
            source: rgb(42, 95, 225),
            forwarded: rgb(209, 105, 20),
            grid: rgb(226, 231, 239),
        }
    }
    pub const fn dark() -> Self {
        Self {
            kind: Kind::Dark,
            bg: rgb(21, 24, 31),
            surface: rgb(30, 36, 48),
            text: rgb(230, 234, 242),
            muted: rgb(170, 180, 196),
            line: rgb(58, 68, 84),
            accent: rgb(134, 172, 255),
            primary: rgb(52, 95, 202),
            primary_text: rgb(255, 255, 255),
            hover: rgb(42, 51, 67),
            selected: rgb(43, 60, 94),
            disabled: rgb(35, 41, 52),
            disabled_text: rgb(116, 128, 146),
            source: rgb(111, 157, 255),
            forwarded: rgb(255, 181, 102),
            grid: rgb(58, 68, 84),
        }
    }
    unsafe fn contrast() -> Self {
        let bg = GetSysColor(COLOR_WINDOW);
        let text = GetSysColor(COLOR_WINDOWTEXT);
        Self {
            kind: Kind::Contrast,
            bg,
            surface: bg,
            text,
            muted: text,
            line: text,
            accent: GetSysColor(COLOR_HIGHLIGHT),
            primary: GetSysColor(COLOR_HIGHLIGHT),
            primary_text: GetSysColor(COLOR_HIGHLIGHTTEXT),
            hover: GetSysColor(COLOR_BTNFACE),
            selected: GetSysColor(COLOR_HIGHLIGHT),
            disabled: bg,
            disabled_text: GetSysColor(COLOR_GRAYTEXT),
            source: GetSysColor(COLOR_HOTLIGHT),
            forwarded: text,
            grid: GetSysColor(COLOR_GRAYTEXT),
        }
    }
}
pub fn resolve(mode: ThemeMode, system_light: Option<bool>, contrast: bool) -> Kind {
    if contrast {
        Kind::Contrast
    } else {
        match mode {
            ThemeMode::Dark => Kind::Dark,
            ThemeMode::Light => Kind::Light,
            ThemeMode::System if system_light == Some(false) => Kind::Dark,
            _ => Kind::Light,
        }
    }
}
struct Brushes {
    light: HBRUSH,
    dark: HBRUSH,
}
impl Brushes {
    fn new() -> Self {
        unsafe {
            Self {
                light: CreateSolidBrush(Palette::light().surface),
                dark: CreateSolidBrush(Palette::dark().surface),
            }
        }
    }
}
impl Drop for Brushes {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.light as _);
            DeleteObject(self.dark as _);
        }
    }
}
struct State {
    mode: ThemeMode,
    palette: Palette,
    windows: Vec<HWND>,
    pending: bool,
    brushes: Brushes,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State {
    mode: ThemeMode::System, palette: Palette::light(), windows: vec![], pending: false, brushes: Brushes::new(),
}); }
pub fn palette() -> Palette {
    STATE.with(|s| s.borrow().palette)
}
pub fn mode() -> ThemeMode {
    STATE.with(|s| s.borrow().mode)
}
pub fn surface_brush() -> HBRUSH {
    STATE.with(|s| {
        let s = s.borrow();
        unsafe {
            match s.palette.kind {
                Kind::Light => s.brushes.light,
                Kind::Dark => s.brushes.dark,
                Kind::Contrast => GetSysColorBrush(COLOR_WINDOW),
            }
        }
    })
}
fn detect(mode: ThemeMode) -> Palette {
    let (light, contrast) = system_settings();
    match resolve(mode, light, contrast) {
        Kind::Light => Palette::light(),
        Kind::Dark => Palette::dark(),
        Kind::Contrast => unsafe { Palette::contrast() },
    }
}
fn system_settings() -> (Option<bool>, bool) {
    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            key: *mut std::ffi::c_void,
            sub: *const u16,
            value: *const u16,
            flags: u32,
            ty: *mut u32,
            data: *mut std::ffi::c_void,
            size: *mut u32,
        ) -> i32;
    }
    unsafe {
        let mut value = 1u32;
        let mut len = 4;
        let ok = RegGetValueW(
            0x80000001u32 as i32 as isize as _,
            wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize").as_ptr(),
            wide("AppsUseLightTheme").as_ptr(),
            0x10,
            null_mut(),
            &mut value as *mut _ as _,
            &mut len,
        ) == 0;
        let mut hc: HIGHCONTRASTW = std::mem::zeroed();
        hc.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
        let high = SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, &mut hc as *mut _ as _, 0)
            != 0
            && hc.dwFlags & HCF_HIGHCONTRASTON != 0;
        (ok.then_some(value != 0), high)
    }
}
pub fn initialize(mode: ThemeMode) {
    let p = detect(mode);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.mode = mode;
        s.palette = p;
    });
}
pub fn set_mode(mode: ThemeMode) {
    let p = detect(mode);
    update(mode, p);
}
fn update(mode: ThemeMode, palette: Palette) {
    let windows = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.mode = mode;
        s.pending = false;
        if s.palette == palette {
            return vec![];
        }
        s.palette = palette;
        s.windows.clone()
    });
    for h in windows {
        unsafe {
            PostMessageW(h, APPLY, 0, 0);
        }
    }
}
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub unsafe fn install(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }
    let first = STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.windows.contains(&hwnd) {
            false
        } else {
            s.windows.push(hwnd);
            true
        }
    });
    if first {
        SetWindowSubclass(hwnd, Some(window_proc), SUBCLASS, 0);
    }
    apply(hwnd);
}
unsafe fn apply(hwnd: HWND) {
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: HWND,
            attribute: u32,
            value: *const std::ffi::c_void,
            size: u32,
        ) -> i32;
    }
    let p = palette();
    let dark = (p.kind == Kind::Dark) as i32;
    DwmSetWindowAttribute(hwnd, 20, &dark as *const _ as _, 4);
    // Explicit colors also let a manual choice differ from the Windows preference.
    for (attribute, value) in [(35, p.surface), (36, p.surface), (37, p.text)] {
        let value = if p.kind == Kind::Contrast {
            0xffffffff
        } else {
            value
        };
        DwmSetWindowAttribute(hwnd, attribute, &value as *const _ as _, 4);
    }
    crate::theme_controls::install_children(hwnd);
    RedrawWindow(
        hwnd,
        null(),
        null_mut(),
        RDW_INVALIDATE | RDW_FRAME | RDW_ALLCHILDREN | RDW_ERASE,
    );
}
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: UINT,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    _: usize,
) -> LRESULT {
    if msg == WM_NCDESTROY {
        STATE.with(|s| s.borrow_mut().windows.retain(|h| *h != hwnd));
        RemoveWindowSubclass(hwnd, Some(window_proc), SUBCLASS);
        return DefSubclassProc(hwnd, msg, w, l);
    }
    if msg == WM_SETTINGCHANGE || msg == WM_THEMECHANGED || msg == WM_SYSCOLORCHANGE {
        let queue = STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.pending {
                false
            } else {
                s.pending = true;
                true
            }
        });
        if queue {
            PostMessageW(hwnd, REFRESH, 0, 0);
        }
    }
    if msg == REFRESH {
        let mode = mode();
        update(mode, detect(mode));
        return 0;
    }
    if msg == APPLY {
        apply(hwnd);
        return 0;
    }
    if let Some(result) = crate::theme_controls::parent_message(hwnd, msg, w, l) {
        return result;
    }
    DefSubclassProc(hwnd, msg, w, l)
}
/// Injects only into explicitly requested UI smoke runs, never changes Windows settings.
pub fn smoke_palette(kind: Kind) {
    let p = match kind {
        Kind::Light => Palette::light(),
        Kind::Dark => Palette::dark(),
        Kind::Contrast => unsafe { Palette::contrast() },
    };
    update(mode(), p);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preference_system_and_high_contrast_precedence() {
        for mode in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
            for light in [None, Some(true), Some(false)] {
                assert_eq!(resolve(mode, light, true), Kind::Contrast);
                assert_eq!(
                    resolve(mode, light, false),
                    match mode {
                        ThemeMode::Dark => Kind::Dark,
                        ThemeMode::Light => Kind::Light,
                        ThemeMode::System if light == Some(false) => Kind::Dark,
                        _ => Kind::Light,
                    }
                );
            }
        }
    }
}
