//! libopus 运行时绑定。
//!
//! 通过 `LoadLibraryW` 动态加载 `libopus-0.dll`，缺失时自动降级到空实现，
//! 不影响其他功能。DLL 放在可执行文件同目录即可（见 `vendor/opus/`）。
//!
//! 编码参数与实测一致：48 kHz / 2ch / 10 ms / 160 kbps CBR / complexity 0。
//! 解码为单声道 48 kHz。

use crate::audio::{OpusDecoder, OpusEncoder, OPUS_BYTES, OPUS_FRAMES, SAMPLE_RATE};
use std::ffi::{c_char, c_int, c_void};
use std::ptr;
use std::sync::OnceLock;

// ---- libopus 常量（opus_defines.h）----
const OPUS_APPLICATION_AUDIO: c_int = 2049;
const OPUS_SET_BITRATE_REQUEST: c_int = 4002;
const OPUS_SET_VBR_REQUEST: c_int = 4006;
const OPUS_SET_COMPLEXITY_REQUEST: c_int = 4010;
const OPUS_SET_EXPERT_FRAME_DURATION_REQUEST: c_int = 4040;
const OPUS_FRAMESIZE_10_MS: c_int = 5003;
const OPUS_BITRATE: c_int = 160_000;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetModuleFileNameW(module: *mut c_void, filename: *mut u16, size: u32) -> u32;
}

type EncoderCreate = unsafe extern "C" fn(c_int, c_int, c_int, *mut c_int) -> *mut c_void;
type EncoderCtl = unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int;
type Encode = unsafe extern "C" fn(*mut c_void, *const i16, c_int, *mut u8, i32) -> i32;
type EncoderDestroy = unsafe extern "C" fn(*mut c_void);

type DecoderCreate = unsafe extern "C" fn(c_int, c_int, *mut c_int) -> *mut c_void;
type Decode = unsafe extern "C" fn(*mut c_void, *const u8, i32, *mut i16, c_int, c_int) -> c_int;
type DecoderDestroy = unsafe extern "C" fn(*mut c_void);

/// 已解析的函数表。
pub struct Api {
    encoder_create: EncoderCreate,
    encoder_ctl: EncoderCtl,
    encode: Encode,
    encoder_destroy: EncoderDestroy,
    decoder_create: DecoderCreate,
    decoder_ctl: EncoderCtl,
    decode: Decode,
    decoder_destroy: DecoderDestroy,
}

unsafe fn resolve(module: *mut c_void, name: &[u8]) -> Option<*mut c_void> {
    let p = GetProcAddress(module, name.as_ptr() as *const c_char);
    if p.is_null() {
        None
    } else {
        Some(p)
    }
}

unsafe fn load_api() -> Option<Api> {
    // 1) 已加载
    let mut module = GetModuleHandleW(wide("libopus-0.dll").as_ptr());

    // 2) exe 同目录
    if module.is_null() {
        if let Some(dir) = exe_dir() {
            module = LoadLibraryW(wide(&format!("{dir}\\libopus-0.dll")).as_ptr());
        }
    }

    // 3) 开发期：使用绝对源码路径，不从调用者的工作目录加载 DLL。
    if module.is_null() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/opus/libopus-0.dll");
        module = LoadLibraryW(wide(&path.to_string_lossy()).as_ptr());
    }

    if module.is_null() {
        return None;
    }

    macro_rules! sym {
        ($name:literal) => {
            resolve(module, concat!($name, "\0").as_bytes())?
        };
    }

    Some(Api {
        encoder_create: std::mem::transmute(sym!("opus_encoder_create")),
        encoder_ctl: std::mem::transmute(sym!("opus_encoder_ctl")),
        encode: std::mem::transmute(sym!("opus_encode")),
        encoder_destroy: std::mem::transmute(sym!("opus_encoder_destroy")),
        decoder_create: std::mem::transmute(sym!("opus_decoder_create")),
        decoder_ctl: std::mem::transmute(sym!("opus_decoder_ctl")),
        decode: std::mem::transmute(sym!("opus_decode")),
        decoder_destroy: std::mem::transmute(sym!("opus_decoder_destroy")),
    })
}

/// 当前可执行文件所在目录。
fn exe_dir() -> Option<String> {
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetModuleFileNameW(ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) };
    if n == 0 || n as usize >= buf.len() {
        return None;
    }
    let s = String::from_utf16_lossy(&buf[..n as usize]);
    let idx = s.rfind('\\')?;
    Some(s[..idx].to_string())
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 获取全局 API 句柄；DLL 不可用时返回 None。
pub fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(|| unsafe { load_api() }).as_ref()
}

/// 是否成功加载 libopus。
pub fn is_available() -> bool {
    api().is_some()
}

/// 基于 libopus 的编码器。
pub struct LibOpusEncoder {
    api: &'static Api,
    st: *mut c_void,
}

// 编码器句柄不跨线程共享，但移动是安全的。
unsafe impl Send for LibOpusEncoder {}

impl LibOpusEncoder {
    /// 创建编码器；失败返回 None。
    pub fn new() -> Option<Self> {
        Self::with_complexity(0)
    }

