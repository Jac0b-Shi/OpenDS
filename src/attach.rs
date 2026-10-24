//! 预载特性后再挂接；跟踪本会话端口并清理同一端点的旧会话。
use std::os::windows::process::CommandExt;
use std::{
    path::Path,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

fn run(
    exe: &Path,
    args: Vec<String>,
    cancelled: impl Fn() -> bool,
    timeout: Duration,
) -> Result<Output, String> {
    let mut child = Command::new(exe)
        .args(args)
        .creation_flags(0x08000000)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| crate::tr!(attach_could_not_start_usb_ip, error = crate::i18n_arg!(e)))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().map_err(|e| e.to_string())?;
                if status.success() {
                    return Ok(output);
                }
                return Err(crate::tr!(
                    attach_usb_ip_failed,
                    output = crate::i18n_arg!(String::from_utf8_lossy(&output.stdout)),
                    error = crate::i18n_arg!(String::from_utf8_lossy(&output.stderr))
                ));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
            Ok(None) => {}
        }
        if cancelled() || start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(crate::tr!(attach_usb_ip_operation_cancelled_or_timed_out).into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
pub fn first_device(
    exe: &Path,
    port: u16,
    bus: u32,
    cancelled: impl Fn() -> bool,
) -> Result<u32, String> {
    // VIIPER 每个新建私有总线的第一个 DevID 为 1。
    let args = vec![
        "--tcp-port".into(),
        port.to_string(),
        "attach".into(),
        "-r".into(),
        "127.0.0.1".into(),
        "-b".into(),
        format!("{bus}-1"),
        "--once".into(),
        "--terse".into(),
    ];
    let output = run(exe, args, cancelled, Duration::from_secs(5))?;
    let port = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .map_err(|_| {
            crate::tr!(attach_attachment_succeeded_but_returned_no_valid_ude).to_string()
        })?;
    if !(1..=255).contains(&port) {
        return Err(crate::tr!(attach_ude_port_is_out_of_range).into());
    }
    Ok(port)
}
pub fn detach(exe: &Path, port: u32) -> Result<(), String> {
    if !(1..=255).contains(&port) {
        return Err(crate::tr!(attach_refusing_to_detach_an_invalid_ude_port).into());
    }
    run(
        exe,
        vec!["detach".into(), "-p".into(), port.to_string()],
        || false,
        Duration::from_secs(2),
    )
    .map(|_| ())
}
fn owned_ports(text: &str, service: u16, bus: u32) -> Vec<u32> {
    let expected = [
        format!("-> usbip://localhost:{service}/{bus}-1"),
        format!("-> usbip://127.0.0.1:{service}/{bus}-1"),
    ];
    let mut result = vec![];
    let mut current = None;
    let mut sony = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Port ") {
            current = rest
                .split(':')
                .next()
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|n| (1..=255).contains(n));
            sony = false;
        } else if line.contains("(054c:0ce6)") || line.contains("(054c:0df2)") {
            sony = true;
        } else if sony && expected.iter().any(|s| s == line) {
            if let Some(port) = current {
                result.push(port);
            }
        }
    }
    result
}
pub fn cleanup_owned(
    exe: &Path,
    port: u16,
    bus: u32,
    cancelled: impl Fn() -> bool,
) -> Result<(), String> {
    for host in ["localhost", "127.0.0.1"] {
        run(
            exe,
            vec![
                "--tcp-port".into(),
                port.to_string(),
                "attach".into(),
                "-r".into(),
                host.into(),
                "-b".into(),
                format!("{bus}-1"),
                "--stop".into(),
            ],
            &cancelled,
            Duration::from_secs(2),
        )?;
    }
    let list = run(exe, vec!["port".into()], cancelled, Duration::from_secs(2))?;
    for own in owned_ports(&String::from_utf8_lossy(&list.stdout), port, bus) {
        detach(exe, own)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_uses_the_selected_executable_in_a_unicode_directory() {
        let root =
            std::env::temp_dir().join(format!("opends-command-{}-中文 空格", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let selected = root.join("usbip.exe");
        std::fs::copy(
            Path::new(&std::env::var_os("SystemRoot").unwrap()).join("System32/cmd.exe"),
            &selected,
        )
        .unwrap();
        let output = run(
            &selected,
            vec!["/D".into(), "/C".into(), "echo selected-tool".into()],
            || false,
            Duration::from_secs(3),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "selected-tool"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cleanup_only_selects_sony_devices_on_our_exact_loopback_endpoint() {
        let text="Port 01: connected\n Sony (054c:0ce6)\n -> usbip://localhost:3241/1-1\nPort 02: connected\n Sony (054c:0ce6)\n -> usbip://192.168.1.5:3241/1-1\nPort 03: connected\n Sony (054c:0ce6)\n -> usbip://localhost:3242/1-1\nPort 04: connected\n Other (1234:5678)\n -> usbip://localhost:3241/1-1\nPort 05: connected\n Sony (054c:0df2)\n -> usbip://127.0.0.1:3241/1-1";
        assert_eq!(owned_ports(text, 3241, 1), vec![1, 5]);
        assert!(owned_ports(text, 3241, 2).is_empty());
    }
}
