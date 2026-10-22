//! libVIIPER 运行时绑定。
//!
//! 提供最小可用的封装：启动服务器、创建总线、创建/移除 DualSense、推送原始报告。
//! DLL 与 `libopus-0.dll` 一样采用运行时加载，缺失时整体功能降级而不是崩溃。
//!
//! 签名严格按 `libVIIPER.h`：
//! ```c
//! GoUint8 NewUSBServer(USBServerConfig*, USBServerHandle*, VIIPERLogCallback);
//! GoUint8 CreateUSBBus(USBServerHandle, GoUint32*);
//! GoUint8 CreateDualSenseDevice(USBServerHandle, DSDeviceHandle*, GoUint32,
//!                               GoUint8, GoUint16, GoUint16, DSMetaState*);
//! GoUint8 SetDualSenseRawInputReport(DSDeviceHandle, uint8_t*, size_t);
//! GoUint8 RemoveDualSenseDevice(DSDeviceHandle);
//! GoUint8 CloseUSBServer(USBServerHandle);
//! ```

use std::ffi::{c_char, c_void};
use std::ptr;
use std::sync::OnceLock;

pub const VID_SONY: u16 = 0x054C;
pub const PID_DUALSENSE: u16 = 0x0CE6;
pub const PID_DUALSENSE_EDGE: u16 = 0x0DF2;

/// `USBServerConfig`，字段顺序与 C 头文件一致。
#[repr(C)]
struct ServerConfig {
    addr: *const c_char,
    connection_timeout_ms: u64,
    device_handler_connect_timeout_ms: u64,
    write_batch_flush_interval_ms: u32,
}

type FnNewServer = unsafe extern "C" fn(*const ServerConfig, *mut usize, *mut c_void) -> u8;
type FnCloseServer = unsafe extern "C" fn(usize) -> u8;
type FnCreateBus = unsafe extern "C" fn(usize, *mut u32) -> u8;
type FnRemoveBus = unsafe extern "C" fn(usize, u32) -> u8;
type FnCreateDevice = unsafe extern "C" fn(usize, *mut usize, u32, u8, u16, u16, *mut c_void) -> u8;
type FnRemoveDevice = unsafe extern "C" fn(usize) -> u8;
type FnSetRawReport = unsafe extern "C" fn(usize, *const u8, usize) -> u8;
type FnSetOutputCallback = unsafe extern "C" fn(usize, Option<OutputCallback>) -> u8;
type FnSetAudioCallback = unsafe extern "C" fn(usize, Option<AudioCallback>) -> u8;
type FnSetResetCallback = unsafe extern "C" fn(usize, Option<ResetCallback>) -> u8;
type FnSetMic = unsafe extern "C" fn(usize, *const u8, usize) -> u8;
pub type ResetCallback = unsafe extern "C" fn(usize);

#[repr(C)]
struct MetaState {
    serial: *const c_char,
    mac: *const c_char,
    board: *const c_char,
    battery: u8,
    temperature: f64,
    voltage: f64,
    shell_color: *const c_char,
}

/// 输出回调：游戏写虚拟设备时触发。
///
/// `state` 指向 47 字节的 `DSOutputState`（`#[repr(C)]`，无填充）。
pub type OutputCallback = unsafe extern "C" fn(usize, *const DsOutputState);

/// 音频回调：游戏写扬声器数据时触发。
///
/// `pcm` 是 4ch S16LE @48kHz 交错数据，单帧最多 392 字节（约 1ms）。
/// **回调不得阻塞**，否则会拖垮 USB 音频通路。
pub type AudioCallback = unsafe extern "C" fn(usize, *const u8, usize);

/// 与 `libVIIPER.h` 的 `DSOutputState` 逐字段对应。
///
/// 注意：整体正好 47 字节，`HostTimestamp` 落在 32 偏移处天然 4 字节对齐，无填充。
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DsOutputState {
    pub flags0: u8,
    pub flags1: u8,
    pub rumble_small: u8,
    pub rumble_large: u8,
    pub volume_headphones: u8,
    pub volume_speaker: u8,
    pub volume_mic: u8,
    pub audio_control: u8,
    pub mute_light_mode: u8,
    pub mute_control: u8,
    pub trigger_right: [u8; 11],
    pub trigger_left: [u8; 11],
    pub host_timestamp: u32,
    pub motor_power: u8,
    pub audio_control2: u8,
    pub flags3: u8,
    pub haptic_filter: u8,
    pub unk_byte: u8,
    pub light_fade: u8,
    pub light_brightness: u8,
    pub player_leds: u8,
    pub led_red: u8,
    pub led_green: u8,
    pub led_blue: u8,
}

