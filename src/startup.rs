//! Per-user logon startup. No service, elevation or scheduler dependency.
use std::{ffi::c_void, path::Path};
type Hkey = *mut c_void;
#[link(name = "advapi32")]
extern "system" {
    fn RegCreateKeyExW(
        key: Hkey,
        sub: *const u16,
        reserved: u32,
        class: *mut u16,
        options: u32,
        access: u32,
        security: *mut c_void,
        result: *mut Hkey,
        disposition: *mut u32,
    ) -> i32;
    fn RegOpenKeyExW(
        key: Hkey,
        sub: *const u16,
        options: u32,
        access: u32,
        result: *mut Hkey,
    ) -> i32;
    fn RegQueryValueExW(
        key: Hkey,
        name: *const u16,
        reserved: *mut u32,
        kind: *mut u32,
        data: *mut u8,
        len: *mut u32,
    ) -> i32;
    fn RegSetValueExW(
        key: Hkey,
        name: *const u16,
        reserved: u32,
        kind: u32,
        data: *const u8,
        len: u32,
    ) -> i32;
    fn RegDeleteValueW(key: Hkey, name: *const u16) -> i32;
    fn RegCloseKey(key: Hkey) -> i32;
}
struct Key(Hkey);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
const RUN: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
fn hkcu() -> Hkey {
    0x80000001u32 as i32 as isize as Hkey
}
fn error(code: i32) -> String {
    crate::tr!(
        startup_windows_sign_in_startup_setup_failed,
        error = format!("{}", std::io::Error::from_raw_os_error(code))
    )
}
pub fn command(exe: &Path) -> Result<String, String> {
    let s = exe
        .to_str()
        .ok_or(crate::tr!(startup_the_executable_path_is_not_valid_unicode))?;
    if s.contains(['"', '\r', '\n']) {
        return Err(crate::tr!(startup_the_executable_path_contains_invalid_characters).into());
    }
    let command = format!("\"{s}\" tray");
    if command.encode_utf16().count() > 260 {
        return Err(crate::tr!(startup_the_sign_in_startup_command_exceeds_the).into());
    }
    Ok(command)
}
pub fn enabled() -> Result<bool, String> {
    let mut key = std::ptr::null_mut();
    let result = unsafe { RegOpenKeyExW(hkcu(), wide(RUN).as_ptr(), 0, 1, &mut key) };
    if result == 2 {
        return Ok(false);
    }
    if result != 0 {
        return Err(error(result));
    }
    let key = Key(key);
    let mut size = 0;
    let mut kind = 0;
    let result = unsafe {
        RegQueryValueExW(
            key.0,
            wide("OpenDS").as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if result == 2 {
        return Ok(false);
    }
    if result != 0 {
        return Err(error(result));
    }
    Ok(kind == 1 && size > 2)
}
pub fn set_enabled(enable: bool) -> Result<(), String> {
    let mut key = std::ptr::null_mut();
    let result = unsafe {
        RegCreateKeyExW(
            hkcu(),
            wide(RUN).as_ptr(),
            0,
            std::ptr::null_mut(),
            0,
            2,
            std::ptr::null_mut(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    if result != 0 {
        return Err(error(result));
    }
    let key = Key(key);
    let name = wide("OpenDS");
    let result = if enable {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let value = wide(&command(&exe)?);
        unsafe {
            RegSetValueExW(
                key.0,
                name.as_ptr(),
                0,
                1,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        }
    } else {
        unsafe { RegDeleteValueW(key.0, name.as_ptr()) }
    };
    if result != 0 && !(result == 2 && !enable) {
        return Err(error(result));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_quotes_paths_and_starts_background_bridge() {
        assert_eq!(
            command(Path::new("C:\\Program Files\\OpenDS\\opends.exe")).unwrap(),
            "\"C:\\Program Files\\OpenDS\\opends.exe\" tray"
        );
        assert!(command(Path::new("bad\"path")).is_err());
    }
}
