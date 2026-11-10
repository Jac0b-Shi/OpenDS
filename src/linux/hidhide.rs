//! Evdev grabs are scoped to file descriptors and released automatically on exit.
//! SDL's direct hidraw access is separate; disable Steam Input/HIDAPI for the game.
use std::{
    fs::{File, OpenOptions},
    os::fd::AsRawFd,
    path::PathBuf,
};
pub struct Hider {
    files: Vec<File>,
}
pub fn interface_path_to_instance_id(path: &str) -> Option<String> {
    let name = std::path::Path::new(path).file_name()?;
    let path =
        std::fs::canonicalize(PathBuf::from("/sys/class/hidraw").join(name).join("device")).ok()?;
    Some(path.to_string_lossy().into_owned())
}
impl Hider {
    pub fn engage(ids: &[String]) -> Result<Self, String> {
        let mut hider = Self { files: Vec::new() };
        for id in ids {
            let entries = std::fs::read_dir(PathBuf::from(id).join("input"))
                .map_err(|e| format!("Cannot find physical controller input nodes: {e}"))?;
            for input in entries.flatten() {
                for node in std::fs::read_dir(input.path())
                    .map_err(|e| e.to_string())?
                    .flatten()
                {
                    if !node.file_name().to_string_lossy().starts_with("event") {
                        continue;
                    }
                    let path = PathBuf::from("/dev/input").join(node.file_name());
                    let file = OpenOptions::new().read(true).open(&path).map_err(|e| format!("Cannot grab {}: {e}. Run scripts/setup-ubuntu.sh, or set hide_device=0.", path.display()))?;
                    if unsafe {
                        libc::ioctl(
                            file.as_raw_fd(),
                            0x40044590 as libc::c_ulong,
                            1 as libc::c_int,
                        )
                    } != 0
                    {
                        return Err(format!(
                            "Cannot exclusively grab {}: {}",
                            path.display(),
                            std::io::Error::last_os_error()
                        ));
                    }
                    hider.files.push(file);
                }
            }
        }
        if hider.files.is_empty() {
            return Err("No evdev nodes found for the physical controller; set hide_device=0 to bridge without grabbing".into());
        }
        Ok(hider)
    }
    pub fn is_active(&self) -> bool {
        !self.files.is_empty()
    }
    pub fn restore(&mut self) -> Result<(), String> {
        self.files.clear();
        Ok(())
    }
}
pub fn recover_if_needed() -> Result<Option<()>, String> {
    Ok(None)
}
