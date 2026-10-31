//! Paint adapters preserve native control classes, input handling and accessibility.
use crate::appearance::{self as a, wide};
use std::{
    cell::RefCell,
    collections::HashMap,
    ptr::{null, null_mut},
};
use winapi::{
    shared::{minwindef::*, windef::*},
    um::{commctrl::*, wingdi::*, winuser::*},
};
const ID: usize = 0x4f444354;
thread_local! { static MENUS: RefCell<HashMap<u32,String>>=RefCell::new(HashMap::new()); }
unsafe fn class(hwnd: HWND) -> String {
    let mut b = [0u16; 128];
    let n = GetClassNameW(hwnd, b.as_mut_ptr(), 128);
    String::from_utf16_lossy(&b[..n.max(0) as usize]).to_ascii_lowercase()
}
pub unsafe fn install_children(hwnd: HWND) {
    unsafe extern "system" fn each(h: HWND, _: LPARAM) -> BOOL {
        install(h);
        1
    }
    EnumChildWindows(hwnd, Some(each), 0);
}
pub unsafe fn install(h: HWND) {
    let c = class(h);
    let kind = match c.as_str() {
        "button" => 1,
        "combobox" => 2,
        "edit" => 3,
        "static" => 4,
        "listbox" => 5,
        "msctls_trackbar32" => 6,
        _ => 0,
    };
    SetWindowSubclass(h, Some(control_proc), ID, kind);
    if kind == 0 {
        SetWindowLongPtrW(
            h,
            GWL_STYLE,
            GetWindowLongPtrW(h, GWL_STYLE) | WS_CLIPCHILDREN as isize,
        );
    }
    if kind == 2 {
        let style = GetWindowLongPtrW(h, GWL_STYLE);
        SetWindowLongPtrW(
            h,
            GWL_STYLE,
            style | CBS_OWNERDRAWFIXED as isize | CBS_HASSTRINGS as isize,
        );
        let height = crate::gui_theme::px(22, crate::gui_theme::window_dpi(h));
        SendMessageW(h, CB_SETITEMHEIGHT, 0, height as isize);
        SendMessageW(h, CB_SETITEMHEIGHT, usize::MAX, height as isize);
        let mut info: COMBOBOXINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<COMBOBOXINFO>() as u32;
        if GetComboBoxInfo(h, &mut info) != 0 && !info.hwndList.is_null() {
            SetWindowSubclass(info.hwndList, Some(control_proc), ID, 5);
        }
    }
    InvalidateRect(h, null(), 1);
}
unsafe fn text(hwnd: HWND) -> String {
    let n = GetWindowTextLengthW(hwnd).max(0).min(32768);
    let mut b = vec![0u16; n as usize + 1];
    let len = GetWindowTextW(hwnd, b.as_mut_ptr(), b.len() as i32);
    String::from_utf16_lossy(&b[..len.max(0) as usize])
}
unsafe fn label(dc: HDC, hwnd: HWND, s: &str, r: &RECT, color: u32, flags: u32) {
    let font = SendMessageW(hwnd, WM_GETFONT, 0, 0);
    let old = SelectObject(
        dc,
        if font == 0 {
            GetStockObject(DEFAULT_GUI_FONT as i32)
        } else {
            font as _
        },
    );
    SetTextColor(dc, color);
    SetBkMode(dc, TRANSPARENT as i32);
    let mut r = *r;
    DrawTextW(dc, wide(s).as_ptr(), -1, &mut r, flags);
    SelectObject(dc, old);
}
unsafe fn fill(dc: HDC, r: &RECT, color: u32) {
    crate::gui_theme::fill(dc, r, color);
}
unsafe fn border(dc: HDC, r: &RECT, color: u32) {
    let b = CreateSolidBrush(color);
    FrameRect(dc, r, b);
    DeleteObject(b as _);
}
unsafe fn checkbox(hwnd: HWND, dc: HDC, r: RECT) {
    let p = a::palette();
    fill(dc, &r, p.surface);
    let dpi = crate::gui_theme::window_dpi(hwnd);
    let edge = crate::gui_theme::px(13, dpi);
    let box_rect = RECT {
        left: 2,
        top: (r.bottom - edge) / 2,
        right: 2 + edge,
        bottom: (r.bottom + edge) / 2,
    };
    let enabled = IsWindowEnabled(hwnd) != 0;
    let checked = SendMessageW(hwnd, BM_GETCHECK, 0, 0) != 0;
    fill(dc, &box_rect, if checked { p.primary } else { p.surface });
    border(
        dc,
        &box_rect,
        if enabled { p.accent } else { p.disabled_text },
    );
    if checked {
        let pen = CreatePen(
            PS_SOLID as i32,
            crate::gui_theme::px(2, dpi),
            p.primary_text,
        );
        let old = SelectObject(dc, pen as _);
        MoveToEx(
            dc,
            box_rect.left + edge / 5,
            box_rect.top + edge / 2,
            null_mut(),
        );
        LineTo(dc, box_rect.left + edge * 2 / 5, box_rect.bottom - edge / 4);
        LineTo(dc, box_rect.right - edge / 6, box_rect.top + edge / 4);
        SelectObject(dc, old);
        DeleteObject(pen as _);
    }
    let mut tr = r;
    tr.left = box_rect.right + crate::gui_theme::px(5, dpi);
    label(
        dc,
        hwnd,
        &text(hwnd),
        &tr,
        if enabled { p.text } else { p.disabled_text },
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    if GetFocus() == hwnd
        && SendMessageW(hwnd, WM_QUERYUISTATE, 0, 0) & UISF_HIDEFOCUS as isize == 0
    {
        DrawFocusRect(dc, &tr);
    }
}
unsafe fn push_button(hwnd: HWND, dc: HDC, r: RECT) {
    let p = a::palette();
    let enabled = IsWindowEnabled(hwnd) != 0;
    let pressed = SendMessageW(hwnd, BM_GETSTATE, 0, 0) & BST_PUSHED as isize != 0;
    let mut pt: POINT = std::mem::zeroed();
    GetCursorPos(&mut pt);
    ScreenToClient(hwnd, &mut pt);
    let hot = PtInRect(&r, pt) != 0;
    let primary = GetWindowLongPtrW(hwnd, GWL_STYLE) & 0xf == BS_DEFPUSHBUTTON as isize;
    let bg = if !enabled {
        p.disabled
    } else if primary {
        a::button_color(p.primary, pressed, hot)
    } else if pressed {
        p.selected
    } else if hot {
        p.hover
    } else {
        p.surface
    };
    fill(dc, &r, bg);
    border(dc, &r, if GetFocus() == hwnd { p.accent } else { p.line });
    label(
        dc,
        hwnd,
        &text(hwnd),
        &r,
        if !enabled {
            p.disabled_text
        } else if primary {
            p.primary_text
        } else {
            p.text
        },
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    if GetFocus() == hwnd
        && SendMessageW(hwnd, WM_QUERYUISTATE, 0, 0) & UISF_HIDEFOCUS as isize == 0
    {
        let mut rr = r;
        InflateRect(&mut rr, -3, -3);
        DrawFocusRect(dc, &rr);
    }
}
unsafe fn combo(hwnd: HWND, dc: HDC, r: RECT) {
    let p = a::palette();
    let enabled = IsWindowEnabled(hwnd) != 0;
    fill(dc, &r, if enabled { p.surface } else { p.disabled });
    border(dc, &r, if GetFocus() == hwnd { p.accent } else { p.line });
    let mut tr = r;
    tr.left += 5;
    tr.right -= 23;
    label(
        dc,
        hwnd,
        &text(hwnd),
        &tr,
        if enabled { p.text } else { p.disabled_text },
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
    );
    let px = |v| crate::gui_theme::px(v, crate::gui_theme::window_dpi(hwnd));
    let x = r.right - px(13);
    let y = (r.bottom + r.top) / 2;
    let pen = CreatePen(PS_SOLID as i32, 1, p.muted);
    let old = SelectObject(dc, pen as _);
    MoveToEx(dc, x - px(4), y - px(2), null_mut());
    LineTo(dc, x, y + px(2));
    LineTo(dc, x + px(4), y - px(2));
    SelectObject(dc, old);
    DeleteObject(pen as _);
}
const SCROLL_TIMER: usize = 0x4f445343;
#[derive(Clone, Copy)]
struct ScrollGesture {
    offset: i32,
    action: isize,
    last_y: i32,
}
thread_local! {static SCROLL_GESTURES:RefCell<HashMap<usize,ScrollGesture>>=RefCell::new(HashMap::new());}
unsafe fn vertical_bar(hwnd: HWND) -> Option<SCROLLBARINFO> {
    if a::palette().kind == a::Kind::Contrast
        || GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_VSCROLL as isize == 0
    {
        return None;
    }
    let mut bar: SCROLLBARINFO = std::mem::zeroed();
    bar.cbSize = std::mem::size_of::<SCROLLBARINFO>() as u32;
    if GetScrollBarInfo(hwnd, OBJID_VSCROLL, &mut bar) == 0
        || bar.rgstate[0] & (STATE_SYSTEM_INVISIBLE | STATE_SYSTEM_OFFSCREEN) != 0
    {
        None
    } else {
        Some(bar)
    }
}
fn thumb_position(
    pointer: i32,
    track_start: i32,
    travel: i32,
    minimum: i32,
    maximum: i32,
    page: u32,
) -> i32 {
    let last = (maximum as i64 - page.saturating_sub(1) as i64).max(minimum as i64);
    if travel <= 0 {
        return minimum;
    }
    (minimum as i64
        + ((pointer - track_start).clamp(0, travel) as i64 * (last - minimum as i64)
            + travel as i64 / 2)
            / travel as i64) as i32
}
unsafe fn drag_scroll(hwnd: HWND, bar: &SCROLLBARINFO, pointer: i32, offset: i32) {
    let mut info: SCROLLINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
    info.fMask = SIF_RANGE | SIF_PAGE | SIF_POS;
    if GetScrollInfo(hwnd, SB_VERT as i32, &mut info) == 0 {
        return;
    }
    let travel = bar.rcScrollBar.bottom
        - bar.rcScrollBar.top
        - 2 * bar.dxyLineButton
        - (bar.xyThumbBottom - bar.xyThumbTop);
    if travel <= 0 {
        return;
    }
    let position = thumb_position(
        pointer - offset,
        bar.rcScrollBar.top + bar.dxyLineButton,
        travel,
        info.nMin,
        info.nMax,
        info.nPage,
    );
    if position == info.nPos {
        return;
    }
    match class(hwnd).as_str() {
        "listbox" => {
            SendMessageW(hwnd, LB_SETTOPINDEX, position as usize, 0);
        }
        "edit" => {
            SendMessageW(
                hwnd,
                EM_LINESCROLL as u32,
                0,
                (position - info.nPos) as isize,
            );
        }
        _ => {
            SendMessageW(
                hwnd,
                WM_VSCROLL,
                SB_THUMBTRACK as usize | ((position as usize & 65535) << 16),
                0,
            );
        }
    }
    scroll_bar(hwnd);
}
/// Own nonclient scrollbar input as well as painting. DefWindowProc's modal
/// thumb tracking paints native light scrollbars directly, bypassing WM_NCPAINT.
pub unsafe fn scroll_message(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> Option<LRESULT> {
    if matches!(msg, WM_NCDESTROY | WM_CAPTURECHANGED | WM_CANCELMODE) {
        SCROLL_GESTURES.with(|g| g.borrow_mut().remove(&(hwnd as usize)));
        KillTimer(hwnd, SCROLL_TIMER);
        if msg == WM_CANCELMODE && GetCapture() == hwnd {
            ReleaseCapture();
        }
        return None;
    }
    if !matches!(
        msg,
        WM_NCPAINT
            | WM_NCMOUSEMOVE
            | WM_NCMOUSEHOVER
            | WM_NCMOUSELEAVE
            | WM_NCLBUTTONDOWN
            | WM_NCLBUTTONDBLCLK
            | WM_MOUSEMOVE
            | WM_LBUTTONUP
            | WM_TIMER
    ) {
        return None;
    }
    let gesture = SCROLL_GESTURES.with(|g| g.borrow().get(&(hwnd as usize)).copied());
    if msg == WM_LBUTTONUP && gesture.is_some() {
        SCROLL_GESTURES.with(|g| g.borrow_mut().remove(&(hwnd as usize)));
        KillTimer(hwnd, SCROLL_TIMER);
        if GetCapture() == hwnd {
            ReleaseCapture();
        }
        SendMessageW(hwnd, WM_VSCROLL, SB_ENDSCROLL as usize, 0);
        scroll_bar(hwnd);
        return Some(0);
    }
    let bar = vertical_bar(hwnd)?;
    if msg == WM_NCPAINT
        && class(hwnd) != "edit"
        && GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_CHILD as isize != 0
    {
        let dc = GetWindowDC(hwnd);
        if !dc.is_null() {
            let mut wr: RECT = std::mem::zeroed();
            let mut cr: RECT = std::mem::zeroed();
            let mut origin = POINT { x: 0, y: 0 };
            GetWindowRect(hwnd, &mut wr);
            GetClientRect(hwnd, &mut cr);
            ClientToScreen(hwnd, &mut origin);
            let (width, height, left, top) = (
                wr.right - wr.left,
                wr.bottom - wr.top,
                origin.x - wr.left,
                origin.y - wr.top,
            );
            let saved = SaveDC(dc);
            ExcludeClipRect(
                dc,
                bar.rcScrollBar.left - wr.left,
                bar.rcScrollBar.top - wr.top,
                bar.rcScrollBar.right - wr.left,
                bar.rcScrollBar.bottom - wr.top,
            );
            for rect in [
                RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: top,
                },
                RECT {
                    left: 0,
                    top: top + cr.bottom,
                    right: width,
                    bottom: height,
                },
                RECT {
                    left: 0,
                    top,
                    right: left,
                    bottom: top + cr.bottom,
                },
                RECT {
                    left: left + cr.right,
                    top,
                    right: width,
                    bottom: top + cr.bottom,
                },
            ] {
                fill(dc, &rect, a::palette().surface);
            }
            if GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_BORDER as isize != 0 {
                border(
                    dc,
                    &RECT {
                        left: 0,
                        top: 0,
                        right: width,
                        bottom: height,
                    },
                    a::palette().line,
                );
            }
            RestoreDC(dc, saved);
            ReleaseDC(hwnd, dc);
        }
        scroll_bar(hwnd);
        return Some(0);
    }
    if matches!(msg, WM_NCMOUSEMOVE | WM_NCMOUSEHOVER) && w as isize == HTVSCROLL {
        scroll_bar(hwnd);
        return Some(0);
    }
    if msg == WM_NCMOUSELEAVE {
        scroll_bar(hwnd);
        return Some(0);
    }
    if matches!(msg, WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK) && w as isize == HTVSCROLL {
        if bar.rgstate[0] & STATE_SYSTEM_UNAVAILABLE != 0 {
            return Some(0);
        }
        let y = ((l >> 16) as i16) as i32;
        let local = y - bar.rcScrollBar.top;
        let action = if local < bar.dxyLineButton {
            SB_LINEUP
        } else if local >= bar.rcScrollBar.bottom - bar.rcScrollBar.top - bar.dxyLineButton {
            SB_LINEDOWN
        } else if local < bar.xyThumbTop {
            SB_PAGEUP
        } else if local >= bar.xyThumbBottom {
            SB_PAGEDOWN
        } else {
            SB_THUMBTRACK
        };
        SCROLL_GESTURES.with(|g| {
            g.borrow_mut().insert(
                hwnd as usize,
                ScrollGesture {
                    offset: local - bar.xyThumbTop,
                    action,
                    last_y: y,
                },
            )
        });
        SetCapture(hwnd);
        if action != SB_THUMBTRACK {
            SendMessageW(hwnd, WM_VSCROLL, action as usize, 0);
            SetTimer(hwnd, SCROLL_TIMER, 350, None);
        }
        scroll_bar(hwnd);
        return Some(0);
    }
    if msg == WM_MOUSEMOVE {
        if let Some(gesture) = gesture {
            if gesture.action == SB_THUMBTRACK {
                let mut cursor = POINT {
                    x: (l as i16) as i32,
                    y: ((l >> 16) as i16) as i32,
                };
                ClientToScreen(hwnd, &mut cursor);
                if cursor.y == gesture.last_y {
                    return Some(0);
                }
                SCROLL_GESTURES.with(|g| {
                    if let Some(state) = g.borrow_mut().get_mut(&(hwnd as usize)) {
                        state.last_y = cursor.y;
                    }
                });
                drag_scroll(hwnd, &bar, cursor.y, gesture.offset);
            }
            return Some(0);
        }
    }
    if msg == WM_TIMER && w == SCROLL_TIMER {
        if let Some(gesture) = gesture {
            let mut cursor: POINT = std::mem::zeroed();
            GetCursorPos(&mut cursor);
            if PtInRect(&bar.rcScrollBar, cursor) != 0 {
                SendMessageW(hwnd, WM_VSCROLL, gesture.action as usize, 0);
            }
            SetTimer(hwnd, SCROLL_TIMER, 60, None);
            scroll_bar(hwnd);
        } else {
            KillTimer(hwnd, SCROLL_TIMER);
        }
        return Some(0);
    }
    None
}
pub unsafe fn refresh_scrollbar(hwnd: HWND) {
    scroll_bar(hwnd);
}
unsafe fn scroll_bar(hwnd: HWND) {
    let Some(bar) = vertical_bar(hwnd) else {
        return;
    };
    let dc = GetWindowDC(hwnd);
    if dc.is_null() {
        return;
    }
    let mut wr: RECT = std::mem::zeroed();
    GetWindowRect(hwnd, &mut wr);
    let mut r = bar.rcScrollBar;
    OffsetRect(&mut r, -wr.left, -wr.top);
    let output = dc;
    let cached = crate::gdi_buffer::PaintBuffer::acquire(
        hwnd,
        1,
        output,
        r.right - r.left,
        r.bottom - r.top,
    );
    let buffer = cached
        .as_ref()
        .map(|lease| lease.dc())
        .unwrap_or(null_mut());
    let buffered = !buffer.is_null();
    if buffered {
        SetWindowOrgEx(buffer, r.left, r.top, null_mut());
    }
    let dc = if buffered { buffer } else { output };
    let p = a::palette();
    fill(dc, &r, p.surface);
    if bar.xyThumbBottom > bar.xyThumbTop {
        let thumb = RECT {
            left: r.left + 3,
            right: r.right - 3,
            top: r.top + bar.xyThumbTop,
            bottom: r.top + bar.xyThumbBottom,
        };
        fill(dc, &thumb, p.muted);
    }
    let mut top = r;
    top.bottom = top.top + bar.dxyLineButton;
    label(
        dc,
        hwnd,
        "˄",
        &top,
        p.muted,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    let mut bot = r;
    bot.top = bot.bottom - bar.dxyLineButton;
    label(
        dc,
        hwnd,
        "˅",
        &bot,
        p.muted,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    if buffered {
        BitBlt(
            output,
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            buffer,
            r.left,
            r.top,
            SRCCOPY,
        );
    }
    drop(cached);
    ReleaseDC(hwnd, output);
}
unsafe extern "system" fn control_proc(
    hwnd: HWND,
    msg: UINT,
    w: WPARAM,
    l: LPARAM,
    _: usize,
    kind: usize,
) -> LRESULT {
    if msg == WM_NCDESTROY {
        scroll_message(hwnd, msg, w, l);
        crate::gdi_buffer::release(hwnd);
        RemovePropW(hwnd, wide("OpenDS.ControlHot").as_ptr());
        RemoveWindowSubclass(hwnd, Some(control_proc), ID);
        return DefSubclassProc(hwnd, msg, w, l);
    }
    if let Some(result) = scroll_message(hwnd, msg, w, l) {
        return result;
    }
    if let Some(r) = parent_message(hwnd, msg, w, l) {
        return r;
    }
    let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
    let readonly_text =
        kind == 3 && style & ES_MULTILINE as isize != 0 && style & ES_READONLY as isize != 0;
    if (kind == 5 || readonly_text) && msg == WM_ERASEBKGND {
        return 1;
    }
    if (kind == 5 || readonly_text) && matches!(msg, WM_PAINT | WM_PRINTCLIENT) {
        // Render the complete list client atomically. Native scrolling can leave
        // partial rows exposed between background erasure and text painting.
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let dc = if msg == WM_PAINT {
            BeginPaint(hwnd, &mut ps)
        } else {
            w as HDC
        };
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rect);
        let cached = crate::gdi_buffer::PaintBuffer::acquire(hwnd, 0, dc, rect.right, rect.bottom);
        if let Some(lease) = cached.as_ref() {
            let buffer = lease.dc();
            FillRect(buffer, &rect, a::surface_brush());
            DefSubclassProc(hwnd, WM_PRINTCLIENT, buffer as usize, PRF_CLIENT as isize);
            BitBlt(dc, 0, 0, rect.right, rect.bottom, buffer, 0, 0, SRCCOPY);
        } else {
            DefSubclassProc(hwnd, WM_PRINTCLIENT, dc as usize, PRF_CLIENT as isize);
        }
        drop(cached);
        if msg == WM_PAINT {
            EndPaint(hwnd, &ps);
            scroll_bar(hwnd);
        }
        return 0;
    }
    let cb = kind == 1 && matches!(style & 0xf, 2 | 3 | 5 | 6 | 9);
    let push = kind == 1 && !cb && style & 0xf != BS_OWNERDRAW as isize;
    if (msg == WM_PAINT || msg == WM_PRINTCLIENT) && (cb || push || kind == 2 || kind == 4) {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let dc = if msg == WM_PRINTCLIENT {
            w as HDC
        } else {
            BeginPaint(hwnd, &mut ps)
        };
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        let output = dc;
        let cached = crate::gdi_buffer::PaintBuffer::acquire(hwnd, 0, output, r.right, r.bottom);
        let buffer = cached
            .as_ref()
            .map(|lease| lease.dc())
            .unwrap_or(null_mut());
        let buffered = !buffer.is_null();
        let dc = if buffered { buffer } else { output };
        if cb {
            checkbox(hwnd, dc, r);
        } else if push {
            push_button(hwnd, dc, r);
        } else if kind == 2 {
            combo(hwnd, dc, r);
        } else {
            let p = a::palette();
            fill(dc, &r, p.surface);
            label(
                dc,
                hwnd,
                &text(hwnd),
                &r,
                if IsWindowEnabled(hwnd) != 0 {
                    p.text
                } else {
                    p.disabled_text
                },
                DT_LEFT | DT_WORDBREAK | DT_NOPREFIX,
            );
        }
        if buffered {
            BitBlt(output, 0, 0, r.right, r.bottom, buffer, 0, 0, SRCCOPY);
        }
        drop(cached);
        if msg == WM_PAINT {
            EndPaint(hwnd, &ps);
        }
        return 0;
    }
    if msg == WM_ERASEBKGND && (kind == 1 || kind == 2 || kind == 4) {
        return 1;
    }
    if (msg == WM_NCPAINT || (msg == WM_PRINT && l & PRF_NONCLIENT as isize != 0))
        && (kind == 4 || kind == 3)
    {
        let printing = msg == WM_PRINT;
        if printing {
            DefSubclassProc(hwnd, msg, w, l);
        }
        let mut wr: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut wr);
        let mut cr: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut cr);
        let mut point = POINT { x: 0, y: 0 };
        ClientToScreen(hwnd, &mut point);
        let dc = if printing {
            w as HDC
        } else {
            GetWindowDC(hwnd)
        };
        let width = wr.right - wr.left;
        let height = wr.bottom - wr.top;
        let top = point.y - wr.top;
        let left = point.x - wr.left;
        let bottom = top + cr.bottom;
        let right = left + cr.right;
        let saved = SaveDC(dc);
        if let Some(bar) = vertical_bar(hwnd) {
            ExcludeClipRect(
                dc,
                bar.rcScrollBar.left - wr.left,
                bar.rcScrollBar.top - wr.top,
                bar.rcScrollBar.right - wr.left,
                bar.rcScrollBar.bottom - wr.top,
            );
        }
        // NWG centers single-line edit text by reserving nonclient top/bottom
        // margins. Those margins are background, not a thick border.
        let color = a::palette().surface;
        for r in [
            RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: top,
            },
            RECT {
                left: 0,
                top: bottom,
                right: width,
                bottom: height,
            },
            RECT {
                left: 0,
                top: top,
                right: left,
                bottom,
            },
            RECT {
                left: right,
                top: top,
                right: width,
                bottom,
            },
        ] {
            if r.right > r.left && r.bottom > r.top {
                fill(dc, &r, color);
            }
        }
        if kind == 3 {
            border(
                dc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                },
                if GetFocus() == hwnd {
                    a::palette().accent
                } else {
                    a::palette().line
                },
            );
        }
        RestoreDC(dc, saved);
        if !printing {
            ReleaseDC(hwnd, dc);
        }
        if kind == 3 && !printing {
            scroll_bar(hwnd);
        }
        return 0;
    }
    // Checkboxes have no custom hover appearance; native hot tracking can
    // paint a transient system-colored box over our palette. Keep drag handling.
    if cb && matches!(msg, WM_MOUSEMOVE | WM_MOUSELEAVE) && w & MK_LBUTTON as usize == 0 {
        return 0;
    }
    let mut hover_changed = true;
    if kind == 1 && matches!(msg, WM_MOUSEMOVE | WM_MOUSELEAVE) {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rect);
        let point = POINT {
            x: (l as i16) as i32,
            y: ((l >> 16) as i16) as i32,
        };
        let hot = msg == WM_MOUSEMOVE && PtInRect(&rect, point) != 0;
        let name = wide("OpenDS.ControlHot");
        let previous = !GetPropW(hwnd, name.as_ptr()).is_null();
        hover_changed = previous != hot;
        if hot {
            SetPropW(hwnd, name.as_ptr(), 1usize as _);
        } else {
            RemovePropW(hwnd, name.as_ptr());
        }
    }
    if msg == WM_MOUSEMOVE && (kind == 1 || kind == 2) {
        let mut track: TRACKMOUSEEVENT = std::mem::zeroed();
        track.cbSize = std::mem::size_of::<TRACKMOUSEEVENT>() as u32;
        track.dwFlags = TME_LEAVE;
        track.hwndTrack = hwnd;
        TrackMouseEvent(&mut track);
    }
    if kind == 1 && matches!(msg, WM_MOUSEMOVE | WM_MOUSELEAVE) && w & MK_LBUTTON as usize == 0 {
        if hover_changed {
            InvalidateRect(hwnd, null(), 0);
        }
        return 0;
    }
    if kind == 3 && matches!(msg, WM_SETFOCUS | WM_KILLFOCUS) {
        RedrawWindow(hwnd, null(), null_mut(), RDW_INVALIDATE | RDW_FRAME);
    }
    let result = DefSubclassProc(hwnd, msg, w, l);
    if matches!(
        msg,
        WM_NCPAINT | WM_PAINT | WM_VSCROLL | WM_NCMOUSEMOVE | WM_NCLBUTTONUP | WM_MOUSEWHEEL
    ) {
        scroll_bar(hwnd);
    }
    let hover = matches!(msg, WM_MOUSEMOVE | WM_MOUSELEAVE);
    if kind != 5
        && !(hover && (kind == 3 || kind == 4 || cb || !hover_changed))
        && matches!(
            msg,
            WM_SETTEXT
                | BM_SETSTATE
                | WM_SETFOCUS
                | WM_KILLFOCUS
                | WM_ENABLE
                | BM_SETCHECK
                | WM_UPDATEUISTATE
                | WM_LBUTTONDOWN
                | WM_LBUTTONUP
                | WM_MOUSEMOVE
                | WM_MOUSELEAVE
                | WM_KEYUP
        )
    {
        InvalidateRect(hwnd, null(), 0);
        // Native buttons repaint synchronously when their label or enabled state
        // changes. Restore our palette before returning to the caller.
        if kind == 1 && matches!(msg, WM_SETTEXT | WM_ENABLE | BM_SETSTATE) {
            UpdateWindow(hwnd);
        }
    }
    result
}
/// Checks nonclient interaction, including the path that formerly entered the
/// system scrollbar modal loop and repainted a different theme during dragging.
pub unsafe fn validate_scrollbar(hwnd: HWND) -> Result<(), String> {
    UpdateWindow(hwnd);
    let mut initial: SCROLLINFO = std::mem::zeroed();
    initial.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
    initial.fMask = SIF_RANGE | SIF_PAGE | SIF_POS;
    if GetScrollInfo(hwnd, SB_VERT as i32, &mut initial) == 0
        || initial.nMax as i64 - initial.nPage.saturating_sub(1) as i64 <= initial.nMin as i64
    {
        return Ok(());
    }
    let Some(bar) = vertical_bar(hwnd) else {
        return Ok(());
    };
    if bar.rgstate[0] & STATE_SYSTEM_UNAVAILABLE != 0 || bar.xyThumbBottom <= bar.xyThumbTop {
        return Ok(());
    }
    let pack = |x: i32, y: i32| ((x as u32 & 65535) | ((y as u32 & 65535) << 16)) as isize;
    let x = (bar.rcScrollBar.left + bar.rcScrollBar.right) / 2;
    let thumb_y = bar.rcScrollBar.top + (bar.xyThumbTop + bar.xyThumbBottom) / 2;
    for msg in [WM_NCMOUSEMOVE, WM_NCMOUSEHOVER, WM_NCMOUSELEAVE] {
        if scroll_message(hwnd, msg, HTVSCROLL as usize, pack(x, thumb_y)) != Some(0) {
            return Err("Native scrollbar hover was not intercepted".into());
        }
    }
    let travel = bar.rcScrollBar.bottom
        - bar.rcScrollBar.top
        - 2 * bar.dxyLineButton
        - (bar.xyThumbBottom - bar.xyThumbTop);
    if travel <= 0 {
        SendMessageW(
            hwnd,
            WM_NCLBUTTONDOWN,
            HTVSCROLL as usize,
            pack(x, bar.rcScrollBar.bottom - 2),
        );
        SendMessageW(hwnd, WM_LBUTTONUP, 0, 0);
        GetScrollInfo(hwnd, SB_VERT as i32, &mut initial);
        if initial.nPos <= initial.nMin {
            return Err("Short scrollbar arrow did not scroll".into());
        }
        SendMessageW(hwnd, WM_VSCROLL, SB_TOP as usize, 0);
        scroll_bar(hwnd);
        return Ok(());
    }
    SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTVSCROLL as usize, pack(x, thumb_y));
    if GetCapture() != hwnd {
        return Err("Scrollbar thumb did not capture dragging".into());
    }
    let mut point = POINT {
        x,
        y: bar.rcScrollBar.bottom - bar.dxyLineButton - 1,
    };
    ScreenToClient(hwnd, &mut point);
    unsafe extern "system" fn count_scroll(
        hwnd: HWND,
        msg: UINT,
        w: WPARAM,
        l: LPARAM,
        _: usize,
        data: usize,
    ) -> LRESULT {
        if (msg == WM_VSCROLL && w & 65535 == SB_THUMBTRACK as usize)
            || msg == EM_LINESCROLL as u32
            || msg == LB_SETTOPINDEX
        {
            let count = &*(data as *const std::cell::Cell<usize>);
            count.set(count.get() + 1);
        }
        DefSubclassProc(hwnd, msg, w, l)
    }
    let count = std::cell::Cell::new(0usize);
    const WATCH: usize = 0x4f445357;
    SetWindowSubclass(hwnd, Some(count_scroll), WATCH, &count as *const _ as usize);
    SendMessageW(
        hwnd,
        WM_MOUSEMOVE,
        MK_LBUTTON as usize,
        pack(point.x, point.y),
    );
    let before = count.get();
    for offset in [-80, -40, 0, 40, 80] {
        SendMessageW(
            hwnd,
            WM_MOUSEMOVE,
            MK_LBUTTON as usize,
            pack(point.x + offset, point.y),
        );
    }
    RemoveWindowSubclass(hwnd, Some(count_scroll), WATCH);
    let redundant = count.get() != before;
    let mut info: SCROLLINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
    info.fMask = SIF_POS | SIF_RANGE | SIF_PAGE;
    GetScrollInfo(hwnd, SB_VERT as i32, &mut info);
    SendMessageW(hwnd, WM_LBUTTONUP, 0, pack(point.x, point.y));
    if redundant {
        return Err("Sideways thumb movement submitted redundant scroll updates".into());
    }
    if GetCapture() == hwnd || info.nPos <= info.nMin {
        return Err(format!(
            "Scrollbar thumb did not scroll/release: {} id={} initial={}/{}/{} current={}/{}/{} capture={} rect={:?} thumb={}/{}",
            class(hwnd),GetDlgCtrlID(hwnd),initial.nPos,initial.nMax,initial.nPage,info.nPos,info.nMax,info.nPage,GetCapture()==hwnd,(bar.rcScrollBar.top,bar.rcScrollBar.bottom),bar.xyThumbTop,bar.xyThumbBottom
        ));
    }
    SendMessageW(hwnd, WM_VSCROLL, SB_TOP as usize, 0);
    scroll_bar(hwnd);
    Ok(())
}
/// Isolated smoke check for hover erasure and native text scroll/selection paths.
pub unsafe fn validate_hover_stability(hwnd: HWND) -> Result<(), String> {
    let c = class(hwnd);
    let saved = text(hwnd);
    let check = if c == "button" {
        Some(SendMessageW(hwnd, BM_GETCHECK, 0, 0))
    } else {
        None
    };
    if c == "edit" {
        SetWindowTextW(
            hwnd,
            wide(
                &(0..80)
                    .map(|i| format!("Line {i}: themed text remains visible\r\n"))
                    .collect::<String>(),
            )
            .as_ptr(),
        );
    }
    let result = (|| {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rect);
        let (width, height) = (rect.right.max(1), rect.bottom.max(1));
        let dc = CreateCompatibleDC(null_mut());
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut bits = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if dc.is_null() || bitmap.is_null() {
            if !dc.is_null() {
                DeleteDC(dc);
            }
            return Err("Hover snapshot unavailable".into());
        }
        let old = SelectObject(dc, bitmap as _);
        let capture = || {
            SendMessageW(hwnd, WM_PRINTCLIENT, dc as usize, PRF_CLIENT as isize);
            GdiFlush();
            let data = std::slice::from_raw_parts(bits as *const u32, (width * height) as usize);
            data.chunks(width as usize)
                .take(128)
                .flat_map(|row| row.iter().take(512))
                .fold(0u64, |hash, pixel| {
                    hash.wrapping_mul(16777619) ^ *pixel as u64
                })
        };
        let mut error = validate_scrollbar(hwnd).err();
        for stage in 0..3 {
            if c == "edit" {
                SendMessageW(hwnd, EM_LINESCROLL as u32, 0, 3);
            }
            if check.is_some() {
                SendMessageW(
                    hwnd,
                    BM_SETCHECK,
                    if stage == 1 {
                        BST_CHECKED as usize
                    } else {
                        BST_UNCHECKED as usize
                    },
                    0,
                );
            }
            UpdateWindow(hwnd);
            let baseline = capture();
            for (x, y) in [(2, 2), (width / 2, height / 2), (width - 2, height - 2)] {
                SendMessageW(
                    hwnd,
                    WM_MOUSEMOVE,
                    0,
                    ((y as u32 & 65535) << 16 | (x as u32 & 65535)) as isize,
                );
                SendMessageW(hwnd, WM_MOUSELEAVE, 0, 0);
                if GetUpdateRect(hwnd, null_mut(), 0) != 0 || capture() != baseline {
                    error = Some(format!(
                        "{c} control {} repaints or changes on idle hover",
                        GetDlgCtrlID(hwnd)
                    ));
                    break;
                }
            }
        }
        SelectObject(dc, old);
        DeleteObject(bitmap as _);
        DeleteDC(dc);
        error.map_or(Ok(()), Err)
    })();
    if let Some(state) = check {
        SendMessageW(hwnd, BM_SETCHECK, state as usize, 0);
    } else {
        SetWindowTextW(hwnd, wide(&saved).as_ptr());
    }
    result
}