    pub fn with_complexity(complexity: u8) -> Option<Self> {
        let api = api()?;
        let mut err: c_int = 0;
        let st = unsafe {
            (api.encoder_create)(SAMPLE_RATE as c_int, 2, OPUS_APPLICATION_AUDIO, &mut err)
        };
        if st.is_null() || err != 0 {
            return None;
        }
        let me = Self { api, st };
        // 四个控制项
        let configured = unsafe {
            (api.encoder_ctl)(
                me.st,
                OPUS_SET_EXPERT_FRAME_DURATION_REQUEST,
                OPUS_FRAMESIZE_10_MS,
            ) == 0
                && (api.encoder_ctl)(me.st, OPUS_SET_BITRATE_REQUEST, OPUS_BITRATE) == 0
                && (api.encoder_ctl)(me.st, OPUS_SET_VBR_REQUEST, 0) == 0
                && (api.encoder_ctl)(
                    me.st,
                    OPUS_SET_COMPLEXITY_REQUEST,
                    complexity.min(10) as c_int,
                ) == 0
        };
        if !configured {
            return None;
        }
        Some(me)
    }
}

impl OpusEncoder for LibOpusEncoder {
    fn encode(&mut self, pcm: &[i16], out: &mut [u8; OPUS_BYTES]) -> usize {
        let need = OPUS_FRAMES * 2;
        if pcm.len() < need {
            return 0;
        }
        let n = unsafe {
            (self.api.encode)(
                self.st,
                pcm.as_ptr(),
                OPUS_FRAMES as c_int,
                out.as_mut_ptr(),
                OPUS_BYTES as i32,
            )
        };
        if n <= 0 {
            0
        } else {
            n as usize
        }
    }

    fn reset(&mut self) {
        // OPUS_RESET_STATE = 4028，无需参数
        unsafe {
            (self.api.encoder_ctl)(self.st, 4028, 0);
        }
    }
}

impl Drop for LibOpusEncoder {
    fn drop(&mut self) {
        if !self.st.is_null() {
            unsafe { (self.api.encoder_destroy)(self.st) };
            self.st = ptr::null_mut();
        }
    }
}

/// 基于 libopus 的解码器（麦克风单声道）。
pub struct LibOpusDecoder {
    api: &'static Api,
    st: *mut c_void,
}

unsafe impl Send for LibOpusDecoder {}

impl LibOpusDecoder {
    pub fn new() -> Option<Self> {
        let api = api()?;
        let mut err: c_int = 0;
        let st = unsafe { (api.decoder_create)(SAMPLE_RATE as c_int, 1, &mut err) };
        if st.is_null() || err != 0 {
            return None;
        }
        Some(Self { api, st })
    }
}

impl OpusDecoder for LibOpusDecoder {
    fn decode(&mut self, packet: &[u8], out: &mut [i16; OPUS_FRAMES]) -> usize {
        let n = unsafe {
            (self.api.decode)(
                self.st,
                packet.as_ptr(),
                packet.len() as i32,
                out.as_mut_ptr(),
                OPUS_FRAMES as c_int,
                0,
            )
        };
        if n <= 0 {
            0
        } else {
            n as usize
        }
    }

    fn reset(&mut self) {
        // OPUS_RESET_STATE = 4028，必须用 decoder_ctl，不能混用 encoder_ctl
        unsafe {
            (self.api.decoder_ctl)(self.st, 4028, 0);
        }
    }
}

impl Drop for LibOpusDecoder {
    fn drop(&mut self) {
        if !self.st.is_null() {
            unsafe { (self.api.decoder_destroy)(self.st) };
            self.st = ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_libopus() {
        // DLL 需放在可执行文件同目录（target/debug 下）
        assert!(is_available(), "libopus-0.dll 未找到，请放到 exe 同目录");
    }

    #[test]
    fn encode_silence_produces_frame() {
        if !is_available() {
            return;
        }
        let mut enc = LibOpusEncoder::new().expect("create encoder");
        let pcm = vec![0i16; OPUS_FRAMES * 2];
        let mut out = [0u8; OPUS_BYTES];
        let n = enc.encode(&pcm, &mut out);
        assert!(n > 0 && n <= OPUS_BYTES, "encode returned {n}");
    }

    #[test]
    fn encode_decode_roundtrip_sine() {
        if !is_available() {
            return;
        }
        let mut enc = LibOpusEncoder::new().expect("encoder");
        // 1 kHz 正弦，左右同相
        let mut pcm = vec![0i16; OPUS_FRAMES * 2];
        for i in 0..OPUS_FRAMES {
            let v =
                ((i as f32 * 2.0 * std::f32::consts::PI * 1000.0 / 48_000.0).sin() * 8000.0) as i16;
            pcm[i * 2] = v;
            pcm[i * 2 + 1] = v;
        }
        let mut frame = [0u8; OPUS_BYTES];
        let n = enc.encode(&pcm, &mut frame);
        assert!(n > 0, "encode failed");

        // 解码器是单声道，用帧长度估算能解出样本
        let mut dec = LibOpusDecoder::new().expect("decoder");
        let mut out = [0i16; OPUS_FRAMES];
        let got = dec.decode(&frame[..n], &mut out);
        assert!(got > 0, "decode failed");
    }

    #[test]
    fn reset_does_not_panic() {
        if !is_available() {
            return;
        }
        let mut enc = LibOpusEncoder::new().expect("encoder");
        enc.reset();
        let mut dec = LibOpusDecoder::new().expect("decoder");
        dec.reset();
    }
}
