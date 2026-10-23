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
        std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .args([
                "gui",
                "--restart-state",
                if running { "running" } else { "stopped" },
            ])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
