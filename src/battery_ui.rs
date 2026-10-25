//! Shared battery geometry/colors for the GDI overview and Direct2D monitor.
use crate::{
    appearance,
    battery::{Band, View},
    gui_theme as theme,
};
use std::ptr::{null, null_mut};
use winapi::{
    shared::windef::*,
    um::{commctrl::*, libloaderapi::GetModuleHandleW, wingdi::*, winuser::*},
};

pub fn color(view: View) -> u32 {
    let p = appearance::palette();
    if p.kind == appearance::Kind::Contrast {
        return p.text;
    }
    let dark = p.kind == appearance::Kind::Dark;
    if matches!(view.status, crate::battery::Status::Fault(_)) {
        return if dark {
            theme::rgb(255, 122, 122)
        } else {
            theme::rgb(210, 54, 54)
        };
    }
    match view.band() {
        Band::Green => {
            if dark {
                theme::rgb(103, 207, 138)
            } else {
                theme::rgb(34, 139, 78)
            }
        }
        Band::Yellow => {
            if dark {
                theme::rgb(229, 186, 87)
            } else {
                theme::rgb(176, 121, 0)
            }
        }
        Band::Red => {
            if dark {
                theme::rgb(255, 122, 122)
            } else {
                theme::rgb(210, 54, 54)
            }
        }
        Band::Unknown => p.muted,
    }
}
pub fn fill_fraction(view: View) -> f32 {
    view.percent.unwrap_or(0).min(100) as f32 / 100.
}
pub const BOLT: [(f32, f32); 6] = [
    (0.58, 0.12),
    (0.35, 0.53),
    (0.52, 0.53),
    (0.42, 0.89),
    (0.69, 0.43),
    (0.52, 0.43),
];
#[derive(Clone, Copy)]
pub struct OverviewLayout {
    pub left_width: i32,
    pub title_height: i32,
    pub subtitle_y: i32,
    pub subtitle_height: i32,
    pub status_y: i32,
    pub button_y: i32,
    pub card_height: i32,
    pub stats_y: i32,
    pub logs_y: i32,
    pub battery: RECT,
    pub compact: bool,
}
pub unsafe fn overview_layout(
    dc: HDC,
    dpi: u32,
    width: i32,
    fonts: &theme::Fonts,
) -> OverviewLayout {
    let compact = width < 660;
    let left_width = if compact { width - 56 } else { width - 224 };
    let measure = |s: &str, font: &native_windows_gui::Font, minimum: i32| {
        let old = SelectObject(dc, font.handle as _);
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: theme::px(left_width, dpi),
            bottom: 0,
        };
        DrawTextW(
            dc,
            theme::wide(s).as_ptr(),
            -1,
            &mut rect,
            DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
        );
        SelectObject(dc, old);
        ((rect.bottom as i64 * 96 + dpi as i64 - 1) / dpi as i64).max(minimum as i64) as i32
    };
    let title_height = measure(
        crate::tr!(gui_native_feedback_wireless_freedom),
        &fonts.title,
        42,
    );
    let subtitle_y = 132 + title_height + 2;
    let subtitle_height = measure(
        crate::tr!(gui_dualsense_bluetooth_local_virtual_usb_controller),
        &fonts.body,
        23,
    );
    let status_y = subtitle_y + subtitle_height;
    let button_y = if compact {
        status_y + 28 + 42 + 12
    } else {
        (status_y + 36).max(240)
    };
    let card_height = button_y + 36 + 24 - 112;
    let battery = if compact {
        RECT {
            left: 28,
            top: status_y + 28,
            right: width - 28,
            bottom: status_y + 70,
        }
    } else {
        RECT {
            left: width - 172,
            top: 132,
            right: width - 28,
            bottom: 112 + card_height - 20,
        }
    };
    let stats_y = 112 + card_height + 20;
    OverviewLayout {
        left_width,
        title_height,
        subtitle_y,
        subtitle_height,
        status_y,
        button_y,
        card_height,
        stats_y,
        logs_y: stats_y + 132,
        battery,
        compact,
    }
}
pub fn header(width: f32) -> (f32, f32, f32) {
    // title width, battery x, additional header height
    if width < 260. {
        (width - 32., 16., 28.)
    } else {
        (width - 132., width - 110., 0.)
    }
}
pub unsafe fn icon(dc: HDC, rect: RECT, dpi: u32, view: View) {
    let p = appearance::palette();
    let px = |n| theme::px(n, dpi);
    let body = RECT {
        right: rect.right - px(4),
        ..rect
    };
    theme::round(
        dc,
        &body,
        px(4),
        p.surface,
        if view.percent.is_some() || matches!(view.status, crate::battery::Status::Fault(_)) {
            color(view)
        } else {
            p.muted
        },
    );
    let terminal = RECT {
        left: body.right,
        right: rect.right,
        top: body.top + (body.bottom - body.top) / 3,
        bottom: body.bottom - (body.bottom - body.top) / 3,
    };
    theme::fill(dc, &terminal, p.muted);
    let inset = px(3).max(2);
    let fill = RECT {
        left: body.left + inset,
        top: body.top + inset,
        right: body.left
            + inset
            + ((body.right - body.left - 2 * inset) as f32 * fill_fraction(view)).round() as i32,
        bottom: body.bottom - inset,
    };
    if fill.right > fill.left {
        theme::fill(dc, &fill, color(view));
    }
    if view.status == crate::battery::Status::Charging {
        let ink = if p.kind == appearance::Kind::Contrast {
            p.surface
        } else {
            p.text
        };
        let pen = CreatePen(PS_SOLID as i32, px(2).max(1), ink);
        let old = SelectObject(dc, pen as _);
        for (index, (x, y)) in BOLT.iter().enumerate() {
            let x = body.left + (*x * (body.right - body.left) as f32) as i32;
            let y = body.top + (*y * (body.bottom - body.top) as f32) as i32;
            if index == 0 {
                MoveToEx(dc, x, y, null_mut());
            } else {
                LineTo(dc, x, y);
            }
        }
        SelectObject(dc, old);
        DeleteObject(pen as _);
    }
    if matches!(view.status, crate::battery::Status::Fault(_)) {
        let x = (body.left + body.right) / 2;
        let pen = CreatePen(PS_SOLID as i32, px(2).max(1), color(view));
        let old = SelectObject(dc, pen as _);
        MoveToEx(dc, x, body.top + px(4), null_mut());
        LineTo(dc, x, body.bottom - px(8));
        SelectObject(dc, old);
        DeleteObject(pen as _);
        theme::fill(
            dc,
            &RECT {
                left: x - px(1),
                right: x + px(1),
                top: body.bottom - px(5),
                bottom: body.bottom - px(3),
            },
            color(view),
        );
    }
}
pub unsafe fn overview(
    dc: HDC,
    rect: RECT,
    dpi: u32,
    view: View,
    fonts: &theme::Fonts,
    compact: bool,
) {
    let p = appearance::palette();
    let px = |n| theme::px(n, dpi);
    let text = |r: RECT, s: &str, font: &native_windows_gui::Font, color| {
        theme::text(
            dc,
            &r,
            s,
            font,
            color,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        )
    };
    let mid = (rect.top + rect.bottom) / 2;
    if compact {
        icon(
            dc,
            RECT {
                left: rect.left,
                top: mid - px(11),
                right: rect.left + px(46),
                bottom: mid + px(11),
            },
            dpi,
            view,
        );
        text(
            RECT {
                left: rect.left + px(58),
                right: rect.left + px(142),
                ..rect
            },
            &view.number(),
            &fonts.strong,
            p.text,
        );
        text(
            RECT {
                left: rect.left + px(148),
                ..rect
            },
            view.caption(),
            &fonts.small,
            p.muted,
        );
    } else {
        text(
            RECT {
                bottom: rect.top + px(28),
                ..rect
            },
            crate::tr!(battery_label),
            &fonts.body,
            p.muted,
        );
        icon(
            dc,
            RECT {
                left: rect.left,
                top: mid - px(11),
                right: rect.left + px(48),
                bottom: mid + px(13),
            },
            dpi,
            view,
        );
        text(
            RECT {
                left: rect.left + px(58),
                top: mid - px(22),
                bottom: mid + px(24),
                ..rect
            },
            &view.number(),
            &fonts.value,
            p.text,
        );
        text(
            RECT {
                top: rect.bottom - px(40),
                ..rect
            },
            view.caption(),
            &fonts.small,
            p.muted,
        );
    }
}

