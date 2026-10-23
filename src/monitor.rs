use crate::report::ParsedState;
use crossbeam_queue::ArrayQueue;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::*},
        Arc, Mutex, OnceLock,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub const HORIZON: u64 = 900;
pub const QUEUE_CAPACITY: usize = 512;
pub const GAME_QUEUE_CAPACITY: usize = 2048;
const CHUNK: usize = 960;
const LEVELS: usize = 6;
pub const CACHE_LIMIT: usize = 32 * 1024 * 1024;
const SILENCE_HOLD_NS: u64 = 1_500_000_000;
const DATA_TIMEOUT_NS: u64 = 500_000_000;

#[derive(Debug, PartialEq, Eq)]
pub enum WaveState {
    NoSignal,
    Waiting,
    Interrupted,
    Silent,
    Active,
}

pub fn wave_state(
    group: usize,
    stages: [bool; 2],
    last: &[u64; 10],
    nonzero: &[u64; 10],
    now: u64,
) -> WaveState {
    if !stages.iter().any(|enabled| *enabled) {
        return WaveState::NoSignal;
    }
    let offset = [0, 2, 4][group];
    let channels = if group == 2 { 1 } else { 2 };
    let mut received = 0;
    let mut active = 0;
    for (stage, enabled) in stages.into_iter().enumerate() {
        if enabled {
            for ch in 0..channels {
                let lane = offset + stage * 5 + ch;
                received = received.max(last[lane]);
                active = active.max(nonzero[lane]);
            }
        }
    }
    if received == 0 {
        WaveState::Waiting
    } else if now.saturating_sub(received) > DATA_TIMEOUT_NS {
        WaveState::Interrupted
    } else if active != 0 && now.saturating_sub(active) < SILENCE_HOLD_NS {
        WaveState::Active
    } else {
        WaveState::Silent
    }
}

#[derive(Clone, Copy)]
pub struct Ticket {
    epoch: u64,
    generation: u64,
    signal_epoch: u64,
}

#[derive(Clone, Copy)]
struct Chunk {
    epoch: u64,
    generation: u64,
    signal_epoch: u64,
    time: u64,
    index: u64,
    rate: u32,
    channels: u8,
    len: u16,
    samples: [i16; CHUNK],
}
struct Stream {
    queue: ArrayQueue<Chunk>,
    index: AtomicU64,
    dropped: AtomicU64,
    epoch: AtomicU64,
}
impl Stream {
    fn new(index: usize) -> Self {
        Self {
            queue: ArrayQueue::new(if index == 0 {
                GAME_QUEUE_CAPACITY
            } else {
                QUEUE_CAPACITY
            }),
            index: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
        }
    }
}
struct Queues {
    streams: [Stream; 5],
    input: ArrayQueue<(u64, u64, ParsedState)>,
}

