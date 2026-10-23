//! Owned Win32 dialogs share the application palette, keyboard semantics and DPI.
use crate::appearance::{self, wide};
use std::ptr::{null, null_mut};
use winapi::{
    shared::{minwindef::*, windef::*},
    um::{libloaderapi::GetModuleHandleW, wingdi::*, winuser::*},
};
pub struct Request<'a> {
    pub title: &'a str,
    pub message: &'a str,
    pub buttons: &'a [(i32, &'a str)],
    pub default: i32,
    pub cancel: i32,
    pub verification: Option<&'a str>,
    pub test: Option<(i32, bool)>,
}
#[derive(Debug)]
pub struct Response {
    pub button: i32,
    pub checked: bool,
}
struct State {
    result: Option<i32>,
    cancel: i32,
    default: i32,
    verify: HWND,
    buttons: Vec<(i32, HWND, String)>,
}
unsafe extern "system" fn proc(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_ERASEBKGND {
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        FillRect(w as HDC, &r, appearance::surface_brush());
        return 1;
    }
    if !matches!(msg, WM_COMMAND | WM_CLOSE) {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let s = &mut *ptr;
    if msg == WM_CLOSE {
        s.result = Some(s.cancel);
        return 0;
    }
    let id = (w & 65535) as i32;
    if s.buttons.iter().any(|(key, _, _)| *key == id) {
        s.result = Some(id);
    } else if id == IDCANCEL {
        s.result = Some(s.cancel);
    } else if id == IDOK {
        s.result = Some(s.default);
    }
    0
}
pub fn show(parent: HWND, request: Request<'_>) -> Result<Response, String> {
    unsafe {
        let instance = GetModuleHandleW(null());
        let name = wide("OpenDSThemedDialog");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.hInstance = instance;
        wc.lpszClassName = name.as_ptr();
        wc.lpfnWndProc = Some(proc);
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        RegisterClassW(&wc);
        let dpi = crate::gui_theme::window_dpi(parent);
        let p = |x| crate::gui_theme::px(x, dpi);
        let font = CreateFontW(
            -p(14),
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide(crate::gui_theme::font_family()).as_ptr(),
        );
        if font.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let dc = GetDC(parent);
        let old = SelectObject(dc, font as _);
        let mut mr = RECT {
            left: 0,
            top: 0,
            right: p(490),
            bottom: 0,
        };
        DrawTextW(
            dc,
            wide(request.message).as_ptr(),
            -1,
            &mut mr,
            DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
        );
        let message_h = (mr.bottom - mr.top).max(p(30));
        let widths: Vec<_> = request
            .buttons
            .iter()
            .map(|(_, label)| {
                let mut r: RECT = std::mem::zeroed();
                DrawTextW(
                    dc,
                    wide(label).as_ptr(),
                    -1,
                    &mut r,
                    DT_CALCRECT | DT_SINGLELINE,
                );
                (r.right + p(32)).max(p(96))
            })
            .collect();
        SelectObject(dc, old);
        ReleaseDC(parent, dc);
        let width = p(538).max(
            widths.iter().sum::<i32>() + p(48) + p(10) * (widths.len().saturating_sub(1) as i32),
        );
        let verify_h = if request.verification.is_some() {
            p(40)
        } else {
            0
        };
        let height = p(24) + message_h + p(24) + verify_h + p(38) + p(24);
        let mut frame = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        AdjustWindowRectEx(
            &mut frame,
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            0,
            WS_EX_DLGMODALFRAME,
        );
        let mut pr: RECT = std::mem::zeroed();
        if parent.is_null() {
            SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut pr as *mut _ as _, 0);
        } else {
            GetWindowRect(parent, &mut pr);
        }
        let x = pr.left + (pr.right - pr.left - (frame.right - frame.left)) / 2;
        let y = pr.top + (pr.bottom - pr.top - (frame.bottom - frame.top)) / 2;
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            name.as_ptr(),
            wide(request.title).as_ptr(),
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
            x.max(0),
            y.max(0),
            frame.right - frame.left,
            frame.bottom - frame.top,
            parent,
            null_mut(),
            instance,
            null_mut(),
        );
        if hwnd.is_null() {
            DeleteObject(font as _);
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut state = Box::new(State {
            result: None,
            cancel: request.cancel,
            default: request.default,
            verify: null_mut(),
            buttons: vec![],
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *state as *mut State as isize);
        let content = CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide(request.message).as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_LEFT | SS_NOPREFIX,
            p(24),
            p(24),
            width - p(48),
            message_h,
            hwnd,
            null_mut(),
            instance,
            null_mut(),
        );
        SendMessageW(content, WM_SETFONT, font as usize, 0);
        if let Some(label) = request.verification {
            state.verify = CreateWindowExW(
                0,
                wide("BUTTON").as_ptr(),
                wide(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX,
                p(24),
                p(24) + message_h + p(16),
                width - p(48),
                p(28),
                hwnd,
                0x7001 as HMENU,
                instance,
                null_mut(),
            );
            SendMessageW(state.verify, WM_SETFONT, font as usize, 0);
        }
        let mut xx = width
            - p(24)
            - widths.iter().sum::<i32>()
            - p(10) * (widths.len().saturating_sub(1) as i32);
        for ((id, label), bw) in request.buttons.iter().zip(widths) {
            let button = CreateWindowExW(
                0,
                wide("BUTTON").as_ptr(),
                wide(label).as_ptr(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | if *id == request.default {
                        BS_DEFPUSHBUTTON
                    } else {
                        BS_PUSHBUTTON
                    },
                xx,
                height - p(62),
                bw,
                p(38),
                hwnd,
                *id as HMENU,
                instance,
                null_mut(),
            );
            SendMessageW(button, WM_SETFONT, font as usize, 0);
            state.buttons.push((*id, button, (*label).into()));
            xx += bw + p(10);
        }
        appearance::install(hwnd);
        let was_enabled = !parent.is_null() && IsWindowEnabled(parent) != 0;
        if was_enabled {
            EnableWindow(parent, 0);
        }
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        if let Some((_, button, _)) = state
            .buttons
            .iter()
            .find(|(id, _, _)| *id == request.default)
        {
            SetFocus(*button);
        }
        if let Some((id, checked)) = request.test {
            if checked && !state.verify.is_null() {
                SendMessageW(state.verify, BM_SETCHECK, BST_CHECKED as usize, 0);
            }
            PostMessageW(hwnd, WM_COMMAND, id as usize, 0);
        }
        let mut quit = None;
        let mut msg: MSG = std::mem::zeroed();
        while state.result.is_none() {
            let ok = GetMessageW(&mut msg, null_mut(), 0, 0);
            if ok <= 0 {
                if ok == 0 {
                    quit = Some(msg.wParam);
                }
                break;
            }
            if msg.message == WM_KEYDOWN && msg.wParam == VK_ESCAPE as usize {
                state.result = Some(request.cancel);
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam == VK_RETURN as usize {
                state.result = Some(
                    state
                        .buttons
                        .iter()
                        .find(|(_, h, _)| *h == GetFocus())
                        .map_or(request.default, |(id, _, _)| *id),
                );
                continue;
            }
            if msg.message == WM_SYSCHAR {
                let c = char::from_u32(msg.wParam as u32)
                    .unwrap_or('\0')
                    .to_ascii_lowercase();
                if let Some((id, _, _)) = state.buttons.iter().find(|(_, _, label)| {
                    label
                        .split_once('&')
                        .and_then(|(_, s)| s.chars().next())
                        .is_some_and(|key| key.to_ascii_lowercase() == c)
                }) {
                    state.result = Some(*id);
                    continue;
                }
            }
            if IsDialogMessageW(hwnd, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let checked = !state.verify.is_null()
            && SendMessageW(state.verify, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
        let response = Response {
            button: state.result.unwrap_or(request.cancel),
            checked,
        };
        if was_enabled {
            EnableWindow(parent, 1);
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        DestroyWindow(hwnd);
        DeleteObject(font as _);
        if was_enabled {
            SetActiveWindow(parent);
        }
        if let Some(code) = quit {
            PostQuitMessage(code as i32);
        }
        Ok(response)
    }
}
pub fn message(parent: HWND, title: &str, message: &str) {
    if show(
        parent,
        Request {
            title,
            message,
            buttons: &[(IDOK, crate::tr!(dialog_ok))],
            default: IDOK,
            cancel: IDOK,
            verification: None,
            test: None,
        },
    )
    .is_err()
    {
        unsafe {
            MessageBoxW(
                parent,
                wide(message).as_ptr(),
                wide(title).as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
}
