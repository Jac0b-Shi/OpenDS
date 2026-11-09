//! 单一物理输出线程：统一状态、原生触觉、音频与序号。
use crate::{
    audio::{self, OpusEncoder, HAPTICS_BYTES, OPUS_BYTES},
    config::Config,
    hid::HidDevice,
    opus::LibOpusEncoder,
};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

struct Queue {
    generation: u64,
    samples: VecDeque<i16>,
}
pub struct PcmBuffer {
    inner: Mutex<Queue>,
    channels: usize,
    limit: usize,
    pub dropped: AtomicU64,
}
impl PcmBuffer {
    fn new(blocks: usize, channels: usize) -> Self {
        let limit = blocks.clamp(1, 128) * audio::FRAMES_PER_BLOCK * channels;
        Self {
            inner: Mutex::new(Queue {
                generation: 0,
                samples: VecDeque::with_capacity(limit),
            }),
            channels,
            limit,
            dropped: AtomicU64::new(0),
        }
    }
    pub fn push_bytes(&self, bytes: &[u8], generation: u64) {
        if bytes.is_empty() || bytes.len() % (self.channels * 2) != 0 {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let Ok(mut q) = self.inner.try_lock() else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        if generation < q.generation {
            return;
        }
        if generation != q.generation {
            q.samples.clear();
            q.generation = generation;
        }
        let count = bytes.len() / 2;
        let skip = count.saturating_sub(self.limit);
        let excess = (q.samples.len() + count - skip).saturating_sub(self.limit);
        if excess > 0 {
            q.samples.drain(..excess);
        }
        if excess > 0 || skip > 0 {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        q.samples.extend(
            bytes[skip * 2..]
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]])),
        );
    }
    pub(crate) fn take(&self, generation: u64) -> Option<Vec<i16>> {
        let mut q = self.inner.lock().ok()?;
        if generation != q.generation {
            if generation > q.generation {
                q.samples.clear();
                q.generation = generation;
            }
            return None;
        }
        let n = audio::FRAMES_PER_BLOCK * self.channels;
        if q.samples.len() < n {
            return None;
        }
        Some(q.samples.drain(..n).collect())
    }
}

#[derive(Default)]
pub struct AudioStats {
    pub received_output: AtomicU64,
    pub rumble_updates: AtomicU64,
    pub haptics_nonzero: AtomicU64,
    pub pcm_callbacks: AtomicU64,
    pub pcm_rejected: AtomicU64,
    pub pcm_max_bytes: AtomicU64,
    pub pcm_nonzero: AtomicU64,
    pub last_output: AtomicU64,
    pub blocks: AtomicU64,
    pub output_frames: AtomicU64,
    pub write_fail: AtomicU64,
    pub encode_fail: AtomicU64,
    pub resets: AtomicU64,
    pub ready: AtomicBool,
    pub failed: AtomicBool,
}
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum FeedbackTest {
    NativeHaptics = 1,
    ClassicRumble = 2,
}

