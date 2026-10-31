//! Event CSV with optional dwell filtering and explicit bounded-queue/disk errors.
use crate::commands::{DurationFilter, Event, Settings};
use crossbeam_queue::ArrayQueue;
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering::*},
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
    clock: Arc<AtomicU64>,
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
        let clock = Arc::new(AtomicU64::new(0));
        let (q, s, e, c) = (queue.clone(), stop.clone(), error.clone(), clock.clone());
        let worker = std::thread::Builder::new()
            .name("opends-command-file".into())
            .spawn(move || {
                if let Err(err) = write_events(file, &location, &q, &s, &c) {
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
            clock,
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
    pub fn advance(&self, now: u64) {
        if let Ok(g) = self.session.lock() {
            if let Some(s) = g.as_ref() {
                s.clock.fetch_max(now, Release);
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
    clock: &AtomicU64,
) -> std::io::Result<()> {
    let mut out = BufWriter::new(file);
    writeln!(out,"session,sequence,event,elapsed_ns,interval_ns,lx,ly,rx,ry,l2,r2,buttons0,buttons1,buttons2,ls_direction,ls_strength,rs_direction,rs_strength,l2_strength,r2_strength,dpad,buttons,combination,classification")?;
    let mut filter = DurationFilter::default();
    let mut pending_settings = Settings::default();
    let mut flushed = std::time::Instant::now();
    loop {
        for _ in 0..128 {
            let Some(row) = queue.pop() else { break };
            let current_sequence = row.event.sequence;
            let prior_settings = pending_settings.clone();
            for event in filter
                .receive(row.event, row.settings.record_min_ms)
                .into_iter()
                .flatten()
            {
                let settings = if event.sequence == current_sequence {
                    &row.settings
                } else {
                    &prior_settings
                };
                write_event(&mut out, &event, settings)?;
            }
            pending_settings = row.settings;
        }
        if queue.is_empty() {
            if let Some(event) = filter.advance(clock.load(Acquire)) {
                write_event(&mut out, &event, &pending_settings)?;
            }
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
            k.starts_with("stick_")
                || k.starts_with("trigger_")
                || k.starts_with("ignore_")
                || k.starts_with("triggerdead_")
                || k == "right"
                || k == "enabled"
                || k == "record_min_ms"
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
    fn ungraded_csv_records_active_mode_and_does_not_repeat_pressure_changes() {
        let mut h = crate::commands::History::default();
        h.settings.ignore_strength = [true; 4];
        h.settings.right = true;
        h.settings.trigger_deadzone = [10, 0];
        h.start(0, true, 8);
        let mut p = crate::monitor::neutral();
        p.lx = 150;
        p.rx = 150;
        p.l2 = 11;
        p.r2 = 1;
        let event = h.receive(1, crate::commands::Message::Input(p), 8).unwrap();
        p.lx = 255;
        p.rx = 255;
        p.l2 = 255;
        p.r2 = 255;
        assert!(h
            .receive(2, crate::commands::Message::Input(p), 8)
            .is_none());
        let mut out = Vec::new();
        write_event(&mut out, &event, &h.settings).unwrap();
        let row = String::from_utf8(out).unwrap();
        let columns = row.split(',').collect::<Vec<_>>();
        for index in [15, 17, 18, 19] {
            assert_eq!(columns[index], "4");
        }
        assert!(row.contains("ignore_0=true") && row.contains("triggerdead_0=10"));
        assert!(!columns[22].contains("L2:4"));
    }
    #[test]
    fn file_filter_is_independent_and_flushes_held_state_at_stop() {
        for minimum in [0., 16.67] {
            let path = std::env::temp_dir().join(format!(
                "opends-filter-{}-{minimum}.csv",
                std::process::id()
            ));
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            let queue = ArrayQueue::new(32);
            let mut h = crate::commands::History::default();
            h.settings.display_min_ms = 50.;
            h.settings.record_min_ms = minimum;
            h.start(0, true, 8);
            queue
                .push(Row {
                    event: h.latest.clone().unwrap(),
                    settings: h.settings.clone(),
                })
                .ok()
                .unwrap();
            for (ms, pressed) in [
                (0, false),
                (100, true),
                (110, false),
                (200, true),
                (230, false),
                (249, true),
            ] {
                let mut p = crate::monitor::neutral();
                if pressed {
                    p.buttons0 |= 16;
                }
                let event = h
                    .receive(ms * 1_000_000, crate::commands::Message::Input(p), 8)
                    .unwrap();
                queue
                    .push(Row {
                        event,
                        settings: h.settings.clone(),
                    })
                    .ok()
                    .unwrap();
            }
            h.advance_display(254_000_000, 8);
            assert!(!h
                .events
                .iter()
                .any(|e| e.kind == crate::commands::Kind::Input && e.state.buttons != 0));
            write_events(
                file,
                &path,
                &queue,
                &AtomicBool::new(true),
                &AtomicU64::new(254_000_000),
            )
            .unwrap();
            let csv = std::fs::read_to_string(&path).unwrap();
            let inputs = csv
                .lines()
                .filter(|line| line.contains(",input,"))
                .collect::<Vec<_>>();
            assert_eq!(inputs.len(), if minimum == 0. { 6 } else { 3 });
            if minimum != 0. {
                assert!(inputs[1].contains(",200000000,200000000,"));
                assert!(inputs[2].contains(",230000000,30000000,"));
                assert!(csv.contains("record_min_ms=16.67"));
            }
            std::fs::remove_file(path).unwrap();
        }
    }
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
            clock: Arc::new(AtomicU64::new(0)),
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
