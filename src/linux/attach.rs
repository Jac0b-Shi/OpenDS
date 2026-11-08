//! Linux usbip has neither --once/--terse nor the Windows port numbering.
use std::{
    path::Path,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

fn run(
    exe: &Path,
    args: &[String],
    privileged: bool,
    cancelled: impl Fn() -> bool,
) -> Result<Output, String> {
    let mut command = if privileged && unsafe { libc::geteuid() } != 0 {
        let mut command = Command::new("pkexec");
        command.arg(exe);
        command
    } else {
        Command::new(exe)
    };
    let mut child = command
        .args(args)
        .env("LC_ALL", "C")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start USB/IP: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child.wait_with_output().map_err(|e| e.to_string())?;
                if output.status.success() {
                    return Ok(output);
                }
                return Err(format!(
                    "USB/IP failed: {}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
            _ => {}
        }
        if cancelled() || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("USB/IP operation cancelled or timed out".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn owned_ports(text: &str, service: u16, bus: u32) -> Vec<u32> {
    let endpoints = [
        format!("usbip://127.0.0.1:{service}/{bus}-1"),
        format!("usbip://localhost:{service}/{bus}-1"),
    ];
    let mut port = None;
    let mut sony = false;
    let mut result = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("Port ") {
            port = rest
                .split(':')
                .next()
                .and_then(|s| s.parse::<u32>().ok())
                .filter(|p| *p <= 65535);
            sony = false;
        } else if line.contains("(054c:0ce6)") || line.contains("(054c:0df2)") {
            sony = true;
        } else if sony
            && line
                .split_once("-> ")
                .is_some_and(|(_, v)| endpoints.iter().any(|e| v == e))
        {
            if let Some(port) = port {
                result.push(port);
            }
        }
    }
    result
}
fn ports(
    exe: &Path,
    service: u16,
    bus: u32,
    cancelled: impl Fn() -> bool,
) -> Result<Vec<u32>, String> {
    Ok(owned_ports(&port_listing(exe, cancelled)?, service, bus))
}
fn port_listing(exe: &Path, cancelled: impl Fn() -> bool) -> Result<String, String> {
    let output = run(exe, &["port".into()], false, &cancelled)?;
    let mut listing = String::from_utf8_lossy(&output.stdout).into_owned();
    // Ubuntu's usbip creates /run/vhci_hcd with mode 0700. Never infer ownership
    // from a VID/PID alone when an unprivileged listing cannot read the endpoint.
    if listing.contains("unknown host, remote port and remote busid") {
        let output = run(exe, &["port".into()], true, &cancelled)?;
        listing = String::from_utf8_lossy(&output.stdout).into_owned();
    }
    Ok(listing)
}
pub fn first_device(
    exe: &Path,
    port: u16,
    bus: u32,
    cancelled: impl Fn() -> bool,
) -> Result<u32, String> {
    run(
        exe,
        &[
            "--tcp-port".into(),
            port.to_string(),
            "attach".into(),
            "-r".into(),
            "127.0.0.1".into(),
            "-b".into(),
            format!("{bus}-1"),
        ],
        true,
        &cancelled,
    )?;
    // Enumeration is asynchronous. If authentication is cancelled, session cleanup
    // closes the server connection and the kernel releases the imported device.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if cancelled() {
            return Err("USB/IP port discovery cancelled".into());
        }
        let listing = match port_listing(exe, &cancelled) {
            Ok(listing) => listing,
            Err(error) => error,
        };
        if let Some(port) = owned_ports(&listing, port, bus).into_iter().next() {
            return Ok(port);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "USB/IP attached but its local VHCI port could not be identified: {listing}"
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
pub fn detach(exe: &Path, port: u32) -> Result<(), String> {
    if port > 65535 {
        return Err("Invalid VHCI port".into());
    }
    run(
        exe,
        &["detach".into(), "-p".into(), port.to_string()],
        true,
        || false,
    )
    .map(|_| ())
}
pub fn cleanup_owned(
    exe: &Path,
    service: u16,
    bus: u32,
    cancelled: impl Fn() -> bool,
) -> Result<(), String> {
    for port in ports(exe, service, bus, &cancelled)? {
        if cancelled() {
            return Err("USB/IP cleanup cancelled".into());
        }
        detach(exe, port)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_our_sony_loopback_endpoint_is_owned_and_port_zero_is_valid() {
        // Actual Linux output prefixes the URL with the local USB bus ID.
        let text = "Port 00: <Port in Use> at High Speed(480Mbps)\n Sony Corp. : DualSense wireless controller (054c:0ce6)\n 5-1 -> usbip://127.0.0.1:3241/1-1\nPort 01: busy\n Sony (054c:0df2)\n 5-2 -> usbip://localhost:3241/1-1\nPort 02: busy\n Sony (054c:0ce6)\n 5-3 -> usbip://remote:3241/1-1\nPort 03: busy\n Other (1234:5678)\n 5-4 -> usbip://localhost:3241/1-1\nPort 04: busy\n Sony (054c:0ce6)\n 5-5 -> usbip://localhost:3241/1-10\n";
        assert_eq!(owned_ports(text, 3241, 1), vec![0, 1]);
        assert!(owned_ports(text, 3242, 1).is_empty());
        assert!(owned_ports(text, 3241, 2).is_empty());
    }
}