#[derive(Clone, Copy, Default)]
pub struct Bucket {
    pub min: [i16; 10],
    pub max: [i16; 10],
    pub valid: u16,
    pub gap: u16,
}
struct Level {
    first: u64,
    step: u64,
    buckets: VecDeque<Bucket>,
    capacity: usize,
}
impl Level {
    fn new(hz: u32, step: u64) -> Self {
        let capacity = (HORIZON * hz as u64 / step + 3) as usize;
        Self {
            first: 0,
            step,
            buckets: VecDeque::with_capacity(capacity),
            capacity,
        }
    }
    fn get_mut(&mut self, tick: u64) -> Option<&mut Bucket> {
        let tick = tick / self.step;
        if self.buckets.is_empty() {
            self.first = tick;
        }
        if tick < self.first {
            return None;
        }
        if tick >= self.first + self.capacity as u64 {
            let first = tick + 1 - self.capacity as u64;
            let remove = (first - self.first).min(self.buckets.len() as u64) as usize;
            self.buckets.drain(..remove);
            if self.buckets.is_empty() {
                self.first = first;
            } else {
                self.first += remove as u64;
            }
        }
        while self.first + self.buckets.len() as u64 <= tick {
            self.buckets.push_back(Bucket::default());
        }
        self.buckets.get_mut((tick - self.first) as usize)
    }
}
struct Clock {
    generation: u64,
    anchor: u64,
    first_index: u64,
    next: u64,
    last_arrival: u64,
}
pub struct History {
    levels: Vec<Level>,
    clocks: [Option<Clock>; 5],
    hz: u32,
    pub input: ParsedState,
    pub pressed: [u8; 3],
    pub input_time: u64,
    input_samples: VecDeque<(u64, ParsedState)>,
    pub last: [u64; 10],
    pub nonzero: [u64; 10],
    pub now: u64,
    mask: u16,
}
impl History {
    fn allocated_bytes(&self) -> usize {
        self.levels.capacity() * std::mem::size_of::<Level>()
            + self.input_samples.capacity() * std::mem::size_of::<(u64, ParsedState)>()
            + self
                .levels
                .iter()
                .map(|level| level.buckets.capacity() * std::mem::size_of::<Bucket>())
                .sum::<usize>()
    }
    fn input_at(&self, end: u64) -> (u64, ParsedState) {
        self.input_samples
            .iter()
            .rev()
            .find(|(time, _)| *time <= end)
            .copied()
            .unwrap_or((0, neutral()))
    }
    fn new(hz: u32) -> Self {
        Self {
            levels: (0..LEVELS)
                .map(|n| Level::new(hz, 4u64.pow(n as u32)))
                .collect(),
            clocks: std::array::from_fn(|_| None),
            hz,
            input: neutral(),
            pressed: [0; 3],
            input_time: 0,
            input_samples: VecDeque::new(),
            last: [0; 10],
            nonzero: [0; 10],
            now: 0,
            mask: 1023,
        }
    }
    fn empty(hz: u32) -> Self {
        let mut h = Self::new(0);
        h.hz = hz;
        for l in &mut h.levels {
            l.capacity = (HORIZON * hz as u64 / l.step + 3) as usize;
            l.buckets = VecDeque::new();
        }
        h
    }
    fn insert(&mut self, time: u64, lane: usize, min: i16, max: i16, gap: bool) {
        let tick = time.saturating_mul(self.hz as u64) / 1_000_000_000;
        for level in &mut self.levels {
            if let Some(b) = level.get_mut(tick) {
                let bit = 1 << lane;
                if gap {
                    b.gap |= bit;
                } else {
                    if b.valid & bit == 0 {
                        b.min[lane] = min;
                        b.max[lane] = max;
                    } else {
                        b.min[lane] = b.min[lane].min(min);
                        b.max[lane] = b.max[lane].max(max);
                    }
                    b.valid |= bit;
                }
            }
        }
    }
    fn consume(&mut self, stream: usize, p: Chunk) {
        let channels = p.channels as usize;
        let frames = p.len as usize / channels;
        let duration = frames as u64 * 1_000_000_000 / p.rate as u64;
        let reset = self.clocks[stream].as_ref().is_none_or(|c| {
            c.generation != p.generation || p.time.saturating_sub(c.last_arrival) > 500_000_000
        });
        let previous = self.clocks[stream].as_ref().map(|c| {
            (
                c.next,
                c.generation,
                c.anchor + c.next.saturating_sub(c.first_index) * 1_000_000_000 / p.rate as u64,
            )
        });
        if reset {
            let offset = [0, 5, 7, 4, 9][stream];
            for ch in 0..channels {
                self.nonzero[offset + ch] = 0;
            }
            self.clocks[stream] = Some(Clock {
                generation: p.generation,
                anchor: p.time.saturating_sub(duration),
                first_index: p.index,
                next: p.index,
                last_arrival: p.time,
            });
        }
        let c = self.clocks[stream].as_mut().unwrap();
        // Keep sample continuity through USB callback jitter, but bound drift
        // against the actual batch timestamp (sleep/transport clocks can differ).
        let predicted =
            c.anchor + p.index.saturating_sub(c.first_index) * 1_000_000_000 / p.rate as u64;
        let observed = p.time.saturating_sub(duration);
        if predicted.abs_diff(observed) > 20_000_000 {
            c.anchor = observed;
            c.first_index = p.index;
        }
        let start =
            c.anchor + p.index.saturating_sub(c.first_index) * 1_000_000_000 / p.rate as u64;
        let missing = previous.is_some_and(|(next, g, _)| next != p.index || g != p.generation);
        c.next = p.index + frames as u64;
        c.last_arrival = p.time;
        let offset = [0, 5, 7, 4, 9][stream];
        if missing {
            let first = previous.map(|(_, _, t)| t).unwrap_or(start).min(start);
            let step = 1_000_000_000 / self.hz as u64;
            for t in (first.max(start.saturating_sub(HORIZON * 1_000_000_000))..start)
                .step_by(step as usize)
            {
                for ch in 0..channels {
                    self.insert(t, offset + ch, 0, 0, true);
                }
            }
            for ch in 0..channels {
                self.insert(start.saturating_sub(1), offset + ch, 0, 0, true);
            }
        }
        // Split at bucket boundaries; never reduce a whole USB batch to one point.
        let mut i = 0;
        while i < frames {
            let t = start + i as u64 * 1_000_000_000 / p.rate as u64;
            let tick = t.saturating_mul(self.hz as u64) / 1_000_000_000;
            let mut min = [i16::MAX; 4];
            let mut max = [i16::MIN; 4];
            loop {
                for ch in 0..channels {
                    let v = p.samples[i * channels + ch];
                    min[ch] = min[ch].min(v);
                    max[ch] = max[ch].max(v);
                }
                i += 1;
                if i == frames
                    || (start + i as u64 * 1_000_000_000 / p.rate as u64)
                        .saturating_mul(self.hz as u64)
                        / 1_000_000_000
                        != tick
                {
                    break;
                }
            }
            for ch in 0..channels {
                self.insert(t, offset + ch, min[ch], max[ch], false);
                self.last[offset + ch] = p.time;
                // Retain short activity even if later chunks are zero before
                // the UI gets a chance to paint. PCM/buckets remain unchanged.
                if min[ch] != 0 || max[ch] != 0 {
                    self.nonzero[offset + ch] = p.time;
                }
            }
        }
    }
    pub fn plot(&self, seconds: u32, width: usize, now: u64) -> Vec<Bucket> {
        let width = width.clamp(1, 4096);
        let ticks = seconds as u64 * self.hz as u64;
        let level = self
            .levels
            .iter()
            .rev()
            .find(|l| l.step <= (ticks / width as u64).max(1))
            .unwrap();
        let end = (now as u128 * self.hz as u128 / 1_000_000_000) as i128;
        let start = end - ticks as i128;
        let mut result = vec![Bucket::default(); width];
        let first = ((start.max(0) as u64 / level.step).saturating_sub(level.first)) as usize;
        for (i, b) in level.buckets.iter().enumerate().skip(first) {
            let tick = ((level.first + i as u64) * level.step) as i128;
            if tick > end {
                break;
            }
            if tick + level.step as i128 <= start {
                continue;
            }
            let x0 = (((tick - start).max(0) * width as i128) / ticks.max(1) as i128)
                .min(width as i128 - 1) as usize;
            let x1 = ((((tick + level.step as i128) - start).max(0) * width as i128)
                / ticks.max(1) as i128)
                .min(width as i128 - 1) as usize;
            for dst in &mut result[x0..=x1] {
                for lane in 0..10 {
                    let bit = 1 << lane;
                    if b.valid & bit & self.mask != 0 {
                        if dst.valid & bit == 0 {
                            dst.min[lane] = b.min[lane];
                            dst.max[lane] = b.max[lane];
                        } else {
                            dst.min[lane] = dst.min[lane].min(b.min[lane]);
                            dst.max[lane] = dst.max[lane].max(b.max[lane]);
                        }
                        dst.valid |= bit;
                    }
                }
                dst.gap |= b.gap & self.mask;
            }
        }
        result
    }
    fn change_mask(&mut self, mask: u16) {
        let removed = self.mask & !mask;
        if removed != 0 {
            for l in &mut self.levels {
                for b in &mut l.buckets {
                    b.valid &= !removed;
                    b.gap &= !removed;
                }
            }
        }
        self.mask = mask;
    }
}
pub fn neutral() -> ParsedState {
    ParsedState {
        lx: 128,
        ly: 128,
        rx: 128,
        ry: 128,
        buttons0: 8,
        ..Default::default()
    }
}
pub fn dpad_bits(value: u8) -> u8 {
    match value & 15 {
        0 => 1,
        1 => 3,
        2 => 2,
        3 => 6,
        4 => 4,
        5 => 12,
        6 => 8,
        7 => 9,
        _ => 0,
    }
}