pub struct AudioSession {
    pub monitor: OnceLock<Arc<crate::monitor::Monitor>>,
    pub forward_pcm: AtomicBool,
    pub pcm: PcmBuffer,
    pub haptics: PcmBuffer,
    pub generation: AtomicU64,
    output: Mutex<[u8; 47]>,
    revision: AtomicU64,
    light_reset: AtomicBool,
    feedback_test: AtomicU8,
    pub headset: AtomicBool,
    pub vibration_v2: AtomicBool,
    pub stats: Arc<AudioStats>,
}
impl AudioSession {
    pub fn new(blocks: usize) -> Arc<Self> {
        Arc::new(Self {
            monitor: OnceLock::new(),
            forward_pcm: AtomicBool::new(true),
            pcm: PcmBuffer::new(blocks, 4),
            haptics: PcmBuffer::new(blocks, 2),
            generation: AtomicU64::new(0),
            output: Mutex::new([0; 47]),
            revision: AtomicU64::new(0),
            light_reset: AtomicBool::new(false),
            feedback_test: AtomicU8::new(0),
            headset: AtomicBool::new(false),
            vibration_v2: AtomicBool::new(false),
            stats: Arc::new(AudioStats::default()),
        })
    }
    pub fn feedback_test_options(&self) -> (bool, bool) {
        let idle = self.feedback_test.load(Ordering::Acquire) == 0
            && !self.stats.failed.load(Ordering::Acquire);
        (idle && self.stats.ready.load(Ordering::Acquire), idle)
    }
    pub fn request_feedback_test(&self, test: FeedbackTest) {
        let (native, classic) = self.feedback_test_options();
        if match test {
            FeedbackTest::NativeHaptics => native,
            FeedbackTest::ClassicRumble => classic,
        } {
            let _ = self.feedback_test.compare_exchange(
                0,
                test as u8,
                Ordering::AcqRel,
                Ordering::Relaxed,
            );
        }
    }
    pub fn update_output(&self, p: [u8; 47]) {
        self.stats.received_output.fetch_add(1, Ordering::Relaxed);
        self.stats.last_output.store(
            u64::from_le_bytes(p[..8].try_into().unwrap()),
            Ordering::Relaxed,
        );
        if (p[0] & 1 != 0 || p[38] & 4 != 0) && (p[2] != 0 || p[3] != 0) {
            self.stats.rumble_updates.fetch_add(1, Ordering::Relaxed);
        }
        // The writer only waits for the state copy, never a Bluetooth write.
        // A stop report may arrive only once, so it must survive contention.
        if let Ok(mut s) = self.output.lock() {
            merge_output(&mut s, &p);
            if p[1] & 8 != 0 {
                self.light_reset.store(true, Ordering::Release);
            }
            self.revision.fetch_add(1, Ordering::Release);
        }
    }
    pub fn reset(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.stats.resets.fetch_add(1, Ordering::Relaxed);
    }
}

/// LED 等局部更新不能清除之前有效的扳机、音量与路由。
pub fn merge_output(old: &mut [u8; 47], new: &[u8; 47]) {
    old[0] |= new[0] & !3;
    if new[0] & 1 != 0 || new[38] & 4 != 0 {
        old[0] = (old[0] & !3) | (new[0] & 3);
        old[2..4].copy_from_slice(&new[2..4]);
        old[38] = (old[38] & !4) | (new[38] & 4);
    } else if new[0] & 2 != 0 || *new == [0; 47] {
        // SDL sends an all-zero payload when rumble ends. Keep the previous
        // selector so the physical controller accepts the zero motor values.
        old[0] |= new[0] & 2;
        old[2..4].fill(0);
    }
    old[1] |= new[1] & !8; // reset-lights 不在每个音频 tick 重复。
    old[38] |= new[38];
    for (flag, start, end) in [
        (4, 4, 5),
        (5, 5, 6),
        (6, 6, 7),
        (7, 7, 8),
        (2, 10, 21),
        (3, 21, 32),
    ] {
        if new[0] & (1 << flag) != 0 {
            old[start..end].copy_from_slice(&new[start..end]);
        }
    }
    for (flag, start, end) in [
        (0, 8, 9),
        (1, 9, 10),
        (4, 43, 44),
        (5, 39, 40),
        (6, 36, 37),
        (7, 37, 38),
        (2, 44, 47),
    ] {
        if new[1] & (1 << flag) != 0 {
            old[start..end].copy_from_slice(&new[start..end]);
        }
    }
    if new[38] & 1 != 0 {
        old[42] = new[42];
    }
    if new[38] & 2 != 0 {
        old[41] = new[41];
    }
    old[32..36].copy_from_slice(&new[32..36]);
}
pub fn audio_state(game: &[u8; 47], cfg: &Config, headset: bool) -> ([u8; 47], u8) {
    let mut s = *game;
    if game[0] & (1 << 4) == 0 {
        s[4] = cfg.headphone_volume;
    }
    if game[0] & (1 << 5) == 0 {
        s[5] = cfg.speaker_volume;
    }
    let path = match cfg.audio_route {
        1 => 0x30,
        2 => 0,
        _ if game[0] & 0x80 != 0 => game[7] & 0x30,
        _ if headset => 0,
        _ => 0x30,
    };
    s[7] = (s[7] & !0x30) | path;
    s[0] |= 0xB0;
    for value in &mut s[2..4] {
        *value = (*value as f32 * cfg.rumble_gain as f32 / 10.0).min(255.0) as u8;
    }
    (
        s,
        if path == 0x30 {
            audio::route::SPEAKER
        } else {
            audio::route::HEADSET
        },
    )
}
fn encode_block(
    encoder: &mut impl OpusEncoder,
    block: &[i16],
    route: u8,
) -> Option<[u8; OPUS_BYTES]> {
    encode_observed(encoder, block, route, |_| {})
}
fn encode_observed(
    encoder: &mut impl OpusEncoder,
    block: &[i16],
    route: u8,
    mut observe: impl FnMut(&[i16]),
) -> Option<[u8; OPUS_BYTES]> {
    let front = if route == audio::route::SPEAKER {
        audio::downmix_4_to_2(block, audio::FRAMES_PER_BLOCK)
    } else {
        block.chunks_exact(4).flat_map(|c| [c[0], c[1]]).collect()
    };
    let left: Vec<i16> = front.chunks_exact(2).map(|c| c[0]).collect();
    let right: Vec<i16> = front.chunks_exact(2).map(|c| c[1]).collect();
    let l = audio::resample_linear(&left, audio::OPUS_FRAMES);
    let r = audio::resample_linear(&right, audio::OPUS_FRAMES);
    let pcm: Vec<i16> = (0..audio::OPUS_FRAMES)
        .flat_map(|i| {
            [
                l.get(i).copied().unwrap_or(0),
                r.get(i).copied().unwrap_or(0),
            ]
        })
        .collect();
    let mut packet = [0; OPUS_BYTES];
    // 不给短 Opus 包补零；短于 200B 的 CBR 包代表配置或编码错误。
    if encoder.encode(&pcm, &mut packet) == OPUS_BYTES {
        observe(&pcm);
        Some(packet)
    } else {
        None
    }
}

