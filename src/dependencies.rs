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
    usbip_custom: bool,
    pub usbip_path: Option<std::path::PathBuf>,
    pub hidhide_path: Option<std::path::PathBuf>,
    hidhide: HidHideStatus,
    viiper: bool,
    opus: bool,
}
pub fn inspect(config: &crate::config::Config, hidhide_owned_healthy: bool) -> Status {
    let usbip_path = crate::dependency_paths::Component::Usbip.resolve(&config.usbip_directory);
    Status {
        usbip_service: service_running("usbip2_ude"),
        usbip_tool: usbip_path.is_some(),
        usbip_custom: !config.usbip_directory.is_empty(),
        usbip_path,
        hidhide_path: crate::dependency_paths::Component::HidHide
            .resolve(&config.hidhide_directory),
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
    pub fn hidhide_available(&self) -> bool {
        self.hidhide != HidHideStatus::Unavailable
    }
    pub fn confirm_hidhide_owned(&mut self, healthy: bool) {
        if healthy {
            self.hidhide = HidHideStatus::InUse;
        }
    }
    pub fn summary(&self) -> String {
        crate::tr!(
            dependencies_usb_ip_win2_ude_hidhide_libviiper_opus,
            usbip = crate::i18n_arg!(if self.usbip_service {
                if self.usbip_tool {
                    crate::tr!(dependencies_service_and_attachment_tool_available)
                } else if self.usbip_custom {
                    crate::tr!(dependency_usbip_custom_invalid)
                } else {
                    crate::tr!(dependencies_service_running_usbip_exe_is_missing)
                }
            } else {
                crate::tr!(dependencies_not_running_or_not_installed_install_reboot)
            }),
            hidhide = crate::i18n_arg!(match self.hidhide {
                HidHideStatus::InUse =>
                    crate::tr!(dependencies_enabled_hiding_the_physical_controller),
                HidHideStatus::Accessible => crate::tr!(dependencies_accessible),
                HidHideStatus::Unavailable =>
                    crate::tr!(dependencies_not_accessible_check_driver_and_permissions),
            }),
            viiper = crate::i18n_arg!(if self.viiper {
                crate::tr!(dependencies_available)
            } else {
                crate::tr!(dependencies_matching_dll_missing_place_it_beside_opends)
            }),
            opus = crate::i18n_arg!(if self.opus {
                crate::tr!(dependencies_available)
            } else {
                crate::tr!(dependencies_libopus_0_dll_missing_audio_and_native)
            }),
            path = crate::i18n_arg!(crate::config::Config::path().display())
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
pub fn summary(config: &crate::config::Config) -> String {
    inspect(config, false).summary()
}

/// One asynchronous installer at a time; the UI polls completion without blocking.
pub struct Installation {
    child: std::process::Child,
    pub component: &'static str,
    pub log_path: std::path::PathBuf,
}
impl Installation {
    pub fn poll(&mut self) -> Result<Option<std::process::ExitStatus>, String> {
        self.child.try_wait().map_err(|e| {
            crate::tr!(
                dependencies_could_not_read_installation_result,
                error = crate::i18n_arg!(e)
            )
        })
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
        _ => return Err(crate::tr!(dependencies_unsupported_driver_component).into()),
    };
    let dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or(crate::tr!(
            dependencies_could_not_locate_the_application_directory
        ))?
        .to_owned();
    let mut script = dir.join("scripts/Install-Drivers.ps1");
    if !script.is_file() {
        script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("scripts/Install-Drivers.ps1");
    }
    if !script.is_file() {
        return Err(crate::tr!(dependencies_scripts_install_drivers_ps1_is_missing_extract).into());
    }
    let log_dir = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .ok_or(crate::tr!(
            dependencies_could_not_locate_the_installation_log_directory
        ))?
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
        .map_err(|e| {
            crate::tr!(
                dependencies_could_not_start_installer,
                error = crate::i18n_arg!(e)
            )
        })?;
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
            usbip_custom: false,
            usbip_path: None,
            hidhide_path: None,
            hidhide: HidHideStatus::InUse,
            viiper: true,
            opus: true,
        };
        assert!(status.ready());
        // HidHideClient is a convenience, never a prerequisite for direct IOCTL access.
        assert!(status.hidhide_path.is_none());
        status.usbip_tool = false;
        assert!(!status.ready());
        assert!(status.summary().contains(crate::tr!(
            dependencies_service_running_usbip_exe_is_missing
        )));
        status.usbip_custom = true;
        assert!(status
            .summary()
            .contains(crate::tr!(dependency_usbip_custom_invalid)));
        status.usbip_tool = true;
        assert!(status.summary().contains(crate::tr!(
            dependencies_enabled_hiding_the_physical_controller
        )));
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
