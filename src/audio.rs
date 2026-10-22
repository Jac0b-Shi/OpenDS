//! 音频处理链：4 声道 PCM → 触觉抽取 / 重采样 → Opus 帧 → 蓝牙报告载荷。
//!
//! 常量与算法依据 USB Audio 规范、Opus 规范与设备实测确定，
//! 并与公开实现交叉验证。
//!
//! 当前阶段只实现确定的部分（重采样、块结构、下混），Opus 编解码通过
//! [`OpusEncoder`] / [`OpusDecoder`] trait 留出接口，尚未绑定具体库。

/// 采样率。
pub const SAMPLE_RATE: u32 = 48_000;
/// USB 音频端点的输出声道数（4ch 等时 OUT）。
pub const USB_CHANNELS: usize = 4;
/// 一个处理块的帧数，512 / 48000 = 10.667 ms。
pub const FRAMES_PER_BLOCK: usize = 512;
/// Opus 帧的样本数，480 / 48000 = 10 ms。
pub const OPUS_FRAMES: usize = 480;
/// Opus 压缩帧的固定字节数（160 kbps CBR）。
pub const OPUS_BYTES: usize = 200;
/// 触觉帧数，48000 / 16 = 3 kHz。
pub const HAPTICS_FRAMES: usize = 32;
/// 触觉载荷字节数（32 帧 × 2 声道 × 1 字节）。
pub const HAPTICS_BYTES: usize = 64;
/// 单个载荷块的总字节数：200 字节 Opus + 64 字节触觉。
pub const BLOCK_BYTES: usize = OPUS_BYTES + HAPTICS_BYTES;

/// 触觉抽取的抽取比：512 / 32 = 16。
pub const HAPTICS_DECIMATION: usize = FRAMES_PER_BLOCK / HAPTICS_FRAMES;

/// 音频通道打开后，手柄不再执行状态块的传统马达字段；将其混入触觉 PCM。
#[derive(Default)]
pub struct RumbleMixer {
    magnitude: [f64; 2],
    phase: [f64; 2],
}
impl RumbleMixer {
    pub fn mix(&mut self, haptics: &mut [u8; HAPTICS_BYTES], left: u8, right: u8) {
        let rate = HAPTICS_FRAMES as f64 / (FRAMES_PER_BLOCK as f64 / SAMPLE_RATE as f64);
        let smooth = 1.0 - (-1.0 / (0.05 * rate)).exp();
        let target = [left as f64 / 255.0, right as f64 / 255.0];
        for frame in haptics.chunks_exact_mut(2) {
            let mut wave = 0.0;
            for (i, hz) in [60.0, 180.0].into_iter().enumerate() {
                self.magnitude[i] += (target[i] - self.magnitude[i]) * smooth;
                self.phase[i] = (self.phase[i] + hz / rate).fract();
                wave += self.magnitude[i] * (std::f64::consts::TAU * self.phase[i]).sin();
            }
            let sample = (wave * 127.0).round() as i16;
            for ch in frame {
                *ch = ((*ch as i8 as i16 + sample).clamp(-128, 127) as i8) as u8;
            }
        }
    }
}

// ---- 蓝牙音频报告 ----

/// 组合报告 ID（state + haptics + speaker 一次发出）。
pub const REPORT_ID_COMBINED: u8 = 0x36;
/// 组合报告总长。
pub const COMBINED_REPORT_SIZE: usize = 398;
/// 纯触觉报告 ID。
pub const REPORT_ID_HAPTICS: u8 = 0x32;
/// 纯触觉报告总长。
pub const HAPTICS_REPORT_SIZE: usize = 142;

/// 音频路由标记。
pub mod route {
    pub const SPEAKER: u8 = 0x93;
    pub const HEADSET: u8 = 0x96;
}

