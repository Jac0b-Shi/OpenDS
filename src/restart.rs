use std::sync::Mutex;
static REQUEST: Mutex<Option<bool>> = Mutex::new(None);
pub fn request(bridging: bool) {
    if let Ok(mut requested) = REQUEST.lock() {
        *requested = Some(bridging);
    }
}
pub fn startup_override() -> Option<bool> {
    let args: Vec<_> = std::env::args().collect();
    args.windows(2)
        .find(|a| a[0] == "--restart-state")
        .and_then(|a| match a[1].as_str() {
            "running" => Some(true),
            "stopped" => Some(false),
            _ => None,
        })
}
pub fn launch_pending() -> Result<(), String> {
    let requested = REQUEST.lock().map_err(|e| e.to_string())?.take();
    if let Some(running) = requested {
        use std::os::windows::process::CommandExt;
        let testing = std::env::args().nth(1).as_deref() == Some("ui-smoke-test");
        let mut command =
            std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
        if testing {
            command.env(
                "OPENDS_UI_SMOKE_RESTART_PARENT",
                std::process::id().to_string(),
            );
        }
        command
            .args([
                if testing { "ui-smoke-test" } else { "gui" },
                "--restart-state",
                if running { "running" } else { "stopped" },
            ])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn finish_smoke() -> Result<(), String> {
    let parent = std::env::var("OPENDS_UI_SMOKE_RESTART_PARENT")
        .ok()
        .and_then(|s| s.parse::<u32>().ok());
    if let Some(parent) = parent {
        let path = std::env::temp_dir().join(format!("opends-restart-smoke-{parent}.done"));
        let data = format!(
            "{}:{}",
            std::process::id(),
            startup_override().unwrap_or(false)
        );
        crate::persistence::atomic_write(&path, data.as_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(())
}
