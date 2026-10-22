//! 可取消、可重复启停的单手柄桥接；所有回调限定到当前设备会话。
use crate::{
    audio_thread::{self, AudioSession},
    config::Config,
    hid::HidDevice,
    hidhide,
    report::{self, InputReport},
    viiper,
};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct BridgeState {
    pub running: AtomicBool,
    pub connected: AtomicBool,
    /// Confirmed through the current session's HidHide control handle.
    pub hidhide_healthy: AtomicBool,
    pub stop: AtomicBool,
    pub frames_in: AtomicU64,
    pub frames_out: AtomicU64,
    pub invalid_frames: AtomicU64,
    pub audio_blocks: AtomicU64,
    pub output_frames: AtomicU64,
    pub output_callbacks: AtomicU64,
    pub reconnects: AtomicU64,
    pub last_error: Mutex<Option<String>>,
    logs: Mutex<VecDeque<String>>,
}
impl BridgeState {
    pub fn log(&self, s: impl Into<String>) {
        let s = crate::logging::bounded(&s.into());
        eprintln!("{s}");
        crate::logging::record(&s);
        if let Ok(mut l) = self.logs.lock() {
            if l.len() >= 200 {
                l.pop_front();
            }
            l.push_back(s);
        }
    }
    pub fn logs(&self) -> String {
        self.logs
            .lock()
            .map(|l| l.iter().cloned().collect::<Vec<_>>().join("\r\n"))
            .unwrap_or_default()
    }
    pub fn error(&self, s: String) {
        self.log(&s);
        if let Ok(mut e) = self.last_error.lock() {
            *e = Some(s);
        }
    }
}

pub struct BridgeController {
    pub state: Arc<BridgeState>,
    worker: Option<JoinHandle<()>>,
}
impl Default for BridgeController {
    fn default() -> Self {
        Self {
            state: Arc::new(BridgeState::default()),
            worker: None,
        }
    }
}
impl BridgeController {
    pub fn start(&mut self, cfg: Config, limit: Option<Duration>) -> Result<(), String> {
        self.poll();
        if self.worker.is_some() {
            return Err("桥接已运行或正在停止".into());
        }
        cfg.validate()?;
        if !viiper::is_available() {
            return Err("未找到配套 libVIIPER.dll，请先检查依赖。".into());
        }
        if cfg.microphone_enabled
            && (!viiper::supports_microphone() || crate::opus::LibOpusDecoder::new().is_none())
        {
            return Err("配套运行库不支持麦克风转发；请更新 DLL 或关闭麦克风设置。".into());
        }
        if (cfg.audio_enabled || cfg.haptics_enabled || cfg.microphone_enabled)
            && crate::opus::LibOpusEncoder::with_complexity(cfg.opus_complexity).is_none()
        {
            return Err("Opus 编码器不可用；请安装配套运行库，或关闭音频和原生触觉转发。".into());
        }
        let state = Arc::new(BridgeState::default());
        state.running.store(true, Ordering::Release);
        self.state = state.clone();
        self.worker = Some(
            std::thread::Builder::new()
                .name("opends-bridge".into())
                .spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run(cfg, &state, limit)
                    }));
                    match result {
                        Ok(Err(e)) => state.error(e),
                        Err(_) => state.error("桥接线程异常；已执行资源清理，请检查日志。".into()),
                        _ => {}
                    }
                    state.connected.store(false, Ordering::Release);
                    state.hidhide_healthy.store(false, Ordering::Release);
                    state.running.store(false, Ordering::Release);
                    state.log("桥接已停止");
                })
                .map_err(|e| {
                    self.state.running.store(false, Ordering::Release);
                    format!("启动桥接线程失败：{e}")
                })?,
        );
        Ok(())
    }
    pub fn stop(&self) {
        self.state.stop.store(true, Ordering::Release);
    }
    pub fn poll(&mut self) {
        if self.worker.as_ref().is_some_and(|h| h.is_finished()) {
            if let Some(h) = self.worker.take() {
                let _ = h.join();
            }
        }
    }
    pub fn is_idle(&self) -> bool {
        self.worker.is_none()
    }
    pub fn wait(&mut self) {
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}
impl Drop for BridgeController {
    fn drop(&mut self) {
        self.stop();
        self.wait();
    }
}