pub unsafe fn validate_button_motion(hwnd: HWND) -> Result<(), String> {
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rect);
    let pack = |x: i32, y: i32| ((x as u32 & 65535) | ((y as u32 & 65535) << 16)) as isize;
    SendMessageW(hwnd, WM_MOUSEMOVE, 0, pack(2, rect.bottom / 2));
    UpdateWindow(hwnd);
    for step in 1..10 {
        SendMessageW(
            hwnd,
            WM_MOUSEMOVE,
            0,
            pack((rect.right - 4) * step / 10 + 2, rect.bottom / 2),
        );
        if GetUpdateRect(hwnd, null_mut(), 0) != 0 {
            return Err("Navigation repainted while hover state stayed unchanged".into());
        }
    }
    SendMessageW(hwnd, WM_MOUSELEAVE, 0, 0);
    UpdateWindow(hwnd);
    Ok(())
}
/// Checks the real native-control paint path during isolated UI smoke runs.
pub unsafe fn validate_palette(hwnd: HWND) -> Result<(), String> {
    let mut rect: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rect);
    let dc = CreateCompatibleDC(null_mut());
    let screen = GetDC(hwnd);
    let bitmap = CreateCompatibleBitmap(screen, rect.right.max(1), rect.bottom.max(1));
    ReleaseDC(hwnd, screen);
    let old = SelectObject(dc, bitmap as _);
    SendMessageW(hwnd, WM_PRINTCLIENT, dc as usize, PRF_CLIENT as isize);
    let color = GetPixel(dc, (rect.right - 5).max(0), (rect.bottom - 5).max(0));
    SelectObject(dc, old);
    DeleteObject(bitmap as _);
    DeleteDC(dc);
    let p = a::palette();
    let expected =
        if IsWindowEnabled(hwnd) == 0 && matches!(class(hwnd).as_str(), "button" | "combobox") {
            p.disabled
        } else {
            p.surface
        };
    if color != expected {
        return Err(format!(
            "{} control {} background {color:06x} != {expected:06x}",
            class(hwnd),
            GetDlgCtrlID(hwnd)
        ));
    }
    Ok(())
}

