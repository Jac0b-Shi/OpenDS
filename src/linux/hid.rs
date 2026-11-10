//! Linux hidraw backend. Only Sony Bluetooth gamepads are opened by the bridge.
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    sync::Mutex,
};

pub struct HidDevice {
    file: File,
    read_lock: Mutex<()>,
    write_lock: Mutex<()>,
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub input_len: u16,
    pub output_len: u16,
    bluetooth: bool,
    serial: Option<String>,
}

fn attributes(text: &str) -> Option<(u16, u16, u16, Option<String>)> {
    let id = text.lines().find_map(|s| s.strip_prefix("HID_ID="))?;
    let mut parts = id.split(':').map(|s| u32::from_str_radix(s, 16).ok());
    let bus = u16::try_from(parts.next()??).ok()?;
    let vid = u16::try_from(parts.next()??).ok()?;
    let pid = u16::try_from(parts.next()??).ok()?;
    let serial = text
        .lines()
        .find_map(|s| s.strip_prefix("HID_UNIQ="))
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    Some((bus, vid, pid, serial))
}

impl HidDevice {
    pub fn enum_paths() -> Vec<String> {
        let mut paths: Vec<_> = std::fs::read_dir("/sys/class/hidraw")
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let text = std::fs::read_to_string(entry.path().join("device/uevent")).ok()?;
                let (bus, vid, pid, _) = attributes(&text)?;
                (bus == 5 && vid == 0x054c && matches!(pid, 0x0ce6 | 0x0df2))
                    .then(|| format!("/dev/{}", entry.file_name().to_string_lossy()))
            })
            .collect();
        paths.sort();
        paths
    }
    pub fn open(path: &str) -> Option<Self> {
        let name = std::path::Path::new(path).file_name()?.to_str()?;
        if !name.starts_with("hidraw")
            || std::path::Path::new(path).parent()? != std::path::Path::new("/dev")
        {
            return None;
        }
        let text =
            std::fs::read_to_string(format!("/sys/class/hidraw/{name}/device/uevent")).ok()?;
        let (bus, vendor_id, product_id, serial) = attributes(&text)?;
        if bus != 5 || vendor_id != 0x054c || !matches!(product_id, 0x0ce6 | 0x0df2) {
            return None;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)
            .ok()?;
        Some(Self {
            file,
            read_lock: Mutex::new(()),
            write_lock: Mutex::new(()),
            path: path.into(),
            vendor_id,
            product_id,
            input_len: 78,
            output_len: 78,
            bluetooth: bus == 5,
            serial,
        })
    }
    fn ready(&self, events: i16, timeout_ms: u32) -> io::Result<bool> {
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms.into());
        loop {
            let mut fd = libc::pollfd {
                fd: self.file.as_raw_fd(),
                events,
                revents: 0,
            };
            let remaining = deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_millis()
                .min(i32::MAX as u128) as i32;
            let result = unsafe { libc::poll(&mut fd, 1, remaining) };
            if result < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            if fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::NotConnected,
                    "DualSense disconnected",
                ));
            }
            return Ok(result > 0 && fd.revents & events != 0);
        }
    }
    pub fn read_result(&self, timeout_ms: u32) -> io::Result<Option<Vec<u8>>> {
        let _guard = self
            .read_lock
            .lock()
            .map_err(|_| io::Error::other("HID read lock poisoned"))?;
        if !self.ready(libc::POLLIN, timeout_ms)? {
            return Ok(None);
        }
        let mut report = vec![0; 1024];
        match (&self.file).read(&mut report) {
            Ok(0) => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "DualSense disconnected",
            )),
            Ok(n) => {
                report.truncate(n);
                Ok(Some(report))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }
    pub fn write_result(&self, data: &[u8], timeout_ms: u32) -> io::Result<()> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| io::Error::other("HID write lock poisoned"))?;
        if !self.ready(libc::POLLOUT, timeout_ms)? {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "HID write timed out",
            ));
        }
        // A hidraw write is one report, so never split a short write into a second report.
        if (&self.file).write(data)? != data.len() {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "Incomplete HID report",
            ));
        }
        Ok(())
    }
    pub fn write(&self, data: &[u8]) -> bool {
        self.write_result(data, 200).is_ok()
    }
    pub fn serial_number(&self) -> Option<String> {
        self.serial.clone()
    }
    pub fn is_dualsense_bluetooth(&self) -> bool {
        self.bluetooth && self.is_sony() && matches!(self.product_id, 0x0ce6 | 0x0df2)
    }
    pub fn is_sony(&self) -> bool {
        self.vendor_id == 0x054c
    }
    pub fn feature_report(&self, id: u8, length: usize, _timeout: u32) -> io::Result<Vec<u8>> {
        if !(2..=1024).contains(&length) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid feature report length",
            ));
        }
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| io::Error::other("HID write lock poisoned"))?;
        let mut report = vec![0; length];
        report[0] = id;
        // HIDIOCGFEATURE(len), from linux/hidraw.h. The kernel bounds this synchronous request.
        let request = 0xc0004807u64 | ((length as u64) << 16);
        let n = unsafe {
            libc::ioctl(
                self.file.as_raw_fd(),
                request as libc::c_ulong,
                report.as_mut_ptr(),
            )
        };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        if (n as usize) < length {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Short HID feature report",
            ));
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_bus_and_identity_without_confusing_usb_for_bluetooth() {
        let (bus, vid, pid, serial) =
            attributes("HID_ID=0005:0000054C:00000CE6\nHID_UNIQ=aa:bb:cc:dd:ee:ff\n").unwrap();
        assert_eq!((bus, vid, pid), (5, 0x054c, 0x0ce6));
        assert_eq!(serial.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
        assert!(attributes("HID_ID=invalid").is_none());
        assert!(HidDevice::open("/dev/null").is_none());
    }
}
