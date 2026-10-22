//! 首次运行诊断；仅查询驱动服务，不修改或安装系统组件。
use std::{ffi::c_void, ptr};
#[repr(C)]
#[derive(Default)]
struct ServiceStatus {
    kind: u32,
    state: u32,
    controls: u32,
    win32_exit: u32,
    specific_exit: u32,
    checkpoint: u32,
    hint: u32,
}
#[link(name = "advapi32")]
extern "system" {
    fn OpenSCManagerW(machine: *const u16, database: *const u16, access: u32) -> *mut c_void;
    fn OpenServiceW(manager: *mut c_void, name: *const u16, access: u32) -> *mut c_void;
    fn QueryServiceStatus(service: *mut c_void, status: *mut ServiceStatus) -> i32;
    fn CloseServiceHandle(handle: *mut c_void) -> i32;
}
pub fn service_running(name: &str) -> bool {
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let manager = OpenSCManagerW(ptr::null(), ptr::null(), 1);
        if manager.is_null() {
            return false;
        }
        let service = OpenServiceW(manager, name.as_ptr(), 4);
        CloseServiceHandle(manager);
        if service.is_null() {
            return false;
        }
        let mut status = ServiceStatus::default();
        let ok = QueryServiceStatus(service, &mut status);
        CloseServiceHandle(service);
        ok != 0 && status.state == 4
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HidHideStatus {
    Accessible,
    InUse,
    Unavailable,
}
fn hidhide_status(owned_healthy: bool, probe: impl FnOnce() -> bool) -> HidHideStatus {
    if owned_healthy {
        HidHideStatus::InUse
    } else if probe() {
        HidHideStatus::Accessible
    } else {
        HidHideStatus::Unavailable
    }
}
pub struct Status {
    usbip_service: bool,
    usbip_tool: bool,
    hidhide: HidHideStatus,
    viiper: bool,
    opus: bool,
}
pub fn inspect(hidhide_owned_healthy: bool) -> Status {
    Status {
        usbip_service: service_running("usbip2_ude"),
        usbip_tool: crate::attach::usbip_path().is_some(),
        hidhide: hidhide_status(hidhide_owned_healthy, || {
            crate::hidhide::HidHide::open()
                .and_then(|h| h.get_active())
                .is_some()
        }),
        viiper: crate::viiper::is_available(),
        opus: crate::opus::is_available(),
    }
}
impl Status {
    pub fn summary(&self) -> String {
        format!(
            "USB/IP Win2 UDE：{}\r\nHidHide：{}\r\nlibVIIPER：{}\r\nOpus：{}\r\n配置：{}",
            if self.usbip_service {
                if self.usbip_tool {
                    "服务与挂接工具可用"
                } else {
                    "服务已运行，缺少 usbip.exe 主程序"
                }
            } else {
                "未运行或未安装，请安装后重启并重新检查"
            },
            match self.hidhide {
                HidHideStatus::InUse => "已启用，正在隐藏物理手柄",
                HidHideStatus::Accessible => "可访问",
                HidHideStatus::Unavailable => "不可访问，请检查驱动或权限",
            },
            if self.viiper {
                "可用"
            } else {
                "缺少配套 DLL，请放到 opends.exe 同目录"
            },
            if self.opus {
                "可用"
            } else {
                "缺少 libopus-0.dll；音频和原生触觉不可用"
            },
            crate::config::Config::path().display()
        )
    }

    pub fn ready(&self) -> bool {
        self.usbip_service
            && self.hidhide != HidHideStatus::Unavailable
            && self.viiper
            && self.opus
            && self.usbip_tool
    }
}
pub fn summary() -> String {
    inspect(false).summary()
}
pub fn ready() -> bool {
    inspect(false).ready()
}

/// One asynchronous installer at a time; the UI polls completion without blocking.
pub struct Installation {
    child: std::process::Child,
    pub component: &'static str,
    pub log_path: std::path::PathBuf,
}
impl Installation {
    pub fn poll(&mut self) -> Result<Option<std::process::ExitStatus>, String> {
        self.child
            .try_wait()
            .map_err(|e| format!("无法读取安装结果：{e}"))
    }
    pub fn log_tail(&self) -> String {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut file) = std::fs::File::open(&self.log_path) else {
            return String::new();
        };
        let Ok(length) = file.metadata().map(|m| m.len()) else {
            return String::new();
        };
        let _ = file.seek(SeekFrom::Start(length.saturating_sub(8192)));
        let mut data = Vec::new();
        let _ = file.take(8192).read_to_end(&mut data);
        String::from_utf8_lossy(&data).into_owned()
    }
}

/// 用户点击安装按钮后才启动官方安装向导；下载器会验证固定摘要和签名。
pub fn install(component: &str) -> Result<Installation, String> {
    use std::{fs::OpenOptions, os::windows::process::CommandExt, process::Stdio};
    let name = match component {
        "usbip" => "USB/IP Win2",
        "hidhide" => "HidHide",
        _ => return Err("不支持的驱动组件".into()),
    };
    let dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("无法获取程序目录")?
        .to_owned();
    let mut script = dir.join("scripts/Install-Drivers.ps1");
    if !script.is_file() {
        script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/Install-Drivers.ps1");
    }
    if !script.is_file() {
        return Err("缺少 scripts/Install-Drivers.ps1，请重新解压完整发布包。".into());
    }
    let log_dir = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .ok_or("无法取得安装日志目录")?
        .join("OpenDS/logs");
    std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    let log_path = log_dir.join(format!("install-{component}.log"));
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .map_err(|e| e.to_string())?;
    let stderr = log.try_clone().map_err(|e| e.to_string())?;
    let child = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-NonInteractive",
            "-File",
        ])
        .arg(script)
        .args(["-Component", component, "-Install"])
        .creation_flags(0x08000000)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|e| format!("无法启动安装向导：{e}"))?;
    Ok(Installation {
        child,
        component: name,
        log_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_hidhide_uses_verified_session_instead_of_reopening_control_channel() {
        assert_eq!(
            hidhide_status(true, || panic!("must reuse session health")),
            HidHideStatus::InUse
        );
        assert_eq!(hidhide_status(false, || true), HidHideStatus::Accessible);
        assert_eq!(hidhide_status(false, || false), HidHideStatus::Unavailable);
        let mut status = Status {
            usbip_service: true,
            usbip_tool: true,
            hidhide: HidHideStatus::InUse,
            viiper: true,
            opus: true,
        };
        assert!(status.ready());
        assert!(status
            .summary()
            .contains("HidHide：已启用，正在隐藏物理手柄"));
        status.hidhide = HidHideStatus::Unavailable;
        assert!(!status.ready());
    }
    #[test]
    fn unsupported_component_never_launches_an_installer() {
        assert!(install("unexpected").is_err());
    }
    #[test]
    fn installer_completion_and_log_tail_are_bounded() {
        use std::{
            os::windows::process::CommandExt,
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        let path = std::env::temp_dir().join(format!(
            "opends-installer-test-{}-{}.log",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, format!("{}\nresult-sentinel", "x".repeat(10_000))).unwrap();
        let child = Command::new("cmd.exe")
            .args(["/D", "/C", "exit 7"])
            .creation_flags(0x08000000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut job = Installation {
            child,
            component: "test",
            log_path: path.clone(),
        };
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = job.poll().unwrap() {
                assert_eq!(status.code(), Some(7));
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let tail = job.log_tail();
        assert!(tail.len() <= 8192 && tail.ends_with("result-sentinel"));
        std::fs::remove_file(path).unwrap();
    }
}