struct CallbackTarget {
    handle: usize,
    session: Arc<AudioSession>,
}
static CALLBACKS: RwLock<Option<CallbackTarget>> = RwLock::new(None);
fn callback(handle: usize, f: impl FnOnce(&AudioSession)) {
    if let Ok(g) = CALLBACKS.try_read() {
        if let Some(t) = g.as_ref() {
            if t.handle == handle {
                f(&t.session);
            }
        }
    }
}
extern "C" fn on_output(handle: usize, p: *const viiper::DsOutputState) {
    if !p.is_null() {
        let state = unsafe { std::ptr::read_unaligned(p) }.to_payload();
        callback(handle, |s| s.update_output(state));
    }
}
extern "C" fn on_audio(handle: usize, p: *const u8, len: usize) {
    if !p.is_null() {
        callback(handle, |s| {
            s.stats.pcm_callbacks.fetch_add(1, Ordering::Relaxed);
            s.stats
                .pcm_max_bytes
                .fetch_max(len as u64, Ordering::Relaxed);
            if len == 0 || len > 65536 || len % 8 != 0 {
                s.stats.pcm_rejected.fetch_add(1, Ordering::Relaxed);
                return;
            }
            let data = unsafe { std::slice::from_raw_parts(p, len) };
            if data.iter().any(|&b| b != 0) {
                s.stats.pcm_nonzero.fetch_add(1, Ordering::Relaxed);
            }
            let generation = s.generation.load(Ordering::Acquire);
            s.pcm.push_bytes(data, generation);
        });
    }
}
extern "C" fn on_haptics(handle: usize, p: *const u8, len: usize) {
    if !p.is_null() && len > 0 && len <= 32768 && len % 4 == 0 {
        callback(handle, |s| {
            if unsafe { std::slice::from_raw_parts(p, len) }
                .iter()
                .any(|&b| b != 0)
            {
                s.stats.haptics_nonzero.fetch_add(1, Ordering::Relaxed);
            }
            let generation = s.generation.load(Ordering::Acquire);
            s.haptics
                .push_bytes(unsafe { std::slice::from_raw_parts(p, len) }, generation);
        });
    }
}
extern "C" fn on_reset(handle: usize) {
    callback(handle, |s| s.reset());
}

struct Session {
    device: viiper::Device,
    // server 必须比 device 活得更久，按字段声明顺序析构。
    _server: viiper::Viiper,
    hider: Option<hidhide::Hider>,
    output_running: Arc<AtomicBool>,
    output: Option<JoinHandle<()>>,
    state: Arc<BridgeState>,
    ude_port: Option<u32>,
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.device.send_raw_report(&report::neutral_usb_report());
        if !self.device.clear_callbacks() {
            self.state
                .log("部分回调注销失败；回调入口已隔离，随后移除虚拟设备");
        }
        if let Ok(mut g) = CALLBACKS.write() {
            if g.as_ref().is_some_and(|t| t.handle == self.device.handle()) {
                *g = None;
            }
        }
        self.output_running.store(false, Ordering::Release);
        if let Some(h) = self.output.take() {
            if h.join().is_err() {
                self.state.error("输出线程异常，已停止会话".into());
            }
        }
        if let Some(port) = self.ude_port.take() {
            if let Err(e) = crate::attach::detach(port) {
                self.state.error(e);
            }
        }
        if !self.device.remove() {
            self.state
                .error("移除虚拟设备失败；请检查 UDE 驱动状态".into());
        }
        if let Some(mut h) = self.hider.take() {
            if let Err(e) = h.restore() {
                self.state.error(e);
            }
        }
        self.state.hidhide_healthy.store(false, Ordering::Release);
    }
}

