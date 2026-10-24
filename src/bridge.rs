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

const STABLE_CONNECTION: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct ConnectionIssue {
    pub error: String,
    pub connected_for: Option<Duration>,
    pub resource_busy: bool,
}
impl ConnectionIssue {
    pub fn detail(&self) -> String {
        if self.resource_busy {
            return crate::tr!(bridge_connection_resource_busy, error = self.error.as_str());
        }
        match self.connected_for {
            Some(age) if age < STABLE_CONNECTION => crate::tr!(
                bridge_connection_lost_immediately,
                milliseconds = crate::i18n_arg!(age.as_millis()),
                error = self.error.as_str()
            ),
            Some(age) => crate::tr!(
                bridge_connection_lost,
                seconds = crate::i18n_arg!(age.as_secs()),
                error = self.error.as_str()
            ),
            None => crate::tr!(bridge_connection_setup_failed, error = self.error.as_str()),
        }
    }
}

#[derive(Default)]
struct SessionProgress {
    connected_for: Option<Duration>,
    resource_busy: bool,
}

#[derive(Default)]
pub struct BridgeState {
    pub monitor: Arc<crate::monitor::Monitor>,
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
    pub connection_issue: Mutex<Option<ConnectionIssue>>,
    logs: Mutex<VecDeque<String>>,
}
impl BridgeState {
    fn session_failed(&self, error: String, connected_for: Option<Duration>, resource_busy: bool) {
        let issue = ConnectionIssue {
            error,
            connected_for,
            resource_busy,
        };
        self.log_message(
            crate::i18n::MessageId::bridge_connection_diagnostic,
            issue.detail(),
        );
        if let Ok(mut slot) = self.connection_issue.lock() {
            *slot = Some(issue);
        }
    }
    fn confirm_stable_connection(&self, age: Duration) -> bool {
        if age < STABLE_CONNECTION {
            return false;
        }
        if let Ok(mut slot) = self.connection_issue.lock() {
            slot.take();
        }
        true
    }
    pub fn log_message(&self, id: crate::i18n::MessageId, message: impl Into<String>) {
        self.log(format!("[{}] {}", id.key(), message.into()));
    }
    pub fn error_message(&self, id: crate::i18n::MessageId, message: String) {
        self.log_message(id, message.clone());
        if let Ok(mut error) = self.last_error.lock() {
            *error = Some(message);
        }
    }
    pub fn log(&self, s: impl Into<String>) {
        let s = crate::logging::bounded(&s.into());
        eprintln!("{s}");
        #[cfg(not(test))]
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
        self.log(format!("[bridge-error] {s}"));
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
            return Err(crate::tr!(bridge_bridging_is_already_running_or_stopping).into());
        }
        cfg.validate()?;
        if !viiper::is_available() {
            return Err(
                crate::tr!(bridge_matching_libviiper_dll_not_found_check_dependencies).into(),
            );
        }
        if cfg.microphone_enabled
            && (!viiper::supports_microphone() || crate::opus::LibOpusDecoder::new().is_none())
        {
            return Err(
                crate::tr!(bridge_the_runtime_does_not_support_microphone_forwarding).into(),
            );
        }
        if (cfg.audio_enabled || cfg.haptics_enabled || cfg.microphone_enabled)
            && crate::opus::LibOpusEncoder::with_complexity(cfg.opus_complexity).is_none()
        {
            return Err(
                crate::tr!(bridge_opus_encoder_unavailable_install_the_matching_runtime).into(),
            );
        }
        let state = Arc::new(BridgeState {
            monitor: self.state.monitor.clone(),
            ..BridgeState::default()
        });
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
                        Err(_) => state.error_message(crate::i18n::MessageId::bridge_bridge_thread_failed_unexpectedly_resources_have_been, crate::tr!(bridge_bridge_thread_failed_unexpectedly_resources_have_been).into()),
                        _ => {}
                    }
                    state.connected.store(false, Ordering::Release);
                    state.monitor.connected.store(false, Ordering::Release);
                    state.monitor.reset();
                    state.hidhide_healthy.store(false, Ordering::Release);
                    state.running.store(false, Ordering::Release);
                    state.log_message(crate::i18n::MessageId::bridge_bridge_stopped, crate::tr!(bridge_bridge_stopped));
                })
                .map_err(|e| {
                    self.state.running.store(false, Ordering::Release);
                    crate::tr!(bridge_could_not_start_bridge_thread, error = crate::i18n_arg!(e))
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
            let _ = crate::shutdown::join(h);
        }
    }
    pub fn test_stalled_shutdown_worker(&mut self) {
        // Reachable only from the isolated ui-smoke-test branch. No HID/DLL use.
        self.worker = Some(std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(10))
        }));
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
        let mut monitor = None;
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
            if s.forward_pcm.load(Ordering::Relaxed) {
                s.pcm.push_bytes(data, generation);
            }
            monitor = s
                .monitor
                .get()
                .and_then(|m| m.ticket(0).map(|ticket| (m.clone(), ticket)));
        });
        if let Some((m, ticket)) = monitor {
            m.bytes_with(ticket, unsafe { std::slice::from_raw_parts(p, len) });
        }
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
            if s.forward_pcm.load(Ordering::Relaxed) {
                s.haptics
                    .push_bytes(unsafe { std::slice::from_raw_parts(p, len) }, generation);
            }
        });
    }
}
extern "C" fn on_reset(handle: usize) {
    callback(handle, |s| {
        s.reset();
        if let Some(m) = s.monitor.get() {
            m.reset();
        }
    });
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
    usbip_exe: std::path::PathBuf,
}
impl Drop for Session {
    fn drop(&mut self) {
        // During OS shutdown, release hiding before potentially slow DLL/driver
        // teardown. Normal disconnects retain the usual virtual-then-physical order.
        if crate::shutdown::ending() {
            self.restore_hiding();
        }
        let _ = self.device.send_raw_report(&report::neutral_usb_report());
        if !self.device.clear_callbacks() {
            self.state.log_message(
                crate::i18n::MessageId::bridge_some_callback_removals_failed_callbacks_have_been,
                crate::tr!(bridge_some_callback_removals_failed_callbacks_have_been),
            );
        }
        if let Ok(mut g) = CALLBACKS.write() {
            if g.as_ref().is_some_and(|t| t.handle == self.device.handle()) {
                *g = None;
            }
        }
        self.output_running.store(false, Ordering::Release);
        if let Some(h) = self.output.take() {
            if matches!(crate::shutdown::join(h), Some(Err(_))) {
                self.state.error_message(crate::i18n::MessageId::bridge_output_thread_failed_unexpectedly_stopping_the_session, crate::tr!(bridge_output_thread_failed_unexpectedly_stopping_the_session).into());
            }
        }
        if let Some(port) = self.ude_port.take() {
            if let Err(e) = crate::attach::detach(&self.usbip_exe, port) {
                self.state.error(e);
            }
        }
        if !self.device.remove() {
            self.state.error_message(
                crate::i18n::MessageId::bridge_could_not_remove_virtual_device_check_the,
                crate::tr!(bridge_could_not_remove_virtual_device_check_the).into(),
            );
        }
        self.restore_hiding();
    }
}
impl Session {
    fn restore_hiding(&mut self) {
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
fn wait_retry(start: Instant, limit: Option<Duration>, state: &BridgeState, delay: Duration) {
    let retry_at = Instant::now() + delay;
    while Instant::now() < retry_at {
        if expired(start, limit, state) {
            break;
        }
        std::thread::sleep(
            retry_at
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(50)),
        );
    }
}

#[derive(Default)]
struct RetryBackoff {
    failures: u32,
}
impl RetryBackoff {
    fn failed(&mut self, connected_for: Option<Duration>) -> Duration {
        if connected_for.is_some_and(|age| age >= STABLE_CONNECTION) {
            self.failures = 0;
        }
        // Keep ordinary reconnects responsive; slow down only repeated failures.
        const DELAYS_MS: [u64; 8] = [250, 500, 1000, 2000, 4000, 8000, 16000, 30000];
        let milliseconds = DELAYS_MS[(self.failures as usize).min(DELAYS_MS.len() - 1)];
        self.failures = self.failures.saturating_add(1);
        Duration::from_millis(milliseconds)
    }
}

// A successful open/feature query does not prove that the BT input stream is live.
// No hiding, virtual USB device or output report is created by this probe.
fn probe_input(
    mut read: impl FnMut() -> std::io::Result<Option<Vec<u8>>>,
    cancelled: impl Fn() -> bool,
) -> Result<Option<[u8; 64]>, String> {
    let until = Instant::now() + Duration::from_millis(750);
    let mut valid = 0;
    while !cancelled() && Instant::now() < until {
        let data = read().map_err(|e| {
            crate::tr!(
                bridge_could_not_read_bluetooth_controller,
                error = crate::i18n_arg!(e)
            )
        })?;
        let Some(data) = data else { continue };
        if data.first() == Some(&0x31) && !report::verify_input_crc(&data) {
            valid = 0;
            continue;
        }
        let Some(input) = InputReport::new(data) else {
            valid = 0;
            continue;
        };
        if let Some(usb) = input.usb_payload() {
            valid += 1;
            if valid >= 3 {
                return Ok(Some(usb));
            }
        }
    }
    if cancelled() {
        Ok(None)
    } else {
        Err(crate::tr!(bridge_input_probe_timeout).into())
    }
}

// A PS press used to wake/connect the physical pad must not arrive on a newly
// enumerated virtual pad as a fresh Guide chord. Only this initial press is held.
struct InitialGuide {
    ready: bool,
    released_at: Option<Duration>,
}
impl InitialGuide {
    fn apply(&mut self, usb: &mut [u8; 64], age: Duration) {
        if !self.ready {
            if usb[10] & 1 != 0 {
                self.released_at = None;
            } else {
                let since = *self.released_at.get_or_insert(age);
                self.ready = age.saturating_sub(since) >= Duration::from_millis(120);
            }
            usb[10] &= !1;
        }
    }
}

fn run(cfg: Config, state: &Arc<BridgeState>, limit: Option<Duration>) -> Result<(), String> {
    let started = Instant::now();
    let mut waiting = false;
    let mut backoff = RetryBackoff::default();
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
                return Err(
                    crate::tr!(bridge_the_selected_bluetooth_dualsense_was_not_found).into(),
                );
            }
            if !waiting {
                state.log_message(
                    crate::i18n::MessageId::bridge_waiting_for_bluetooth_dualsense,
                    crate::tr!(bridge_waiting_for_bluetooth_dualsense),
                );
                waiting = true;
            }
            wait_retry(started, limit, state, Duration::from_secs(1));
            continue;
        };
        if expired(started, limit, state) {
            break;
        }
        waiting = false;
        let mut progress = SessionProgress::default();
        let result = run_session(
            &cfg,
            Arc::new(physical),
            state,
            started,
            limit,
            &mut progress,
        );
        state.connected.store(false, Ordering::Release);
        state.monitor.connected.store(false, Ordering::Release);
        state.monitor.reset();
        match result {
            Ok(()) => break,
            Err(e) => {
                if expired(started, limit, state) {
                    break;
                }
                // Cleanup time is not evidence of a stable input connection.
                let age = progress.connected_for;
                if !expired(started, limit, state) {
                    state.session_failed(e.clone(), age, progress.resource_busy);
                }
                if !cfg.reconnect {
                    return Err(e);
                }
                // 可恢复的断线留在日志中；不污染最终退出码或覆盖清理失败。
                state.log(e);
                state.reconnects.fetch_add(1, Ordering::Relaxed);
                state.log_message(
                    crate::i18n::MessageId::bridge_input_released_waiting_to_reconnect,
                    crate::tr!(bridge_input_released_waiting_to_reconnect),
                );
                let delay = backoff.failed(age);
                state.log_message(
                    crate::i18n::MessageId::bridge_retry_backoff,
                    crate::tr!(
                        bridge_retry_backoff,
                        seconds = crate::i18n_arg!(delay.as_secs_f64())
                    ),
                );
                wait_retry(started, limit, state, delay);
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
    progress: &mut SessionProgress,
) -> Result<(), String> {
    if probe_input(
        || physical.read_result(20),
        || expired(started, limit, state),
    )?
    .is_none()
    {
        return Ok(());
    }
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
                    state.log_message(
                        crate::i18n::MessageId::bridge_feature_0x_has_an_invalid_bluetooth_crc,
                        crate::tr!(
                            bridge_feature_0x_has_an_invalid_bluetooth_crc,
                            report = format!("{:02X}", id)
                        ),
                    );
                }
                Err(e) => {
                    progress.resource_busy |= e.raw_os_error() == Some(170);
                    state.log_message(
                        crate::i18n::MessageId::bridge_could_not_read_feature_0x_retrying,
                        crate::tr!(
                            bridge_could_not_read_feature_0x_retrying,
                            report = format!("{:02X}", id),
                            error = crate::i18n_arg!(e)
                        ),
                    );
                }
            }
        }
        features.push(report.ok_or_else(|| {
            crate::tr!(
                bridge_could_not_obtain_physical_feature_0x_no,
                report = format!("{:02X}", id)
            )
        })?);
    }
    let firmware = Some(u16::from_le_bytes([features[2][44], features[2][45]]));
    let mac = features[1][1..7]
        .iter()
        .rev()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":");
    // Enable and validate full input before causing any device-topology changes.
    let mut enable = [0u8; 48];
    enable[0] = 2;
    if let Some(r) = crate::output::usb_to_bluetooth(&enable, 0) {
        physical.write_result(&r, 200).map_err(|e| {
            crate::tr!(
                bridge_could_not_enable_full_input_mode,
                error = crate::i18n_arg!(e)
            )
        })?;
    }
    if probe_input(
        || physical.read_result(20),
        || expired(started, limit, state),
    )?
    .is_none()
    {
        return Ok(());
    }
    let mut guide = InitialGuide {
        ready: false,
        released_at: None,
    };
    state.log_message(
        crate::i18n::MessageId::bridge_physical_pid_creating_a_virtual_usb_device,
        crate::tr!(
            bridge_physical_pid_creating_a_virtual_usb_device,
            pid = format!("{:04X}", physical.product_id)
        ),
    );
    let usbip_exe = crate::dependency_paths::Component::Usbip
        .resolve(&cfg.usbip_directory)
        .ok_or(crate::tr!(attach_cannot_find_usb_ip_win2_usbip_exe))?;
    let server = viiper::Viiper::start(&format!("127.0.0.1:{}", cfg.port))
        .ok_or(crate::tr!(bridge_could_not_start_the_usb_server_check))?;
    crate::attach::cleanup_owned(&usbip_exe, cfg.port, server.bus_id(), || {
        expired(started, limit, state)
    })?;
    let pid = if physical.product_id == viiper::PID_DUALSENSE_EDGE && !cfg.edge_as_ds {
        viiper::PID_DUALSENSE_EDGE
    } else {
        viiper::PID_DUALSENSE
    };
    let serial = physical.serial_number();
    let device = server
        .create_dualsense_with_identity(viiper::VID_SONY, pid, false, serial.as_deref(), Some(&mac))
        .ok_or(crate::tr!(bridge_could_not_create_virtual_dualsense))?;
    if pid == viiper::PID_DUALSENSE && !device.use_physical_usb_profile() {
        return Err(crate::tr!(bridge_viiper_does_not_support_the_usb_audio).into());
    }
    let audio = AudioSession::new(cfg.buffer as usize);
    let _ = audio.monitor.set(state.monitor.clone());
    audio
        .forward_pcm
        .store(cfg.audio_enabled || cfg.haptics_enabled, Ordering::Relaxed);
    state.monitor.edge.store(
        physical.product_id == viiper::PID_DUALSENSE_EDGE,
        Ordering::Relaxed,
    );
    state
        .monitor
        .microphone
        .store(cfg.microphone_enabled, Ordering::Relaxed);
    state
        .monitor
        .microphone_gain_db
        .store(cfg.microphone_gain_db as u32, Ordering::Relaxed);
    let vibration_v2 = crate::output::uses_vibration_v2(physical.product_id, firmware);
    audio.vibration_v2.store(vibration_v2, Ordering::Relaxed);
    state.log_message(
        crate::i18n::MessageId::bridge_physical_controller_firmware_audio_protocol,
        crate::tr!(
            bridge_physical_controller_firmware_audio_protocol,
            firmware = crate::i18n_arg!(firmware
                .map(|v| format!("{}.{:02}", v >> 8, v & 255))
                .unwrap_or_else(|| crate::tr!(bridge_read_failed_using_compatibility_mode).into())),
            protocol = crate::i18n_arg!(if vibration_v2 { "v2" } else { "v1" })
        ),
    );
    let mut session = Session {
        device,
        _server: server,
        hider: None,
        output_running: Arc::new(AtomicBool::new(true)),
        output: None,
        state: state.clone(),
        ude_port: None,
        usbip_exe,
    };
    for report in &features {
        if !session.device.set_feature_report(report) {
            return Err(crate::tr!(
                bridge_viiper_could_not_set_physical_feature_0x,
                report = format!("{:02X}", report[0])
            ));
        }
    }
    session
        .device
        .send_raw_report(&report::neutral_usb_report());
    state.log_message(
        crate::i18n::MessageId::bridge_bluetooth_crc_verified_calibration_pairing_and_firmware,
        crate::tr!(bridge_bluetooth_crc_verified_calibration_pairing_and_firmware),
    );
    if cfg.hide_device {
        let id = hidhide::interface_path_to_instance_id(&physical.path).ok_or(crate::tr!(
            bridge_could_not_identify_the_physical_device_instance
        ))?;
        session.hider = Some(hidhide::Hider::engage(&[id])?);
        state.hidhide_healthy.store(true, Ordering::Release);
    }
    if expired(started, limit, state) {
        return Ok(());
    }
    {
        let mut g = CALLBACKS
            .write()
            .map_err(|_| crate::tr!(bridge_callback_registration_incomplete))?;
        *g = Some(CallbackTarget {
            handle: session.device.handle(),
            session: audio.clone(),
        });
    }
    if !session.device.set_output_callback(on_output) {
        return Err(crate::tr!(bridge_libviiper_does_not_support_output_callbacks_use).into());
    }
    let mut audio_subscribed = cfg.audio_enabled || cfg.haptics_enabled || state.monitor.wants(0);
    if audio_subscribed {
        if !session.device.set_audio_callback(on_audio)
            || !session.device.set_haptics_callback(on_haptics)
            || !session.device.set_reset_callback(on_reset)
        {
            return Err(crate::tr!(bridge_libviiper_does_not_support_audio_haptics_or).into());
        }
    }
    session.output = Some(
        audio_thread::spawn(
            audio.clone(),
            physical.clone(),
            session.output_running.clone(),
            cfg.clone(),
        )
        .map_err(|e| {
            crate::tr!(
                bridge_could_not_start_output_thread,
                error = crate::i18n_arg!(e)
            )
        })?,
    );
    session.ude_port = Some(session._server.attach_prepared(
        &session.usbip_exe,
        cfg.port,
        || expired(started, limit, state),
    )?);
    let attached_at = Instant::now();
    progress.connected_for = Some(Duration::ZERO);
    let mut stable = false;
    state.connected.store(true, Ordering::Release);
    state.monitor.reset();
    state.monitor.connected.store(true, Ordering::Release);
    state.log_message(
        crate::i18n::MessageId::bridge_bridge_connected_input_output_and_audio_are,
        crate::tr!(bridge_bridge_connected_input_output_and_audio_are),
    );
    let mut last_valid = Instant::now();
    let mut last_stats = Instant::now();
    let mut last_hidhide_check = Instant::now();
    let mut old_blocks = 0;
    let mut old_outputs = 0;
    let mut old_callbacks = 0;
    let mut diagnostic = Instant::now();
    let (mut full, mut simple) = (0u64, 0u64);
    let mut mic_buffer_ms = cfg.microphone_buffer_ms as u32;
    state
        .monitor
        .microphone_buffer_ms
        .store(mic_buffer_ms, Ordering::Relaxed);
    if cfg.microphone_enabled && !session.device.microphone_buffer(mic_buffer_ms) {
        return Err(crate::tr!(bridge_could_not_configure_microphone_buffering_use_the).into());
    }
    state.monitor.mic_buffered.store(0, Ordering::Relaxed);
    state.monitor.mic_underruns.store(0, Ordering::Relaxed);
    state.monitor.mic_dropped.store(0, Ordering::Relaxed);
    let mut mic_stats_at = Instant::now();
    let mut gyro = [0i16; 3];
    let mut microphone = if cfg.microphone_enabled {
        let mut mic = crate::microphone::Microphone::new()
            .ok_or(crate::tr!(bridge_could_not_create_microphone_decoder))?;
        mic.set_test_capture(state.monitor.mic_test_capture.clone());
        Some(mic)
    } else {
        None
    };
    while !expired(started, limit, state) {
        let subscribe = cfg.audio_enabled || cfg.haptics_enabled || state.monitor.wants(0);
        if subscribe != audio_subscribed {
            if !session
                .device
                .monitor_audio_callbacks(subscribe, on_audio, on_haptics, on_reset)
            {
                return Err(crate::tr!(bridge_could_not_update_audio_monitor_subscription).into());
            }
            audio_subscribed = subscribe;
        }
        if audio.stats.failed.load(Ordering::Acquire) {
            return Err(
                crate::tr!(bridge_repeated_writes_to_the_physical_controller_failed).into(),
            );
        }
        if last_valid.elapsed() >= Duration::from_secs(3) {
            return Err(crate::tr!(bridge_no_valid_controller_input_for_3_seconds).into());
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
                    let wanted = state.monitor.microphone_buffer_ms.load(Ordering::Relaxed);
                    if wanted != mic_buffer_ms && session.device.microphone_buffer(wanted) {
                        mic_buffer_ms = wanted;
                        state.log_message(
                            crate::i18n::MessageId::bridge_microphone_safety_buffer_changed_to_ms,
                            crate::tr!(
                                bridge_microphone_safety_buffer_changed_to_ms,
                                milliseconds = crate::i18n_arg!(wanted)
                            ),
                        );
                    }
                    if mic_stats_at.elapsed() >= Duration::from_millis(250) {
                        if let Some(s) = session.device.microphone_stats() {
                            state.monitor.mic_buffered.store(s[0], Ordering::Relaxed);
                            state.monitor.mic_underruns.store(s[1], Ordering::Relaxed);
                            state.monitor.mic_dropped.store(s[2], Ordering::Relaxed);
                        }
                        mic_stats_at = Instant::now();
                    }
                    state.monitor.mic_tagged.fetch_add(1, Ordering::Relaxed);
                    state
                        .monitor
                        .mic_prefix
                        .store(report.data[2] as u32, Ordering::Relaxed);
                    if let (Some(mic), Some(packet)) =
                        (microphone.as_mut(), report.microphone_frame())
                    {
                        mic.set_gain_db(
                            state.monitor.microphone_gain_db.load(Ordering::Relaxed) as u8
                        );
                        state.monitor.mic_packets.fetch_add(1, Ordering::Relaxed);
                        if !mic.decode_observed(
                            packet,
                            state.monitor.wants(4),
                            |pcm, forwarded| {
                                if !forwarded {
                                    state
                                        .monitor
                                        .mic_samples
                                        .fetch_add(pcm.len() as u64, Ordering::Relaxed);
                                }
                                state
                                    .monitor
                                    .pcm(if forwarded { 4 } else { 3 }, 1, 48000, pcm)
                            },
                            |frame| {
                                let ok = session.device.send_microphone(frame);
                                if ok {
                                    state.monitor.mic_usb_frames.fetch_add(1, Ordering::Relaxed);
                                }
                                ok
                            },
                        ) {
                            state.monitor.mic_failed();
                        }
                    }
                    continue;
                }
                if let Some(s) = report.parse() {
                    state.monitor.input(s);
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
                if let Some(mut usb) = report.usb_payload() {
                    guide.apply(&mut usb, attached_at.elapsed());
                    if !session.device.send_raw_report(&usb) {
                        return Err(crate::tr!(bridge_could_not_submit_virtual_device_input).into());
                    }
                    state.frames_out.fetch_add(1, Ordering::Relaxed);
                    last_valid = Instant::now();
                    progress.connected_for = Some(attached_at.elapsed());
                    if !stable {
                        stable = state.confirm_stable_connection(attached_at.elapsed());
                    }
                } else {
                    state.invalid_frames.fetch_add(1, Ordering::Relaxed);
                }
            }
            Ok(None) => {
                if last_valid.elapsed() >= Duration::from_secs(3) {
                    return Err(crate::tr!(bridge_no_valid_controller_input_for_3_seconds).into());
                }
            }
            Err(e) => {
                return Err(crate::tr!(
                    bridge_could_not_read_bluetooth_controller,
                    error = crate::i18n_arg!(e)
                ))
            }
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
                state.log_message(crate::i18n::MessageId::bridge_microphone_audio_flags_header_opus_packets_decoded, crate::tr!(bridge_microphone_audio_flags_header_opus_packets_decoded, flags = crate::i18n_arg!(state.monitor.mic_tagged.load(Ordering::Relaxed)), header = format!("{:02X}", state.monitor.mic_prefix.load(Ordering::Relaxed)), packets = crate::i18n_arg!(state.monitor.mic_packets.load(Ordering::Relaxed)), samples = crate::i18n_arg!(state.monitor.mic_samples.load(Ordering::Relaxed)), frames = crate::i18n_arg!(state.monitor.mic_usb_frames.load(Ordering::Relaxed))));
                let raw = audio
                    .stats
                    .last_output
                    .load(Ordering::Relaxed)
                    .to_le_bytes();
                state.log_message(
                    crate::i18n::MessageId::bridge_diagnostics_full_input_simple_input_gyro_game,
                    crate::tr!(
                        bridge_diagnostics_full_input_simple_input_gyro_game,
                        full = crate::i18n_arg!(full),
                        simple = crate::i18n_arg!(simple),
                        gyro = format!("{:?}", gyro),
                        callbacks = crate::i18n_arg!(callbacks),
                        rumble =
                            crate::i18n_arg!(audio.stats.rumble_updates.load(Ordering::Relaxed)),
                        haptics =
                            crate::i18n_arg!(audio.stats.haptics_nonzero.load(Ordering::Relaxed)),
                        audio = crate::i18n_arg!(audio.stats.pcm_callbacks.load(Ordering::Relaxed)),
                        flags = format!("{:02X}", raw[0]),
                        motor_left = crate::i18n_arg!(raw[3]),
                        motor_right = crate::i18n_arg!(raw[2]),
                        outputs = crate::i18n_arg!(outputs),
                        failures = crate::i18n_arg!(audio.stats.write_fail.load(Ordering::Relaxed))
                    ),
                );
                state.log_message(
                    crate::i18n::MessageId::bridge_audio_batches_max_b_rejected_nonzero_encoding,
                    crate::tr!(
                        bridge_audio_batches_max_b_rejected_nonzero_encoding,
                        bytes = crate::i18n_arg!(audio.stats.pcm_max_bytes.load(Ordering::Relaxed)),
                        rejected =
                            crate::i18n_arg!(audio.stats.pcm_rejected.load(Ordering::Relaxed)),
                        nonzero = crate::i18n_arg!(audio.stats.pcm_nonzero.load(Ordering::Relaxed)),
                        failures =
                            crate::i18n_arg!(audio.stats.encode_fail.load(Ordering::Relaxed))
                    ),
                );
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
    fn brief_failures_back_off_but_a_stable_session_resets_retry_delay() {
        let mut retry = RetryBackoff::default();
        let delays: Vec<_> = (0..10).map(|_| retry.failed(None).as_millis()).collect();
        assert_eq!(
            delays,
            [250, 500, 1000, 2000, 4000, 8000, 16000, 30000, 30000, 30000]
        );
        assert_eq!(retry.failed(Some(Duration::from_secs(4))).as_secs(), 30);
        assert_eq!(retry.failed(Some(STABLE_CONNECTION)).as_millis(), 250);
        assert_eq!(retry.failed(None).as_millis(), 500);
    }
    #[test]
    fn retry_can_be_cancelled_without_waiting_for_the_backoff() {
        let state = BridgeState::default();
        state.stop.store(true, Ordering::Release);
        let now = Instant::now();
        wait_retry(now, None, &state, Duration::from_secs(30));
        assert!(now.elapsed() < Duration::from_millis(200));
        wait_retry(
            now,
            Some(Duration::ZERO),
            &BridgeState::default(),
            Duration::from_secs(30),
        );
        assert!(now.elapsed() < Duration::from_millis(200));
    }
    #[test]
    fn input_probe_requires_real_input_and_preserves_disconnect_errors() {
        let mut reads = 0;
        let input = report::neutral_usb_report().to_vec();
        let result = probe_input(
            || {
                reads += 1;
                Ok(if reads == 1 {
                    None
                } else {
                    Some(input.clone())
                })
            },
            || false,
        )
        .unwrap();
        assert!(result.is_some());
        assert_eq!(reads, 4);
        assert!(
            probe_input(|| Err(std::io::Error::from_raw_os_error(1167)), || false)
                .unwrap_err()
                .contains("1167")
        );
        assert!(
            probe_input(|| panic!("cancelled probe must not read"), || true)
                .unwrap()
                .is_none()
        );
        let mut reads = 0;
        let result = probe_input(
            || {
                reads += 1;
                if reads < 4 {
                    Ok(Some(vec![0; 78]))
                } else {
                    Err(std::io::Error::from_raw_os_error(1167))
                }
            },
            || false,
        );
        assert!(result.is_err());
        assert_eq!(reads, 4);
    }
    #[test]
    fn connection_ps_press_is_released_before_subsequent_guide_chords_are_forwarded() {
        let mut gate = InitialGuide {
            ready: false,
            released_at: None,
        };
        let mut usb = report::neutral_usb_report();
        usb[8] |= report::buttons::CROSS;
        usb[10] = 0b111;
        let other = usb;
        for _ in 0..4 {
            usb = other;
            gate.apply(&mut usb, Duration::from_millis(10));
            assert_eq!(usb[10], 0b110);
            assert_eq!(&usb[..10], &other[..10]);
            assert_eq!(&usb[11..], &other[11..]);
        }
        usb[10] = 0;
        gate.apply(&mut usb, Duration::from_millis(20));
        gate.apply(&mut usb, Duration::from_millis(140));
        usb[10] = 1;
        gate.apply(&mut usb, Duration::from_millis(150));
        assert_eq!(usb[10], 1);
        assert!(gate.ready);
    }
    #[test]
    fn audio_only_and_corrupt_reports_cannot_pass_the_connection_probe() {
        let mut input = vec![0u8; 78];
        input[0] = 0x31;
        let crc = report::bt_input_crc32(&input[..74]);
        input[74..].copy_from_slice(&crc.to_le_bytes());
        let mut audio = input.clone();
        audio[1] = 2;
        let crc = report::bt_input_crc32(&audio[..74]);
        audio[74..].copy_from_slice(&crc.to_le_bytes());
        let mut corrupt = input.clone();
        corrupt[74] ^= 1;
        let mut reports = [
            audio.clone(),
            audio,
            input.clone(),
            corrupt,
            input.clone(),
            input.clone(),
            input,
        ]
        .into_iter();
        assert!(probe_input(
            || Ok(Some(reports.next().expect(
                "probe should finish after three valid input reports"
            ))),
            || false
        )
        .unwrap()
        .is_some());
        assert!(reports.next().is_none());
    }
    #[test]
    fn reconnect_warning_survives_brief_reconnect_without_becoming_a_fatal_error() {
        let state = BridgeState::default();
        state.session_failed(
            "device disconnected (os error 1167)".into(),
            Some(Duration::from_millis(300)),
            false,
        );
        assert!(state.last_error.lock().unwrap().is_none());
        assert!(state
            .connection_issue
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .detail()
            .contains("1167"));
        state.connected.store(true, Ordering::Release);
        assert!(!state.confirm_stable_connection(Duration::from_millis(4999)));
        assert!(state.connection_issue.lock().unwrap().is_some());
        assert!(state.confirm_stable_connection(STABLE_CONNECTION));
        assert!(state.connection_issue.lock().unwrap().is_none());
        state.session_failed("feature read failed".into(), None, true);
        assert!(state
            .connection_issue
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .detail()
            .contains("170"));
        assert!(state
            .connection_issue
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .connected_for
            .is_none());
    }
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
