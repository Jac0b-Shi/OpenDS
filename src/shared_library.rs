//! Keep libraries loaded for the lifetime of their cached function pointers.
use std::{
    ffi::{c_void, CString},
    path::Path,
};

pub unsafe fn load(name: &str, vendor: &str) -> *mut c_void {
    let mut paths = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join(name));
        }
    }
    paths.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("vendor")
            .join(vendor)
            .join(name),
    );
    paths.push(name.into());
    for path in paths {
        use std::os::unix::ffi::OsStrExt;
        if let Ok(path) = CString::new(path.as_os_str().as_bytes()) {
            let handle = libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if !handle.is_null() {
                return handle;
            }
        }
    }
    std::ptr::null_mut()
}