/// Tooltip text and native handles remain owned until destruction.
pub struct Tooltip {
    hwnd: HWND,
    owner: HWND,
    text: Vec<u16>,
    rect: RECT,
}
impl Tooltip {
    pub fn needs_rect(&self, rect: RECT) -> bool {
        self.rect.left != rect.left
            || self.rect.top != rect.top
            || self.rect.right != rect.right
            || self.rect.bottom != rect.bottom
    }
    pub unsafe fn new(owner: HWND) -> Self {
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            theme::wide("tooltips_class32").as_ptr(),
            null(),
            WS_POPUP | TTS_ALWAYSTIP | TTS_NOPREFIX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            owner,
            null_mut(),
            GetModuleHandleW(null()),
            null_mut(),
        );
        let mut result = Self {
            hwnd,
            owner,
            text: vec![0],
            rect: std::mem::zeroed(),
        };
        if !hwnd.is_null() {
            let info = result.info();
            SendMessageW(hwnd, TTM_ADDTOOLW, 0, &info as *const _ as isize);
            SendMessageW(
                hwnd,
                TTM_SETMAXTIPWIDTH,
                0,
                theme::px(360, theme::window_dpi(owner)) as isize,
            );
            appearance::install(hwnd);
        }
        result
    }
    fn info(&mut self) -> TOOLINFOW {
        let mut info: TOOLINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<TOOLINFOW>() as u32;
        info.uFlags = TTF_SUBCLASS;
        info.hwnd = self.owner;
        info.uId = 0x4f444241;
        info.rect = self.rect;
        info.lpszText = self.text.as_mut_ptr();
        info
    }
    pub unsafe fn update(&mut self, rect: RECT, text: &str) {
        if self.hwnd.is_null() {
            return;
        }
        let same = self.rect.left == rect.left
            && self.rect.top == rect.top
            && self.rect.right == rect.right
            && self.rect.bottom == rect.bottom;
        if !same {
            self.rect = rect;
            let info = self.info();
            SendMessageW(self.hwnd, TTM_NEWTOOLRECTW, 0, &info as *const _ as isize);
            if rect.right <= rect.left || rect.bottom <= rect.top {
                SendMessageW(self.hwnd, TTM_POP, 0, 0);
            }
        }
        let text = theme::wide(text);
        if self.text != text {
            self.text = text;
            let info = self.info();
            SendMessageW(self.hwnd, TTM_UPDATETIPTEXTW, 0, &info as *const _ as isize);
        }
    }
}
impl Drop for Tooltip {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.is_null() {
                DestroyWindow(self.hwnd);
            }
        }
    }
}
