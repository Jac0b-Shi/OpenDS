//! OpenDS：蓝牙 DualSense 与本机虚拟 USB DualSense 之间的桥接。
#![cfg_attr(
    all(windows, not(debug_assertions), not(test)),
    windows_subsystem = "windows"
)]
mod attach;
mod audio;
mod audio_test;
mod audio_test_source;
mod audio_test_view;
mod audio_thread;
mod bridge;
mod config;
mod dependencies;
mod gui;
mod gui_close;
mod gui_theme;
mod hid;
mod hidhide;
mod logging;
mod microphone;
mod monitor;
mod monitor_recording;
mod monitor_view;
#[cfg(not(feature = "bundled-opus"))]
mod opus;
#[cfg(feature = "bundled-opus")]
#[path = "opus_bundled.rs"]
mod opus;
mod output;
mod persistence;
mod report;
mod startup;
mod viiper;

use std::{
    sync::{atomic::Ordering, Arc, RwLock},
    time::Duration,
};
static CONSOLE_STATE: RwLock<Option<Arc<bridge::BridgeState>>> = RwLock::new(None);
#[link(name = "kernel32")]
extern "system" {
    fn SetConsoleCtrlHandler(handler: Option<extern "system" fn(u32) -> i32>, add: i32) -> i32;
}
extern "system" fn console_stop(kind: u32) -> i32 {
    if !matches!(kind, 0 | 1 | 2 | 5 | 6) {
        return 0;
    }
    if let Ok(g) = CONSOLE_STATE.try_read() {
        if let Some(s) = g.as_ref() {
            s.stop.store(true, Ordering::Release);
        }
    }
    if matches!(kind, 2 | 5 | 6) {
        for _ in 0..40 {
            let done = CONSOLE_STATE
                .try_read()
                .map(|g| {
                    g.as_ref()
                        .is_none_or(|s| !s.running.load(Ordering::Acquire))
                })
                .unwrap_or(false);
            if done {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    1
}
fn cli() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("gui");
    match command {
        "startup" => {
            match args.get(2).map(String::as_str) {
                Some("enable") => startup::set_enabled(true)?,
                Some("disable") => startup::set_enabled(false)?,
                Some("status") | None => {}
                _ => return Err("使用 startup enable / disable / status".into()),
            }
            println!(
                "Windows 登录自启：{}",
                if startup::enabled()? {
                    "开启"
                } else {
                    "关闭"
                }
            );
            return Ok(());
        }
        "--help" | "-h" | "help" => {
            println!("OpenDS\n  opends gui                 设置与托盘（默认）\n  opends bridge [秒数]        前台桥接；省略或 0 表示持续运行，Ctrl+C 停止\n  opends restore             恢复未完成的 HidHide 会话\n  opends doctor              依赖诊断（只读）\n  opends list                枚举物理蓝牙 DualSense\n  opends probe               读取一帧输入（不写设备）");
            return Ok(());
        }
        "doctor" => {
            println!("{}", dependencies::summary());
            return Ok(());
        }
        "ui-smoke-test" => {
            gui::smoke_test()?;
            println!("GUI 创建、图标加载、事件循环和正常退出检查通过");
            return Ok(());
        }
        "list" | "probe" => {
            for p in hid::HidDevice::enum_paths() {
                if let Some(d) = hid::HidDevice::open(&p) {
                    if d.is_dualsense_bluetooth() {
                        println!(
                            "PID={:04X} IN={} OUT={}\n{}",
                            d.product_id, d.input_len, d.output_len, p
                        );
                        if command == "probe" {
                            match d.read_result(500) {
                                Ok(Some(b)) => {
                                    println!("{:02X?}", b);
                                    if let Some(s) =
                                        report::InputReport::new(b).and_then(|r| r.parse())
                                    {
                                        println!("{s:?}");
                                    }
                                }
                                Ok(None) => println!("500ms 内无输入"),
                                Err(e) => return Err(e.to_string()),
                            }
                        }
                    }
                }
            }
            return Ok(());
        }
        "gui" | "tray" | "bridge" | "restore" => {}
        _ => return Err(format!("未知命令：{command}；使用 opends --help")),
    }
    let _instance = match persistence::InstanceGuard::acquire() {
        Err(e) if command == "tray" && e.starts_with("OpenDS 已在运行") => return Ok(()),
        result => result?,
    };
    let recovery = hidhide::recover_if_needed();
    if command == "restore" {
        match recovery? {
            Some(_) => println!("HidHide 恢复成功且回读一致"),
            None => println!("没有待恢复的 HidHide 会话"),
        };
        return Ok(());
    }
    let path = config::Config::path();
    let cfg = config::Config::load(&path)?;
    if command == "gui" || command == "tray" {
        return gui::run(cfg, path, recovery.err(), command == "tray");
    }
    recovery?;
    let limit = match args.get(2) {
        Some(v) => {
            let secs = v.parse::<u64>().map_err(|_| "秒数必须为非负整数")?;
            (secs > 0).then_some(Duration::from_secs(secs))
        }
        None => None,
    };
    let mut controller = bridge::BridgeController::default();
    controller.start(cfg, limit)?;
    *CONSOLE_STATE.write().map_err(|_| "控制台状态锁损坏")? = Some(controller.state.clone());
    unsafe {
        SetConsoleCtrlHandler(Some(console_stop), 1);
    }
    controller.wait();
    unsafe {
        SetConsoleCtrlHandler(Some(console_stop), 0);
    }
    *CONSOLE_STATE.write().map_err(|_| "控制台状态锁损坏")? = None;
    let error = controller
        .state
        .last_error
        .lock()
        .ok()
        .and_then(|e| e.clone());
    error.map_or(Ok(()), Err)
}
fn main() {
    let gui = std::env::args()
        .nth(1)
        .is_none_or(|s| s == "gui" || s == "tray");
    #[cfg(all(windows, not(debug_assertions), not(test)))]
    if !gui {
        // 保留调用方的重定向句柄；GUI 子系统启动时不弹出控制台。
        unsafe {
            extern "system" {
                fn AttachConsole(id: u32) -> i32;
                fn GetStdHandle(kind: u32) -> isize;
                fn SetStdHandle(kind: u32, handle: isize) -> i32;
            }
            let kinds = [-10i32 as u32, -11i32 as u32, -12i32 as u32];
            let original = kinds.map(|k| GetStdHandle(k));
            AttachConsole(u32::MAX);
            for (kind, handle) in kinds.into_iter().zip(original) {
                if handle != 0 && handle != -1 {
                    SetStdHandle(kind, handle);
                }
            }
        }
    }
    if let Err(e) = cli() {
        eprintln!("OpenDS：{e}");
        if gui {
            native_windows_gui::simple_message("OpenDS", &e);
        }
        std::process::exit(1);
    }
}