/// 构造 `0x36` 组合报告（398 字节）。
///
/// 布局（报文段标记 + 长度 + 数据）：
/// ```text
/// [0]      = 0x36
/// [1]      = seq << 4
/// [2..11]  = 会话块  0x91 0x07 0xFE 0x40 ×5 counter
/// [11..13] = 状态块头 0x90 0x3F
/// [13..76] = 47 字节状态 + 16 字节零填充（共 63）
/// [76..78] = 触觉块头 0x92 0x40
/// [78..142]= 64 字节触觉 PCM
/// [142]    = 路由标记 0x93/0x96
/// [143]    = 0xC8（200）
/// [144..344]= 200 字节 Opus
/// [344..394]= 零填充
/// [394..398]= CRC32
/// ```
///
/// 触觉与音频必须放在**同一份报告**里：分开发送会让序列与间隔计数器每 tick
/// 前进两次，破坏音频流。
pub fn build_combined_report(
    seq: u8,
    packet_counter: u8,
    state: &[u8; 47],
    opus: &[u8; OPUS_BYTES],
    haptics: &[u8; HAPTICS_BYTES],
    audio_route: u8,
) -> [u8; COMBINED_REPORT_SIZE] {
    let mut r = [0u8; COMBINED_REPORT_SIZE];
    r[0] = REPORT_ID_COMBINED;
    r[1] = (seq & 0x0F) << 4;

    // 会话块（packet 0x11，带长度标志）
    r[2] = 0x91;
    r[3] = 0x07;
    r[4] = 0xFE;
    r[5..10].fill(0x40);
    r[10] = packet_counter;

    // 状态块（packet 0x10，长度 63）
    r[11] = 0x90;
    r[12] = 0x3F;
    r[13..13 + 47].copy_from_slice(state);

    // 触觉块（packet 0x12，带长度标志）
    r[76] = 0x92;
    r[77] = HAPTICS_BYTES as u8;
    r[78..78 + HAPTICS_BYTES].copy_from_slice(haptics);

    // 音频块（路由标记 + 长度 + Opus）
    r[142] = audio_route;
    r[143] = OPUS_BYTES as u8;
    r[144..144 + OPUS_BYTES].copy_from_slice(opus);

    let crc = crate::report::bt_output_crc32(&r[0..COMBINED_REPORT_SIZE - 4]);
    r[COMBINED_REPORT_SIZE - 4..].copy_from_slice(&crc.to_le_bytes());
    r
}

/// 启用 0x11 会话块的麦克风返回通道，更新 CRC。仅在用户显式启用时调用。
pub fn enable_microphone(report: &mut [u8; COMBINED_REPORT_SIZE]) {
    report[4] |= 1;
    let crc = crate::report::bt_output_crc32(&report[..COMBINED_REPORT_SIZE - 4]);
    report[COMBINED_REPORT_SIZE - 4..].copy_from_slice(&crc.to_le_bytes());
}

/// 构造 `0x32` init-prime 报告（142 字节），用于**打开蓝牙音频/触觉通道**。
///
/// 不先发这个，`0x36` 里的音频段不会被播放。布局（DualSenseClient
/// `BuildInitPrime`）：
/// ```text
/// [0]      = 0x32
/// [1]      = 0x10
/// [2..4]   = 0x90 0x3F       状态块头（长度 63）
/// [4..51]  = 47 字节输出状态
/// [51..138]= 零
/// [138..142]= CRC32
/// ```
pub fn build_init_prime(state: &[u8; 47]) -> [u8; HAPTICS_REPORT_SIZE] {
    let mut r = [0u8; HAPTICS_REPORT_SIZE];
    r[0] = REPORT_ID_HAPTICS;
    r[1] = 0x10;
    r[2] = 0x90;
    r[3] = 0x3F;
    r[4..4 + 47].copy_from_slice(state);
    let crc = crate::report::bt_output_crc32(&r[0..HAPTICS_REPORT_SIZE - 4]);
    r[HAPTICS_REPORT_SIZE - 4..].copy_from_slice(&crc.to_le_bytes());
    r
}

/// 构造纯触觉报告 `0x32`（142 字节）。
pub fn build_haptics_report(
    seq: u8,
    packet_counter: u8,
    haptics: &[u8; HAPTICS_BYTES],
) -> [u8; HAPTICS_REPORT_SIZE] {
    let mut r = [0u8; HAPTICS_REPORT_SIZE];
    r[0] = REPORT_ID_HAPTICS;
    r[1] = (seq & 0x0F) << 4;
    r[2] = 0x91;
    r[3] = 0x07;
    r[4] = 0xFE;
    r[5..10].fill(0x40);
    r[10] = packet_counter;
    r[11] = 0x92;
    r[12] = HAPTICS_BYTES as u8;
    r[13..13 + HAPTICS_BYTES].copy_from_slice(haptics);

    let crc = crate::report::bt_output_crc32(&r[0..HAPTICS_REPORT_SIZE - 4]);
    r[HAPTICS_REPORT_SIZE - 4..].copy_from_slice(&crc.to_le_bytes());
    r
}

/// 一个待发送的音频载荷块。
#[derive(Debug, Clone, Copy)]
pub struct AudioBlock {
    /// Opus 压缩帧。
    pub opus: [u8; OPUS_BYTES],
    /// 触觉载荷（32 帧立体声 s8）。
    pub haptics: [u8; HAPTICS_BYTES],
}

