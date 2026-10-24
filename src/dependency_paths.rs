//! Locate installed user-mode tools. Kernel driver readiness is checked separately.
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub enum Component {
    Usbip,
    HidHide,
}
impl Component {
    pub fn executable(self) -> &'static str {
        match self {
            Self::Usbip => "usbip.exe",
            Self::HidHide => "HidHideClient.exe",
        }
    }
    pub fn directory(self, config: &crate::config::Config) -> &str {
        match self {
            Self::Usbip => &config.usbip_directory,
            Self::HidHide => &config.hidhide_directory,
        }
    }
    pub fn set_directory(self, config: &mut crate::config::Config, directory: String) {
        match self {
            Self::Usbip => config.usbip_directory = directory,
            Self::HidHide => config.hidhide_directory = directory,
        }
    }
    pub fn in_directory(self, directory: &Path) -> Option<PathBuf> {
        let subdirs: &[&str] = match self {
            Self::Usbip => &[""],
            Self::HidHide => &["", "x64", "bin"],
        };
        subdirs
            .iter()
            .map(|sub| directory.join(sub).join(self.executable()))
            .find(|path| path.is_file())
    }
    pub fn resolve(self, directory: &str) -> Option<PathBuf> {
        // An explicit selection must never silently fall back to another installation.
        if !directory.is_empty() {
            return self.in_directory(Path::new(directory));
        }
        for key in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
            let Some(root) = std::env::var_os(key).map(PathBuf::from) else {
                continue;
            };
            let folders: &[&str] = match self {
                Self::Usbip => &["USBip"],
                Self::HidHide => &[
                    "Nefarius Software Solutions/HidHide",
                    "Nefarius Software Solutions e.U/HidHide",
                    "Nefarius Software Solutions e.U./HidHide",
                ],
            };
            for folder in folders {
                if let Some(path) = self.in_directory(&root.join(folder)) {
                    return Some(path);
                }
            }
        }
        std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join(self.executable()))
                .find(|path| path.is_file())
        })
    }
    pub fn validate_directory(self, directory: &Path) -> Result<(), String> {
        if directory.is_absolute() && directory.is_dir() && self.in_directory(directory).is_some() {
            Ok(())
        } else {
            Err(crate::tr!(
                dependency_directory_invalid,
                path = directory.display().to_string(),
                executable = self.executable()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_unicode_directories_and_hidhide_layouts_are_supported() {
        let root =
            std::env::temp_dir().join(format!("opends-paths-{}-中文 空格", std::process::id()));
        std::fs::create_dir_all(root.join("x64")).unwrap();
        std::fs::write(root.join("wusbip.exe"), []).unwrap();
        assert!(Component::Usbip.validate_directory(&root).is_err());
        std::fs::write(root.join("usbip.exe"), []).unwrap();
        let configured = root.to_str().unwrap();
        assert_eq!(
            Component::Usbip.resolve(configured),
            Some(root.join("usbip.exe"))
        );
        assert!(Component::Usbip
            .resolve(root.join("missing").to_str().unwrap())
            .is_none());
        std::fs::write(root.join("x64/HidHideClient.exe"), []).unwrap();
        assert_eq!(
            Component::HidHide.resolve(configured),
            Some(root.join("x64/HidHideClient.exe"))
        );
        std::fs::write(root.join("HidHideClient.exe"), []).unwrap();
        assert_eq!(
            Component::HidHide.resolve(configured),
            Some(root.join("HidHideClient.exe"))
        );
        assert!(Component::Usbip
            .validate_directory(Path::new("relative"))
            .is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
