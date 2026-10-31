//! NWG 的轻量 GDI 外观层：原生控件仍负责键盘、焦点和可访问性。
pub use crate::appearance::palette;
use native_windows_gui as nwg;
use std::collections::HashMap;
use winapi::{
    shared::windef::*,
    um::{wingdi::*, winuser::*},
};
pub const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
pub fn px(v: i32, dpi: u32) -> i32 {
    ((v as i64 * dpi as i64 + 48) / 96) as i32
}
/// Decimal SI units, integer rounding; keep the actual counter as u64.
pub fn compact_count(value: u64) -> String {
    if value < 1000 {
        return value.to_string();
    }
    let units = ["", "K", "M", "G", "T", "P", "E"];
    let mut unit = 1;
    let mut scale = 1000u128;
    loop {
        let tenths = (value as u128 * 10 + scale / 2) / scale;
        if tenths >= 10_000 && unit + 1 < units.len() {
            unit += 1;
            scale *= 1000;
            continue;
        }
        return if tenths % 10 == 0 {
            format!("{}{}", tenths / 10, units[unit])
        } else {
            format!("{}.{:01}{}", tenths / 10, tenths % 10, units[unit])
        };
    }
}
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn hwnd(h: &nwg::ControlHandle) -> HWND {
    h.hwnd().unwrap_or(std::ptr::null_mut())
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Placement {
    rect: Option<(i32, i32, i32, i32)>,
    visible: bool,
}
/// Apply only changed child geometry/visibility, in one native layout batch.
#[derive(Default)]
pub struct ControlLayout {
    previous: HashMap<HWND, Placement>,
}
impl ControlLayout {
    pub fn apply(&mut self, all: &[HWND], wanted: &[(HWND, i32, i32, i32, i32)], dpi: u32) {
        unsafe {
            let mut changes = Vec::new();
            for &handle in all {
                let old = *self.previous.entry(handle).or_insert_with(|| Placement {
                    rect: None,
                    visible: GetWindowLongPtrW(handle, GWL_STYLE) & WS_VISIBLE as isize != 0,
                });
                let new = wanted.iter().find(|p| p.0 == handle).map_or(
                    Placement {
                        visible: false,
                        ..old
                    },
                    |&(_, x, y, w, h)| Placement {
                        rect: Some((px(x, dpi), px(y, dpi), px(w.max(1), dpi), px(h.max(1), dpi))),
                        visible: true,
                    },
                );
                if new != old {
                    let flags = SWP_NOZORDER
                        | SWP_NOACTIVATE
                        | SWP_NOREDRAW
                        | SWP_NOCOPYBITS
                        | if old.rect == new.rect {
                            SWP_NOMOVE | SWP_NOSIZE
                        } else {
                            0
                        }
                        | if old.visible != new.visible {
                            if new.visible {
                                SWP_SHOWWINDOW
                            } else {
                                SWP_HIDEWINDOW
                            }
                        } else {
                            0
                        };
                    changes.push((handle, new, flags));
                }
            }
            if changes.is_empty() {
                return;
            }
            let mut batch = BeginDeferWindowPos(changes.len() as i32);
            for &(handle, new, flags) in &changes {
                if batch.is_null() {
                    break;
                }
                let (x, y, w, h) = new.rect.unwrap_or_default();
                batch = DeferWindowPos(batch, handle, std::ptr::null_mut(), x, y, w, h, flags);
            }
            let applied = !batch.is_null() && EndDeferWindowPos(batch) != 0;
            for (handle, new, flags) in changes {
                let (x, y, w, h) = new.rect.unwrap_or_default();
                if applied || SetWindowPos(handle, std::ptr::null_mut(), x, y, w, h, flags) != 0 {
                    self.previous.insert(handle, new);
                }
            }
        }
    }
}
pub fn set_font(h: &nwg::ControlHandle, f: &nwg::Font) {
    unsafe {
        SendMessageW(hwnd(h), WM_SETFONT, f.handle as usize, 1);
    }
}
pub fn owner_button(h: &nwg::ControlHandle) {
    unsafe {
        let h = hwnd(h);
        let s = GetWindowLongPtrW(h, GWL_STYLE);
        SetWindowLongPtrW(h, GWL_STYLE, (s & !0xF) | BS_OWNERDRAW as isize);
    }
}
pub struct Fonts {
    pub body: nwg::Font,
    pub small: nwg::Font,
    pub title: nwg::Font,
    pub value: nwg::Font,
    pub strong: nwg::Font,
}
pub fn font_family() -> &'static str {
    static FAMILY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    FAMILY.get_or_init(|| unsafe {
        let mut metrics: NONCLIENTMETRICSW = std::mem::zeroed();
        metrics.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        if SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            &mut metrics as *mut _ as *mut _,
            0,
        ) != 0
        {
            let end = metrics
                .lfMessageFont
                .lfFaceName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(32);
            String::from_utf16_lossy(&metrics.lfMessageFont.lfFaceName[..end])
        } else {
            "Segoe UI".into()
        }
    })
}
fn smoke_dpi() -> Option<u32> {
    static DPI: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    *DPI.get_or_init(|| {
        if std::env::args().nth(1).as_deref() != Some("ui-smoke-test") {
            return None;
        }
        std::env::var("OPENDS_UI_SMOKE_DPI")
            .ok()?
            .parse::<u32>()
            .ok()
            .filter(|d| [96, 144, 192].contains(d))
    })
}
pub fn window_dpi(hwnd: HWND) -> u32 {
    smoke_dpi()
        .unwrap_or_else(|| unsafe {
            let dpi = GetDpiForWindow(hwnd);
            if dpi == 0 {
                GetDpiForSystem()
            } else {
                dpi
            }
        })
        .max(96)
}
pub fn system_dpi() -> u32 {
    smoke_dpi()
        .unwrap_or_else(|| unsafe { GetDpiForSystem() })
        .max(96)
}
impl Fonts {
    pub fn new(dpi: u32) -> Result<Self, String> {
        fn font(size: i32, weight: u32, dpi: u32) -> Result<nwg::Font, String> {
            let mut f = Default::default();
            nwg::Font::builder()
                .family(font_family())
                .size_absolute(px(size, dpi) as u32)
                .weight(weight)
                .build(&mut f)
                .map_err(|e| e.to_string())?;
            Ok(f)
        }
        Ok(Self {
            body: font(14, 400, dpi)?,
            small: font(12, 400, dpi)?,
            title: font(26, 600, dpi)?,
            value: font(30, 600, dpi)?,
            strong: font(16, 600, dpi)?,
        })
    }
}
pub unsafe fn fill(dc: HDC, r: &RECT, color: u32) {
    let b = CreateSolidBrush(color);
    FillRect(dc, r, b);
    DeleteObject(b as _);
}
pub unsafe fn round(dc: HDC, r: &RECT, radius: i32, bg: u32, border: u32) {
    let brush = CreateSolidBrush(bg);
    let pen = CreatePen(PS_SOLID as i32, 1, border);
    let ob = SelectObject(dc, brush as _);
    let op = SelectObject(dc, pen as _);
    RoundRect(dc, r.left, r.top, r.right, r.bottom, radius, radius);
    SelectObject(dc, ob);
    SelectObject(dc, op);
    DeleteObject(brush as _);
    DeleteObject(pen as _);
}
pub unsafe fn text(dc: HDC, r: &RECT, s: &str, font: &nwg::Font, color: u32, flags: u32) {
    let old = SelectObject(dc, font.handle as _);
    SetBkMode(dc, TRANSPARENT as i32);
    SetTextColor(dc, color);
    let mut rect = *r;
    let s = wide(s);
    let mut measured = rect;
    DrawTextW(
        dc,
        s.as_ptr(),
        -1,
        &mut measured,
        DT_CALCRECT | DT_SINGLELINE,
    );
    let mut metrics: TEXTMETRICW = std::mem::zeroed();
    GetTextMetricsW(dc, &mut metrics);
    let mut actual_flags = flags;
    if measured.right - measured.left > rect.right - rect.left
        && rect.bottom - rect.top >= metrics.tmHeight * 2
    {
        actual_flags = (flags & !(DT_SINGLELINE | DT_END_ELLIPSIS | DT_VCENTER)) | DT_WORDBREAK;
        let mut wrapped = rect;
        DrawTextW(dc, s.as_ptr(), -1, &mut wrapped, actual_flags | DT_CALCRECT);
        if flags & DT_VCENTER != 0 {
            rect.top += ((rect.bottom - rect.top - (wrapped.bottom - wrapped.top)) / 2).max(0);
        }
    }
    DrawTextW(dc, s.as_ptr(), -1, &mut rect, actual_flags);
    SelectObject(dc, old);
}
pub unsafe fn draw_button(
    item: &DRAWITEMSTRUCT,
    fonts: &Fonts,
    primary: bool,
    nav: bool,
    selected: bool,
    dpi: u32,
) {
    let cached = crate::gdi_buffer::PaintBuffer::acquire(
        item.hwndItem,
        0,
        item.hDC,
        item.rcItem.right - item.rcItem.left,
        item.rcItem.bottom - item.rcItem.top,
    );
    if let Some(lease) = cached.as_ref() {
        let buffer = lease.dc();
        SetWindowOrgEx(
            buffer,
            item.rcItem.left,
            item.rcItem.top,
            std::ptr::null_mut(),
        );
        let mut target = *item;
        target.hDC = buffer;
        draw_button_contents(&target, fonts, primary, nav, selected, dpi);
        BitBlt(
            item.hDC,
            item.rcItem.left,
            item.rcItem.top,
            item.rcItem.right - item.rcItem.left,
            item.rcItem.bottom - item.rcItem.top,
            buffer,
            item.rcItem.left,
            item.rcItem.top,
            SRCCOPY,
        );
    } else {
        draw_button_contents(item, fonts, primary, nav, selected, dpi);
    }
}
unsafe fn draw_button_contents(
    item: &DRAWITEMSTRUCT,
    fonts: &Fonts,
    primary: bool,
    nav: bool,
    selected: bool,
    dpi: u32,
) {
    let disabled = item.itemState & ODS_DISABLED != 0;
    let pressed = item.itemState & ODS_SELECTED != 0;
    let mut cursor: POINT = std::mem::zeroed();
    GetCursorPos(&mut cursor);
    ScreenToClient(item.hwndItem, &mut cursor);
    let hot = item.itemState & ODS_HOTLIGHT != 0 || PtInRect(&item.rcItem, cursor) != 0;
    let (bg, fg, border) = if disabled {
        (palette().disabled, palette().disabled_text, palette().line)
    } else if primary {
        (
            crate::appearance::button_color(palette().primary, pressed, hot),
            palette().primary_text,
            palette().primary,
        )
    } else if nav {
        if selected {
            (
                palette().selected,
                if palette().kind == crate::appearance::Kind::Contrast {
                    palette().primary_text
                } else {
                    palette().accent
                },
                palette().selected,
            )
        } else {
            (
                if hot { palette().bg } else { palette().surface },
                palette().text,
                if hot { palette().bg } else { palette().surface },
            )
        }
    } else {
        (
            if pressed {
                palette().selected
            } else if hot {
                palette().hover
            } else {
                palette().surface
            },
            palette().text,
            palette().line,
        )
    };
    fill(
        item.hDC,
        &item.rcItem,
        if nav { palette().surface } else { palette().bg },
    );
    round(
        item.hDC,
        &item.rcItem,
        px(10, dpi),
        bg,
        if item.itemState & ODS_FOCUS != 0 {
            palette().accent
        } else {
            border
        },
    );
    let mut buffer = [0u16; 256];
    let n = GetWindowTextW(item.hwndItem, buffer.as_mut_ptr(), buffer.len() as i32);
    let s = String::from_utf16_lossy(&buffer[..n.max(0) as usize]);
    let mut rect = item.rcItem;
    if nav {
        rect.left += px(18, dpi);
    }
    text(
        item.hDC,
        &rect,
        &s,
        &fonts.body,
        fg,
        DT_VCENTER | DT_SINGLELINE | if nav { DT_LEFT } else { DT_CENTER },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child_layout_preserves_visibility_dpi_and_skips_redundant_resize() {
        use winapi::{shared::minwindef::*, um::commctrl::*};
        unsafe extern "system" fn count_position(
            hwnd: HWND,
            msg: UINT,
            w: WPARAM,
            l: LPARAM,
            _: usize,
            count: usize,
        ) -> LRESULT {
            if msg == WM_WINDOWPOSCHANGED {
                *(count as *mut u32) += 1;
            }
            DefSubclassProc(hwnd, msg, w, l)
        }
        unsafe {
            let parent = CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("").as_ptr(),
                WS_POPUP,
                0,
                0,
                600,
                400,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert!(!parent.is_null());
            let child = CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("").as_ptr(),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                1,
                1,
                parent,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert!(!child.is_null());
            let mut positions = 0u32;
            SetWindowSubclass(
                child,
                Some(count_position),
                1,
                &mut positions as *mut _ as usize,
            );
            let mut layout = ControlLayout::default();
            let desired = [(child, 10, 20, 100, 30)];
            layout.apply(&[child], &desired, 96);
            let count = positions;
            layout.apply(&[child], &desired, 96);
            assert_eq!(
                positions, count,
                "An unchanged layout sent native positioning messages"
            );
            layout.apply(&[child], &desired, 192);
            let mut rect: RECT = std::mem::zeroed();
            GetWindowRect(child, &mut rect);
            MapWindowPoints(
                std::ptr::null_mut(),
                parent,
                &mut rect as *mut _ as *mut POINT,
                2,
            );
            assert_eq!(
                (
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top
                ),
                (20, 40, 200, 60)
            );
            layout.apply(&[child], &[], 192);
            assert_eq!(GetWindowLongPtrW(child, GWL_STYLE) & WS_VISIBLE as isize, 0);
            layout.apply(&[child], &desired, 192);
            assert_ne!(GetWindowLongPtrW(child, GWL_STYLE) & WS_VISIBLE as isize, 0);
            assert_eq!(IsWindowVisible(parent), 0);
            RemoveWindowSubclass(child, Some(count_position), 1);
            DestroyWindow(parent);
        }
    }
    #[test]
    fn count_units_round_at_boundaries_and_cover_u64() {
        for (value, expected) in [
            (0, "0"),
            (999, "999"),
            (1000, "1K"),
            (1250, "1.3K"),
            (999949, "999.9K"),
            (999950, "1M"),
            (1_234_567, "1.2M"),
            (1_000_000_000, "1G"),
            (u64::MAX, "18.4E"),
        ] {
            assert_eq!(compact_count(value), expected);
        }
    }
    #[test]
    fn layout_scales_exactly_at_common_display_factors() {
        assert_eq!(px(32, 96), 32);
        assert_eq!(px(32, 144), 48);
        assert_eq!(px(32, 192), 64);
        assert_eq!(px(32, 240), 80);
    }
}
