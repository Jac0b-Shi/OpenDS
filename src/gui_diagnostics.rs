//! Driver/device queries never run in the window message loop.
use std::thread::JoinHandle;

pub(super) struct Snapshot {
    pub directories: (String, String),
    pub status: crate::dependencies::Status,
    pub devices: Option<Vec<(String, String)>>,
    pub hiding: Option<(bool, Vec<String>)>,
}
#[derive(Default)]
pub(super) struct Diagnostics {
    worker: Option<JoinHandle<Snapshot>>,
    pending_scan: bool,
}
impl Diagnostics {
    pub fn request(
        &mut self,
        scan: bool,
        owned: bool,
        config: crate::config::Config,
    ) -> std::io::Result<()> {
        if self.worker.is_some() {
            self.pending_scan |= scan;
            return Ok(());
        }
        self.worker = Some(
            std::thread::Builder::new()
                .name("opends-diagnostics".into())
                .spawn(move || {
                    let devices = scan.then(|| {
                        crate::hid::HidDevice::enum_paths()
                            .into_iter()
                            .filter_map(|path| {
                                let device = crate::hid::HidDevice::open(&path)?;
                                if !device.is_dualsense_bluetooth() {
                                    return None;
                                }
                                let name = format!(
                                    "{} · {}",
                                    if device.product_id == 0x0DF2 {
                                        "DualSense Edge"
                                    } else {
                                        "DualSense"
                                    },
                                    device.serial_number().unwrap_or_else(|| path.clone())
                                );
                                Some((path, name))
                            })
                            .collect()
                    });
                    let hiding = if scan && !owned {
                        crate::hidhide::HidHide::open()
                            .and_then(|h| Some((h.get_active()?, h.get_whitelist()?)))
                    } else {
                        None
                    };
                    Snapshot {
                        status: crate::dependencies::inspect(&config, owned),
                        directories: (config.usbip_directory, config.hidhide_directory),
                        devices,
                        hiding,
                    }
                })?,
        );
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Result<Snapshot, ()>> {
        if !self.worker.as_ref()?.is_finished() {
            return None;
        }
        Some(self.worker.take().unwrap().join().map_err(|_| ()))
    }
    pub fn take_pending_scan(&mut self) -> bool {
        std::mem::take(&mut self.pending_scan)
    }
}
// No join on drop: queries have no UI pointers or borrowed application state.
// A stuck driver cannot prevent the GUI from exiting.

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stalled_driver_query_never_blocks_poll_or_queues_unbounded_work() {
        let (send, receive) = std::sync::mpsc::channel();
        let mut diagnostics = Diagnostics {
            worker: Some(std::thread::spawn(move || {
                let _ = receive.recv();
                panic!("synthetic failed query")
            })),
            pending_scan: false,
        };
        let start = std::time::Instant::now();
        for _ in 0..100 {
            diagnostics
                .request(true, false, crate::config::Config::default())
                .unwrap();
            assert!(diagnostics.poll().is_none());
        }
        assert!(start.elapsed() < std::time::Duration::from_millis(100));
        assert!(diagnostics.take_pending_scan());
        assert!(!diagnostics.take_pending_scan());
        drop(diagnostics);
        send.send(()).unwrap();
    }
}