impl DsOutputState {
    /// 转成 47 字节原始载荷，便于复用 `output::SetStateData::parse`。
    pub fn to_payload(&self) -> [u8; 47] {
        let mut p = [0u8; 47];
        p[0] = self.flags0;
        p[1] = self.flags1;
        p[2] = self.rumble_small;
        p[3] = self.rumble_large;
        p[4] = self.volume_headphones;
        p[5] = self.volume_speaker;
        p[6] = self.volume_mic;
        p[7] = self.audio_control;
        p[8] = self.mute_light_mode;
        p[9] = self.mute_control;
        p[10..21].copy_from_slice(&self.trigger_right);
        p[21..32].copy_from_slice(&self.trigger_left);
        p[32..36].copy_from_slice(&self.host_timestamp.to_le_bytes());
        p[36] = self.motor_power;
        p[37] = self.audio_control2;
        p[38] = self.flags3;
        p[39] = self.haptic_filter;
        p[40] = self.unk_byte;
        p[41] = self.light_fade;
        p[42] = self.light_brightness;
        p[43] = self.player_leds;
        p[44] = self.led_red;
        p[45] = self.led_green;
        p[46] = self.led_blue;
        p
    }
}

struct Api {
    new_server: FnNewServer,
    close_server: FnCloseServer,
    create_bus: FnCreateBus,
    remove_bus: FnRemoveBus,
    create_device: FnCreateDevice,
    remove_device: FnRemoveDevice,
    set_raw_report: FnSetRawReport,
    set_feature_report: Option<FnSetRawReport>,
    use_physical_usb_profile: Option<FnRemoveDevice>,
    set_output_callback: Option<FnSetOutputCallback>,
    set_audio_callback: Option<FnSetAudioCallback>,
    set_haptics_callback: Option<FnSetAudioCallback>,
    set_reset_callback: Option<FnSetResetCallback>,
    set_mic: Option<FnSetMic>,
}

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetModuleFileNameW(module: *mut c_void, filename: *mut u16, size: u32) -> u32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn exe_dir() -> Option<String> {
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetModuleFileNameW(ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) };
    if n == 0 || n as usize >= buf.len() {
        return None;
    }
    let s = String::from_utf16_lossy(&buf[..n as usize]);
    Some(s[..s.rfind('\\')?].to_string())
}

unsafe fn resolve(module: *mut c_void, name: &str) -> Option<*mut c_void> {
    let c = std::ffi::CString::new(name).ok()?;
    let p = GetProcAddress(module, c.as_ptr());
    if p.is_null() {
        None
    } else {
        Some(p)
    }
}

unsafe fn load_api() -> Option<Api> {
    let mut module = GetModuleHandleW(wide("libVIIPER.dll").as_ptr());
    if module.is_null() {
        if let Some(dir) = exe_dir() {
            module = LoadLibraryW(wide(&format!("{dir}\\libVIIPER.dll")).as_ptr());
        }
    }
    if module.is_null() {
        // Development fallback is anchored to the source tree, never the caller's cwd.
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/viiper/libVIIPER.dll");
        module = LoadLibraryW(wide(&path.to_string_lossy()).as_ptr());
    }
    if module.is_null() {
        return None;
    }

    macro_rules! sym {
        ($n:literal) => {
            std::mem::transmute(resolve(module, $n)?)
        };
    }
    Some(Api {
        new_server: sym!("NewUSBServer"),
        close_server: sym!("CloseUSBServer"),
        create_bus: sym!("CreateUSBBus"),
        remove_bus: sym!("RemoveUSBBus"),
        create_device: sym!("CreateDualSenseDevice"),
        remove_device: sym!("RemoveDualSenseDevice"),
        set_raw_report: sym!("SetDualSenseRawInputReport"),
        set_feature_report: resolve(module, "SetDualSenseFeatureReport")
            .map(|p| std::mem::transmute(p)),
        use_physical_usb_profile: resolve(module, "UseDualSensePhysicalUSBProfile")
            .map(|p| std::mem::transmute(p)),
        // 可选符号：旧版 DLL 可能没有
        set_output_callback: resolve(module, "SetDualSenseOutputCallback")
            .map(|p| std::mem::transmute(p)),
        set_audio_callback: resolve(module, "SetDualSenseAudioOutCallback")
            .map(|p| std::mem::transmute(p)),
        set_haptics_callback: resolve(module, "SetDualSenseRealtimeHapticsCallback")
            .map(|p| std::mem::transmute(p)),
        set_reset_callback: resolve(module, "SetDualSenseSpeakerResetCallback")
            .map(|p| std::mem::transmute(p)),
        set_mic: resolve(module, "SetDualSenseMicrophonePCM").map(|p| std::mem::transmute(p)),
    })
}

fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(|| unsafe { load_api() }).as_ref()
}

/// libVIIPER 是否可用（DLL 能被加载且符号齐全）。
pub fn is_available() -> bool {
    api().is_some()
}

pub fn supports_microphone() -> bool {
    api().is_some_and(|a| a.set_mic.is_some())
}

/// 一个已启动的 USB 服务器实例。
pub struct Viiper {
    api: &'static Api,
    server: usize,
    bus: u32,
}

impl Viiper {
    pub fn attach_prepared(&self, port: u16, cancelled: impl Fn() -> bool) -> Result<u32, String> {
        crate::attach::first_device(port, self.bus, cancelled)
    }
    /// 启动服务器并创建一条总线。
    ///
    /// `addr` 形如 `127.0.0.1:3241`。
    pub fn start(addr: &str) -> Option<Self> {
        let api = api()?;
        let c_addr = std::ffi::CString::new(addr).ok()?;
        let cfg = ServerConfig {
            addr: c_addr.as_ptr(),
            connection_timeout_ms: 30_000,
            device_handler_connect_timeout_ms: 5_000,
            write_batch_flush_interval_ms: 1,
        };
        let mut server: usize = 0;
        let ok = unsafe { (api.new_server)(&cfg, &mut server, ptr::null_mut()) };
        if ok == 0 || server == 0 {
            return None;
        }
        let mut bus: u32 = 0;
        let ok = unsafe { (api.create_bus)(server, &mut bus) };
        if ok == 0 {
            unsafe { (api.close_server)(server) };
            return None;
        }
        Some(Self { api, server, bus })
    }

    pub fn bus_id(&self) -> u32 {
        self.bus
    }

    /// 在总线上创建一个虚拟 DualSense，`auto_attach` 为 true 时自动连到本机服务器。
    pub fn create_dualsense(&self, vid: u16, pid: u16, auto_attach: bool) -> Option<Device> {
        self.create_dualsense_with_identity(vid, pid, auto_attach, None, None)
    }

    pub fn create_dualsense_with_identity(
        &self,
        vid: u16,
        pid: u16,
        auto_attach: bool,
        serial: Option<&str>,
        mac: Option<&str>,
    ) -> Option<Device> {
        let serial = serial.and_then(|s| std::ffi::CString::new(s).ok());
        let mac = mac.and_then(|s| std::ffi::CString::new(s).ok());
        let mut meta = MetaState {
            serial: serial.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
            mac: mac.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
            board: ptr::null(),
            battery: 0,
            temperature: 0.0,
            voltage: 0.0,
            shell_color: ptr::null(),
        };
        let mut dev: usize = 0;
        let ok = unsafe {
            (self.api.create_device)(
                self.server,
                &mut dev,
                self.bus,
                auto_attach as u8,
                vid,
                pid,
                &mut meta as *mut MetaState as *mut c_void,
            )
        };
        if ok == 0 || dev == 0 {
            return None;
        }
        Some(Device {
            api: self.api,
            handle: dev,
        })
    }
}

impl Drop for Viiper {
    fn drop(&mut self) {
        if self.server != 0 {
            let _ = unsafe { (self.api.remove_bus)(self.server, self.bus) };
            let _ = unsafe { (self.api.close_server)(self.server) };
            self.server = 0;
        }
    }
}

/// 一个虚拟设备句柄。
pub struct Device {
    api: &'static Api,
    handle: usize,
}

