//! 预载特性后再挂接；跟踪本会话端口并清理同一端点的旧会话。
use std::os::windows::process::CommandExt;
use std::{
    path::PathBuf,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

pub fn usbip_path() -> Option<PathBuf> {
    for key in ["ProgramW6432", "ProgramFiles"] {
        if let Some(root) = std::env::var_os(key) {
            let path = PathBuf::from(root).join("USBip/usbip.exe");
            if path.is_file() {
                return Some(path);
            }
        }
    }
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .map(|p| p.join("usbip.exe"))
            .find(|p| p.is_file())
    })
}
fn run(
    args: Vec<String>,
    cancelled: impl Fn() -> bool,
    timeout: Duration,
) -> Result<Output, String> {
    let exe = usbip_path()
        .ok_or("找不到 USB/IP Win2 自带的 usbip.exe，请安装 compact 主程序或将其目录加入 PATH")?;
    let mut child = Command::new(exe)
        .args(args)
        .creation_flags(0x08000000)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 USB/IP 操作失败：{e}"))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().map_err(|e| e.to_string())?;
                if status.success() {
                    return Ok(output);
                }
                return Err(format!(
                    "USB/IP 操作失败：{} {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
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
            return Err("USB/IP 操作已取消或超时".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
pub fn first_device(port: u16, bus: u32, cancelled: impl Fn() -> bool) -> Result<u32, String> {
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
    let output = run(args, cancelled, Duration::from_secs(5))?;
    let port = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .map_err(|_| "挂接成功但未返回有效 UDE 端口".to_string())?;
    if !(1..=255).contains(&port) {
        return Err("UDE 端口超出范围".into());
    }
    Ok(port)
}
pub fn detach(port: u32) -> Result<(), String> {
    if !(1..=255).contains(&port) {
        return Err("拒绝卸载无效 UDE 端口".into());
    }
    run(
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
pub fn cleanup_owned(port: u16, bus: u32, cancelled: impl Fn() -> bool) -> Result<(), String> {
    for host in ["localhost", "127.0.0.1"] {
        run(
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
    let list = run(vec!["port".into()], cancelled, Duration::from_secs(2))?;
    for own in owned_ports(&String::from_utf8_lossy(&list.stdout), port, bus) {
        detach(own)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_only_selects_sony_devices_on_our_exact_loopback_endpoint() {
        let text="Port 01: connected\n Sony (054c:0ce6)\n -> usbip://localhost:3241/1-1\nPort 02: connected\n Sony (054c:0ce6)\n -> usbip://192.168.1.5:3241/1-1\nPort 03: connected\n Sony (054c:0ce6)\n -> usbip://localhost:3242/1-1\nPort 04: connected\n Other (1234:5678)\n -> usbip://localhost:3241/1-1\nPort 05: connected\n Sony (054c:0df2)\n -> usbip://127.0.0.1:3241/1-1";
        assert_eq!(owned_ports(text, 3241, 1), vec![1, 5]);
        assert!(owned_ports(text, 3241, 2).is_empty());
    }
}