pub struct Monitor {
    pub recording: crate::monitor_recording::Recording,
    origin: Instant,
    pub active: AtomicBool,
    pub source: AtomicBool,
    pub forwarded: AtomicBool,
    pub connected: AtomicBool,
    pub edge: AtomicBool,
    pub microphone: AtomicBool,
    pub microphone_gain_db: AtomicU32,
    pub microphone_buffer_ms: AtomicU32,
    pub mic_buffered: AtomicU64,
    pub mic_underruns: AtomicU64,
    pub mic_dropped: AtomicU64,
    pub route: AtomicU32,
    pub volume: AtomicU32,
    pub decode_failures: AtomicU64,
    pub resets: AtomicU64,
    pub input_drops: AtomicU64,
    pub mic_tagged: AtomicU64,
    pub mic_packets: AtomicU64,
    pub mic_samples: AtomicU64,
    pub mic_usb_frames: AtomicU64,
    pub mic_prefix: AtomicU32,
    pub mic_test_capture: Arc<crate::audio_test_source::SourceCapture>,
    epoch: AtomicU64,
    generation: AtomicU64,
    hz: AtomicU32,
    frozen: AtomicU64,
    history_bytes: AtomicU64,
    queues: OnceLock<Queues>,
    pub history: Mutex<History>,
    worker: Mutex<Option<JoinHandle<()>>>,
}
impl Monitor {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    pub fn now(&self) -> u64 {
        self.origin.elapsed().as_nanos().min(u64::MAX as u128) as u64
    }
    pub fn display_time(&self) -> u64 {
        if self.active.load(Acquire) {
            self.now()
        } else {
            self.frozen.load(Acquire)
        }
    }
    pub fn wants(&self, stream: usize) -> bool {
        self.active.load(Relaxed)
            && if stream == 0 || stream == 3 {
                self.source.load(Relaxed)
            } else {
                self.forwarded.load(Relaxed)
            }
    }
    pub fn start(self: &Arc<Self>, hz: u32) -> Result<(), String> {
        self.stop();
        let hz = if [20, 50, 100, 200].contains(&hz) {
            hz
        } else {
            100
        };
        self.hz.store(hz, Release);
        let q = self.queues.get_or_init(|| Queues {
            streams: std::array::from_fn(Stream::new),
            input: ArrayQueue::new(4096),
        });
        for s in &q.streams {
            while s.queue.pop().is_some() {}
            s.dropped.store(0, Relaxed);
        }
        while q.input.pop().is_some() {}
        self.input_drops.store(0, Relaxed);
        self.decode_failures.store(0, Relaxed);
        self.resets.store(0, Relaxed);
        let history = History::new(hz);
        let history_bytes = history.allocated_bytes();
        if self.queue_bytes() + history_bytes + self.recording.allocated_bytes() > CACHE_LIMIT {
            return Err(crate::tr!(monitor_monitor_cache_would_exceed_32_mib_reduce).into());
        }
        *self
            .history
            .lock()
            .map_err(|_| crate::tr!(monitor_monitor_cache_unavailable))? = history;
        self.history_bytes.store(history_bytes as u64, Release);
        self.epoch.fetch_add(1, AcqRel);
        self.active.store(true, Release);
        let monitor = self.clone();
        let thread = std::thread::Builder::new()
            .name("opends-monitor".into())
            .spawn(move || monitor.work())
            .map_err(|e| {
                self.active.store(false, Release);
                e.to_string()
            })?;
        *self
            .worker
            .lock()
            .map_err(|_| crate::tr!(monitor_could_not_start_monitor_worker))? = Some(thread);
        Ok(())
    }
    pub fn stop(&self) {
        self.frozen.store(self.now(), Release);
        self.active.store(false, Release);
        if let Ok(mut w) = self.worker.lock() {
            if let Some(w) = w.take() {
                let _ = w.join();
            }
        }
        self.stop_recording();
    }
    pub fn start_recording(&self, path: std::path::PathBuf) -> Result<(), String> {
        if !self.active.load(Acquire) {
            return Err(crate::tr!(monitor_start_capture_before_recording_to_a_file).into());
        }
        if self.queue_bytes()
            + self.history_bytes.load(Acquire) as usize
            + crate::monitor_recording::queue_bytes()
            > CACHE_LIMIT
        {
            return Err(crate::tr!(monitor_recording_would_exceed_the_monitor_cache_limit).into());
        }
        self.recording
            .start(path, self.hz.load(Acquire), self.now())
    }
    pub fn stop_recording(&self) {
        if let Ok(h) = self.history.lock() {
            self.record_history(&h);
        }
        self.recording.stop();
    }
    fn record_history(&self, h: &History) {
        if !self.recording.active() {
            return;
        }
        let now = self.now();
        let hz = self.hz.load(Acquire);
        self.recording.sample(now, hz, |tick| {
            let end = (tick + 1) * 1_000_000_000 / hz as u64;
            let (input_time, input) = h.input_at(end);
            let base = &h.levels[0];
            let bucket = tick
                .checked_sub(base.first)
                .and_then(|i| base.buckets.get(i as usize))
                .copied()
                .unwrap_or_default();
            crate::monitor_recording::Frame {
                time: end,
                input_time,
                epoch: self.epoch.load(Acquire),
                generation: self.generation.load(Acquire),
                connected: self.connected.load(Relaxed),
                source: self.source.load(Relaxed),
                forwarded: self.forwarded.load(Relaxed),
                input: if self.connected.load(Relaxed) {
                    input
                } else {
                    neutral()
                },
                bucket,
                drops: self.dropped(),
                decode_failures: self.decode_failures.load(Relaxed),
                resets: self.resets.load(Relaxed),
            }
        });
    }
    pub fn clear(&self) {
        self.epoch.fetch_add(1, AcqRel);
        if let Ok(mut h) = self.history.lock() {
            *h = if self.active.load(Acquire) {
                History::new(self.hz.load(Relaxed))
            } else {
                History::empty(self.hz.load(Relaxed))
            };
            self.history_bytes
                .store(h.allocated_bytes() as u64, Release);
        }
    }
    pub fn signal(&self, forwarded: bool, enabled: bool) {
        let flag = if forwarded {
            &self.forwarded
        } else {
            &self.source
        };
        flag.store(enabled, Release);
        if let Some(q) = self.queues.get() {
            for (i, s) in q.streams.iter().enumerate() {
                if (i != 0 && i != 3) == forwarded {
                    s.epoch.fetch_add(1, AcqRel);
                }
            }
        }
        if let Ok(mut h) = self.history.lock() {
            let mask = if forwarded { 992 } else { 31 };
            for l in &mut h.levels {
                for b in &mut l.buckets {
                    b.valid &= !mask;
                    b.gap &= !mask;
                }
            }
            for i in 0..5 {
                if (i != 0 && i != 3) == forwarded {
                    h.clocks[i] = None;
                }
            }
            for i in 0..10 {
                if mask & (1 << i) != 0 {
                    h.last[i] = 0;
                    h.nonzero[i] = 0;
                }
            }
        }
    }
    pub fn reset(&self) {
        self.generation.fetch_add(1, AcqRel);
        if self.active.load(Relaxed) {
            self.resets.fetch_add(1, Relaxed);
        }
    }
    pub fn mic_failed(&self) {
        if self.wants(3) || self.wants(4) {
            self.decode_failures.fetch_add(1, Relaxed);
            if let Some(q) = self.queues.get() {
                for i in [3, 4] {
                    q.streams[i].index.fetch_add(480, Relaxed);
                }
            }
        }
    }
    pub fn dropped(&self) -> u64 {
        self.input_drops.load(Relaxed)
            + self
                .queues
                .get()
                .map(|q| {
                    q.streams
                        .iter()
                        .map(|s| s.dropped.load(Relaxed))
                        .sum::<u64>()
                })
                .unwrap_or(0)
    }
    pub fn input(&self, state: ParsedState) {
        if !self.active.load(Relaxed) {
            return;
        }
        if let Some(q) = self.queues.get() {
            if q.input
                .push((self.epoch.load(Acquire), self.now(), state))
                .is_err()
            {
                self.input_drops.fetch_add(1, Relaxed);
            }
        }
    }
    pub fn bytes(&self, bytes: &[u8]) {
        if let Some(t) = self.ticket(0) {
            self.bytes_with(t, bytes);
        }
    }
    pub fn ticket(&self, stream: usize) -> Option<Ticket> {
        if !self.wants(stream) {
            return None;
        }
        let s = self.queues.get()?.streams.get(stream)?;
        Some(Ticket {
            epoch: self.epoch.load(Acquire),
            generation: self.generation.load(Acquire),
            signal_epoch: s.epoch.load(Acquire),
        })
    }
    pub fn bytes_with(&self, ticket: Ticket, bytes: &[u8]) {
        if bytes.len() % 8 != 0 {
            return;
        }
        self.enqueue(ticket, 0, 4, 48000, bytes.len() / 2, |i| {
            i16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]])
        });
    }
    pub fn pcm(&self, stream: usize, channels: usize, rate: u32, pcm: &[i16]) {
        if let Some(ticket) = self.ticket(stream) {
            self.enqueue(ticket, stream, channels, rate, pcm.len(), |i| pcm[i]);
        }
    }
    pub fn speaker_forwarded(&self, pcm: &[i16]) {
        // This is the observation clock, not a new device sample rate: 480 post-
        // resample PCM frames describe each 512/48000-second Bluetooth block.
        let clock = crate::audio::OPUS_FRAMES as u32 * crate::audio::SAMPLE_RATE
            / crate::audio::FRAMES_PER_BLOCK as u32;
        self.pcm(1, 2, clock, pcm);
    }
    pub fn haptics(&self, pcm: &[u8]) {
        if let Some(ticket) = self.ticket(2) {
            self.enqueue(ticket, 2, 2, 3000, pcm.len(), |i| {
                (pcm[i] as i8 as i16) * 256
            });
        }
    }
    fn enqueue(
        &self,
        ticket: Ticket,
        stream: usize,
        channels: usize,
        rate: u32,
        len: usize,
        sample: impl Fn(usize) -> i16,
    ) {
        let Some(q) = self.queues.get() else { return };
        if !(1..=4).contains(&channels) || len % channels != 0 || rate == 0 {
            return;
        }
        let s = &q.streams[stream];
        let time = self.now();
        if !self.wants(stream)
            || ticket.epoch != self.epoch.load(Acquire)
            || ticket.generation != self.generation.load(Acquire)
            || ticket.signal_epoch != s.epoch.load(Acquire)
        {
            return;
        }
        let block = CHUNK / channels * channels;
        for offset in (0..len).step_by(block) {
            let n = (len - offset).min(block);
            let mut p = Chunk {
                epoch: ticket.epoch,
                generation: ticket.generation,
                signal_epoch: ticket.signal_epoch,
                time: time.saturating_sub(
                    ((len - offset - n) / channels) as u64 * 1_000_000_000 / rate as u64,
                ),
                index: s.index.fetch_add((n / channels) as u64, Relaxed),
                rate,
                channels: channels as u8,
                len: n as u16,
                samples: [0; CHUNK],
            };
            for i in 0..n {
                p.samples[i] = sample(offset + i);
            }
            if s.queue.push(p).is_err() {
                s.dropped.fetch_add(1, Relaxed);
            }
        }
    }
    fn work(&self) {
        let q = self.queues.get().unwrap();
        while self.active.load(Acquire) {
            let epoch = self.epoch.load(Acquire);
            if let Ok(mut h) = self.history.lock() {
                let mask = if self.source.load(Relaxed) { 31 } else { 0 }
                    | if self.forwarded.load(Relaxed) { 992 } else { 0 };
                h.change_mask(mask);
                for (i, s) in q.streams.iter().enumerate() {
                    for _ in 0..512 {
                        let Some(p) = s.queue.pop() else { break };
                        if p.epoch == epoch
                            && p.generation == self.generation.load(Acquire)
                            && p.signal_epoch == s.epoch.load(Acquire)
                            && self.wants(i)
                        {
                            h.consume(i, p);
                        }
                    }
                }
                for _ in 0..4096 {
                    let Some((e, t, s)) = q.input.pop() else {
                        break;
                    };
                    if e == epoch {
                        let old = [
                            (h.input.buttons0 & 0xf0) | dpad_bits(h.input.buttons0),
                            h.input.buttons1,
                            h.input.buttons2,
                        ];
                        let new = [
                            (s.buttons0 & 0xf0) | dpad_bits(s.buttons0),
                            s.buttons1,
                            s.buttons2,
                        ];
                        for i in 0..3 {
                            h.pressed[i] |= new[i] & !old[i];
                        }
                        h.input = s;
                        h.input_time = t;
                        if h.input_samples.len() == 4096 {
                            h.input_samples.pop_front();
                        }
                        h.input_samples.push_back((t, s));
                    }
                }
                if !self.connected.load(Relaxed) {
                    h.input = neutral();
                    h.pressed = [0; 3];
                }
                h.now = self.now();
                self.history_bytes
                    .store(h.allocated_bytes() as u64, Release);
                self.record_history(&h);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn queue_bytes(&self) -> usize {
        self.queues
            .get()
            .map(|_| {
                (GAME_QUEUE_CAPACITY + 4 * QUEUE_CAPACITY)
                    * (std::mem::size_of::<Chunk>() + std::mem::size_of::<usize>())
                    + 4096
                        * (std::mem::size_of::<(u64, u64, ParsedState)>()
                            + std::mem::size_of::<usize>())
                    + std::mem::size_of::<Queues>()
            })
            .unwrap_or(0)
    }
    pub fn allocated_bytes(&self) -> usize {
        self.queue_bytes()
            + self.history_bytes.load(Acquire) as usize
            + self.recording.allocated_bytes()
    }
}
impl Default for Monitor {
    fn default() -> Self {
        Self {
            recording: crate::monitor_recording::Recording::default(),
            origin: Instant::now(),
            active: AtomicBool::new(false),
            source: AtomicBool::new(true),
            forwarded: AtomicBool::new(false),
            connected: AtomicBool::new(false),
            edge: AtomicBool::new(false),
            microphone: AtomicBool::new(false),
            microphone_gain_db: AtomicU32::new(0),
            microphone_buffer_ms: AtomicU32::new(128),
            mic_buffered: AtomicU64::new(0),
            mic_underruns: AtomicU64::new(0),
            mic_dropped: AtomicU64::new(0),
            route: AtomicU32::new(0),
            volume: AtomicU32::new(0),
            decode_failures: AtomicU64::new(0),
            resets: AtomicU64::new(0),
            input_drops: AtomicU64::new(0),
            mic_tagged: AtomicU64::new(0),
            mic_packets: AtomicU64::new(0),
            mic_samples: AtomicU64::new(0),
            mic_usb_frames: AtomicU64::new(0),
            mic_prefix: AtomicU32::new(0),
            mic_test_capture: Arc::new(crate::audio_test_source::SourceCapture::default()),
            epoch: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            hz: AtomicU32::new(100),
            frozen: AtomicU64::new(0),
            history_bytes: AtomicU64::new(0),
            queues: OnceLock::new(),
            history: Mutex::new(History::empty(100)),
            worker: Mutex::new(None),
        }
    }
}

// UI acceptance fixture; never compiled into release builds or attached to a device.
#[cfg(debug_assertions)]
pub fn demo(m: Arc<Monitor>, stop: Arc<AtomicBool>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        m.connected.store(true, Release);
        m.edge.store(true, Release);
        m.microphone.store(true, Release);
        let mut index = 0u64;
        while !stop.load(Acquire) {
            if m.active.load(Acquire) {
                let t = m.now() as f64 / 1e9;
                let mut input = neutral();
                input.lx = (128. + t.sin() * 120.) as u8;
                input.ry = (128. + t.cos() * 120.) as u8;
                input.l2 = ((t.sin() + 1.) * 127.) as u8;
                input.r2 = ((t.cos() + 1.) * 127.) as u8;
                input.buttons0 = (index / 50 % 9) as u8 | if index % 100 < 50 { 32 } else { 0 };
                input.buttons1 = if index % 200 < 100 { 0x41 } else { 0x82 };
                input.buttons2 = if index % 150 < 75 { 0x50 } else { 0xa2 };
                input.touch1 = crate::report::TouchPoint {
                    active: true,
                    x: ((t.sin() + 1.) * 950.) as u16,
                    y: ((t.cos() + 1.) * 530.) as u16,
                    ..Default::default()
                };
                m.input(input);
                let mut raw = [0u8; 3840];
                let mut front = [0i16; 960];
                let mut mic = [0i16; 480];
                for frame in 0..480 {
                    for ch in 0..4 {
                        let value = (((index * 480 + frame as u64) as f64
                            * (0.007 + ch as f64 * 0.005))
                            .sin()
                            * (t * 0.8 + ch as f64).sin().abs()
                            * 29000.) as i16;
                        raw[(frame * 4 + ch) * 2..(frame * 4 + ch) * 2 + 2]
                            .copy_from_slice(&value.to_le_bytes());
                        if ch < 2 {
                            front[frame * 2 + ch] = value / 2;
                        }
                    }
                    mic[frame] = front[frame * 2] / 2;
                }
                m.bytes(&raw);
                m.pcm(1, 2, 48000, &front);
                m.pcm(3, 1, 48000, &mic);
                m.pcm(4, 1, 48000, &mic);
                let mut haptic = [0u8; 64];
                for (i, value) in haptic.iter_mut().enumerate() {
                    *value = ((t * 40. + i as f64).sin() * 100.) as i8 as u8;
                }
                m.haptics(&haptic);
            }
            index += 1;
            std::thread::sleep(Duration::from_millis(10));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wave_chunk(time: u64, index: u64) -> Chunk {
        Chunk {
            epoch: 0,
            generation: 0,
            signal_epoch: 0,
            time,
            index,
            rate: 1000,
            channels: 4,
            len: 40,
            samples: [0; CHUNK],
        }
    }
    #[test]
    fn waveform_activity_retains_brief_right_channel_pulses_until_continuous_silence() {
        let mut h = History::new(100);
        let mut p = wave_chunk(1_000_000_000, 0);
        p.samples[1] = i16::MIN; // Right only; a following zero batch used to erase this.
        h.consume(0, p);
        for n in 1..=149 {
            let now = 1_000_000_000 + n * 10_000_000;
            h.consume(0, wave_chunk(now, n * 10));
            assert_eq!(
                wave_state(0, [true, false], &h.last, &h.nonzero, now),
                WaveState::Active
            );
        }
        h.consume(0, wave_chunk(2_500_000_000, 1500));
        assert_eq!(
            wave_state(0, [true, false], &h.last, &h.nonzero, 2_500_000_000),
            WaveState::Silent
        );
        let bins = h.plot(30, 3000, 2_500_000_000);
        assert!(bins
            .iter()
            .any(|b| b.valid & 2 != 0 && b.min[1] == i16::MIN));
        assert!(bins
            .iter()
            .any(|b| b.valid & 2 != 0 && b.min[1] == 0 && b.max[1] == 0));
        // Real activity is shown immediately, with no attack delay.
        let mut p = wave_chunk(2_510_000_000, 1510);
        p.samples[0] = 1;
        h.consume(0, p);
        assert_eq!(
            wave_state(0, [true, false], &h.last, &h.nonzero, p.time),
            WaveState::Active
        );
    }
    #[test]
    fn wave_status_combines_both_channels_of_only_the_selected_signal_stages() {
        let now = 2_000_000_000;
        let mut last = [0; 10];
        let mut nonzero = [0; 10];
        for (group, right) in [(0, 1), (1, 3), (2, 4)] {
            last.fill(0);
            nonzero.fill(0);
            last[right] = now;
            nonzero[right] = now;
            assert_eq!(
                wave_state(group, [true, false], &last, &nonzero, now),
                WaveState::Active
            );
            assert_eq!(
                wave_state(group, [false, true], &last, &nonzero, now),
                WaveState::Waiting
            );
            last[right + 5] = now;
            assert_eq!(
                wave_state(group, [false, true], &last, &nonzero, now),
                WaveState::Silent
            );
            nonzero[right] = 0;
            nonzero[right + 5] = now;
            assert_eq!(
                wave_state(group, [false, true], &last, &nonzero, now),
                WaveState::Active
            );
            assert_eq!(
                wave_state(group, [true, true], &last, &nonzero, now),
                WaveState::Active
            );
            assert_eq!(
                wave_state(group, [false, false], &last, &nonzero, now),
                WaveState::NoSignal
            );
        }
    }
    #[test]
    fn wave_activity_hold_never_hides_a_transport_interruption_or_invents_initial_activity() {
        let mut h = History::new(100);
        assert_eq!(
            wave_state(0, [true, false], &h.last, &h.nonzero, 0),
            WaveState::Waiting
        );
        let mut p = wave_chunk(1_000_000_000, 0);
        h.consume(0, p);
        assert_eq!(
            wave_state(0, [true, false], &h.last, &h.nonzero, p.time),
            WaveState::Silent
        );
        p.samples[0] = 3;
        h.consume(0, p);
        assert_eq!(
            wave_state(
                0,
                [true, false],
                &h.last,
                &h.nonzero,
                p.time + DATA_TIMEOUT_NS + 1
            ),
            WaveState::Interrupted
        );
        // A new connection must not inherit the preceding session's activity.
        let mut next = wave_chunk(p.time + 20_000_000, 0);
        next.generation = 1;
        h.consume(0, next);
        assert_eq!(
            wave_state(0, [true, false], &h.last, &h.nonzero, next.time),
            WaveState::Silent
        );
    }
    #[test]
    fn changing_signal_selection_clears_held_activity() {
        let m = observer();
        {
            let mut h = m.history.lock().unwrap();
            h.last.fill(1_000_000_000);
            h.nonzero.fill(1_000_000_000);
        }
        m.signal(false, false);
        m.signal(false, true);
        let h = m.history.lock().unwrap();
        assert_eq!(
            wave_state(0, [true, false], &h.last, &h.nonzero, 1_000_000_000),
            WaveState::Waiting
        );
        assert_eq!(
            wave_state(0, [false, true], &h.last, &h.nonzero, 1_000_000_000),
            WaveState::Active
        );
        drop(h);
        m.clear();
        let h = m.history.lock().unwrap();
        assert_eq!(
            wave_state(0, [true, true], &h.last, &h.nonzero, 1_000_000_000),
            WaveState::Waiting
        );
    }
    fn observer() -> Arc<Monitor> {
        let m = Monitor::new();
        m.queues.get_or_init(|| Queues {
            streams: std::array::from_fn(Stream::new),
            input: ArrayQueue::new(4096),
        });
        m.active.store(true, Release);
        m
    }
    #[test]
    fn source_owns_cgo_buffer_and_splits_front_rear_channels() {
        let m = observer();
        let mut bytes = Vec::new();
        for _ in 0..48 {
            for v in [123i16, -456, 789, -1234] {
                bytes.extend(v.to_le_bytes());
            }
        }
        m.bytes(&bytes);
        bytes.fill(0);
        let mut h = History::new(100);
        let p = m.queues.get().unwrap().streams[0].queue.pop().unwrap();
        h.consume(
            0,
            Chunk {
                time: 1_000_000_000,
                ..p
            },
        );
        let bins = h.plot(30, 3000, 2_000_000_000);
        let b = bins.iter().find(|b| b.valid & 15 == 15).unwrap();
        assert_eq!(&b.min[..4], &[123, -456, 789, -1234]);
        assert_eq!(b.valid & (1 << 4), 0);
    }
    #[test]
    fn callback_never_waits_for_history_or_ui() {
        let m = observer();
        let guard = m.history.lock().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let producer = m.clone();
        let t = std::thread::spawn(move || {
            producer.bytes(&[0; 3840]);
            tx.send(()).unwrap();
        });
        rx.recv_timeout(Duration::from_secs(1))
            .expect("cgo observer blocked on the history lock");
        drop(guard);
        t.join().unwrap();
    }
    #[test]
    fn old_batches_cannot_cross_clear_reconnect_or_signal_selection() {
        let m = observer();
        let old = m.ticket(0).unwrap();
        m.clear();
        m.bytes_with(old, &[0; 3840]);
        assert!(m.queues.get().unwrap().streams[0].queue.is_empty());
        let old = m.ticket(0).unwrap();
        m.reset();
        m.bytes_with(old, &[0; 3840]);
        assert!(m.queues.get().unwrap().streams[0].queue.is_empty());
        let old = m.ticket(0).unwrap();
        m.signal(false, false);
        m.signal(false, true);
        m.bytes_with(old, &[0; 3840]);
        assert!(m.queues.get().unwrap().streams[0].queue.is_empty());
        m.bytes(&[1; 3840]);
        assert!(!m.queues.get().unwrap().streams[0].queue.is_empty());
    }
    #[test]
    fn known_sample_loss_is_marked_but_idle_timeout_is_not_called_loss() {
        let m = observer();
        m.bytes(&[1; 1920]);
        let p = m.queues.get().unwrap().streams[0].queue.pop().unwrap();
        let mut h = History::new(200);
        h.consume(
            0,
            Chunk {
                time: 1_000_000_000,
                ..p
            },
        );
        h.consume(
            0,
            Chunk {
                time: 1_025_000_000,
                index: 1200,
                ..p
            },
        );
        assert!(
            h.plot(30, 4096, 2_000_000_000)
                .iter()
                .filter(|b| b.gap & 15 != 0)
                .count()
                >= 2
        );
        let mut h = History::new(200);
        h.consume(
            0,
            Chunk {
                time: 1_000_000_000,
                ..p
            },
        );
        h.consume(
            0,
            Chunk {
                time: 1_700_000_000,
                index: 240,
                ..p
            },
        );
        assert!(h.plot(30, 4096, 2_000_000_000).iter().all(|b| b.gap == 0));
    }
    #[test]
    fn forwarded_clock_covers_full_bluetooth_period_without_long_term_drift() {
        let m = observer();
        m.signal(true, true);
        m.speaker_forwarded(&[1; 960]);
        let p = m.queues.get().unwrap().streams[1].queue.pop().unwrap();
        assert_eq!(p.rate, 45000);
        let periods = 84375u64;
        assert_eq!(
            periods * 480 * 1_000_000_000 / p.rate as u64,
            900_000_000_000
        );
    }
    #[test]
    fn observation_clock_tracks_slow_transport_without_inventing_loss() {
        let m = observer();
        m.bytes(&[1; 1920]);
        let p = m.queues.get().unwrap().streams[0].queue.pop().unwrap();
        let mut h = History::new(200);
        for n in 0..600 {
            h.consume(
                0,
                Chunk {
                    time: 1_000_000_000 + n * 5_400_000,
                    index: n * 240,
                    ..p
                },
            );
        }
        let bins = h.plot(30, 4096, 1_000_000_000 + 599 * 5_400_000);
        assert!(bins.iter().all(|b| b.gap == 0));
        assert!(bins.iter().rev().take(5).any(|b| b.valid & 15 == 15));
    }
    #[test]
    fn fifteen_minute_history_evicts_old_data_at_every_lod() {
        let mut h = History::new(200);
        h.insert(1_000_000_000, 0, 1, 1, false);
        for second in 2..=1800 {
            h.insert(second * 1_000_000_000, 0, 2, 2, false);
        }
        assert!(h.levels.iter().all(|l| l.buckets.len() <= l.capacity));
        assert!(h
            .plot(900, 400, 1800_000_000_000)
            .iter()
            .all(|b| b.valid & 1 == 0 || b.min[0] == 2));
    }
    #[test]
    fn disabled_observers_do_not_allocate_queues() {
        let m = Monitor::new();
        m.bytes(&[0; 3840]);
        m.input(neutral());
        assert!(m.queues.get().is_none());
    }
    #[test]
    fn maximum_history_and_raw_queues_fit_budget() {
        let m = Monitor::new();
        m.start(200).unwrap();
        m.stop();
        assert!(m.allocated_bytes() < CACHE_LIMIT);
        assert!(GAME_QUEUE_CAPACITY * 48 >= 48000 * 2);
        let before = m.allocated_bytes();
        let guard = m.history.lock().unwrap();
        assert_eq!(before, m.allocated_bytes());
        drop(guard);
        m.clear();
        assert!(m.allocated_bytes() < before / 2);
    }
    #[test]
    fn csv_stops_with_collection_and_maximum_sampling_stays_within_budget() {
        let m = Monitor::new();
        let path =
            std::env::temp_dir().join(format!("opends-monitor-stop-{}.csv", std::process::id()));
        assert!(m.start_recording(path.clone()).is_err());
        assert!(!path.exists());
        m.start(200).unwrap();
        m.start_recording(path.clone()).unwrap();
        {
            let mut h = m.history.lock().unwrap();
            h.input_samples.reserve(4096);
            m.history_bytes.store(h.allocated_bytes() as u64, Release);
        }
        assert!(m.allocated_bytes() < CACHE_LIMIT);
        m.stop();
        assert!(!m.recording.active());
        // Dropping joins the writer and guarantees its completed file is usable.
        drop(m);
        let csv = std::fs::read_to_string(&path).unwrap();
        assert!(csv.lines().last().unwrap().starts_with("end,"));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn csv_input_uses_interval_end_without_borrowing_future_button_states() {
        let mut h = History::new(100);
        let mut pressed = neutral();
        pressed.buttons1 = 1;
        h.input_samples.push_back((5_000_000, neutral()));
        h.input_samples.push_back((15_000_000, pressed));
        h.input_samples.push_back((25_000_000, neutral()));
        assert_eq!(h.input_at(10_000_000).1.buttons1, 0);
        assert_eq!(h.input_at(20_000_000).1.buttons1, 1);
        assert_eq!(h.input_at(30_000_000).1.buttons1, 0);
        assert_eq!(h.input_at(20_000_000).0, 15_000_000);
    }
    #[test]
    fn opposite_channels_and_single_sample_peaks_survive_lod() {
        let mut h = History::new(200);
        h.insert(1_000_000_000, 0, -32768, 32767, false);
        h.insert(1_000_000_000, 1, -7, -7, false);
        let p = h.plot(900, 400, 2_000_000_000);
        assert!(p
            .iter()
            .any(|b| b.valid & 1 != 0 && b.min[0] == -32768 && b.max[0] == 32767));
        assert!(p.iter().any(|b| b.valid & 2 != 0 && b.max[1] == -7));
    }
    #[test]
    fn overflow_is_nonblocking_and_counted() {
        let m = Monitor::new();
        m.queues.get_or_init(|| Queues {
            streams: std::array::from_fn(Stream::new),
            input: ArrayQueue::new(4096),
        });
        m.active.store(true, Relaxed);
        for _ in 0..GAME_QUEUE_CAPACITY + 3 {
            m.bytes(&[0; 1920]);
        }
        assert_eq!(m.dropped(), 3);
        m.active.store(false, Relaxed);
    }
    #[test]
    fn rate_is_not_a_device_sample_rate_and_stop_freezes_time() {
        let m = Monitor::new();
        m.start(50).unwrap();
        m.stop();
        let t = m.display_time();
        std::thread::sleep(Duration::from_millis(1));
        assert_eq!(t, m.display_time());
        assert_eq!(m.hz.load(Relaxed), 50);
    }
}