impl Default for AudioBlock {
    fn default() -> Self {
        Self {
            opus: [0u8; OPUS_BYTES],
            haptics: [0u8; HAPTICS_BYTES],
        }
    }
}

impl AudioBlock {
    /// 按蓝牙 0x32 报告的载荷顺序拼成 264 字节。
    pub fn to_payload(&self) -> [u8; BLOCK_BYTES] {
        let mut out = [0u8; BLOCK_BYTES];
        out[..OPUS_BYTES].copy_from_slice(&self.opus);
        out[OPUS_BYTES..].copy_from_slice(&self.haptics);
        out
    }
}

/// 线性重采样。
///
/// 等价实现为：`pos = i * src_len / dst_len`，在相邻两个样本间线性插值。
/// 512 → 480 时步长为 `1.0666667`。
pub fn resample_linear(src: &[i16], dst_len: usize) -> Vec<i16> {
    if src.is_empty() || dst_len == 0 {
        return Vec::new();
    }
    let src_len = src.len();
    let step = src_len as f32 / dst_len as f32;
    let mut out = Vec::with_capacity(dst_len);
    for i in 0..dst_len {
        let pos = i as f32 * step;
        let idx = pos as usize;
        if idx + 1 >= src_len {
            out.push(src[src_len - 1]);
            continue;
        }
        let frac = pos - idx as f32;
        let a = src[idx] as f32;
        let b = src[idx + 1] as f32;
        out.push((a * (1.0 - frac) + b * frac) as i16);
    }
    out
}

/// 触觉抽取：每 16 个样本求和，按 `1/4096` 归一化后乘增益，钳制到 i8。
///
/// 实测得到的换算为 `sum * 0.00024414062 * (haptics_gain / 10)`，
/// 其中 `0.00024414062 = 1/4096`。输出为 32 帧立体声（两声道相同）。
///
/// 注意：声道混合的精确方式尚未与真机逐字节比对，这里按单声道输入实现。
pub fn extract_haptics(pcm: &[i16], gain: f32) -> [u8; HAPTICS_BYTES] {
    let mut out = [0u8; HAPTICS_BYTES];
    let n = pcm.len().min(FRAMES_PER_BLOCK);
    // 不足一帧的部分按 0 补齐
    for i in 0..HAPTICS_FRAMES {
        let start = i * HAPTICS_DECIMATION;
        let end = (start + HAPTICS_DECIMATION).min(n);
        let sum: i32 = pcm
            .get(start..end)
            .unwrap_or(&[])
            .iter()
            .map(|&s| s as i32)
            .sum();
        let v = (sum as f32 * (1.0 / 4096.0) * gain).clamp(-128.0, 127.0) as i8;
        out[i * 2] = v as u8;
        out[i * 2 + 1] = v as u8;
    }
    out
}

/// 后两路音圈各自低通抽取；绝不把扬声器的前两路混进触觉。
/// 缺失的帧补静音，左右相位和强度分别保留。
pub fn extract_stereo_haptics(pcm: &[i16], channels: usize, gain: f32) -> [u8; HAPTICS_BYTES] {
    let mut out = [0; HAPTICS_BYTES];
    if !matches!(channels, 2 | 4) {
        return out;
    }
    let first = channels - 2;
    for i in 0..HAPTICS_FRAMES {
        for ch in 0..2 {
            let mut sum = 0i32;
            for f in i * HAPTICS_DECIMATION..(i + 1) * HAPTICS_DECIMATION {
                sum += pcm.get(f * channels + first + ch).copied().unwrap_or(0) as i32;
            }
            out[i * 2 + ch] = (sum as f32 / 4096.0 * gain).clamp(-128.0, 127.0) as i8 as u8;
        }
    }
    out
}

/// 4 声道交错 PCM → 2 声道，并做单声道化。
///
/// **关键**：手柄的 `OutputPathSpeaker (0x30)` 只把**右声道**送到单声道扬声器。
/// 如果按传统方式下混（左=(ch0+ch1)/2、右=(ch2+ch3)/2），而游戏只写前两个声道，
/// 右声道就会是全零——扬声器一片死寂，但触觉照常工作。
///
/// 因此这里把 (ch0+ch1)/2 的结果**同时写入左右两个声道**，无论哪一路被取用都有信号。
pub fn downmix_4_to_2(pcm: &[i16], frames: usize) -> Vec<i16> {
    let mut out = Vec::with_capacity(frames * 2);
    for f in 0..frames {
        let base = f * USB_CHANNELS;
        if base + 1 >= pcm.len() {
            break;
        }
        let l = pcm[base] as i32;
        let r = pcm[base + 1] as i32;
        let m = ((l + r) / 2).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        out.push(m);
        out.push(m);
    }
    out
}

