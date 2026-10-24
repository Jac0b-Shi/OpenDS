//! User-initiated shell links. Installation and application startup do not call this module.
use std::{
    ffi::OsString,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr::null_mut,
    sync::atomic::{AtomicU64, Ordering},
};
use winapi::{
    shared::{guiddef::GUID, wtypesbase::CLSCTX_INPROC_SERVER},
    um::{
        combaseapi::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize},
        knownfolders::{FOLDERID_Desktop, FOLDERID_Programs},
        objbase::COINIT_APARTMENTTHREADED,
        objidl::IPersistFile,
        shlobj::SHGetKnownFolderPath,
        shobjidl_core::IShellLinkW,
        winuser::SW_SHOWNORMAL,
    },
    Interface,
};

#[derive(Clone, Copy)]
pub enum Location {
    Desktop,
    StartMenu,
}
const SHELL_LINK: GUID = GUID {
    Data1: 0x00021401,
    Data2: 0,
    Data3: 0,
    Data4: [0xc0, 0, 0, 0, 0, 0, 0, 0x46],
};
struct Apartment(bool);
impl Apartment {
    fn new() -> Result<Self, String> {
        let result = unsafe { CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED) };
        // An already initialized GUI thread may use a different apartment model.
        if result < 0 && result as u32 != 0x80010106 {
            return Err(hresult(result));
        }
        Ok(Self(result >= 0))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() }
        }
    }
}
fn hresult(code: i32) -> String {
    format!("HRESULT 0x{:08X}", code as u32)
}
fn check(code: i32) -> Result<(), String> {
    if code < 0 {
        Err(hresult(code))
    } else {
        Ok(())
    }
}
fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

pub fn create(location: Location) -> Result<PathBuf, String> {
    let result = (|| {
        let _apartment = Apartment::new()?;
        let id = match location {
            Location::Desktop => &FOLDERID_Desktop,
            Location::StartMenu => &FOLDERID_Programs,
        };
        let mut raw = null_mut();
        check(unsafe { SHGetKnownFolderPath(id, 0, null_mut(), &mut raw) })?;
        let folder = unsafe {
            let mut len = 0;
            while *raw.add(len) != 0 {
                len += 1;
            }
            let path = PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(raw, len)));
            CoTaskMemFree(raw as _);
            path
        };
        let destination = folder.join("OpenDS.lnk");
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        write_link(&destination, &exe)?;
        Ok(destination)
    })();
    result.map_err(|error: String| crate::tr!(shortcut_failed, error = error.as_str()))
}

fn write_link(destination: &Path, exe: &Path) -> Result<(), String> {
    if destination.exists() {
        return Err(crate::tr!(
            shortcut_exists,
            path = destination.display().to_string()
        ));
    }
    let _apartment = Apartment::new()?;
    let mut link = null_mut::<IShellLinkW>();
    check(unsafe {
        CoCreateInstance(
            &SHELL_LINK,
            null_mut(),
            CLSCTX_INPROC_SERVER,
            &IShellLinkW::uuidof(),
            &mut link as *mut _ as _,
        )
    })?;
    let result = (|| {
        let mut persist = null_mut::<IPersistFile>();
        unsafe {
            check((*link).SetPath(wide(exe).as_ptr()))?;
            check((*link).SetWorkingDirectory(
                wide(exe.parent().ok_or("Missing executable folder")?).as_ptr(),
            ))?;
            check((*link).SetIconLocation(wide(exe).as_ptr(), 0))?;
            check((*link).SetShowCmd(SW_SHOWNORMAL))?;
            check((*link).QueryInterface(&IPersistFile::uuidof(), &mut persist as *mut _ as _))?;
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temp = destination.with_file_name(format!(
            ".OpenDS-{}-{}.lnk.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let saved = (|| {
            // Reserve our own temporary file, then commit without replacing any
            // existing destination, including one created during the COM call.
            let reserved = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|e| e.to_string())?;
            drop(reserved);
            let result = (|| {
                check(unsafe { (*persist).Save(wide(&temp).as_ptr(), 1) })?;
                #[link(name = "kernel32")]
                extern "system" {
                    fn MoveFileW(from: *const u16, to: *const u16) -> i32;
                }
                if unsafe { MoveFileW(wide(&temp).as_ptr(), wide(destination).as_ptr()) } == 0 {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                Ok(())
            })();
            if result.is_err() {
                let _ = std::fs::remove_file(&temp);
            }
            result
        })();
        unsafe {
            (*persist).Release();
        }
        saved
    })();
    unsafe {
        (*link).Release();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_shell_link_preserves_target_and_does_not_overwrite() {
        let dir = std::env::temp_dir().join(format!("opends-link-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("OpenDS.lnk");
        let exe = std::env::current_exe().unwrap();
        write_link(&path, &exe).unwrap();
        let _apartment = Apartment::new().unwrap();
        unsafe {
            let mut link = null_mut::<IShellLinkW>();
            check(CoCreateInstance(
                &SHELL_LINK,
                null_mut(),
                CLSCTX_INPROC_SERVER,
                &IShellLinkW::uuidof(),
                &mut link as *mut _ as _,
            ))
            .unwrap();
            let mut persist = null_mut::<IPersistFile>();
            check((*link).QueryInterface(&IPersistFile::uuidof(), &mut persist as *mut _ as _))
                .unwrap();
            check((*persist).Load(wide(&path).as_ptr(), 0)).unwrap();
            let mut target = [0u16; 32768];
            check((*link).GetPath(target.as_mut_ptr(), target.len() as i32, null_mut(), 0))
                .unwrap();
            let length = target.iter().position(|&c| c == 0).unwrap();
            assert_eq!(PathBuf::from(OsString::from_wide(&target[..length])), exe);
            let mut hotkey = 1u16;
            check((*link).GetHotkey(&mut hotkey)).unwrap();
            assert_eq!(hotkey, 0, "Shortcuts must not register global hotkeys");
            (*persist).Release();
            (*link).Release();
        }
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() > 76);
        assert!(write_link(&path, &exe).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