pub unsafe fn parent_message(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> Option<LRESULT> {
    let p = a::palette();
    if matches!(
        msg,
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN | WM_CTLCOLORDLG
    ) {
        SetBkColor(w as HDC, p.surface);
        SetTextColor(
            w as HDC,
            if IsWindowEnabled(l as HWND) == 0 {
                p.disabled_text
            } else {
                p.text
            },
        );
        SetBkMode(w as HDC, TRANSPARENT as i32);
        return Some(a::surface_brush() as isize);
    }
    if msg == WM_NOTIFY && l != 0 {
        let n = &*(l as *const NMHDR);
        if n.code == NM_CUSTOMDRAW && class(n.hwndFrom) == "msctls_trackbar32" {
            let c = &*(l as *const NMCUSTOMDRAW);
            if c.dwDrawStage == CDDS_PREPAINT {
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(n.hwndFrom, &mut client);
                fill(c.hdc, &client, p.surface);
                let mut channel: RECT = std::mem::zeroed();
                let mut thumb: RECT = std::mem::zeroed();
                SendMessageW(
                    n.hwndFrom,
                    TBM_GETCHANNELRECT,
                    0,
                    &mut channel as *mut _ as isize,
                );
                SendMessageW(
                    n.hwndFrom,
                    TBM_GETTHUMBRECT,
                    0,
                    &mut thumb as *mut _ as isize,
                );
                let enabled = IsWindowEnabled(n.hwndFrom) != 0;
                fill(c.hdc, &channel, p.line);
                let mut active = channel;
                active.right = (thumb.left + thumb.right) / 2;
                if enabled && active.right > active.left {
                    fill(c.hdc, &active, p.accent);
                }
                crate::gui_theme::round(
                    c.hdc,
                    &thumb,
                    (thumb.right - thumb.left) / 2,
                    if enabled { p.accent } else { p.disabled_text },
                    p.line,
                );
                if GetFocus() == n.hwndFrom {
                    DrawFocusRect(c.hdc, &client);
                }
                return Some(CDRF_SKIPDEFAULT as isize);
            }
        }
    }
    if msg == WM_MEASUREITEM && l != 0 {
        let m = &mut *(l as *mut MEASUREITEMSTRUCT);
        if m.CtlType == ODT_COMBOBOX
            || (m.CtlType == ODT_LISTBOX
                && class(hwnd) == "opendscommandsettings"
                && m.CtlID == 201)
        {
            m.itemHeight = crate::gui_theme::px(22, crate::gui_theme::window_dpi(hwnd)) as u32;
            return Some(1);
        }
        if m.CtlType == ODT_MENU {
            m.itemHeight = crate::gui_theme::px(30, crate::gui_theme::window_dpi(hwnd)) as u32;
            m.itemWidth = crate::gui_theme::px(230, crate::gui_theme::window_dpi(hwnd)) as u32;
            return Some(1);
        }
    }
    if msg == WM_INITMENUPOPUP && ((l as usize >> 16) & 65535) == 0 {
        let menu = w as HMENU;
        let mut info: MENUINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MENUINFO>() as u32;
        info.fMask = MIM_BACKGROUND;
        info.hbrBack = a::surface_brush();
        SetMenuInfo(menu, &info);
        for i in 0..GetMenuItemCount(menu) {
            let mut b = [0u16; 1024];
            let n = GetMenuStringW(menu, i as u32, b.as_mut_ptr(), 1024, MF_BYPOSITION);
            let id = GetMenuItemID(menu, i);
            if id == u32::MAX {
                continue;
            }
            MENUS.with(|m| {
                m.borrow_mut()
                    .insert(id, String::from_utf16_lossy(&b[..n.max(0) as usize]));
            });
            let mut item: MENUITEMINFOW = std::mem::zeroed();
            item.cbSize = std::mem::size_of::<MENUITEMINFOW>() as u32;
            item.fMask = MIIM_FTYPE;
            GetMenuItemInfoW(menu, i as u32, 1, &mut item);
            item.fType |= MFT_OWNERDRAW;
            SetMenuItemInfoW(menu, i as u32, 1, &item);
        }
        return Some(0);
    }
    if msg == WM_DRAWITEM && l != 0 {
        let d = &*(l as *const DRAWITEMSTRUCT);
        let list =
            d.CtlType == ODT_LISTBOX && class(hwnd) == "opendscommandsettings" && d.CtlID == 201;
        if d.CtlType == ODT_COMBOBOX || d.CtlType == ODT_MENU || list {
            let heading = list && d.itemData == 1;
            let selected = d.itemState & ODS_SELECTED != 0 && !heading;
            let disabled = d.itemState & ODS_DISABLED != 0 || (list && d.itemData == 2);
            fill(
                d.hDC,
                &d.rcItem,
                if heading {
                    p.disabled
                } else if selected {
                    p.selected
                } else {
                    p.surface
                },
            );
            let value = if d.CtlType == ODT_MENU {
                MENUS.with(|m| m.borrow().get(&d.itemID).cloned().unwrap_or_default())
            } else if d.itemID == u32::MAX {
                String::new()
            } else {
                let len = SendMessageW(
                    d.hwndItem,
                    if list { LB_GETTEXTLEN } else { CB_GETLBTEXTLEN },
                    d.itemID as usize,
                    0,
                );
                if !(0..32768).contains(&len) {
                    String::new()
                } else {
                    let mut b = vec![0u16; len as usize + 1];
                    SendMessageW(
                        d.hwndItem,
                        if list { LB_GETTEXT } else { CB_GETLBTEXT },
                        d.itemID as usize,
                        b.as_mut_ptr() as isize,
                    );
                    String::from_utf16_lossy(&b[..len as usize])
                }
            };
            let mut r = d.rcItem;
            r.left += crate::gui_theme::px(
                if d.CtlType == ODT_MENU { 16 } else { 5 },
                crate::gui_theme::window_dpi(hwnd),
            );
            label(
                d.hDC,
                if d.CtlType == ODT_MENU {
                    hwnd
                } else {
                    d.hwndItem
                },
                &value,
                &r,
                if heading {
                    p.muted
                } else if disabled {
                    p.disabled_text
                } else if selected && p.kind == a::Kind::Contrast {
                    p.primary_text
                } else {
                    p.text
                },
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            );
            if d.itemState & ODS_FOCUS != 0 && !heading {
                DrawFocusRect(d.hDC, &r);
            }
            return Some(1);
        }
    }
    None
}

#[cfg(test)]
mod scrollbar_tests {
    use super::*;
    #[test]
    fn dragging_maps_track_ends_and_large_ranges_without_overscroll() {
        assert_eq!(thumb_position(0, 10, 100, 0, 199, 20), 0);
        assert_eq!(thumb_position(60, 10, 100, 0, 199, 20), 90);
        assert_eq!(thumb_position(999, 10, 100, 0, 199, 20), 180);
        assert_eq!(thumb_position(500, 0, 100, 7, 7, 0), 7);
        assert_eq!(thumb_position(50, 0, 0, 0, 100, 10), 0);
        assert_eq!(thumb_position(100, 0, 100, 0, i32::MAX, 1), i32::MAX);
    }
}
