pub fn summary(config: &crate::config::Config) -> String {
    let mut lines = vec![format!("OpenDS {} — Linux", env!("CARGO_PKG_VERSION"))];
    let usbip = crate::dependency_paths::Component::Usbip.resolve(&config.usbip_directory);
    lines.push(format!(
        "USB/IP: {}",
        usbip.map(|p| p.display().to_string()).unwrap_or_else(|| {
            "MISSING — install linux-tools-generic and linux-tools-$(uname -r)".into()
        })
    ));
    lines.push(format!(
        "vhci_hcd: {}",
        if std::path::Path::new("/sys/module/vhci_hcd").exists() {
            "loaded"
        } else {
            "NOT LOADED — run scripts/setup-ubuntu.sh"
        }
    ));
    lines.push(format!(
        "VIIPER: {}",
        if crate::viiper::is_available() {
            "available"
        } else {
            "MISSING — run scripts/build-linux.sh"
        }
    ));
    lines.push(format!(
        "Opus: {}",
        if crate::opus::is_available() {
            "available"
        } else {
            "MISSING — install libopus0"
        }
    ));
    let paths = crate::hid::HidDevice::enum_paths();
    if paths.is_empty() {
        lines.push(
            "Bluetooth DualSense: none connected — pair it in Ubuntu Bluetooth settings".into(),
        );
    }
    for path in paths {
        lines.push(format!(
            "{path}: {}",
            if crate::hid::HidDevice::open(&path).is_some() {
                "accessible"
            } else {
                "PERMISSION DENIED — run scripts/setup-ubuntu.sh and reconnect"
            }
        ));
    }
    lines.push(format!(
        "Configuration: {}",
        crate::config::Config::path().display()
    ));
    lines.push(format!(
        "Logs: {}",
        crate::linux::state_dir().join("logs/opends.log").display()
    ));
    lines.push("USB/IP attach/detach uses the desktop's administrator authentication dialog.\nPhysical input grabs cover evdev; disable Steam Input and SDL HIDAPI in games to prevent direct hidraw duplicates.".into());
    lines.join("\n")
}
