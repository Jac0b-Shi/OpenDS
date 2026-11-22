use std::path::PathBuf;
pub enum Component {
    Usbip,
}
impl Component {
    pub fn resolve(self, directory: &str) -> Option<PathBuf> {
        use std::os::unix::fs::PermissionsExt;
        let candidates: Vec<_> = if directory.is_empty() {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|p| p.join("usbip"))
                .collect()
        } else {
            vec![PathBuf::from(directory).join("usbip")]
        };
        candidates.into_iter().find(|p| {
            p.is_file()
                && p.metadata()
                    .is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
        })
    }
}
