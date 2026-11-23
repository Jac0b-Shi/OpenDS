use std::{os::windows::process::CommandExt, process::Command, sync::mpsc};
use winapi::shared::windef::HWND;

const SOURCES: [(&str, &str); 2] = [
    (
        "https://api.github.com/repos/Jac0b-Shi/OpenDS/releases/latest",
        "https://github.com/Jac0b-Shi/OpenDS/releases",
    ),
    (
        "https://gitee.com/api/v5/repos/Jac0b_Shi/OpenDS/releases/latest",
        "https://gitee.com/Jac0b_Shi/OpenDS/releases",
    ),
];

pub type Version = [u64; 3];

fn parse(tag: &str) -> Option<Version> {
    let mut parts = tag.trim().trim_start_matches('v').split('.');
    let mut version = [0; 3];
    for number in &mut version {
        let part = parts.next()?;
        if !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *number = part.parse().ok()?;
    }
    parts.next().is_none().then_some(version)
}

pub fn start() -> mpsc::Receiver<Version> {
    let (tx, rx) = mpsc::channel();
    for (api, _) in SOURCES {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let script = format!(
                "$ErrorActionPreference='Stop'; [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; $r=Invoke-RestMethod -UserAgent OpenDS -TimeoutSec 12 -Uri '{api}'; if (!$r.draft -and !$r.prerelease) {{ $r.tag_name }}"
            );
            let result = Command::new("powershell.exe")
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    &script,
                ])
                .creation_flags(0x08000000)
                .output();
            if let Ok(output) = result {
                if output.status.success() {
                    if let (Some(remote), Some(local)) = (
                        parse(&String::from_utf8_lossy(&output.stdout)),
                        parse(env!("CARGO_PKG_VERSION")),
                    ) {
                        if remote > local {
                            let _ = tx.send(remote);
                        }
                    }
                }
            }
        });
    }
    rx
}

pub fn show(parent: HWND, [major, minor, patch]: Version) -> Result<(), String> {
    let message = format!(
        "{}  v{}\r\n{}  v{major}.{minor}.{patch}",
        crate::tr!(update_current),
        env!("CARGO_PKG_VERSION"),
        crate::tr!(update_latest)
    );
    let response = crate::theme_dialog::show(
        parent,
        crate::theme_dialog::Request {
            title: crate::tr!(update_title),
            message: &message,
            buttons: &[
                (10, crate::tr!(update_github)),
                (11, crate::tr!(update_gitee)),
            ],
            default: 11,
            cancel: 2,
            verification: None,
            test: None,
        },
    )?;
    let url = match response.button {
        10 => SOURCES[0].1,
        11 => SOURCES[1].1,
        _ => return Ok(()),
    };
    Command::new("explorer.exe")
        .arg(url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn compares_stable_release_tags_numerically() {
        assert!(parse("v0.10.0").unwrap() > parse("0.9.12").unwrap());
        assert_eq!(parse("v0.4.3\r\n"), Some([0, 4, 3]));
        for invalid in ["v0.5.0-beta.1", "v0.5", "v1..2", "v1.2.3.4"] {
            assert_eq!(parse(invalid), None);
        }
    }
}
