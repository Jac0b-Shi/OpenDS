//! Bounded CSV writer. Only the monitor worker submits rows; device callbacks do no I/O.
use crate::{monitor::Bucket, report::ParsedState};
use crossbeam_queue::ArrayQueue;
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    os::windows::ffi::OsStrExt,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering::*},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CAPACITY: usize = 2048;
pub const DISK_RESERVE: u64 = 512 * 1024 * 1024;
const WRITE_BUFFER: usize = 64 * 1024;
#[link(name = "kernel32")]
extern "system" {
    fn GetDiskFreeSpaceExW(
        directory: *const u16,
        available: *mut u64,
        total: *mut u64,
        free: *mut u64,
    ) -> i32;
}
fn available_space(path: &std::path::Path) -> std::io::Result<u64> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut dir = absolute
        .parent()
        .ok_or_else(|| std::io::Error::other("保存目录无效"))?
        .as_os_str()
        .encode_wide()
        .collect::<Vec<_>>();
    if !matches!(dir.last(), Some(47 | 92)) {
        dir.push(92);
    }
    dir.push(0);
    let mut available = 0u64;
    if unsafe {
        GetDiskFreeSpaceExW(
            dir.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::other(format!(
            "无法检查目标磁盘可用空间：{}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(available)
}
fn require_space(available: u64) -> std::io::Result<()> {
    if available < DISK_RESERVE + WRITE_BUFFER as u64 {
        return Err(std::io::Error::other(format!(
            "目标磁盘剩余 {:.1} MiB，已停止记录以预留 512 MiB；桥接和采集继续运行",
            available as f64 / 1048576.
        )));
    }
    Ok(())
}
pub fn queue_bytes() -> usize {
    CAPACITY * (std::mem::size_of::<Frame>() + std::mem::size_of::<usize>()) + WRITE_BUFFER
}
#[derive(Clone, Copy, Default)]
pub struct Frame {
    pub time: u64,
    pub input_time: u64,
    pub epoch: u64,
    pub generation: u64,
    pub connected: bool,
    pub source: bool,
    pub forwarded: bool,
    pub input: ParsedState,
    pub bucket: Bucket,
    pub drops: u64,
    pub decode_failures: u64,
    pub resets: u64,
}
#[derive(Default)]
struct Progress {
    active: AtomicBool,
    ready: AtomicBool,
    rows: AtomicU64,
    bytes: AtomicU64,
    dropped: AtomicU64,
    error: Mutex<Option<String>>,
}
struct Session {
    queue: Arc<ArrayQueue<Frame>>,
    stop: Arc<AtomicBool>,
    progress: Arc<Progress>,
    worker: JoinHandle<()>,
    next: u64,
    path: PathBuf,
}
#[derive(Default)]
pub struct Recording {
    session: Mutex<Option<Session>>,
}
#[derive(Default)]
pub struct Status {
    pub active: bool,
    pub starting: bool,
    pub finishing: bool,
    pub rows: u64,
    pub bytes: u64,
    pub dropped: u64,
    pub path: Option<PathBuf>,
    pub error: Option<String>,
}
impl Recording {
    pub fn start(&self, path: PathBuf, hz: u32, now: u64) -> Result<(), String> {
        let mut guard = self.session.lock().map_err(|_| "记录状态不可用")?;
        if guard.as_ref().is_some_and(|s| !s.worker.is_finished()) {
            return Err("记录仍在运行或正在保存，请稍后重试".into());
        }
        if let Some(old) = guard.take() {
            let _ = old.worker.join();
        }
        let queue = Arc::new(ArrayQueue::new(CAPACITY));
        let stop = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Progress::default());
        progress.active.store(true, Release);
        let (q, end, p, file) = (queue.clone(), stop.clone(), progress.clone(), path.clone());
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let worker = std::thread::Builder::new()
            .name("opends-monitor-csv".into())
            .spawn(move || {
                let result = write_csv(file, q, end, &p, hz, now, unix_ms);
                if let Err(e) = result {
                    crate::logging::record(&format!("CSV 记录失败：{e}"));
                    if let Ok(mut error) = p.error.lock() {
                        *error = Some(format!("CSV 记录失败：{e}"));
                    }
                }
                p.active.store(false, Release);
            })
            .map_err(|e| e.to_string())?;
        *guard = Some(Session {
            queue,
            stop,
            progress,
            worker,
            next: now.saturating_mul(hz as u64).div_ceil(1_000_000_000),
            path,
        });
        Ok(())
    }
    pub fn active(&self) -> bool {
        self.session
            .try_lock()
            .ok()
            .and_then(|s| s.as_ref().map(|s| s.progress.active.load(Acquire)))
            .unwrap_or(false)
    }
    pub fn sample(&self, now: u64, hz: u32, mut frame: impl FnMut(u64) -> Frame) {
        let Ok(mut guard) = self.session.try_lock() else {
            return;
        };
        let Some(s) = guard.as_mut() else { return };
        if !s.progress.active.load(Acquire) {
            return;
        }
        let end = now.saturating_mul(hz as u64) / 1_000_000_000;
        let mut count = 0;
        while s.next < end {
            if count >= CAPACITY || s.queue.push(frame(s.next)).is_err() {
                s.progress.dropped.fetch_add(end - s.next, Relaxed);
                if let Ok(mut error) = s.progress.error.lock() {
                    *error = Some(
                        "记录缓冲已满或采集延迟过长，已停止记录；已有数据正在保存，采集和桥接继续运行"
                            .into(),
                    );
                }
                s.progress.active.store(false, Release);
                s.stop.store(true, Release);
                break;
            }
            s.next += 1;
            count += 1;
        }
        s.worker.thread().unpark();
    }
    pub fn stop(&self) {
        if let Ok(guard) = self.session.lock() {
            if let Some(s) = guard.as_ref() {
                s.progress.active.store(false, Release);
                s.stop.store(true, Release);
                s.worker.thread().unpark();
            }
        }
    }
    pub fn status(&self) -> Status {
        self.session
            .lock()
            .ok()
            .and_then(|s| {
                s.as_ref().map(|s| Status {
                    active: s.progress.active.load(Acquire),
                    starting: s.progress.active.load(Acquire) && !s.progress.ready.load(Acquire),
                    finishing: !s.progress.active.load(Acquire) && !s.worker.is_finished(),
                    rows: s.progress.rows.load(Relaxed),
                    bytes: s.progress.bytes.load(Relaxed),
                    dropped: s.progress.dropped.load(Relaxed),
                    path: Some(s.path.clone()),
                    error: s.progress.error.lock().ok().and_then(|e| e.clone()),
                })
            })
            .unwrap_or_default()
    }
    pub fn allocated_bytes(&self) -> usize {
        self.session
            .lock()
            .ok()
            .and_then(|s| s.as_ref().map(|_| queue_bytes()))
            .unwrap_or(0)
    }
}
impl Drop for Recording {
    fn drop(&mut self) {
        self.stop();
        if let Ok(guard) = self.session.get_mut() {
            if let Some(s) = guard.take() {
                let _ = s.worker.join();
            }
        }
    }
}
fn header() -> String {
    let mut s = String::from("record_type,unix_ms,elapsed_ns,sample_hz,connected,source_enabled,forwarded_enabled,epoch,generation,input_elapsed_ns,lx,ly,rx,ry,l2,r2,buttons0,buttons1,buttons2,input_seq,touch1_active,touch1_id,touch1_x,touch1_y,touch2_active,touch2_id,touch2_x,touch2_y,gyro_x,gyro_y,gyro_z,accel_x,accel_y,accel_z,imu_timestamp,temperature,device_timestamp,battery_level,battery_status,connection,valid_mask,gap_mask,monitor_drops,decode_failures,stream_resets,recording_drops");
    for lane in [
        "source_speaker_l",
        "source_speaker_r",
        "source_haptic_l",
        "source_haptic_r",
        "source_microphone",
        "forwarded_speaker_l",
        "forwarded_speaker_r",
        "forwarded_haptic_l",
        "forwarded_haptic_r",
        "forwarded_microphone",
    ] {
        s.push_str(&format!(",{lane}_min_i16,{lane}_max_i16"));
    }
    s.push('\n');
    s
}
fn row(kind: &str, f: Frame, hz: u32, origin: u64, unix_ms: u128, drops: u64) -> String {
    let s = f.input;
    let mut values = vec![
        kind.to_string(),
        (unix_ms + f.time.saturating_sub(origin) as u128 / 1_000_000).to_string(),
        f.time.saturating_sub(origin).to_string(),
        hz.to_string(),
        u8::from(f.connected).to_string(),
        u8::from(f.source).to_string(),
        u8::from(f.forwarded).to_string(),
        f.epoch.to_string(),
        f.generation.to_string(),
    ];
    macro_rules! fields { ($($v:expr),* $(,)?) => { $(values.push($v.to_string());)* }; }
    fields!(
        f.input_time as i128 - origin as i128,
        s.lx,
        s.ly,
        s.rx,
        s.ry,
        s.l2,
        s.r2,
        s.buttons0,
        s.buttons1,
        s.buttons2,
        s.seq,
        u8::from(s.touch1.active),
        s.touch1.tracking_id,
        s.touch1.x,
        s.touch1.y,
        u8::from(s.touch2.active),
        s.touch2.tracking_id,
        s.touch2.x,
        s.touch2.y,
        s.motion.gyro[0],
        s.motion.gyro[1],
        s.motion.gyro[2],
        s.motion.accel[0],
        s.motion.accel[1],
        s.motion.accel[2],
        s.motion.timestamp,
        s.motion.temperature,
        s.device_timestamp,
        s.battery_level,
        s.battery_status,
        s.connection,
        f.bucket.valid,
        f.bucket.gap,
        f.drops,
        f.decode_failures,
        f.resets,
        drops
    );
    for lane in 0..10 {
        let valid = kind == "sample" && f.bucket.valid & (1 << lane) != 0;
        values.push(if valid {
            f.bucket.min[lane].to_string()
        } else {
            String::new()
        });
        values.push(if valid {
            f.bucket.max[lane].to_string()
        } else {
            String::new()
        });
    }
    values.join(",") + "\n"
}
fn write_csv(
    path: PathBuf,
    queue: Arc<ArrayQueue<Frame>>,
    stop: Arc<AtomicBool>,
    progress: &Progress,
    hz: u32,
    origin: u64,
    unix_ms: u128,
) -> std::io::Result<()> {
    let location = path.clone();
    write_csv_checked(path, queue, stop, progress, hz, origin, unix_ms, || {
        available_space(&location)
    })
}
fn write_csv_checked(
    path: PathBuf,
    queue: Arc<ArrayQueue<Frame>>,
    stop: Arc<AtomicBool>,
    progress: &Progress,
    hz: u32,
    origin: u64,
    unix_ms: u128,
    mut space: impl FnMut() -> std::io::Result<u64>,
) -> std::io::Result<()> {
    let available = space()?;
    require_space(available)?; // Check before opening/truncating an existing file.
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!(
                    "无法写入所选文件 {}：{e}。请检查保存位置的写入权限。",
                    path.display()
                ),
            )
        })?;
    let mut out = BufWriter::with_capacity(WRITE_BUFFER, file);
    let head = header();
    out.write_all(head.as_bytes())?;
    out.flush()?;
    progress.ready.store(true, Release);
    progress.bytes.store(head.len() as u64, Relaxed);
    let mut last = Frame {
        time: origin,
        ..Frame::default()
    };
    let mut flush = Instant::now();
    let mut checked = Instant::now();
    let mut checked_bytes = 0u64;
    let mut space_error = None;
    'writing: loop {
        if checked.elapsed() >= Duration::from_secs(1)
            || progress.bytes.load(Relaxed).saturating_sub(checked_bytes) >= WRITE_BUFFER as u64
        {
            let result = space().and_then(|available| require_space(available));
            if let Err(error) = result {
                progress.active.store(false, Release);
                progress.dropped.fetch_add(queue.len() as u64, Relaxed);
                space_error = Some(error);
                break 'writing;
            }
            checked = Instant::now();
            checked_bytes = progress.bytes.load(Relaxed);
        }
        // Bounded batches also keep the space check responsive if the producer
        // continuously refills the queue while the disk is being written.
        for _ in 0..64 {
            let Some(frame) = queue.pop() else { break };
            last = frame;
            let line = row(
                "sample",
                frame,
                hz,
                origin,
                unix_ms,
                progress.dropped.load(Relaxed),
            );
            out.write_all(line.as_bytes())?;
            progress.bytes.fetch_add(line.len() as u64, Relaxed);
            progress.rows.fetch_add(1, Relaxed);
        }
        if stop.load(Acquire) && queue.is_empty() {
            break;
        }
        if flush.elapsed() >= Duration::from_secs(1) {
            out.flush()?;
            flush = Instant::now();
        }
        if queue.is_empty() {
            std::thread::park_timeout(Duration::from_millis(250));
        }
    }
    let end = row(
        "end",
        last,
        hz,
        origin,
        unix_ms,
        progress.dropped.load(Relaxed),
    );
    out.write_all(end.as_bytes())?;
    out.flush()?;
    out.get_ref().sync_data()?;
    progress.bytes.fetch_add(end.len() as u64, Relaxed);
    if let Some(error) = space_error {
        Err(error)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn temp_file(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "OpenDS CSV 测试-{}-{}-{label}.csv",
            std::process::id(),
            NEXT.fetch_add(1, Relaxed)
        ))
    }
    fn finish(r: &Recording) -> Status {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let status = r.status();
            if !status.active && !status.finishing {
                return status;
            }
            assert!(Instant::now() < deadline, "CSV writer did not finish");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn continuous_rows_keep_schema_signed_extrema_and_missing_lanes() {
        let path = temp_file("continuous");
        let r = Recording::default();
        r.start(path.clone(), 100, 5_000_000).unwrap();
        r.sample(40_000_000, 100, |tick| {
            let mut f = Frame {
                time: (tick + 1) * 10_000_000,
                input_time: 1_000_000,
                epoch: 2,
                generation: 3,
                connected: true,
                source: true,
                ..Frame::default()
            };
            f.input.lx = 128;
            f.input.motion.gyro[0] = -123;
            f.bucket.valid = 1 | (1 << 4);
            f.bucket.gap = 1 << 9;
            f.bucket.min[0] = i16::MIN;
            f.bucket.max[0] = i16::MAX;
            f
        });
        r.stop();
        // A producer call after manual stop must never append more rows.
        r.sample(80_000_000, 100, |_| panic!("recording already stopped"));
        let s = finish(&r);
        assert_eq!(s.rows, 3);
        assert!(s.error.is_none());
        let csv = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = csv
            .lines()
            .map(|l| l.split(',').collect::<Vec<_>>())
            .collect();
        assert_eq!(lines.len(), 5); // header, three complete intervals, end
        assert_eq!(lines[0].len(), 66);
        assert!(lines.iter().all(|l| l.len() == lines[0].len()));
        let field =
            |row: usize, name: &str| lines[row][lines[0].iter().position(|n| *n == name).unwrap()];
        assert_eq!(field(1, "elapsed_ns"), "15000000");
        assert_eq!(field(1, "lx"), "128");
        assert_eq!(field(1, "gyro_x"), "-123");
        assert_eq!(field(1, "source_speaker_l_min_i16"), "-32768");
        assert_eq!(field(1, "source_speaker_l_max_i16"), "32767");
        assert_eq!(field(1, "source_microphone_min_i16"), "0");
        assert_eq!(field(1, "forwarded_microphone_min_i16"), "");
        assert_eq!(field(1, "gap_mask"), "512");
        assert_eq!(field(4, "record_type"), "end");
        assert_eq!(field(4, "source_speaker_l_min_i16"), "");
        assert_eq!(s.bytes, csv.len() as u64);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn unavailable_destination_is_reported_and_next_recording_can_start() {
        let path = temp_file("missing-parent").join("record.csv");
        let r = Recording::default();
        r.start(path, 20, 0).unwrap();
        assert!(finish(&r).error.unwrap().contains("CSV 记录失败"));
        let good = temp_file("retry");
        r.start(good.clone(), 20, 0).unwrap();
        r.stop();
        assert!(finish(&r).error.is_none());
        std::fs::remove_file(good).unwrap();
    }
    #[test]
    fn overflow_is_bounded_stops_only_recording_and_preserves_drop_count() {
        let path = temp_file("overflow");
        let r = Recording::default();
        r.start(path.clone(), 200, 0).unwrap();
        let start = Instant::now();
        r.sample((CAPACITY as u64 + 7) * 5_000_000, 200, |tick| Frame {
            time: (tick + 1) * 5_000_000,
            ..Frame::default()
        });
        assert!(start.elapsed() < Duration::from_secs(1));
        let status = finish(&r);
        assert!(status.dropped >= 7);
        assert!(status.rows <= CAPACITY as u64);
        assert!(status.error.is_some());
        let csv = std::fs::read_to_string(&path).unwrap();
        let end: Vec<_> = csv.lines().last().unwrap().split(',').collect();
        assert_eq!(end[0], "end");
        assert_eq!(end[45], status.dropped.to_string());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn starts_from_current_interval_instead_of_exporting_history() {
        let path = temp_file("no-backfill");
        let r = Recording::default();
        r.start(path.clone(), 20, 900_001_000_000).unwrap();
        r.sample(900_200_000_000, 20, |tick| Frame {
            time: (tick + 1) * 50_000_000,
            ..Frame::default()
        });
        r.stop();
        assert_eq!(finish(&r).rows, 3);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn low_space_refuses_to_truncate_an_existing_file() {
        let path = temp_file("low-space");
        std::fs::write(&path, b"previous recording").unwrap();
        let p = Progress::default();
        let result = write_csv_checked(
            path.clone(),
            Arc::new(ArrayQueue::new(1)),
            Arc::new(AtomicBool::new(true)),
            &p,
            100,
            0,
            0,
            || Ok(DISK_RESERVE - 1),
        );
        assert!(result.unwrap_err().to_string().contains("预留 512 MiB"));
        assert_eq!(std::fs::read(&path).unwrap(), b"previous recording");
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn falling_free_space_stops_before_draining_pending_rows_and_finalizes_csv() {
        let path = temp_file("falling-space");
        let q = Arc::new(ArrayQueue::new(CAPACITY));
        for tick in 0..CAPACITY {
            q.push(Frame {
                time: tick as u64 * 10_000_000,
                ..Frame::default()
            })
            .ok()
            .unwrap();
        }
        let p = Progress::default();
        p.active.store(true, Release);
        let mut queries = 0;
        let result = write_csv_checked(
            path.clone(),
            q.clone(),
            Arc::new(AtomicBool::new(true)),
            &p,
            100,
            0,
            0,
            || {
                queries += 1;
                Ok(if queries == 1 {
                    DISK_RESERVE * 2
                } else {
                    DISK_RESERVE - 1
                })
            },
        );
        assert!(result.is_err());
        assert_eq!(queries, 2);
        assert!(!p.active.load(Acquire));
        assert!(p.rows.load(Relaxed) > 0 && p.rows.load(Relaxed) < CAPACITY as u64);
        assert_eq!(
            p.rows.load(Relaxed) + p.dropped.load(Relaxed),
            CAPACITY as u64
        );
        let csv = std::fs::read_to_string(&path).unwrap();
        assert!(csv.lines().last().unwrap().starts_with("end,"));
        assert_eq!(csv.len() as u64, p.bytes.load(Relaxed));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn disk_space_lookup_failure_preserves_existing_file() {
        let path = temp_file("query-failure");
        std::fs::write(&path, b"keep").unwrap();
        assert!(available_space(&path).unwrap() > 0);
        let result = write_csv_checked(
            path.clone(),
            Arc::new(ArrayQueue::new(1)),
            Arc::new(AtomicBool::new(true)),
            &Progress::default(),
            100,
            0,
            0,
            || Err(std::io::Error::other("disk unavailable")),
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"keep");
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn write_access_failure_never_reports_ready_or_truncates_previous_recording() {
        use std::os::windows::fs::OpenOptionsExt;
        let path = temp_file("write-access");
        std::fs::write(&path, b"preserve existing CSV").unwrap();
        let held = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        let r = Recording::default();
        r.start(path.clone(), 100, 0).unwrap();
        let status = finish(&r);
        assert!(status.error.unwrap().contains("写入权限"));
        assert!(!status.starting);
        assert!(!r
            .session
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .progress
            .ready
            .load(Acquire));
        drop(held);
        assert_eq!(std::fs::read(&path).unwrap(), b"preserve existing CSV");
        std::fs::remove_file(path).unwrap();
    }
}