/// 音频打开后保持固定节拍：欠载发静音，同时继续发送灯光和扳机状态。
trait OutputSink: Send + Sync + 'static {
    fn write(&self, data: &[u8]) -> bool;
}
impl OutputSink for HidDevice {
    fn write(&self, data: &[u8]) -> bool {
        HidDevice::write(self, data)
    }
}

const FEEDBACK_TEST_DURATION: Duration = Duration::from_secs(1);
const HAPTICS_TEST_FRAMES: usize = audio::SAMPLE_RATE as usize / audio::HAPTICS_DECIMATION;

fn haptics_test_sweep(first_frame: usize) -> [u8; HAPTICS_BYTES] {
    let mut block = [0; HAPTICS_BYTES];
    for (offset, stereo) in block.chunks_exact_mut(2).enumerate() {
        let frame = first_frame + offset;
        if frame >= HAPTICS_TEST_FRAMES {
            break;
        }
        let t = frame as f64 / HAPTICS_TEST_FRAMES as f64;
        // Linear 50–250 Hz sweep with a 20 ms fade at each end.
        let phase = std::f64::consts::TAU * (50.0 * t + 100.0 * t * t);
        let fade = (t.min(1.0 - t) / 0.02).min(1.0);
        stereo.fill((64.0 * fade * phase.sin()).round() as i8 as u8);
    }
    block
}

fn write_hid_state(sink: &impl OutputSink, state: &[u8; 47], seq: u8) -> bool {
    let mut usb = [0; 48];
    usb[0] = crate::output::USB_REPORT_ID;
    usb[1..].copy_from_slice(state);
    crate::output::usb_to_bluetooth(&usb, seq).is_some_and(|report| sink.write(&report))
}

