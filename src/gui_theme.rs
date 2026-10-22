//! NWG 的轻量 GDI 外观层：原生控件仍负责键盘、焦点和可访问性。
use native_windows_gui as nwg;
use winapi::{
    shared::windef::*,
    um::{wingdi::*, winuser::*},
};
pub const BG: u32 = rgb(247, 248, 250);
pub const WHITE: u32 = rgb(255, 255, 255);
pub const INK: u32 = rgb(23, 32, 49);
pub const MUTED: u32 = rgb(103, 114, 133);
pub const BLUE: u32 = rgb(42, 95, 225);
pub const LINE: u32 = rgb(226, 231, 239);
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
pub fn place(h: &nwg::ControlHandle, x: i32, y: i32, w: i32, height: i32, dpi: u32, visible: bool) {
    unsafe {
        let h = hwnd(h);
        if h.is_null() {
            return;
        }
        SetWindowPos(
            h,
            std::ptr::null_mut(),
            px(x, dpi),
            px(y, dpi),
            px(w.max(1), dpi),
            px(height.max(1), dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        ShowWindow(h, if visible { SW_SHOWNA } else { SW_HIDE });
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
impl Fonts {
    pub fn new(dpi: u32) -> Result<Self, String> {
        fn font(size: i32, weight: u32, dpi: u32) -> Result<nwg::Font, String> {
            let mut f = Default::default();
            nwg::Font::builder()
                .family("Microsoft YaHei UI")
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
    DrawTextW(dc, s.as_ptr(), -1, &mut rect, flags);
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
    let disabled = item.itemState & ODS_DISABLED != 0;
    let pressed = item.itemState & ODS_SELECTED != 0;
    let hot = item.itemState & ODS_HOTLIGHT != 0;
    let (bg, fg, border) = if disabled {
        (rgb(241, 243, 247), rgb(157, 165, 178), rgb(235, 238, 243))
    } else if primary {
        (if pressed { rgb(32, 73, 184) } else { BLUE }, WHITE, BLUE)
    } else if nav {
        if selected {
            (rgb(232, 238, 255), BLUE, rgb(232, 238, 255))
        } else {
            (
                if hot { BG } else { WHITE },
                INK,
                if hot { BG } else { WHITE },
            )
        }
    } else {
        (
            if pressed || hot {
                rgb(240, 244, 251)
            } else {
                WHITE
            },
            INK,
            LINE,
        )
    };
    fill(item.hDC, &item.rcItem, if nav { WHITE } else { BG });
    round(
        item.hDC,
        &item.rcItem,
        px(10, dpi),
        bg,
        if item.itemState & ODS_FOCUS != 0 {
            BLUE
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
