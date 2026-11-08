use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    os::fd::AsRawFd,
    path::Path,
};

pub fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = (|| {
        file.write_all(content)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)?;
        File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
pub struct InstanceGuard(File);
#[derive(Debug)]
pub enum InstanceError {
    AlreadyRunning,
    System(io::Error),
}
impl std::fmt::Display for InstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => f.write_str("OpenDS is already running"),
            Self::System(e) => write!(f, "OpenDS process lock: {e}"),
        }
    }
}
impl InstanceGuard {
    pub fn acquire() -> Result<Self, InstanceError> {
        let dir = crate::linux::state_dir();
        std::fs::create_dir_all(&dir).map_err(InstanceError::System)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(dir.join("controller.lock"))
            .map_err(InstanceError::System)?;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            return Err(if error.kind() == io::ErrorKind::WouldBlock {
                InstanceError::AlreadyRunning
            } else {
                InstanceError::System(error)
            });
        }
        Ok(Self(file))
    }
}
impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_replacement_roundtrips_and_failed_creation_preserves_target() {
        let root = std::env::temp_dir().join(format!("opends-linux-write-{}", std::process::id()));
        let path = root.join("settings.ini");
        atomic_write(&path, b"old").unwrap();
        atomic_write(&path, b"new").unwrap();
        std::fs::write(
            path.with_extension(format!("tmp-{}", std::process::id())),
            "occupied",
        )
        .unwrap();
        assert!(atomic_write(&path, b"bad").is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"new");
        std::fs::remove_dir_all(root).unwrap();
    }
}
