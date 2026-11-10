use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
static STOP: AtomicBool = AtomicBool::new(false);
#[path = "service.rs"]
mod service;
extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Release);
}
fn xdg(key: &str, fallback: &str) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(fallback)
        })
        .join("opends")
}
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
}
pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state")
}
fn args() -> Vec<String> {
    std::env::args().skip(1).collect()
}
pub fn main() {
    crate::i18n::initialize(
        &crate::config::Config::load(&crate::config::Config::path())
            .map(|c| c.language)
            .unwrap_or_else(|_| "auto".into()),
    );
    if let Err(error) = run(&args()) {
        eprintln!("OpenDS: {error}");
        std::process::exit(1);
    }
}
fn run(args: &[String]) -> Result<(), String> {
    use crate::{config::Config, hid::HidDevice};
    let path = Config::path();
    let cfg = Config::load(&path)?;
    match args.first().map(String::as_str).unwrap_or("gui") {
        "help" | "--help" | "-h" => println!("OpenDS for Linux\n\n  opends [gui]                  Desktop control window\n  opends doctor                 Check dependencies and permissions\n  opends list                   List connected Bluetooth DualSense pads\n  opends probe                  Read a controller report\n  opends bridge [seconds] [--record /path/capture.csv]\n                                Bridge until Ctrl+C/SIGTERM or time limit\n  opends config path            Print configuration path\n  opends config init            Write defaults if configuration is missing\n  opends config check FILE      Validate an INI file\n  opends config import FILE     Validate and save an INI file\n  opends restore                Release notes for Linux input grabs\n\nConfig: {}\nSetup: scripts/setup-ubuntu.sh\nLinux uses USB/IP, vhci_hcd, libVIIPER.so and libopus.so.0.\nThe desktop administrator dialog authorizes USB/IP attachment.", path.display()),
        "doctor" => println!("{}", crate::dependencies::summary(&cfg)),
        "service" => return service::run(),
        "list" | "probe" => {
            let paths = HidDevice::enum_paths();
            if paths.is_empty() { println!("No Bluetooth DualSense connected. Pair it in Ubuntu Bluetooth settings."); }
            for path in paths {
                let device = HidDevice::open(&path).ok_or_else(|| format!("Cannot access {path}; run scripts/setup-ubuntu.sh and reconnect the controller"))?;
                println!("PID={:04X} IN={} OUT={} {}", device.product_id, device.input_len, device.output_len, path);
                if args[0] == "probe" {
                    match device.read_result(500).map_err(|e| e.to_string())? {
                        Some(data) => {
                            println!("{data:02X?}");
                            if let Some(state) = crate::report::InputReport::new(data).and_then(|r| r.parse()) { println!("{state:?}"); }
                        }
                        None => println!("No input within 500 ms"),
                    }
                }
            }
        }
        "config" => match args.get(1).map(String::as_str) {
            Some("path") => println!("{}", path.display()),
            Some("init") => { if !path.exists() { cfg.save(&path)?; } println!("{}", path.display()); }
            Some(action @ ("check" | "import")) if args.len() == 3 => {
                let candidate = Config::load(Path::new(&args[2]))?;
                if !Path::new(&args[2]).is_file() { return Err("INI file does not exist".into()); }
                candidate.validate()?;
                if action == "import" { candidate.save(&path)?; }
                println!("Configuration valid: {}", path.display());
            }
            _ => return Err("Use: opends config path|init|check FILE|import FILE".into()),
        },
        "gui" | "tray" => {
            use std::os::unix::process::CommandExt;
            if !path.exists() { cfg.save(&path)?; }
            let error = std::process::Command::new("/usr/bin/python3").arg("-c").arg(include_str!("gui.py")).arg(std::env::current_exe().map_err(|e| e.to_string())?).arg(path).arg(if args.first().is_some_and(|s| s == "tray") { "--tray" } else { "--window" }).exec();
            return Err(format!("Cannot launch desktop UI: {error}. Install python3-gi, python3-gi-cairo and gir1.2-gtk-3.0, or use opends bridge."));
        }
        "bridge" => {
            let (limit, recording) = bridge_args(&args[1..])?;
            let _instance = crate::persistence::InstanceGuard::acquire().map_err(|e| e.to_string())?;
            cfg.validate()?;
            if !Path::new("/sys/module/vhci_hcd").exists() { return Err("vhci_hcd is not loaded. Run scripts/setup-ubuntu.sh first.".into()); }
            unsafe {
                let mut action: libc::sigaction = std::mem::zeroed();
                action.sa_sigaction = stop as *const () as usize;
                libc::sigemptyset(&mut action.sa_mask);
                if libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut()) != 0 || libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut()) != 0 { return Err(std::io::Error::last_os_error().to_string()); }
            }
            let mut controller = crate::bridge::BridgeController::default();
            controller.start(cfg.clone(), limit)?;
            if let Some(path) = recording {
                controller.state.monitor.commands.configure(cfg.commands, 0);
                let result = controller.state.monitor.start(cfg.monitor_sample_hz).and_then(|_| controller.state.monitor.start_recording(path));
                if let Err(e) = result { controller.stop(); controller.wait(); controller.state.monitor.stop(); return Err(e); }
            }
            while controller.state.running.load(Ordering::Acquire) {
                if STOP.load(Ordering::Acquire) { controller.stop(); }
                println!("[status] connected={} input={} output={} audio={} errors={}", controller.state.connected.load(Ordering::Relaxed), controller.state.frames_in.load(Ordering::Relaxed), controller.state.frames_out.load(Ordering::Relaxed), controller.state.audio_blocks.load(Ordering::Relaxed), controller.state.invalid_frames.load(Ordering::Relaxed));
                std::thread::sleep(Duration::from_millis(250));
            }
            controller.wait();
            controller.state.monitor.stop();
            while controller.state.monitor.recording.status().finishing { std::thread::sleep(Duration::from_millis(10)); }
            let recording_error = controller.state.monitor.recording.status().error.or_else(|| controller.state.monitor.commands.recording.error());
            controller.state.monitor.recording.finish();
            controller.state.monitor.commands.recording.finish();
            if let Some(error) = recording_error { return Err(format!("Recording failed: {error}")); }
            let error = controller.state.last_error.lock().map_err(|_| "Bridge error lock poisoned")?.clone();
            if let Some(e) = error { return Err(e); }
        }
        "restore" => println!("Linux evdev grabs release automatically when OpenDS exits, including after a crash. USB/IP devices detach when the local server closes; inspect remaining ports with usbip port."),
        "startup" => return Err("Use General settings to configure Ubuntu login startup.".into()),
        command => return Err(format!("Unknown command {command:?}; use opends --help")),
    }
    Ok(())
}
fn bridge_args(args: &[String]) -> Result<(Option<Duration>, Option<PathBuf>), String> {
    let mut limit = None;
    let mut record = None;
    let mut seconds_seen = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--record" {
            if record.is_some() {
                return Err("Specify --record only once".into());
            }
            record = Some(PathBuf::from(
                iter.next().ok_or("--record requires a CSV path")?,
            ));
        } else if !seconds_seen {
            let secs = arg
                .parse::<u64>()
                .map_err(|_| "Seconds must be a non-negative integer")?;
            limit = (secs > 0).then(|| Duration::from_secs(secs));
            seconds_seen = true;
        } else {
            return Err(format!("Unexpected argument: {arg}"));
        }
    }
    Ok((limit, record))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_arguments_accept_recording_and_reject_bad_limits() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            bridge_args(&a(&["5", "--record", "/tmp/capture.csv"])).unwrap(),
            (
                Some(Duration::from_secs(5)),
                Some("/tmp/capture.csv".into())
            )
        );
        assert_eq!(bridge_args(&a(&["0"])).unwrap(), (None, None));
        for bad in [a(&["-1"]), a(&["1.5"]), a(&["--record"]), a(&["1", "2"])] {
            assert!(bridge_args(&bad).is_err());
        }
    }
}
