use crate::{audio_test::Test, monitor::Monitor};
use std::{
    cell::RefCell,
    ptr::{null, null_mut},
    sync::{atomic::Ordering::Relaxed, Arc},
};
use winapi::{
    shared::{minwindef::*, windef::*},
    um::{libloaderapi::GetModuleHandleW, wingdi::*, winuser::*},
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct State {
    hwnd: HWND,
    controls: [HWND; 7],
    font: HFONT,
    test: Test,
    monitor: Arc<Monitor>,
}
pub struct Dialog {
    state: Box<RefCell<State>>,
}
impl Dialog {
    pub fn new(parent: HWND, monitor: Arc<Monitor>) -> Result<Self, String> {
        unsafe {
            let class = wide("OpenDSAudioTest");
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.hInstance = GetModuleHandleW(null());
            wc.lpfnWndProc = Some(proc);
            wc.lpszClassName = class.as_ptr();
            wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
            wc.hbrBackground = null_mut();
            RegisterClassW(&wc);
            let state = Box::new(RefCell::new(State {
                hwnd: null_mut(),
                controls: [null_mut(); 7],
                font: null_mut(),
                test: Test::with_source(monitor.mic_test_capture.clone()),
                monitor,
            }));
            let ptr = &*state as *const RefCell<State>;
            let mut s = state.borrow_mut();
            let dpi = crate::gui_theme::window_dpi(parent).max(96);
            let p = |v| crate::gui_theme::px(v, dpi);
            s.hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME,
                class.as_ptr(),
                wide(crate::tr!(audio_test_view_opends_microphone_test)).as_ptr(),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                p(570),
                p(360),
                parent,
                null_mut(),
                wc.hInstance,
                null_mut(),
            );
            if s.hwnd.is_null() {
                return Err(
                    crate::tr!(audio_test_view_could_not_create_microphone_test_window).into(),
                );
            }
            SetWindowLongPtrW(s.hwnd, GWLP_USERDATA, ptr as isize);
            s.font = CreateFontW(
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
            for (i, (class, text, x, y, w, h)) in [
                (
                    "STATIC",
                    crate::tr!(audio_test_view_record_5_seconds_after_a_3_second),
                    20,
                    22,
                    520,
                    38,
                ),
                (
                    "BUTTON",
                    crate::tr!(audio_test_view_record_5_seconds),
                    20,
                    72,
                    110,
                    34,
                ),
                (
                    "BUTTON",
                    crate::tr!(audio_test_view_cancel),
                    138,
                    72,
                    65,
                    34,
                ),
                (
                    "BUTTON",
                    crate::tr!(audio_test_view_play_usb_capture),
                    319,
                    72,
                    110,
                    34,
                ),
                (
                    "BUTTON",
                    crate::tr!(audio_test_view_clear_recording),
                    437,
                    72,
                    100,
                    34,
                ),
                (
                    "STATIC",
                    crate::tr!(audio_test_view_playback_uses_the_default_system_playback_device),
                    20,
                    130,
                    520,
                    110,
                ),
                (
                    "BUTTON",
                    crate::tr!(audio_test_view_play_bluetooth_source),
                    211,
                    72,
                    100,
                    34,
                ),
            ]
            .into_iter()
            .enumerate()
            {
                s.controls[i] = CreateWindowExW(
                    0,
                    wide(class).as_ptr(),
                    wide(text).as_ptr(),
                    WS_CHILD
                        | WS_VISIBLE
                        | if class == "BUTTON" {
                            WS_TABSTOP | BS_PUSHBUTTON
                        } else {
                            SS_LEFT
                        },
                    p(x),
                    p(y),
                    p(w),
                    p(h),
                    s.hwnd,
                    (i + 1) as HMENU,
                    wc.hInstance,
                    null_mut(),
                );
                SendMessageW(s.controls[i], WM_SETFONT, s.font as usize, 1);
            }
            for kind in [ICON_SMALL, ICON_BIG] {
                SendMessageW(
                    s.hwnd,
                    WM_SETICON,
                    kind as usize,
                    SendMessageW(parent, WM_GETICON, kind as usize, 0),
                );
            }
            s.layout(dpi);
            crate::appearance::install(s.hwnd);
            s.refresh();
            drop(s);
            Ok(Self { state })
        }
    }
    pub fn show(&self) {
        unsafe {
            let s = self.state.borrow();
            ShowWindow(s.hwnd, SW_SHOW);
            SetTimer(s.hwnd, 1, 100, None);
        }
    }
    pub fn hide(&self) {
        let mut s = self.state.borrow_mut();
        s.test.cancel(true);
        unsafe {
            KillTimer(s.hwnd, 1);
            ShowWindow(s.hwnd, SW_HIDE);
        }
    }
}
impl Drop for Dialog {
    fn drop(&mut self) {
        unsafe {
            let s = self.state.get_mut();
            s.test.cancel(true);
            SetWindowLongPtrW(s.hwnd, GWLP_USERDATA, 0);
            KillTimer(s.hwnd, 1);
            DestroyWindow(s.hwnd);
            DeleteObject(s.font as _);
        }
    }
}
impl State {
    unsafe fn layout(&mut self, dpi: u32) {
        let p = |v| crate::gui_theme::px(v, dpi);
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
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(self.hwnd, &mut client);
        let width = (client.right - p(40)).max(p(200));
        for control in self.controls {
            SendMessageW(control, WM_SETFONT, font as usize, 1);
        }
        MoveWindow(self.controls[0], p(20), p(22), width, p(42), 1);
        let dc = GetDC(self.hwnd);
        let old = SelectObject(dc, font as _);
        let mut x = p(20);
        let mut y = p(76);
        for index in [1, 2, 6, 3, 4] {
            let control = self.controls[index];
            let length = GetWindowTextLengthW(control).max(0) as usize;
            let mut label = vec![0u16; length + 1];
            GetWindowTextW(control, label.as_mut_ptr(), label.len() as i32);
            let mut measured: RECT = std::mem::zeroed();
            DrawTextW(
                dc,
                label.as_ptr(),
                -1,
                &mut measured,
                DT_CALCRECT | DT_SINGLELINE,
            );
            let button_width = (measured.right + p(28)).max(p(80)).min(width);
            if x > p(20) && x + button_width > p(20) + width {
                x = p(20);
                y += p(44);
            }
            MoveWindow(control, x, y, button_width, p(34), 1);
            x += button_width + p(10);
        }
        SelectObject(dc, old);
        ReleaseDC(self.hwnd, dc);
        let status_y = y + p(58);
        MoveWindow(self.controls[5], p(20), status_y, width, p(110), 1);
        let wanted = (status_y + p(132)).max(p(310));
        if client.bottom < wanted {
            let mut window: RECT = std::mem::zeroed();
            GetWindowRect(self.hwnd, &mut window);
            SetWindowPos(
                self.hwnd,
                null_mut(),
                0,
                0,
                window.right - window.left,
                wanted + (window.bottom - window.top - client.bottom),
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        DeleteObject(self.font as _);
        self.font = font;
    }
    unsafe fn refresh(&mut self) {
        let status = self.test.snapshot();
        EnableWindow(
            self.controls[1],
            (!status.busy
                && self.monitor.microphone.load(Relaxed)
                && self.monitor.connected.load(Relaxed)) as i32,
        );
        EnableWindow(self.controls[2], status.busy as i32);
        EnableWindow(self.controls[3], (!status.busy && status.recorded) as i32);
        EnableWindow(
            self.controls[4],
            (!status.busy && (status.recorded || status.source_recorded)) as i32,
        );
        EnableWindow(
            self.controls[6],
            (!status.busy && status.source_recorded) as i32,
        );
        SetWindowTextW(
            self.controls[5],
            wide(&crate::tr!(
                audio_test_view_bluetooth_opus_packets_decoded_samples_usb_frames,
                status = crate::i18n_arg!(status.message),
                packets = crate::i18n_arg!(self.monitor.mic_packets.load(Relaxed)),
                samples = crate::i18n_arg!(self.monitor.mic_samples.load(Relaxed)),
                frames = crate::i18n_arg!(self.monitor.mic_usb_frames.load(Relaxed)),
                underruns = crate::i18n_arg!(self.monitor.mic_underruns.load(Relaxed)),
                dropped = crate::i18n_arg!(self.monitor.mic_dropped.load(Relaxed) / 48),
                buffered = crate::i18n_arg!(self.monitor.mic_buffered.load(Relaxed) / 48),
                target = crate::i18n_arg!(self.monitor.microphone_buffer_ms.load(Relaxed))
            ))
            .as_ptr(),
        );
        if !self.monitor.connected.load(Relaxed) {
            SetWindowTextW(
                self.controls[5],
                wide(crate::tr!(
                    audio_test_view_connect_the_bluetooth_controller_and_start_bridging
                ))
                .as_ptr(),
            );
        } else if !self.monitor.microphone.load(Relaxed) {
            SetWindowTextW(
                self.controls[5],
                wide(crate::tr!(
                    audio_test_view_enable_microphone_forwarding_in_connection_feedback_save
                ))
                .as_ptr(),
            );
        }
    }
}
unsafe extern "system" fn proc(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT {
    if matches!(msg, WM_CTLCOLORSTATIC | WM_CTLCOLORBTN) {
        SetBkColor(w as HDC, crate::appearance::palette().surface);
        SetTextColor(w as HDC, crate::appearance::palette().text);
        return crate::appearance::surface_brush() as LRESULT;
    }
    // DefWindowProcW can pump timers during a native move/size/menu loop.
    if !matches!(msg, WM_CLOSE | WM_COMMAND | WM_TIMER | WM_DPICHANGED) {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<State>;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let Ok(mut s) = (*ptr).try_borrow_mut() else {
        return DefWindowProcW(hwnd, msg, w, l);
    };
    match msg {
        WM_CLOSE => {
            s.test.cancel(true);
            KillTimer(hwnd, 1);
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_COMMAND => {
            match w & 65535 {
                2 => {
                    let _ = s.test.start(true);
                }
                3 => s.test.cancel(false),
                4 => {
                    let _ = s.test.start(false);
                }
                5 => s.test.cancel(true),
                7 => {
                    let _ = s.test.play_source();
                }
                _ => {}
            };
            s.refresh();
            0
        }
        WM_TIMER => {
            s.refresh();
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
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            s.layout((w as u32 & 65535).max(96));
            0
        }
        _ => unreachable!(),
    }
}
