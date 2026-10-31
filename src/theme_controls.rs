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
unsafe fn scroll_bar(hwnd: HWND) {
    if GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_VSCROLL as isize == 0 {
        return;
    }
    let mut bar: SCROLLBARINFO = std::mem::zeroed();
    bar.cbSize = std::mem::size_of::<SCROLLBARINFO>() as u32;
    if GetScrollBarInfo(hwnd, OBJID_VSCROLL, &mut bar) == 0
        || bar.rgstate[0] & STATE_SYSTEM_INVISIBLE != 0
    {
        return;
    }
    let dc = GetWindowDC(hwnd);
    if dc.is_null() {
        return;
    }
    let mut wr: RECT = std::mem::zeroed();
    GetWindowRect(hwnd, &mut wr);
    let mut r = bar.rcScrollBar;
    OffsetRect(&mut r, -wr.left, -wr.top);
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
    ReleaseDC(hwnd, dc);
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
        RemoveWindowSubclass(hwnd, Some(control_proc), ID);
        return DefSubclassProc(hwnd, msg, w, l);
    }
    if let Some(r) = parent_message(hwnd, msg, w, l) {
        return r;
    }
    let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
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
        if msg == WM_PAINT {
            EndPaint(hwnd, &ps);
        }
        return 0;
    }
    if msg == WM_ERASEBKGND && (cb || push || kind == 2 || kind == 4) {
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
        if !printing {
            ReleaseDC(hwnd, dc);
        }
        if kind == 3 && !printing {
            scroll_bar(hwnd);
        }
        return 0;
    }
    if msg == WM_MOUSEMOVE && (kind == 1 || kind == 2) {
        let mut track: TRACKMOUSEEVENT = std::mem::zeroed();
        track.cbSize = std::mem::size_of::<TRACKMOUSEEVENT>() as u32;
        track.dwFlags = TME_LEAVE;
        track.hwndTrack = hwnd;
        TrackMouseEvent(&mut track);
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
    if matches!(
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
    ) {
        InvalidateRect(hwnd, null(), 0);
        // Native buttons repaint synchronously when their label or enabled state
        // changes. Restore our palette before returning to the caller.
        if kind == 1 && matches!(msg, WM_SETTEXT | WM_ENABLE | BM_SETSTATE) {
            UpdateWindow(hwnd);
        }
    }
    result
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
        if m.CtlType == ODT_COMBOBOX {
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
        if d.CtlType == ODT_COMBOBOX || d.CtlType == ODT_MENU {
            let selected = d.itemState & ODS_SELECTED != 0;
            let disabled = d.itemState & ODS_DISABLED != 0;
            fill(
                d.hDC,
                &d.rcItem,
                if selected { p.selected } else { p.surface },
            );
            let value = if d.CtlType == ODT_MENU {
                MENUS.with(|m| m.borrow().get(&d.itemID).cloned().unwrap_or_default())
            } else if d.itemID == u32::MAX {
                String::new()
            } else {
                let len = SendMessageW(d.hwndItem, CB_GETLBTEXTLEN, d.itemID as usize, 0);
                if !(0..32768).contains(&len) {
                    String::new()
                } else {
                    let mut b = vec![0u16; len as usize + 1];
                    SendMessageW(
                        d.hwndItem,
                        CB_GETLBTEXT,
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
                if disabled {
                    p.disabled_text
                } else if selected && p.kind == a::Kind::Contrast {
                    p.primary_text
                } else {
                    p.text
                },
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            );
            if d.itemState & ODS_FOCUS != 0 {
                DrawFocusRect(d.hDC, &r);
            }
            return Some(1);
        }
    }
    None
}
