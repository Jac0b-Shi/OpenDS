//! 同卷临时文件 + flush + 原子替换。恢复日志必须先于驱动修改持久化。
use std::{
    ffi::c_void,
    fs::OpenOptions,
    io::{self, Write},
    path::Path,
};

#[link(name = "kernel32")]
extern "system" {
    fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    fn CreateMutexW(sa: *mut c_void, owner: i32, name: *const u16) -> *mut c_void;
    fn GetLastError() -> u32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

pub fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = (|| {
        f.write_all(content)?;
        f.sync_all()?;
        drop(f);
        let from: Vec<u16> = temp
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let to: Vec<u16> = path
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 1 | 8) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    })();
    // 不删除别人留下的临时文件：只有本次成功创建后才有清理权。
    if result.is_err() && temp.exists() {
        // 同一进程写入由上层串行化；下一次失败仍保留原目标文件。
        let _ = std::fs::remove_file(temp);
    }
    result
}

/// 所有修改驱动状态的入口共用此进程锁；第二实例不得执行恢复。
pub struct InstanceGuard(*mut c_void);
impl InstanceGuard {
    pub fn acquire() -> Result<Self, String> {
        Self::acquire_named("Global\\OpenDS.Controller.v1")
    }
    fn acquire_named(name: &str) -> Result<Self, String> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let h = unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
        if h.is_null() {
            return Err(format!("无法创建进程锁：{}", io::Error::last_os_error()));
        }
        if unsafe { GetLastError() } == 183 {
            unsafe {
                CloseHandle(h);
            }
            return Err("OpenDS 已在运行；请从现有托盘实例操作。".into());
        }
        Ok(Self(h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "opends-storage-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn config_atomic_replace_roundtrips_all_live_options() {
        let p = temp();
        let mut c = crate::config::Config::default();
        c.buffer = 2;
        c.microphone_enabled = true;
        c.audio_route = 2;
        c.device_path = "\\\\?\\hid#手柄".into();
        c.save(&p).unwrap();
        let v = crate::config::Config::load(&p).unwrap();
        assert!(v.microphone_enabled);
        assert_eq!(
            (v.buffer, v.audio_route, v.device_path),
            (2, 2, c.device_path.clone())
        );
        c.microphone_enabled = false;
        c.save(&p).unwrap();
        assert!(!crate::config::Config::load(&p).unwrap().microphone_enabled);
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn failed_replacement_keeps_original_file() {
        let p = temp();
        atomic_write(&p, b"original").unwrap();
        let mut permissions = std::fs::metadata(&p).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&p, permissions).unwrap();
        let result = atomic_write(&p, b"new");
        let mut permissions = std::fs::metadata(&p).unwrap().permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(&p, permissions).unwrap();
        assert!(result.is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"original");
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn second_instance_cannot_enter_until_first_guard_drops() {
        let name = format!(
            "Local\\OpenDS.Test.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let first = InstanceGuard::acquire_named(&name).unwrap();
        assert!(InstanceGuard::acquire_named(&name).is_err());
        drop(first);
        assert!(InstanceGuard::acquire_named(&name).is_ok());
    }
}
impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