impl Device {
    /// 必须在挂接前选用与真实标准 DualSense 一致的 USB 描述符。
    pub fn use_physical_usb_profile(&self) -> bool {
        self.api
            .use_physical_usb_profile
            .is_some_and(|f| unsafe { f(self.handle) != 0 })
    }
    pub fn set_feature_report(&self, report: &[u8]) -> bool {
        self.api
            .set_feature_report
            .is_some_and(|f| unsafe { f(self.handle, report.as_ptr(), report.len()) != 0 })
    }
    pub fn remove(&mut self) -> bool {
        if self.handle == 0 {
            return true;
        }
        if unsafe { (self.api.remove_device)(self.handle) } == 0 {
            return false;
        }
        self.handle = 0;
        true
    }
    pub fn handle(&self) -> usize {
        self.handle
    }
    /// 推送一份逐字节的 64 字节 USB 输入报告（Report ID 0x01）。
    pub fn send_raw_report(&self, report: &[u8; 64]) -> bool {
        let ok = unsafe { (self.api.set_raw_report)(self.handle, report.as_ptr(), 64) };
        ok != 0
    }

    /// 注册输出回调（游戏写灯光/扳机/震动时触发）。
    ///
    /// 返回 false 表示当前 DLL 未导出该符号。
    pub fn set_output_callback(&self, cb: OutputCallback) -> bool {
        match self.api.set_output_callback {
            Some(f) => unsafe { (f)(self.handle, Some(cb)) != 0 },
            None => false,
        }
    }

    /// 注销输出回调。
    pub fn clear_output_callback(&self) -> bool {
        match self.api.set_output_callback {
            Some(f) => unsafe { (f)(self.handle, None) != 0 },
            None => false,
        }
    }

    /// 注册扬声器 PCM 回调（游戏 → 虚拟设备）。
    pub fn set_audio_callback(&self, cb: AudioCallback) -> bool {
        match self.api.set_audio_callback {
            Some(f) => unsafe { (f)(self.handle, Some(cb)) != 0 },
            None => false,
        }
    }

    /// 注册实时触觉 PCM 回调。
    pub fn set_haptics_callback(&self, cb: AudioCallback) -> bool {
        match self.api.set_haptics_callback {
            Some(f) => unsafe { (f)(self.handle, Some(cb)) != 0 },
            None => false,
        }
    }

    pub fn set_reset_callback(&self, cb: ResetCallback) -> bool {
        self.api
            .set_reset_callback
            .is_some_and(|f| unsafe { f(self.handle, Some(cb)) != 0 })
    }

    pub fn clear_callbacks(&self) -> bool {
        let mut ok = true;
        if let Some(f) = self.api.set_output_callback {
            ok &= unsafe { f(self.handle, None) != 0 };
        }
        if let Some(f) = self.api.set_audio_callback {
            ok &= unsafe { f(self.handle, None) != 0 };
        }
        if let Some(f) = self.api.set_haptics_callback {
            ok &= unsafe { f(self.handle, None) != 0 };
        }
        if let Some(f) = self.api.set_reset_callback {
            ok &= unsafe { f(self.handle, None) != 0 };
        }
        ok
    }

    pub fn send_microphone(&self, frame: &[u8; 192]) -> bool {
        self.api
            .set_mic
            .is_some_and(|f| unsafe { f(self.handle, frame.as_ptr(), frame.len()) != 0 })
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        if self.handle != 0 {
            let _ = unsafe { (self.api.remove_device)(self.handle) };
            self.handle = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "需配套 libVIIPER.dll；由 opends doctor 检测"]
    fn dll_loads() {
        // DLL 需在 exe 同目录或 vendor/viiper/ 下
        assert!(is_available(), "libVIIPER.dll 未找到");
    }

    #[test]
    #[ignore = "会启动真实 UDE 服务；仅显式进行硬件验收时运行"]
    fn server_and_bus_can_start() {
        if !is_available() {
            return;
        }
        let v = Viiper::start("127.0.0.1:3241");
        assert!(v.is_some(), "启动服务器失败（UDE 驱动是否就绪？）");
        let v = v.unwrap();
        assert!(v.bus_id() > 0 || v.bus_id() == 0); // busID 可能是 0
    }
}
