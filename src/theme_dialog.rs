//! Owned Win32 dialogs share the application palette, keyboard semantics and DPI.
use crate::appearance::{self, wide};
use std::ptr::{null, null_mut};
use winapi::{
    shared::{minwindef::*, windef::*},
    um::{libloaderapi::GetModuleHandleW, wingdi::*, winuser::*},
};
#[derive(Clone, Copy)]
pub enum TestAction {
    Select(i32, bool),
    Key(u32),
    Accelerator(char),
    Dpi(u32),
    EndSession,
}
pub struct Request<'a> {
    pub title: &'a str,
    pub message: &'a str,
    pub buttons: &'a [(i32, &'a str)],
    pub default: i32,
    pub cancel: i32,
    pub verification: Option<&'a str>,
    pub test: Option<TestAction>,
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
    content: HWND,
    font: HFONT,
    dpi: u32,
}
unsafe fn make_font(dpi: u32) -> HFONT {
    CreateFontW(
        -crate::gui_theme::px(14, dpi),
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
    )
}
unsafe fn rescale(hwnd: HWND, state: *mut State, dpi: u32, suggested: RECT) {
    let dpi = dpi.max(96);
    let old_dpi = (*state).dpi;
    if dpi != old_dpi {
        let font = make_font(dpi);
        if font.is_null() {
            return;
        }
        let old_font = (*state).font;
        let children: Vec<_> = std::iter::once((*state).content)
            .chain(std::iter::once((*state).verify))
            .chain((*state).buttons.iter().map(|(_, h, _)| *h))
            .filter(|h| !h.is_null())
            .map(|h| {
                let mut rect: RECT = std::mem::zeroed();
                GetWindowRect(h, &mut rect);
                MapWindowPoints(null_mut(), hwnd, &mut rect as *mut _ as *mut POINT, 2);
                (h, rect)
            })
            .collect();
        // Publish state before native calls that can synchronously re-enter the
        // window procedure; no Rust mutable reference survives those calls.
        (*state).dpi = dpi;
        (*state).font = font;
        let scale = |v: i32| ((v as i64 * dpi as i64 + old_dpi as i64 / 2) / old_dpi as i64) as i32;
        for (child, rect) in children {
            SendMessageW(child, WM_SETFONT, font as usize, 1);
            SetWindowPos(
                child,
                null_mut(),
                scale(rect.left),
                scale(rect.top),
                scale(rect.right - rect.left),
                scale(rect.bottom - rect.top),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        DeleteObject(old_font as _);
    }
    SetWindowPos(
        hwnd,
        null_mut(),
        suggested.left,
        suggested.top,
        suggested.right - suggested.left,
        suggested.bottom - suggested.top,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
    InvalidateRect(hwnd, null(), 1);
}
unsafe extern "system" fn proc(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_ERASEBKGND {
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        FillRect(w as HDC, &r, appearance::surface_brush());
        return 1;
    }
    if !matches!(msg, WM_COMMAND | WM_CLOSE | WM_DPICHANGED) {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if msg == WM_DPICHANGED {
        if l != 0 {
            rescale(hwnd, ptr, (w & 65535) as u32, *(l as *const RECT));
        }
        return 0;
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
        let anchor = if !parent.is_null() && IsWindow(parent) != 0 {
            parent
        } else {
            GetForegroundWindow()
        };
        let mut monitor: MONITORINFO = std::mem::zeroed();
        monitor.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        let mut work: RECT = std::mem::zeroed();
        if GetMonitorInfoW(
            MonitorFromWindow(anchor, MONITOR_DEFAULTTOPRIMARY),
            &mut monitor,
        ) != 0
        {
            work = monitor.rcWork;
        } else {
            SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work as *mut _ as _, 0);
        }
        let mut pr = work;
        if !parent.is_null() {
            GetWindowRect(parent, &mut pr);
        }
        // Create hidden on the target monitor first. Startup/second-instance
        // errors have no parent HWND from which a DPI could be inherited.
        let hwnd = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            name.as_ptr(),
            wide(request.title).as_ptr(),
            WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
            work.left + (work.right - work.left) / 2,
            work.top + (work.bottom - work.top) / 2,
            1,
            1,
            parent,
            null_mut(),
            instance,
            null_mut(),
        );
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let dpi = crate::gui_theme::window_dpi(hwnd);
        let p = |x| crate::gui_theme::px(x, dpi);
        let font = make_font(dpi);
        if font.is_null() {
            let error = std::io::Error::last_os_error().to_string();
            DestroyWindow(hwnd);
            return Err(error);
        }
        let dc = GetDC(hwnd);
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
        ReleaseDC(hwnd, dc);
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
        AdjustWindowRectExForDpi(
            &mut frame,
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            0,
            WS_EX_DLGMODALFRAME,
            dpi,
        );
        let x = pr.left + (pr.right - pr.left - (frame.right - frame.left)) / 2;
        let y = pr.top + (pr.bottom - pr.top - (frame.bottom - frame.top)) / 2;
        let outer_w = frame.right - frame.left;
        let outer_h = frame.bottom - frame.top;
        SetWindowPos(
            hwnd,
            null_mut(),
            x.max(work.left).min((work.right - outer_w).max(work.left)),
            y.max(work.top).min((work.bottom - outer_h).max(work.top)),
            outer_w,
            outer_h,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let mut state = Box::new(State {
            result: None,
            cancel: request.cancel,
            default: request.default,
            verify: null_mut(),
            buttons: vec![],
            content: null_mut(),
            font,
            dpi,
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
        state.content = content;
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
        if request.test.is_none() {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
            if let Some((_, button, _)) = state
                .buttons
                .iter()
                .find(|(id, _, _)| *id == request.default)
            {
                SetFocus(*button);
            }
        }
        let mut validation_error = None;
        if let Some(action) = request.test {
            match action {
                TestAction::Select(id, checked) => {
                    if checked && !state.verify.is_null() {
                        SendMessageW(state.verify, BM_SETCHECK, BST_CHECKED as usize, 0);
                    }
                    PostMessageW(hwnd, WM_COMMAND, id as usize, 0);
                }
                TestAction::Key(key) => {
                    PostMessageW(hwnd, WM_KEYDOWN, key as usize, 0);
                }
                TestAction::Accelerator(key) => {
                    PostMessageW(hwnd, WM_SYSCHAR, key as usize, 0);
                }
                TestAction::EndSession => {
                    PostMessageW(parent, WM_ENDSESSION, 1, 0);
                }
                TestAction::Dpi(next) => {
                    let mut font_info: LOGFONTW = std::mem::zeroed();
                    GetObjectW(
                        state.font as _,
                        std::mem::size_of::<LOGFONTW>() as i32,
                        &mut font_info as *mut _ as _,
                    );
                    if font_info.lfHeight
                        != -crate::gui_theme::px(14, crate::gui_theme::window_dpi(hwnd))
                    {
                        validation_error =
                            Some("Initial dialog font does not match its own monitor DPI");
                    }
                    if !state.verify.is_null() {
                        SendMessageW(state.verify, BM_SETCHECK, BST_CHECKED as usize, 0);
                    }
                    let mut r: RECT = std::mem::zeroed();
                    GetWindowRect(hwnd, &mut r);
                    r.right = r.left + (r.right - r.left) * next as i32 / state.dpi as i32;
                    r.bottom = r.top + (r.bottom - r.top) * next as i32 / state.dpi as i32;
                    SendMessageW(
                        hwnd,
                        WM_DPICHANGED,
                        (next | (next << 16)) as usize,
                        &r as *const _ as isize,
                    );
                    GetObjectW(
                        state.font as _,
                        std::mem::size_of::<LOGFONTW>() as i32,
                        &mut font_info as *mut _ as _,
                    );
                    if font_info.lfHeight != -crate::gui_theme::px(14, next) {
                        validation_error = Some("Dialog font did not follow WM_DPICHANGED");
                    }
                    let mut button: RECT = std::mem::zeroed();
                    GetWindowRect(state.buttons[0].1, &mut button);
                    if (button.bottom - button.top - crate::gui_theme::px(38, next)).abs() > 1 {
                        validation_error = Some("Dialog button did not follow WM_DPICHANGED");
                    }
                    PostMessageW(hwnd, WM_COMMAND, request.default as usize, 0);
                }
            }
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
        DeleteObject(state.font as _);
        if was_enabled {
            SetActiveWindow(parent);
        }
        if let Some(code) = quit {
            PostQuitMessage(code as i32);
        }
        if let Some(error) = validation_error {
            return Err(error.into());
        }
        Ok(response)
    }
}
pub fn smoke_unowned() -> Result<(), String> {
    for dpi in [96, 144, 192] {
        let response = show(null_mut(), Request {
            title: "OpenDS · DPI test",
            message: "OpenDS 已在运行；请从现有托盘实例操作。\nOpenDS is already running. Use the existing tray instance.",
            buttons: &[(IDOK, "确定(&O) / &OK"), (IDCANCEL, "稍后(&L) / &Later")],
            default: IDOK,
            cancel: IDCANCEL,
            verification: Some("Remember this choice"),
            test: Some(TestAction::Dpi(dpi)),
        })?;
        if response.button != IDOK || !response.checked {
            return Err("DPI change altered the dialog result or checkbox".into());
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parentless_startup_dialog_and_dpi_changes_use_scaled_fonts_and_buttons() {
        struct Context(DPI_AWARENESS_CONTEXT);
        impl Drop for Context {
            fn drop(&mut self) {
                unsafe {
                    SetThreadDpiAwarenessContext(self.0);
                }
            }
        }
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        assert!(!previous.is_null());
        let _context = Context(previous);
        smoke_unowned().unwrap();
    }
}
