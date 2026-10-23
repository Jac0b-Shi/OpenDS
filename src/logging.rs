//! Bounded diagnostics: 1 MiB current file and two rotated files.
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
const LIMIT: u64 = 1024 * 1024;
struct Log {
    path: PathBuf,
    file: Option<File>,
    bytes: u64,
}
impl Log {
    fn new(path: PathBuf) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            path,
            file: Some(file),
            bytes,
        })
    }
    fn append(&mut self, line: &str) -> io::Result<()> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let data = format!("{stamp} {line}\n");
        if self.bytes + data.len() as u64 > LIMIT {
            self.file.take();
            let first = self.path.with_extension("log.1");
            let second = self.path.with_extension("log.2");
            if second.exists() {
                std::fs::remove_file(&second)?;
            }
            if first.exists() {
                std::fs::rename(&first, &second)?;
            }
            if self.path.exists() {
                std::fs::rename(&self.path, &first)?;
            }
            self.bytes = 0;
        }
        if self.file.is_none() {
            self.file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?,
            );
        }
        self.file.as_mut().unwrap().write_all(data.as_bytes())?;
        self.bytes += data.len() as u64;
        Ok(())
    }
}
pub fn record(line: &str) {
    static LOG: OnceLock<Mutex<Option<Log>>> = OnceLock::new();
    let writer = LOG.get_or_init(|| {
        Mutex::new(
            std::env::var_os("LOCALAPPDATA")
                .and_then(|root| Log::new(PathBuf::from(root).join("OpenDS/logs/opends.log")).ok()),
        )
    });
    if let Ok(mut log) = writer.lock() {
        if let Some(log) = log.as_mut() {
            let _ = log.append(line);
        }
    }
}
pub fn record_message(id: crate::i18n::MessageId, message: &str) {
    record(&format!("[{}] {message}", id.key()));
}
pub fn bounded(line: &str) -> String {
    line.chars().take(2048).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotation_bounds_files_and_keeps_newest_records() {
        let root = std::env::temp_dir().join(format!(
            "opends-log-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("opends.log");
        {
            let mut log = Log::new(path.clone()).unwrap();
            for i in 0..2200 {
                log.append(&format!("{i} {}", "x".repeat(2048))).unwrap();
            }
        }
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 3);
        for f in std::fs::read_dir(&root).unwrap() {
            assert!(f.unwrap().metadata().unwrap().len() <= LIMIT);
        }
        assert!(std::fs::read_to_string(path).unwrap().contains("2199 "));
        std::fs::remove_dir_all(root).unwrap();
    }
}
