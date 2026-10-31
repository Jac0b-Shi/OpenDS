//! Lossless event CSV until a bounded queue or disk error explicitly stops recording.
use crate::commands::{Event, Settings};
use crossbeam_queue::ArrayQueue;
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering::*},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};
struct Row {
    event: Event,
    settings: Settings,
}
struct Session {
    queue: Arc<ArrayQueue<Row>>,
    stop: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    worker: JoinHandle<()>,
}
/// Optional append-only CSV writer for input history.
///
/// One background thread owns the file. `submit` only pushes a row and unparks
/// it; when the 2048-row queue fills, recording stops and the reason is kept in
/// `error` rather than blocking the input path or dropping rows silently.
#[derive(Default)]
pub struct Recording {
    session: Mutex<Option<Session>>,
}
pub fn queue_bytes() -> usize {
    2048 * (std::mem::size_of::<Row>() + std::mem::size_of::<usize>()) + 8192
}
impl Recording {
    pub fn allocated_bytes(&self) -> usize {
        self.session
            .lock()
            .map(|s| {
                if s.is_some() {
                    2048 * (std::mem::size_of::<Row>() + std::mem::size_of::<usize>()) + 8192
                } else {
                    0
                }
            })
            .unwrap_or(0)
    }
    /// Open `path` for a new recording.
    ///
    /// `create_new` is deliberate: an existing `.commands.csv` is never
    /// appended to or overwritten, so a second run fails loudly instead of
    /// mixing two sessions into one file.
    pub fn start(&self, path: &Path) -> Result<(), String> {
        self.finish();
        crate::monitor_recording::require_space(
            crate::monitor_recording::available_space(path).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let location = path.to_owned();
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        let queue: Arc<ArrayQueue<Row>> = Arc::new(ArrayQueue::new(2048));
        let stop = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let (q, s, e) = (queue.clone(), stop.clone(), error.clone());
        let worker = std::thread::Builder::new()
            .name("opends-command-file".into())
            .spawn(move || {
                if let Err(err) = write_events(file, &location, &q, &s) {
                    if let Ok(mut error) = e.lock() {
                        *error = Some(err.to_string());
                    }
                    s.store(true, Release);
                }
            })
            .map_err(|e| e.to_string())?;
        *self
            .session
            .lock()
            .map_err(|_| "Command writer unavailable")? = Some(Session {
            queue,
            stop,
            error,
            worker,
        });
        Ok(())
    }
    pub fn submit(&self, event: &Event, settings: &Settings) {
        if let Ok(g) = self.session.lock() {
            if let Some(s) = g.as_ref() {
                if !s.stop.load(Acquire) {
                    if s.queue
                        .push(Row {
                            event: event.clone(),
                            settings: settings.clone(),
                        })
                        .is_err()
                    {
                        if let Ok(mut e) = s.error.lock() {
                            *e = Some("Command recording queue full; recording stopped".into());
                        }
                        s.stop.store(true, Release);
                    }
                    s.worker.thread().unpark();
                }
            }
        }
    }
    pub fn stop(&self) {
        if let Ok(g) = self.session.lock() {
            if let Some(s) = g.as_ref() {
                s.stop.store(true, Release);
                s.worker.thread().unpark();
            }
        }
    }
    pub fn error(&self) -> Option<String> {
        self.session.lock().ok().and_then(|s| {
            s.as_ref()
                .and_then(|s| s.error.lock().ok().and_then(|e| e.clone()))
        })
    }
    pub fn finish(&self) {
        self.stop();
        if let Some(s) = self.session.lock().ok().and_then(|mut s| s.take()) {
            let _ = crate::shutdown::join(s.worker);
        }
    }
}
fn write_events(
    file: std::fs::File,
    location: &Path,
    queue: &ArrayQueue<Row>,
    stop: &AtomicBool,
) -> std::io::Result<()> {
    let mut out = BufWriter::new(file);
    writeln!(out,"session,sequence,event,elapsed_ns,interval_ns,lx,ly,rx,ry,l2,r2,buttons0,buttons1,buttons2,ls_direction,ls_strength,rs_direction,rs_strength,l2_strength,r2_strength,dpad,buttons,combination,classification")?;
    let mut flushed = std::time::Instant::now();
    loop {
        for _ in 0..128 {
            let Some(row) = queue.pop() else { break };
            write_event(&mut out, &row.event, &row.settings)?;
        }
        if stop.load(Acquire) && queue.is_empty() {
            break;
        }
        if flushed.elapsed() >= Duration::from_secs(1) {
            crate::monitor_recording::require_space(crate::monitor_recording::available_space(
                location,
            )?)?;
            out.flush()?;
            flushed = std::time::Instant::now();
        }
        if queue.is_empty() {
            std::thread::park_timeout(Duration::from_millis(100));
        }
    }
    out.flush()?;
    out.get_ref().sync_data()?;
    Ok(())
}
fn write_event(out: &mut impl Write, event: &Event, settings: &Settings) -> std::io::Result<()> {
    let classification = settings
        .fields()
        .into_iter()
        .filter(|(k, _)| {
            k.starts_with("stick_") || k.starts_with("trigger_") || k == "right" || k == "enabled"
        })
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(";");
    let raw = event
        .raw
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let s = event.state;
    writeln!(
        out,
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},\"{}\",\"{}\"",
        event.session,
        event.sequence,
        event.kind.name(),
        event.time,
        event.delta.map(|v| v.to_string()).unwrap_or_default(),
        raw,
        s.sticks[0].direction,
        s.sticks[0].strength,
        s.sticks[1].direction,
        s.sticks[1].strength,
        s.triggers[0],
        s.triggers[1],
        s.dpad,
        s.buttons,
        crate::commands::combination(s, settings.right),
        classification
    )
}
impl Drop for Recording {
    fn drop(&mut self) {
        self.finish();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_overflow_is_explicit_and_bounded() {
        let recording = Recording::default();
        let queue = Arc::new(ArrayQueue::new(2));
        let stop = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let (send, receive) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _ = receive.recv();
        });
        *recording.session.lock().unwrap() = Some(Session {
            queue: queue.clone(),
            stop: stop.clone(),
            error,
            worker,
        });
        let mut history = crate::commands::History::default();
        history.start(0, true, 8);
        for _ in 0..3 {
            recording.submit(history.latest.as_ref().unwrap(), &history.settings);
        }
        assert_eq!(queue.len(), 2);
        assert!(stop.load(Acquire));
        assert!(recording.error().is_some());
        send.send(()).unwrap();
        recording.finish();
    }
    #[test]
    fn disk_history_outlives_visible_history_and_preserves_subframe_events() {
        let path = std::env::temp_dir().join(format!(
            "opends-command-stream-{}-{}.csv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut companion = path.clone();
        companion.set_extension("commands.csv");
        let monitor = crate::monitor::Monitor::new();
        monitor.commands.main_capacity.store(2, Relaxed);
        monitor.start(20).unwrap();
        monitor.connection(true);
        monitor.start_recording(path.clone()).unwrap();
        for index in 0..200 {
            let mut p = crate::monitor::neutral();
            if index % 2 == 0 {
                p.buttons0 |= 16;
            }
            monitor.input(p);
        }
        monitor.stop();
        monitor.commands.recording.finish();
        monitor.recording.finish();
        assert!(monitor.commands.history.lock().unwrap().events.len() <= 2);
        let csv = std::fs::read_to_string(&companion).unwrap();
        assert_eq!(csv.lines().filter(|r| r.contains(",input,")).count(), 200);
        for line in csv.lines() {
            assert_eq!(line.split(',').count(), 24);
        }
        assert!(csv.contains("stick_0_0=12"));
        assert!(!csv.contains("touch1_x"));
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(companion).unwrap();
    }
    #[test]
    fn writer_failure_is_returned() {
        struct Failed;
        impl Write for Failed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("test disk error"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut history = crate::commands::History::default();
        history.start(0, true, 8);
        assert!(write_event(
            &mut Failed,
            history.latest.as_ref().unwrap(),
            &history.settings
        )
        .is_err());
    }
    #[test]
    fn never_overwrites_and_records_events() {
        let p = std::env::temp_dir().join(format!(
            "opends-commands-{}-{}.csv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let r = Recording::default();
        r.start(&p).unwrap();
        let mut h = crate::commands::History::default();
        h.start(0, true, 8);
        r.submit(h.latest.as_ref().unwrap(), &h.settings);
        r.finish();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("connected"));
        assert_eq!(text.lines().count(), 2);
        assert!(r.start(&p).is_err());
        assert_eq!(text, std::fs::read_to_string(&p).unwrap());
        std::fs::remove_file(p).unwrap();
    }
}