fn expired(start: Instant, limit: Option<Duration>, state: &BridgeState) -> bool {
    state.stop.load(Ordering::Acquire) || limit.is_some_and(|d| start.elapsed() >= d)
}
fn wait_retry(start: Instant, limit: Option<Duration>, state: &BridgeState) {
    for _ in 0..10 {
        if expired(start, limit, state) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn run(cfg: Config, state: &Arc<BridgeState>, limit: Option<Duration>) -> Result<(), String> {
    let started = Instant::now();
    let mut waiting = false;
    while !expired(started, limit, state) {
        // 上一次恢复失败时绝不覆盖原日志继续隐藏。
        hidhide::recover_if_needed()?;
        let physical = HidDevice::enum_paths()
            .into_iter()
            .filter(|p| cfg.device_path.is_empty() || p.eq_ignore_ascii_case(&cfg.device_path))
            .filter_map(|p| HidDevice::open(&p))
            .find(|d| d.is_dualsense_bluetooth());
        let Some(physical) = physical else {
            if !cfg.reconnect {
                return Err("未找到指定的蓝牙 DualSense；请检查连接和 HidHide 白名单。".into());
            }
            if !waiting {
                state.log("等待蓝牙 DualSense 连接…");
                waiting = true;
            }
            wait_retry(started, limit, state);
            continue;
        };
        if expired(started, limit, state) {
            break;
        }
        waiting = false;
        let result = run_session(&cfg, Arc::new(physical), state, started, limit);
        state.connected.store(false, Ordering::Release);
        match result {
            Ok(()) => break,
            Err(e) => {
                if !cfg.reconnect {
                    return Err(e);
                }
                // 可恢复的断线留在日志中；不污染最终退出码或覆盖清理失败。
                state.log(e);
                state.reconnects.fetch_add(1, Ordering::Relaxed);
                state.log("已释放输入，等待重新连接");
                wait_retry(started, limit, state);
            }
        }
    }
    Ok(())
}

fn run_session(
    cfg: &Config,
    physical: Arc<HidDevice>,
    state: &Arc<BridgeState>,
    started: Instant,
    limit: Option<Duration>,
) -> Result<(), String> {
    state.log(format!(
        "连接 PID={:04X}，创建虚拟 USB 设备…",
        physical.product_id
    ));
    let mut features = Vec::new();
    for (id, length) in [(0x05, 41), (0x09, 20), (0x20, 64)] {
        let mut report = None;
        for _ in 0..3 {
            if expired(started, limit, state) {
                return Ok(());
            }
            match physical.feature_report(id, length, 500) {
                Ok(value) => {
                    if let Some(usb) = report::usb_identity_feature(&value) {
                        report = Some(usb);
                        break;
                    }
                    state.log(format!("特性 0x{id:02X} 蓝牙 CRC 无效，重试…"));
                }
                Err(e) => state.log(format!("读取特性 0x{id:02X} 失败：{e}，重试…")),
            }
        }
        features.push(
            report.ok_or_else(|| format!("无法取得真实特性 0x{id:02X}；未创建不完整的虚拟身份"))?,
        );
    }
    let firmware = Some(u16::from_le_bytes([features[2][44], features[2][45]]));
    let mac = features[1][1..7]
        .iter()
        .rev()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":");
    let server = viiper::Viiper::start(&format!("127.0.0.1:{}", cfg.port))
        .ok_or("USB 服务启动失败，请检查 UDE 驱动和端口占用")?;
    crate::attach::cleanup_owned(cfg.port, server.bus_id(), || expired(started, limit, state))?;
    let pid = if physical.product_id == viiper::PID_DUALSENSE_EDGE && !cfg.edge_as_ds {
        viiper::PID_DUALSENSE_EDGE
    } else {
        viiper::PID_DUALSENSE
    };
    let serial = physical.serial_number();
    let device = server
        .create_dualsense_with_identity(viiper::VID_SONY, pid, false, serial.as_deref(), Some(&mac))
        .ok_or("创建虚拟 DualSense 失败")?;
    if pid == viiper::PID_DUALSENSE && !device.use_physical_usb_profile() {
        return Err("配套 VIIPER 缺少真机 USB 描述符配置，请使用当前发布包中的 DLL".into());
    }
    let audio = AudioSession::new(cfg.buffer as usize);
    let vibration_v2 = crate::output::uses_vibration_v2(physical.product_id, firmware);
    audio.vibration_v2.store(vibration_v2, Ordering::Relaxed);
    state.log(format!(
        "真实手柄固件特性={}，震动协议={}",
        firmware
            .map(|v| format!("{}.{:02}", v >> 8, v & 255))
            .unwrap_or_else(|| "读取失败，按兼容模式处理".into()),
        if vibration_v2 { "v2" } else { "v1" }
    ));
    let mut session = Session {
        device,
        _server: server,
        hider: None,
        output_running: Arc::new(AtomicBool::new(true)),
        output: None,
        state: state.clone(),
        ude_port: None,
    };
    for report in &features {
        if !session.device.set_feature_report(report) {
            return Err(format!("配套 VIIPER 无法设置真实特性 0x{:02X}", report[0]));
        }
    }
    session
        .device
        .send_raw_report(&report::neutral_usb_report());
    state.log("已校验蓝牙 CRC 并转换为 USB 特性，预载真实校准、配对与固件；随后挂接 USB");
    if cfg.hide_device {
        let id = hidhide::interface_path_to_instance_id(&physical.path)
            .ok_or("无法识别物理设备实例 ID")?;
        session.hider = Some(hidhide::Hider::engage(&[id])?);
        state.hidhide_healthy.store(true, Ordering::Release);
    }
    if expired(started, limit, state) {
        return Ok(());
    }
    // 主动切到完整输入模式；即使固件暂未切换，兼容报告也按真实布局转换。
    let mut enable = [0u8; 48];
    enable[0] = 2;
    if let Some(r) = crate::output::usb_to_bluetooth(&enable, 0) {
        physical
            .write_result(&r, 200)
            .map_err(|e| format!("激活完整模式失败：{e}"))?;
    }
    {
        let mut g = CALLBACKS.write().map_err(|_| "回调注册锁损坏")?;
        *g = Some(CallbackTarget {
            handle: session.device.handle(),
            session: audio.clone(),
        });
    }
    if !session.device.set_output_callback(on_output) {
        return Err("libVIIPER 缺少输出回调；请使用配套版本".into());
    }
    if cfg.audio_enabled || cfg.haptics_enabled {
        if !session.device.set_audio_callback(on_audio)
            || !session.device.set_haptics_callback(on_haptics)
            || !session.device.set_reset_callback(on_reset)
        {
            return Err("libVIIPER 缺少音频/触觉/流重置回调；请使用配套版本".into());
        }
    }
    session.output = Some(
        audio_thread::spawn(
            audio.clone(),
            physical.clone(),
            session.output_running.clone(),
            cfg.clone(),
        )
        .map_err(|e| format!("输出线程启动失败：{e}"))?,
    );
    session.ude_port = Some(
        session
            ._server
            .attach_prepared(cfg.port, || expired(started, limit, state))?,
    );
    state.connected.store(true, Ordering::Release);
    state.log("桥接已连接；输入、输出、音频使用独立调度");
    let mut last_valid = Instant::now();
    let mut last_stats = Instant::now();
    let mut last_hidhide_check = Instant::now();
    let mut old_blocks = 0;
    let mut old_outputs = 0;
    let mut old_callbacks = 0;
    let mut diagnostic = Instant::now();
    let (mut full, mut simple) = (0u64, 0u64);
    let mut gyro = [0i16; 3];
    let mut microphone = if cfg.microphone_enabled {
        Some(crate::microphone::Microphone::new().ok_or("麦克风解码器不可用")?)
    } else {
        None
    };
    while !expired(started, limit, state) {
        if audio.stats.failed.load(Ordering::Acquire) {
            return Err("连续写入物理手柄失败".into());
        }
        if last_valid.elapsed() >= Duration::from_secs(3) {
            return Err("手柄连续 3 秒没有有效输入，已判定断线".into());
        }
        match physical.read_result(20) {
            Ok(Some(data)) => {
                state.frames_in.fetch_add(1, Ordering::Relaxed);
                if data.first() == Some(&0x31) && !report::verify_input_crc(&data) {
                    state.invalid_frames.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                let Some(report) = InputReport::new(data) else {
                    state.invalid_frames.fetch_add(1, Ordering::Relaxed);
                    continue;
                };
                if report.has_audio_payload() {
                    if let (Some(mic), Some(packet)) =
                        (microphone.as_mut(), report.microphone_frame())
                    {
                        let _ = mic.decode(packet, |frame| session.device.send_microphone(frame));
                    }
                    continue;
                }
                if let Some(s) = report.parse() {
                    gyro = s.motion.gyro;
                    if report.is_full_bluetooth() {
                        full += 1;
                    } else {
                        simple += 1;
                    }
                    audio
                        .headset
                        .store(s.connection & 1 != 0, Ordering::Relaxed);
                }
                if let Some(usb) = report.usb_payload() {
                    if !session.device.send_raw_report(&usb) {
                        return Err("虚拟设备输入提交失败".into());
                    }
                    state.frames_out.fetch_add(1, Ordering::Relaxed);
                    last_valid = Instant::now();
                } else {
                    state.invalid_frames.fetch_add(1, Ordering::Relaxed);
                }
            }
            Ok(None) => {
                if last_valid.elapsed() >= Duration::from_secs(3) {
                    return Err("手柄连续 3 秒没有有效输入，已判定断线".into());
                }
            }
            Err(e) => return Err(format!("蓝牙手柄读取失败：{e}")),
        }
        if last_stats.elapsed() >= Duration::from_millis(250) {
            if last_hidhide_check.elapsed() >= Duration::from_secs(5) {
                state.hidhide_healthy.store(
                    session.hider.as_ref().is_some_and(|h| h.is_active()),
                    Ordering::Release,
                );
                last_hidhide_check = Instant::now();
            }
            let blocks = audio.stats.blocks.load(Ordering::Relaxed);
            let outputs = audio.stats.output_frames.load(Ordering::Relaxed);
            let callbacks = audio.stats.received_output.load(Ordering::Relaxed);
            state
                .output_callbacks
                .fetch_add(callbacks.saturating_sub(old_callbacks), Ordering::Relaxed);
            old_callbacks = callbacks;
            state
                .audio_blocks
                .fetch_add(blocks.saturating_sub(old_blocks), Ordering::Relaxed);
            state
                .output_frames
                .fetch_add(outputs.saturating_sub(old_outputs), Ordering::Relaxed);
            old_blocks = blocks;
            old_outputs = outputs;
            last_stats = Instant::now();
            if diagnostic.elapsed() >= Duration::from_secs(60) {
                let raw = audio
                    .stats
                    .last_output
                    .load(Ordering::Relaxed)
                    .to_le_bytes();
                state.log(format!("诊断：完整输入={full} 兼容输入={simple} 陀螺仪={gyro:?}；游戏反馈={callbacks} 非零震动={} 原生触觉={} 音频回调={}；最近标志={:02X} 马达=({}, {}) 蓝牙发送={outputs} 写失败={}",audio.stats.rumble_updates.load(Ordering::Relaxed),audio.stats.haptics_nonzero.load(Ordering::Relaxed),audio.stats.pcm_callbacks.load(Ordering::Relaxed),raw[0],raw[3],raw[2],audio.stats.write_fail.load(Ordering::Relaxed)));
                state.log(format!(
                    "音频批次：最大={}B 拒绝={} 非零={} 编码失败={}",
                    audio.stats.pcm_max_bytes.load(Ordering::Relaxed),
                    audio.stats.pcm_rejected.load(Ordering::Relaxed),
                    audio.stats.pcm_nonzero.load(Ordering::Relaxed),
                    audio.stats.encode_fail.load(Ordering::Relaxed)
                ));
                diagnostic = Instant::now();
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    static CALLBACK_TEST_LOCK: Mutex<()> = Mutex::new(());
    #[test]
    fn batched_usb_iso_pcm_and_haptics_are_accepted_instead_of_silently_dropped() {
        let _lock = CALLBACK_TEST_LOCK.lock().unwrap();
        let s = AudioSession::new(4);
        *CALLBACKS.write().unwrap() = Some(CallbackTarget {
            handle: 789,
            session: s.clone(),
        });
        let pcm: Vec<u8> = (0..480)
            .flat_map(|_| [1i16, 2, 3, 4])
            .flat_map(i16::to_le_bytes)
            .collect();
        let haptic: Vec<u8> = (0..480)
            .flat_map(|_| [3i16, 4])
            .flat_map(i16::to_le_bytes)
            .collect();
        for _ in 0..2 {
            on_audio(789, pcm.as_ptr(), pcm.len());
            on_haptics(789, haptic.as_ptr(), haptic.len());
        }
        assert_eq!(s.stats.pcm_callbacks.load(Ordering::Relaxed), 2);
        assert_eq!(s.stats.haptics_nonzero.load(Ordering::Relaxed), 2);
        assert_eq!(s.pcm.take(0).unwrap()[..4], [1, 2, 3, 4]);
        assert_eq!(s.haptics.take(0).unwrap()[..2], [3, 4]);
        *CALLBACKS.write().unwrap() = None;
    }
    #[test]
    fn old_device_callbacks_cannot_reach_new_session() {
        let _lock = CALLBACK_TEST_LOCK.lock().unwrap();
        let s = AudioSession::new(1);
        *CALLBACKS.write().unwrap() = Some(CallbackTarget {
            handle: 123,
            session: s.clone(),
        });
        on_reset(124);
        assert_eq!(s.generation.load(Ordering::Acquire), 0);
        on_reset(123);
        assert_eq!(s.generation.load(Ordering::Acquire), 1);
        *CALLBACKS.write().unwrap() = None;
    }
    #[test]
    fn stop_and_duration_are_both_honored() {
        let s = BridgeState::default();
        let t = Instant::now();
        assert!(!expired(t, None, &s));
        assert!(expired(t, Some(Duration::ZERO), &s));
        s.stop.store(true, Ordering::Release);
        assert!(expired(t, None, &s));
    }
}