pub fn spawn(
    session: Arc<AudioSession>,
    physical: Arc<HidDevice>,
    running: Arc<AtomicBool>,
    cfg: Config,
) -> std::io::Result<JoinHandle<()>> {
    spawn_with_sink(session, physical, running, cfg)
}
fn spawn_with_sink<S: OutputSink>(
    session: Arc<AudioSession>,
    physical: Arc<S>,
    running: Arc<AtomicBool>,
    cfg: Config,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("opends-output".into())
        .spawn(move || {
            let stats = session.stats.clone();
            let mut encoder = LibOpusEncoder::with_complexity(cfg.opus_complexity);
            if encoder.is_none()
                && (cfg.audio_enabled || cfg.haptics_enabled || cfg.microphone_enabled)
            {
                stats.encode_fail.fetch_add(1, Ordering::Relaxed);
            }
            let period = Duration::from_secs_f64(512.0 / 48000.0);
            let mut tick = Instant::now();
            let (mut seq, mut counter) = (1u8, 0u8);
            let (mut primed, mut needs_prime) = (false, true);
            let mut warmup = 0u8;
            let (mut generation, mut revision) = (0, 0);
            let mut game = [0u8; 47];
            let mut failures = 0;
            let mut pending_pcm = None;
            let mut pending_haptics = None;
            let mut light_reset = false;
            let mut rumble = audio::RumbleMixer::default();
            let silence = vec![0i16; audio::FRAMES_PER_BLOCK * 4];
            let mut silent_packet = encoder
                .as_mut()
                .and_then(|e| encode_block(e, &silence, audio::route::SPEAKER));
            stats
                .ready
                .store(silent_packet.is_some(), Ordering::Release);
            let mut test_start = None;
            let mut test_frame = 0;
            let mut test_primed_audio = false;
            let mut resume_audio = false;
            while running.load(Ordering::Acquire) {
                let test = session.feedback_test.load(Ordering::Acquire);
                let native_test = test == FeedbackTest::NativeHaptics as u8;
                let classic_test = test == FeedbackTest::ClassicRumble as u8;
                let mut test_finished = false;
                light_reset |= session.light_reset.swap(false, Ordering::AcqRel);
                let next_gen = session.generation.load(Ordering::Acquire);
                if next_gen != generation {
                    generation = next_gen;
                    needs_prime = true;
                    pending_pcm = None;
                    pending_haptics = None;
                    if let Some(e) = encoder.as_mut() {
                        e.reset();
                        silent_packet = encode_block(e, &silence, audio::route::SPEAKER);
                        stats
                            .ready
                            .store(silent_packet.is_some(), Ordering::Release);
                    }
                }
                let rev = session.revision.load(Ordering::Acquire);
                if rev != revision {
                    if let Ok(s) = session.output.try_lock() {
                        game = *s;
                        revision = rev;
                    }
                }
                let mut pcm = if warmup == 0 || classic_test {
                    pending_pcm.take().or_else(|| session.pcm.take(generation))
                } else {
                    None
                };
                let mut native = if warmup == 0 || classic_test {
                    pending_haptics
                        .take()
                        .or_else(|| session.haptics.take(generation))
                } else {
                    None
                };
                let has_data = pcm.is_some() || native.is_some();
                let (mut state, route) =
                    audio_state(&game, &cfg, session.headset.load(Ordering::Relaxed));
                if let Some(m) = session.monitor.get() {
                    m.route.store(route as u32, Ordering::Relaxed);
                    m.volume.store(state[5] as u32, Ordering::Relaxed);
                }
                crate::output::adapt_rumble(
                    &mut state,
                    session.vibration_v2.load(Ordering::Relaxed),
                );
                if light_reset {
                    state[1] |= 8;
                }
                let mut ok = true;
                if !classic_test
                    && (cfg.audio_enabled
                        || cfg.haptics_enabled
                        || cfg.microphone_enabled
                        || native_test)
                    && encoder.is_some()
                    && silent_packet.is_some()
                    && (primed || has_data || cfg.microphone_enabled || native_test)
                {
                    // 音频/原生触觉模式必须清除传统马达选择器，震动改走 PCM 混合。
                    state[0] &= !3;
                    state[38] &= !4;
                    state[1] |= 0x80;
                    state[37] = (state[37] & !7) | 2;
                    if resume_audio
                        || (needs_prime && (has_data || cfg.microphone_enabled || native_test))
                    {
                        ok = physical.write(&audio::build_init_prime(&state));
                        if ok && cfg.microphone_enabled {
                            ok = physical.write(&audio::microphone_control(seq, true));
                            if ok {
                                seq = seq.wrapping_add(1);
                            }
                        }
                        if ok {
                            test_primed_audio |= native_test && !primed;
                            primed = true;
                            needs_prime = false;
                            resume_audio = false;
                            warmup = 8;
                            pending_pcm = pcm.take();
                            pending_haptics = native.take();
                        }
                    }
                    if primed {
                        let warm = warmup > 0;
                        warmup = warmup.saturating_sub(1);
                        let mut h = if native_test {
                            if warm {
                                [0; HAPTICS_BYTES]
                            } else {
                                let start = test_start.get_or_insert_with(Instant::now);
                                test_finished = start.elapsed() >= FEEDBACK_TEST_DURATION
                                    || test_frame >= HAPTICS_TEST_FRAMES;
                                if test_finished {
                                    [0; HAPTICS_BYTES]
                                } else {
                                    let block = haptics_test_sweep(test_frame);
                                    test_frame += audio::HAPTICS_FRAMES;
                                    block
                                }
                            }
                        } else if cfg.haptics_enabled && !warm {
                            if let Some(p) = native.as_ref() {
                                audio::extract_stereo_haptics(p, 2, cfg.haptics_scale())
                            } else if let Some(p) = pcm.as_ref() {
                                audio::extract_stereo_haptics(p, 4, cfg.haptics_scale())
                            } else {
                                [0; HAPTICS_BYTES]
                            }
                        } else {
                            [0; HAPTICS_BYTES]
                        };
                        if !warm && !native_test {
                            let selected = game[0] & 1 != 0 || game[38] & 4 != 0;
                            rumble.mix(
                                &mut h,
                                if selected { state[3] } else { 0 },
                                if selected { state[2] } else { 0 },
                            );
                        }
                        let mut front = [0i16; audio::OPUS_FRAMES * 2];
                        let packet = if cfg.audio_enabled && !warm {
                            pcm.as_ref().and_then(|p| {
                                encoder.as_mut().and_then(|e| {
                                    encode_observed(e, p, route, |samples| {
                                        if session.monitor.get().is_some_and(|m| m.wants(1)) {
                                            front.copy_from_slice(samples);
                                        }
                                    })
                                })
                            })
                        } else {
                            None
                        };
                        if cfg.audio_enabled && pcm.is_some() && !warm && packet.is_none() {
                            stats.encode_fail.fetch_add(1, Ordering::Relaxed);
                        }
                        if let Some(p) = packet.as_ref().or(silent_packet.as_ref()) {
                            let mut report =
                                audio::build_combined_report(seq, counter, &state, p, &h, route);
                            if cfg.microphone_enabled {
                                audio::enable_microphone(&mut report);
                            }
                            ok &= physical.write(&report);
                            if ok {
                                if let Some(m) = session.monitor.get() {
                                    m.speaker_forwarded(&front);
                                    m.haptics(&h);
                                }
                                stats.blocks.fetch_add(1, Ordering::Relaxed);
                                stats.output_frames.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                } else if classic_test || rev != 0 {
                    let mut adapted = game;
                    if classic_test {
                        let start = test_start.get_or_insert_with(Instant::now);
                        test_finished = start.elapsed() >= FEEDBACK_TEST_DURATION;
                        adapted[0] |= 1;
                        adapted[2..4].fill(if test_finished { 0 } else { 96 });
                        // Resume an interrupted PCM stream through its normal priming path.
                        resume_audio |= primed;
                        warmup = 0;
                    }
                    crate::output::adapt_rumble(
                        &mut adapted,
                        session.vibration_v2.load(Ordering::Relaxed),
                    );
                    if light_reset {
                        adapted[1] |= 8;
                    }
                    ok = write_hid_state(physical.as_ref(), &adapted, seq);
                    if ok {
                        stats.output_frames.fetch_add(1, Ordering::Relaxed);
                    }
                }
                if ok {
                    if test_finished || (native_test && silent_packet.is_none()) {
                        if test_primed_audio {
                            primed = false;
                            needs_prime = true;
                        }
                        test_start = None;
                        test_frame = 0;
                        test_primed_audio = false;
                        rumble = audio::RumbleMixer::default();
                        session.feedback_test.store(0, Ordering::Release);
                    }
                    light_reset = false;
                    failures = 0;
                    seq = seq.wrapping_add(1);
                    counter = counter.wrapping_add(1);
                } else {
                    failures += 1;
                    stats.write_fail.fetch_add(1, Ordering::Relaxed);
                }
                if failures >= 5 {
                    stats.failed.store(true, Ordering::Release);
                    break;
                }
                tick += period;
                let now = Instant::now();
                if tick > now {
                    std::thread::sleep(tick - now);
                } else {
                    tick = now;
                }
            }
            let mut off = [0u8; 47];
            off[0] = 0x0F;
            crate::output::adapt_rumble(&mut off, session.vibration_v2.load(Ordering::Relaxed));
            if !primed
                || silent_packet.is_none()
                || session.feedback_test.load(Ordering::Acquire)
                    == FeedbackTest::ClassicRumble as u8
            {
                let _ = write_hid_state(physical.as_ref(), &off, seq);
                seq = seq.wrapping_add(1);
            }
            if primed {
                if let Some(p) = silent_packet {
                    let _ = physical.write(&audio::build_combined_report(
                        seq,
                        counter,
                        &off,
                        &p,
                        &[0; HAPTICS_BYTES],
                        audio::route::SPEAKER,
                    ));
                }
            }
            if cfg.microphone_enabled {
                let _ = physical.write(&audio::microphone_control(seq.wrapping_add(1), false));
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(p: &[i16]) -> Vec<u8> {
        p.iter().flat_map(|s| s.to_le_bytes()).collect()
    }

    #[derive(Default)]
    struct PulseSink(Mutex<Vec<Vec<u8>>>);
    impl OutputSink for PulseSink {
        fn write(&self, report: &[u8]) -> bool {
            self.0.lock().unwrap().push(report.to_vec());
            true
        }
    }

    fn wait_for_output(condition: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(4);
        while !condition() {
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        true
    }

    #[test]
    fn feedback_tests_use_distinct_bluetooth_paths_and_stop() {
        if !crate::opus::is_available() {
            return;
        }
        for (test, competing, prime_audio) in [
            (
                FeedbackTest::NativeHaptics,
                FeedbackTest::ClassicRumble,
                false,
            ),
            (
                FeedbackTest::ClassicRumble,
                FeedbackTest::NativeHaptics,
                true,
            ),
        ] {
            let session = AudioSession::new(1);
            session.vibration_v2.store(true, Ordering::Relaxed);
            let running = Arc::new(AtomicBool::new(true));
            let sink = Arc::new(PulseSink::default());
            let cfg = Config {
                audio_enabled: false,
                haptics_enabled: false,
                microphone_enabled: prime_audio,
                ..Config::default()
            };
            let worker =
                spawn_with_sink(session.clone(), sink.clone(), running.clone(), cfg).unwrap();
            let ready = wait_for_output(|| {
                session.feedback_test_options().0
                    && (!prime_audio || session.stats.blocks.load(Ordering::Relaxed) >= 9)
            });
            session.request_feedback_test(test);
            session.request_feedback_test(competing);
            let accepted = session.feedback_test.load(Ordering::Acquire) == test as u8;
            let finished = wait_for_output(|| session.feedback_test_options().0);
            let resumed = !prime_audio
                || wait_for_output(|| {
                    let reports = sink.0.lock().unwrap();
                    reports
                        .last()
                        .is_some_and(|r| r[0] == audio::REPORT_ID_COMBINED)
                });
            running.store(false, Ordering::Release);
            worker.join().unwrap();
            assert!(ready && accepted && finished && resumed);

            let reports = sink.0.lock().unwrap();
            if prime_audio {
                let start = reports.iter().position(|r| r[0] == 0x31).unwrap();
                let pulse = &reports[start..];
                assert!(start > 0 && reports[start - 1][0] == audio::REPORT_ID_COMBINED);
                assert_eq!(&pulse[0][5..7], &[96, 96]);
                let stop = pulse
                    .iter()
                    .position(|r| r[0] == 0x31 && r[5..7] == [0, 0])
                    .unwrap();
                assert!(pulse[..=stop]
                    .iter()
                    .all(|r| r[0] == 0x31 && r[41] & 4 != 0));
                assert!(pulse[stop + 1..]
                    .iter()
                    .any(|r| r[0] == audio::REPORT_ID_COMBINED));
            } else {
                let last = reports
                    .iter()
                    .rposition(|r| {
                        r[0] == audio::REPORT_ID_COMBINED && r[78..142].iter().any(|&v| v != 0)
                    })
                    .unwrap();
                let stop = &reports[last + 1];
                assert_eq!(stop[0], audio::REPORT_ID_COMBINED);
                assert!(stop[78..142].iter().all(|&v| v == 0));
                assert!(reports[..=last].iter().all(|r| r[0] != 0x31));
            }
        }
    }
    #[test]
    fn oversized_pcm_keeps_whole_latest_frames_with_bounded_memory() {
        let q = PcmBuffer::new(1, 4);
        q.push_bytes(&bytes(&vec![1; 4096]), 0);
        assert_eq!(q.inner.lock().unwrap().samples.len(), 2048);
        assert!(q.take(0).is_some());
        assert!(q.take(0).is_none());
        q.push_bytes(&[0, 1, 2], 0);
        assert!(q.take(0).is_none());
    }
    #[test]
    fn reset_discards_old_pcm_but_preserves_new_generation() {
        let q = PcmBuffer::new(2, 2);
        q.push_bytes(&bytes(&vec![1; 1024]), 0);
        assert!(q.take(1).is_none());
        q.push_bytes(&bytes(&vec![2; 1024]), 1);
        q.push_bytes(&bytes(&vec![3; 1024]), 0);
        assert!(q.take(1).unwrap().iter().all(|&x| x == 2));
    }
    #[test]
    fn led_update_preserves_trigger_and_game_audio_settings() {
        let mut state = [0; 47];
        let mut p = [0; 47];
        p[0] = 0xA4;
        p[5] = 12;
        p[10] = 0x21;
        merge_output(&mut state, &p);
        let mut led = [0; 47];
        led[1] = 4;
        led[44] = 200;
        merge_output(&mut state, &led);
        let (out, route) = audio_state(&state, &Config::default(), false);
        assert_eq!((out[5], out[10], out[44]), (12, 0x21, 200));
        assert_eq!(route, audio::route::HEADSET);
    }
    #[test]
    fn partial_led_update_preserves_rumble_and_light_reset_is_one_shot() {
        let s = AudioSession::new(1);
        let mut rumble = [0; 47];
        rumble[0] = 3;
        rumble[2] = 42;
        s.update_output(rumble);
        let mut led = [0; 47];
        led[1] = 12;
        led[44] = 200;
        s.update_output(led);
        let current = *s.output.lock().unwrap();
        assert_eq!((current[0] & 3, current[2], current[44]), (3, 42, 200));
        assert_eq!(current[1] & 8, 0);
        assert!(s.light_reset.swap(false, Ordering::AcqRel));
        assert!(!s.light_reset.swap(false, Ordering::AcqRel));
        let mut off = [0; 47];
        off[0] = 2;
        s.update_output(off);
        assert_eq!(s.output.lock().unwrap()[2], 0);
    }
    #[test]
    fn sdl_zero_payload_stops_both_rumble_protocols_and_preserves_other_feedback() {
        for v2 in [false, true] {
            let s = AudioSession::new(1);
            let mut start = [0; 47];
            start[0] = 0x0F;
            start[1] = 4;
            start[2..4].copy_from_slice(&[32, 96]);
            start[10] = 0x21;
            start[44] = 180;
            crate::output::adapt_rumble(&mut start, v2);
            s.update_output(start);
            s.update_output([0; 47]);

            let mut stopped = *s.output.lock().unwrap();
            crate::output::adapt_rumble(&mut stopped, v2);
            assert_eq!(&stopped[2..4], &[0, 0]);
            assert_eq!(stopped[0] & 3, if v2 { 2 } else { 3 });
            assert_eq!(stopped[38] & 4, if v2 { 4 } else { 0 });
            assert_eq!((stopped[10], stopped[44]), (0x21, 180));
        }
    }
    #[test]
    fn a_single_stop_report_survives_output_state_contention() {
        let session = AudioSession::new(1);
        let mut start = [0; 47];
        start[0] = 3;
        start[2] = 96;
        session.update_output(start);

        let snapshot = session.output.lock().unwrap();
        let (started, callback_started) = std::sync::mpsc::channel();
        let (done, callback_done) = std::sync::mpsc::channel();
        let producer = session.clone();
        let worker = std::thread::spawn(move || {
            started.send(()).unwrap();
            let mut stop = [0; 47];
            stop[0] = 3;
            producer.update_output(stop);
            done.send(()).unwrap();
        });
        callback_started.recv().unwrap();
        let completed_while_locked = callback_done
            .recv_timeout(Duration::from_millis(50))
            .is_ok();
        drop(snapshot);
        worker.join().unwrap();

        assert!(!completed_while_locked);
        assert_eq!(session.output.lock().unwrap()[2], 0);
    }
    #[test]
    fn fallback_honors_headset_and_user_override() {
        let mut c = Config::default();
        assert_eq!(audio_state(&[0; 47], &c, false).1, audio::route::SPEAKER);
        assert_eq!(audio_state(&[0; 47], &c, true).1, audio::route::HEADSET);
        c.audio_route = 1;
        assert_eq!(audio_state(&[0; 47], &c, true).1, audio::route::SPEAKER);
    }
    #[test]
    fn silent_keepalive_is_fixed_size_opus() {
        if let Some(mut e) = LibOpusEncoder::new() {
            assert!(encode_block(&mut e, &vec![0; 2048], audio::route::SPEAKER).is_some());
        }
    }

    struct RecordingSink {
        reports: Mutex<Vec<Vec<u8>>>,
        running: Arc<AtomicBool>,
        session: Arc<AudioSession>,
    }
    impl OutputSink for RecordingSink {
        fn write(&self, data: &[u8]) -> bool {
            let mut r = self.reports.lock().unwrap();
            r.push(data.to_vec());
            let n = r.iter().filter(|r| r.first() == Some(&0x36)).count();
            if n == 10 {
                let mut p = [0; 47];
                p[0] = 4;
                p[1] = 8;
                p[10] = 0x21;
                self.session.update_output(p);
            }
            if n >= 13 {
                self.running.store(false, Ordering::Release);
            }
            true
        }
    }
    #[test]
    fn consecutive_10ms_usb_batches_preserve_rear_channels_across_bluetooth_blocks() {
        if !crate::opus::is_available() {
            return;
        }
        let s = AudioSession::new(4);
        // 原神真实 USB 抓包：每次回调 480 帧/3840B；蓝牙每块消费 512 帧。
        // 检查跨批次拼接，不让帧边界丢数据、交换左右音圈或混入前两路。
        for frame in [
            [20000i16, -20000, 4096, -8192],
            [20000i16, -20000, -4096, 8192],
            [20000i16, -20000, 12288, -16384],
        ] {
            let batch: Vec<i16> = (0..480).flat_map(|_| frame).collect();
            s.pcm.push_bytes(&bytes(&batch), 0);
        }
        let running = Arc::new(AtomicBool::new(true));
        let sink = Arc::new(RecordingSink {
            reports: Mutex::new(vec![]),
            running: running.clone(),
            session: s.clone(),
        });
        spawn_with_sink(s, sink.clone(), running, Config::default())
            .unwrap()
            .join()
            .unwrap();
        let reports = sink.reports.lock().unwrap();
        let combined: Vec<_> = reports.iter().filter(|r| r[0] == 0x36).collect();
        let first: Vec<_> = combined[8][78..142]
            .chunks_exact(2)
            .map(|v| [v[0] as i8, v[1] as i8])
            .collect();
        let second: Vec<_> = combined[9][78..142]
            .chunks_exact(2)
            .map(|v| [v[0] as i8, v[1] as i8])
            .collect();
        assert!(first[..30].iter().all(|&v| v == [16, -32]));
        assert!(first[30..].iter().all(|&v| v == [-16, 32]));
        assert!(second[..28].iter().all(|&v| v == [-16, 32]));
        assert!(second[28..].iter().all(|&v| v == [48, -64]));
    }
    #[test]
    fn short_effect_survives_preroll_and_paused_audio_keeps_trigger_updates() {
        if !crate::opus::is_available() {
            return;
        }
        let s = AudioSession::new(4);
        let running = Arc::new(AtomicBool::new(true));
        let pcm: Vec<i16> = (0..512).flat_map(|_| [1000, 1000, 4096, -8192]).collect();
        s.pcm.push_bytes(&bytes(&pcm), 0);
        let sink = Arc::new(RecordingSink {
            reports: Mutex::new(vec![]),
            running: running.clone(),
            session: s.clone(),
        });
        spawn_with_sink(s.clone(), sink.clone(), running, Config::default())
            .unwrap()
            .join()
            .unwrap();
        let r = sink.reports.lock().unwrap();
        let combined: Vec<_> = r.iter().filter(|r| r[0] == 0x36).collect();
        assert!(combined.len() >= 13);
        assert!(combined[8][78..142]
            .chunks_exact(2)
            .all(|v| v[0] as i8 == 16 && v[1] as i8 == -32));
        assert!(combined[9][78..142].iter().all(|&v| v == 0));
        assert_eq!(
            combined[10][13 + 10],
            0x21,
            "无 PCM 时也必须发送新的扳机状态"
        );
        assert_eq!(combined[10][14] & 8, 8);
        assert_eq!(combined[11][14] & 8, 0, "重置灯光只发送一次");
        for r in combined {
            let end = r.len() - 4;
            assert_eq!(
                u32::from_le_bytes(r[end..].try_into().unwrap()),
                crate::report::bt_output_crc32(&r[..end])
            );
        }
    }
}