/// Opus 编码器接口。
///
/// 参数固定为 48 kHz / 2ch / 10 ms / 160 kbps CBR / complexity 0，
/// 与实测结果一致。
pub trait OpusEncoder {
    /// 编码一帧 480 个立体声样本，输出固定 200 字节。
    fn encode(&mut self, pcm: &[i16], out: &mut [u8; OPUS_BYTES]) -> usize;
    /// 复位编码器状态（对应 `OPUS_RESET_STATE`）。
    fn reset(&mut self);
}

/// Opus 解码器接口。麦克风方向为单声道 48 kHz。
pub trait OpusDecoder {
    /// 解码一帧 71 字节的 Opus 数据，输出最多 480 个单声道样本。
    fn decode(&mut self, packet: &[u8], out: &mut [i16; OPUS_FRAMES]) -> usize;
    fn reset(&mut self);
}

/// 占位编码器：不做压缩，用静音填充，仅用于打通链路。
pub struct NullEncoder;

impl OpusEncoder for NullEncoder {
    fn encode(&mut self, _pcm: &[i16], out: &mut [u8; OPUS_BYTES]) -> usize {
        out.fill(0);
        OPUS_BYTES
    }
    fn reset(&mut self) {}
}

/// 占位解码器：返回 0 样本。
pub struct NullDecoder;

impl OpusDecoder for NullDecoder {
    fn decode(&mut self, _packet: &[u8], _out: &mut [i16; OPUS_FRAMES]) -> usize {
        0
    }
    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classic_rumble_is_encoded_as_pcm_during_audio_and_stops_with_zero_motors() {
        let mut mixer = RumbleMixer::default();
        let mut last = [0; HAPTICS_BYTES];
        for _ in 0..20 {
            last.fill(0);
            mixer.mix(&mut last, 180, 80);
        }
        assert!(last.iter().any(|&v| v != 0));
        assert!(last.chunks_exact(2).all(|c| c[0] == c[1]));
        for _ in 0..100 {
            last.fill(0);
            mixer.mix(&mut last, 0, 0);
        }
        assert!(last.iter().all(|&v| v == 0));
        let mut untouched = [0; HAPTICS_BYTES];
        for p in untouched.chunks_exact_mut(2) {
            p[0] = 17;
            p[1] = (-23i8) as u8;
        }
        let expected = untouched;
        RumbleMixer::default().mix(&mut untouched, 0, 0);
        assert_eq!(untouched, expected);
    }

    #[test]
    fn constants_match_reverse_engineering() {
        assert_eq!(FRAMES_PER_BLOCK, 512);
        assert_eq!(OPUS_FRAMES, 480);
        assert_eq!(OPUS_BYTES, 200);
        assert_eq!(HAPTICS_FRAMES, 32);
        assert_eq!(HAPTICS_BYTES, 64);
        assert_eq!(BLOCK_BYTES, 264);
        assert_eq!(HAPTICS_DECIMATION, 16);
    }

