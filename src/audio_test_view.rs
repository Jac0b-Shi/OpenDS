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
            wc.hbrBackground = GetStockObject(WHITE_BRUSH as i32) as _;
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
            let dpi = GetDpiForWindow(parent).max(96);
            let p = |v| crate::gui_theme::px(v, dpi);
            s.hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME,
                class.as_ptr(),
                wide("OpenDS · 麦克风测试").as_ptr(),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                p(570),
                p(320),
                parent,
                null_mut(),
                wc.hInstance,
                null_mut(),
            );
            if s.hwnd.is_null() {
                return Err("无法打开麦克风测试窗口".into());
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
                wide("Microsoft YaHei UI").as_ptr(),
            );
            for (i, (class, text, x, y, w, h)) in [
                (
                    "STATIC",
                    "倒数 3 秒后录音 5 秒，可分别回听蓝牙端和 USB 端。\r\n录音仅暂存在内存，关闭窗口即清除。",
                    20,
                    22,
                    520,
                    38,
                ),
                ("BUTTON", "录音 5 秒", 20, 72, 110, 34),
                ("BUTTON", "取消", 138, 72, 65, 34),
                ("BUTTON", "回听 USB 端", 319, 72, 110, 34),
                ("BUTTON", "清除录音", 437, 72, 100, 34),
                (
                    "STATIC",
                    "回听使用系统默认播放设备。关闭此窗口会清除录音。",
                    20,
                    130,
                    520,
                    110,
                ),
                ("BUTTON", "回听蓝牙端", 211, 72, 100, 34),
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
            wide("Microsoft YaHei UI").as_ptr(),
        );
        for (control, (x, y, w, h)) in self.controls.iter().zip([
            (20, 22, 520, 38),
            (20, 72, 110, 34),
            (138, 72, 65, 34),
            (319, 72, 110, 34),
            (437, 72, 100, 34),
            (20, 130, 520, 110),
            (211, 72, 100, 34),
        ]) {
            SendMessageW(*control, WM_SETFONT, font as usize, 1);
            MoveWindow(*control, p(x), p(y), p(w), p(h), 1);
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
            wide(&format!(
                "{}\r\n蓝牙 Opus {} 包 · 解码 {} 样本 · USB 接收 {} 帧\r\nUSB 欠载 {} 次 · 丢弃 {} ms · 当前缓冲 {}/{} ms",
                status.message,
                self.monitor.mic_packets.load(Relaxed),
                self.monitor.mic_samples.load(Relaxed),
                self.monitor.mic_usb_frames.load(Relaxed),
                self.monitor.mic_underruns.load(Relaxed),
                self.monitor.mic_dropped.load(Relaxed) / 48,
                self.monitor.mic_buffered.load(Relaxed) / 48,
                self.monitor.microphone_buffer_ms.load(Relaxed)
            ))
            .as_ptr(),
        );
        if !self.monitor.connected.load(Relaxed) {
            SetWindowTextW(self.controls[5],wide("请先连接蓝牙手柄并启动桥接。\r\n回听使用系统默认播放设备。关闭此窗口会清除录音。").as_ptr());
        } else if !self.monitor.microphone.load(Relaxed) {
            SetWindowTextW(self.controls[5],wide("请先在连接与反馈中启用麦克风转发，保存并重启桥接。\r\n回听使用系统默认播放设备。关闭此窗口会清除录音。").as_ptr());
        }
    }
}
unsafe extern "system" fn proc(hwnd: HWND, msg: UINT, w: WPARAM, l: LPARAM) -> LRESULT {
    if matches!(msg, WM_CTLCOLORSTATIC | WM_CTLCOLORBTN) {
        SetBkColor(w as HDC, 0xffffff);
        SetTextColor(w as HDC, 0x302217);
        return GetStockObject(WHITE_BRUSH as i32) as LRESULT;
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