    #[test]
    fn resample_preserves_length_and_endpoints() {
        let src: Vec<i16> = (0..512).map(|i| (i * 10) as i16).collect();
        let dst = resample_linear(&src, 480);
        assert_eq!(dst.len(), 480);
        assert_eq!(dst[0], src[0]);
        // 单调递增的输入，输出也应单调
        assert!(dst.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn resample_step_is_512_over_480() {
        // 第 15 个输出样本对应源位置 15 * 512/480 = 16.0，应正好取 src[16]
        let src: Vec<i16> = (0..512).map(|i| i as i16).collect();
        let dst = resample_linear(&src, 480);
        assert_eq!(dst[15], src[16]);
    }

    #[test]
    fn haptics_average_and_gain() {
        // 16 个 4096 求和 = 65536，/4096 = 16，×1.0 = 16
        let pcm = vec![4096i16; FRAMES_PER_BLOCK];
        let h = extract_haptics(&pcm, 1.0);
        assert_eq!(h[0] as i8, 16);
        assert_eq!(h[1] as i8, 16);
        // 增益翻倍
        let h2 = extract_haptics(&pcm, 2.0);
        assert_eq!(h2[0] as i8, 32);
    }

    #[test]
    fn haptics_clamps() {
        let pcm = vec![i16::MAX; FRAMES_PER_BLOCK];
        let h = extract_haptics(&pcm, 10.0);
        assert_eq!(h[0] as i8, 127);
    }

    #[test]
    fn native_haptics_preserve_independent_sides_and_ignore_speaker() {
        let block: Vec<i16> = (0..FRAMES_PER_BLOCK)
            .flat_map(|_| [i16::MAX, i16::MAX, 4096, -8192])
            .collect();
        let h = extract_stereo_haptics(&block, 4, 1.0);
        assert!(h
            .chunks_exact(2)
            .all(|c| c[0] as i8 == 16 && c[1] as i8 == -32));
        let speaker_only: Vec<i16> = (0..FRAMES_PER_BLOCK)
            .flat_map(|_| [10000, 10000, 0, 0])
            .collect();
        assert_eq!(
            extract_stereo_haptics(&speaker_only, 4, 1.0),
            [0; HAPTICS_BYTES]
        );
    }

    #[test]
    fn haptics_short_input_is_silence_padded() {
        let h = extract_stereo_haptics(&[4096, -4096], 2, 1.0);
        assert_eq!((h[0] as i8, h[1] as i8), (1, -1));
        assert!(h[2..].iter().all(|&x| x == 0));
        assert_eq!(extract_haptics(&[], 1.0), [0; HAPTICS_BYTES]);
    }

    #[test]
    fn downmix_is_mono_and_uses_front_pair() {
        // 4 声道：L=100, R=200, 后两路=300/400
        // 结果取 (100+200)/2 = 150，且左右相同（扬声器只取右声道）
        let pcm = vec![100i16, 200, 300, 400];
        let out = downmix_4_to_2(&pcm, 1);
        assert_eq!(out, vec![150, 150]);
    }

    #[test]
    fn audio_block_layout() {
        let mut blk = AudioBlock::default();
        blk.opus[0] = 0xAA;
        blk.haptics[0] = 0xBB;
        let p = blk.to_payload();
        assert_eq!(p.len(), 264);
        assert_eq!(p[0], 0xAA);
        assert_eq!(p[200], 0xBB);
    }

    #[test]
    fn null_codec_roundtrip_compiles() {
        let mut enc = NullEncoder;
        let mut buf = [0u8; OPUS_BYTES];
        let n = enc.encode(&[0i16; OPUS_FRAMES], &mut buf);
        assert_eq!(n, OPUS_BYTES);
        let mut dec = NullDecoder;
        let mut pcm = [0i16; OPUS_FRAMES];
        assert_eq!(dec.decode(&buf, &mut pcm), 0);
    }

    #[test]
    fn combined_report_layout() {
        let state = [0x11u8; 47];
        let opus = [0x22u8; OPUS_BYTES];
        let haptics = [0x33u8; HAPTICS_BYTES];
        let r = build_combined_report(0xA, 0x05, &state, &opus, &haptics, route::SPEAKER);

        assert_eq!(r.len(), 398);
        assert_eq!(r[0], 0x36);
        assert_eq!(r[1], 0xA0);
        // 会话块
        assert_eq!(&r[2..5], &[0x91, 0x07, 0xFE]);
        assert!(r[5..10].iter().all(|&b| b == 0x40));
        assert_eq!(r[10], 0x05);
        // 状态块
        assert_eq!(r[11], 0x90);
        assert_eq!(r[12], 0x3F);
        assert_eq!(&r[13..60], &state[..]);
        assert!(r[60..76].iter().all(|&b| b == 0));
        // 触觉块
        assert_eq!(r[76], 0x92);
        assert_eq!(r[77], 64);
        assert_eq!(&r[78..142], &haptics[..]);
        // 音频块
        assert_eq!(r[142], route::SPEAKER);
        assert_eq!(r[143], 200);
        assert_eq!(&r[144..344], &opus[..]);
        // 尾部填充 + CRC
        assert!(r[344..394].iter().all(|&b| b == 0));
        let crc = u32::from_le_bytes([r[394], r[395], r[396], r[397]]);
        assert_eq!(crc, crate::report::bt_output_crc32(&r[0..394]));
    }

    #[test]
    fn haptics_report_layout() {
        let haptics = [0x44u8; HAPTICS_BYTES];
        let r = build_haptics_report(0x3, 0x01, &haptics);
        assert_eq!(r.len(), 142);
        assert_eq!(r[0], 0x32);
        assert_eq!(r[1], 0x30);
        assert_eq!(r[11], 0x92);
        assert_eq!(r[12], 64);
        assert_eq!(&r[13..77], &haptics[..]);
        let crc = u32::from_le_bytes([r[138], r[139], r[140], r[141]]);
        assert_eq!(crc, crate::report::bt_output_crc32(&r[0..138]));
    }

    #[test]
    fn route_tags() {
        assert_eq!(route::SPEAKER, 0x93);
        assert_eq!(route::HEADSET, 0x96);
    }
}
