//! DualSense 输入报告的识别与转换。
//!
//! 实测结论：
//! - USB 模式：64 字节，报告 ID 0x01，四轴在 byte 1..5。
//! - 蓝牙模式：0x01 兼容报告或 78 字节 0x31 完整报告。
//!   - 0x31 时：偏移 2 取 63 字节载荷，前置 USB 报告 ID 0x01。
//!   - 0x01 时：摇杆在 byte 1..5，是 DS4 兼容布局。
//!
//! 本模块只做与硬件相关的最小转换，不涉及虚拟设备。

/// 设备连接模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// USB 有线连接，64 字节报告。
    Usb,
    /// 蓝牙连接，78 字节报告。
    Bluetooth,
}

impl Mode {
    /// 由 HID 输入报告长度推断连接模式。
    pub fn from_input_len(len: usize) -> Option<Self> {
        match len {
            // USB 复合设备的 HID 接口固定 64 字节。
            64 => Some(Mode::Usb),
            // 蓝牙报告含时间戳与 CRC，固定 78 字节。
            78 => Some(Mode::Bluetooth),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Usb => "USB",
            Mode::Bluetooth => "Bluetooth",
        }
    }
}

/// 一条原始输入报告。
#[derive(Debug, Clone)]
pub struct InputReport {
    pub mode: Mode,
    pub report_id: u8,
    pub data: Vec<u8>,
}

/// 一个触摸点。
///
/// 编码方式（4 字节）：
/// - `[0]` 高 1 位 = 未激活，低 7 位 = 触点 ID
/// - `[1]` X 低 8 位
/// - `[2]` 高 4 位 = Y 低 4 位，低 4 位 = X 高 4 位
/// - `[3]` Y 高 8 位
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TouchPoint {
    pub tracking_id: u8,
    pub active: bool,
    pub x: u16,
    pub y: u16,
}

impl TouchPoint {
    pub fn from_bytes(b: &[u8]) -> Self {
        let status = b[0];
        Self {
            tracking_id: status & 0x7F,
            active: (status & 0x80) == 0,
            x: (((b[2] & 0x0F) as u16) << 8) | b[1] as u16,
            y: ((b[3] as u16) << 4) | (b[2] as u16 >> 4),
        }
    }
}

/// IMU 数据。
#[derive(Debug, Clone, Copy, Default)]
pub struct Motion {
    pub gyro: [i16; 3],
    pub accel: [i16; 3],
    pub timestamp: u32,
    /// IMU 温度（摄氏度）。
    pub temperature: i8,
}

/// 解析后的完整输入状态。
///
/// 字段偏移以「负载起点」计：USB 报告的负载从 offset 1 开始，
/// 蓝牙 0x31 从 offset 2 开始（多一个序号字节）。
#[derive(Debug, Clone, Copy, Default)]
pub struct ParsedState {
    pub lx: u8,
    pub ly: u8,
    pub rx: u8,
    pub ry: u8,
    /// 扳机模拟量，0..255。
    pub l2: u8,
    pub r2: u8,
    /// 计数器/时间戳，每帧递增。
    pub seq: u8,
    /// 面键 + 方向键（低 4 位）。
    pub buttons0: u8,
    /// 肩键 + 系统键。
    pub buttons1: u8,
    /// PS / 触摸板 / 静音 / Edge 扩展键。
    pub buttons2: u8,
    /// 两个触摸点。
    pub touch1: TouchPoint,
    pub touch2: TouchPoint,
    /// IMU。
    pub motion: Motion,
    /// 设备时间戳（uint32 LE）。
    pub device_timestamp: u32,
    /// 电量 0..10。
    pub battery_level: u8,
    /// 电源状态：0 放电 / 1 充电 / 2 满电 等。
    pub battery_status: u8,
    /// 连接状态原始字节（耳机/麦克风等）。
    pub connection: u8,
}

/// 按键位定义。来源：DualSenseClient `InputState.cs`，与实测帧一致。
///
/// `buttons0` 的低 4 位是方向键，`0x8` 表示中立。
pub mod buttons {
    // buttons0
    pub const SQUARE: u8 = 0x10;
    pub const CROSS: u8 = 0x20;
    pub const CIRCLE: u8 = 0x40;
    pub const TRIANGLE: u8 = 0x80;
    /// 方向键掩码，低 4 位。
    pub const DPAD_MASK: u8 = 0x0F;
    /// 方向键中立值。
    pub const DPAD_NEUTRAL: u8 = 0x08;

    // buttons1
    pub const L1: u8 = 0x01;
    pub const R1: u8 = 0x02;
    pub const L2_CLICK: u8 = 0x04;
    pub const R2_CLICK: u8 = 0x08;
    pub const CREATE: u8 = 0x10;
    pub const OPTIONS: u8 = 0x20;
    pub const L3: u8 = 0x40;
    pub const R3: u8 = 0x80;

    // buttons2
    pub const PS: u8 = 0x01;
    pub const TOUCHPAD: u8 = 0x02;
    pub const MUTE: u8 = 0x04;
}

impl InputReport {
    pub fn new(data: Vec<u8>) -> Option<Self> {
        let mode = Mode::from_input_len(data.len())?;
        let report_id = *data.first()?;
        Some(Self {
            mode,
            report_id,
            data,
        })
    }

    /// 是否为蓝牙完整报告（0x31）。
    pub fn is_full_bluetooth(&self) -> bool {
        self.mode == Mode::Bluetooth && self.report_id == 0x31
    }

    /// 蓝牙完整报告里，byte 1 的 bit1 表示本帧是否携带音频载荷。
    pub fn has_audio_payload(&self) -> bool {
        self.is_full_bluetooth() && (self.data[1] & 0x02) != 0
    }

    /// 取出 64 字节 USB 报告负载。
    ///
    /// - USB 模式：本身就是 64 字节，直接返回。
    /// - 蓝牙 0x31：跳过 2 字节头，取 64 字节（实测结论）。
    /// - 蓝牙 0x01：将兼容布局的基本按键重排，缺失的触摸点标为未激活。
    pub fn usb_payload(&self) -> Option<[u8; 64]> {
        if self.has_audio_payload() {
            return None;
        }
        match (self.mode, self.report_id) {
            (Mode::Usb, 0x01) if self.data.len() >= 64 => {
                let mut out = [0u8; 64];
                out.copy_from_slice(&self.data[..64]);
                Some(out)
            }
            (Mode::Bluetooth, 0x31) if self.data.len() >= 66 => {
                let mut out = [0u8; 64];
                out[0] = 0x01; // USB 报告 ID
                out[1..].copy_from_slice(&self.data[2..65]);
                // 这是虚拟 USB 连接；不能把物理蓝牙的“USB 未连接”状态带给游戏。
                out[54] |= 0x18;
                Some(out)
            }
            (Mode::Bluetooth, 0x01) if self.data.len() >= 10 => {
                let mut out = neutral_usb_report();
                out[1..5].copy_from_slice(&self.data[1..5]);
                out[5] = self.data[8];
                out[6] = self.data[9];
                out[7] = self.data[7] >> 2;
                out[8] = self.data[5];
                out[9] = self.data[6];
                out[10] = self.data[7] & 0x03;
                Some(out)
            }
            _ => None,
        }
    }

    /// 按各自布局解析出完整状态。
    ///
    /// 负载布局（与 DualSenseClient `InputReport` 一致，已用实测帧验证）：
    ///
    /// | 负载偏移 | 字段 |
    /// | --- | --- |
    /// | 0-3 | LX / LY / RX / RY |
    /// | 4 | L2 |
    /// | 5 | R2 |
    /// | 6 | 序号 |
    /// | 7 | buttons0（面键 + 方向键） |
    /// | 8 | buttons1（肩键 + 系统键） |
    /// | 9 | buttons2（PS / 触摸板 / 静音 / Edge） |
    /// | 15-20 | 陀螺仪 XYZ（int16 LE） |
    /// | 21-26 | 加速度计 XYZ（int16 LE） |
    /// | 27-30 | IMU 时间戳 |
    /// | 31 | IMU 温度 |
    /// | 32-35 | 触摸点 1 |
    /// | 36-39 | 触摸点 2 |
    /// | 48-51 | 设备时间戳 |
    /// | 52 | 电量（低 4 位）+ 状态（高 4 位） |
    /// | 53 | 连接状态 |
    ///
    /// USB 报告的负载从 offset 1 开始，蓝牙 0x31 从 offset 2 开始。
    pub fn parse(&self) -> Option<ParsedState> {
        if self.has_audio_payload() {
            return None;
        }
        let base = match (self.mode, self.report_id) {
            (Mode::Usb, 0x01) => 1,
            (Mode::Bluetooth, 0x31) => 2,
            // 蓝牙 0x01（DS4 兼容模式）布局不同，字段不全，单独处理。
            (Mode::Bluetooth, 0x01) if self.data.len() >= 10 => {
                return Some(ParsedState {
                    lx: self.data[1],
                    ly: self.data[2],
                    rx: self.data[3],
                    ry: self.data[4],
                    l2: self.data[8],
                    r2: self.data[9],
                    seq: self.data[7] >> 2,
                    buttons0: self.data[5],
                    buttons1: self.data[6],
                    buttons2: self.data[7] & 0x03,
                    ..Default::default()
                });
            }
            _ => return None,
        };
        let d = &self.data;
        // 至少需要读到 offset 53（连接状态）
        if d.len() < base + 54 {
            return None;
        }
        let at = |i: usize| d[base + i];
        let u16_at = |i: usize| u16::from_le_bytes([d[base + i], d[base + i + 1]]);
        let i16_at = |i: usize| u16_at(i) as i16;
        let u32_at = |i: usize| {
            u32::from_le_bytes([
                d[base + i],
                d[base + i + 1],
                d[base + i + 2],
                d[base + i + 3],
            ])
        };
        Some(ParsedState {
            lx: at(0),
            ly: at(1),
            rx: at(2),
            ry: at(3),
            l2: at(4),
            r2: at(5),
            seq: at(6),
            buttons0: at(7),
            buttons1: at(8),
            buttons2: at(9),
            motion: Motion {
                gyro: [i16_at(15), i16_at(17), i16_at(19)],
                accel: [i16_at(21), i16_at(23), i16_at(25)],
                timestamp: u32_at(27),
                temperature: at(31) as i8,
            },
            touch1: TouchPoint::from_bytes(&d[base + 32..base + 36]),
            touch2: TouchPoint::from_bytes(&d[base + 36..base + 40]),
            device_timestamp: u32_at(48),
            battery_level: at(52) & 0x0F,
            battery_status: at(52) >> 4,
            connection: at(53),
        })
    }

    /// 蓝牙 0x31 报告里携带的麦克风 Opus 帧（71 字节）。
    pub fn microphone_frame(&self) -> Option<&[u8]> {
        if self.has_audio_payload() && self.data.get(2) == Some(&0xD4) && self.data.len() >= 3 + 71
        {
            Some(&self.data[3..3 + 71])
        } else {
            None
        }
    }
}

/// 断线/停止时释放全部按键，摇杆居中，两个触摸点均未激活。
pub fn neutral_usb_report() -> [u8; 64] {
    let mut out = [0u8; 64];
    out[0] = 0x01;
    out[1..5].fill(128);
    out[8] = buttons::DPAD_NEUTRAL;
    out[33] = 0x80;
    out[37] = 0x80;
    out[54] = 0x18;
    out
}

/// CRC32 查找表（标准多项式 0xEDB88320 的反射表）。
///
/// 实测确认：该表与标准 CRC32 表**逐项一致**
/// （见 `scripts/extract_assets.py` 的输出 `与标准 CRC32 表一致: True`）。
/// 差异只在**种子**：作者用 `0x1525D2B6`，而标准实现用 `0xFFFFFFFF`。
const CRC32_TABLE: [u32; 256] = build_crc32_table();

const fn build_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

/// 蓝牙**输出**报告的 CRC 种子 = `~crc32([0xA2])`。
///
/// `0xA2` 是 HIDP 的 `DATA | OUTPUT` 前缀。手柄计算 CRC 时把这个前缀字节
/// 也算进去，而应用层拿到的是不含前缀的报告，所以要用"预计算了前缀"的种子。
pub const BT_OUTPUT_CRC_SEED: u32 = 0x1525_D2B6;

/// 蓝牙**输入**报告的 CRC 种子 = `~crc32([0xA1])`。
///
/// `0xA1` 是 HIDP 的 `DATA | INPUT` 前缀。实测验证：
/// 用标准 `crc32(0xA1 || report[0..74])` 与报告尾部 4 字节完全一致。
pub const BT_INPUT_CRC_SEED: u32 = 0x8C2C_830C;

/// 标准 CRC32 种子。
pub const STD_CRC_SEED: u32 = 0xFFFF_FFFF;

/// 按指定种子计算 CRC32（种子已在调用方完成取反）。
pub fn crc32_with_seed(data: &[u8], seed: u32) -> u32 {
    let mut crc = seed;
    for &b in data {
        crc = (crc >> 8) ^ CRC32_TABLE[((b as u32) ^ crc) as usize & 0xFF];
    }
    !crc
}

/// 标准 CRC32（种子 0xFFFFFFFF）。
pub fn crc32(data: &[u8]) -> u32 {
    crc32_with_seed(data, STD_CRC_SEED)
}

/// 蓝牙输出报告（0x31 / 0x32）的 CRC，覆盖前 74 字节。
pub fn bt_output_crc32(data: &[u8]) -> u32 {
    crc32_with_seed(data, BT_OUTPUT_CRC_SEED)
}

/// 蓝牙输入报告（0x31）的 CRC，覆盖前 74 字节。
pub fn bt_input_crc32(data: &[u8]) -> u32 {
    crc32_with_seed(data, BT_INPUT_CRC_SEED)
}

/// 将真实蓝牙身份特性转换为 USB GET_REPORT 的表示。
/// 0x05/0x09/0x20 固定长度相同，但蓝牙尾部 4B 是带 A3 前缀的 CRC，
/// 真机 USB 的对应保留字节为零。先验证传输 CRC，再清除该传输字段。
pub fn usb_identity_feature(report: &[u8]) -> Option<Vec<u8>> {
    let length = match report.first()? {
        0x05 => 41,
        0x09 => 20,
        0x20 => 64,
        _ => return None,
    };
    if report.len() != length {
        return None;
    }
    let end = length - 4;
    let crc = u32::from_le_bytes(report[end..].try_into().ok()?);
    if crc32_with_seed(&report[..end], !crc32(&[0xA3])) != crc {
        return None;
    }
    let mut usb = report.to_vec();
    usb[end..].fill(0);
    Some(usb)
}

/// 校验一条蓝牙输入报告的尾部 CRC。
///
/// 报告布局：`[0..74]` 为内容，`[74..78]` 为 CRC32 小端。
pub fn verify_input_crc(report: &[u8]) -> bool {
    if report.len() < 78 {
        return false;
    }
    let expect = u32::from_le_bytes([report[74], report[75], report[76], report[77]]);
    bt_input_crc32(&report[0..74]) == expect
}

/// 构造一个 78 字节的蓝牙输出报告骨架。
///
/// CRC 使用 `BT_OUTPUT_CRC_SEED`，覆盖前 74 字节。
pub fn build_bluetooth_output(report_id: u8, payload: &[u8]) -> Vec<u8> {
    let mut rpt = vec![0u8; 78];
    rpt[0] = report_id;
    let n = payload.len().min(73);
    rpt[1..1 + n].copy_from_slice(&payload[..n]);
    let crc = bt_output_crc32(&rpt[0..74]);
    rpt[74..78].copy_from_slice(&crc.to_le_bytes());
    rpt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_features_validate_bluetooth_crc_and_remove_it_for_usb() {
        // CRC 向量由独立的 zlib.crc32(A3 || payload) 生成。
        for (id, length, crc) in [
            (0x05u8, 41, 0xE39E_06AFu32),
            (0x09, 20, 0x7893_C0CE),
            (0x20, 64, 0x4E50_97A5),
        ] {
            let mut bt = vec![id];
            bt.extend((0..length - 5).map(|i| (i * 7 + 3) as u8));
            bt.extend(crc.to_le_bytes());
            let usb = usb_identity_feature(&bt).unwrap();
            assert_eq!(&usb[..length - 4], &bt[..length - 4]);
            assert_eq!(&usb[length - 4..], &[0; 4]);
            bt[1] ^= 1;
            assert!(usb_identity_feature(&bt).is_none());
            assert!(usb_identity_feature(&bt[..length - 1]).is_none());
        }
        assert!(usb_identity_feature(&[]).is_none());
        assert!(usb_identity_feature(&[0x80; 64]).is_none());
    }
    #[test]
    fn virtual_usb_transport_flags_preserve_headset_mute_and_motion() {
        let mut bt = vec![0u8; 78];
        bt[0] = 0x31;
        bt[1] = 1;
        bt[55] = 5;
        for (i, b) in bt[17..33].iter_mut().enumerate() {
            *b = i as u8;
        }
        let usb = InputReport::new(bt.clone()).unwrap().usb_payload().unwrap();
        assert_eq!(usb[54], 0x1D);
        assert_eq!(&usb[16..32], &bt[17..33]);
        assert_eq!(neutral_usb_report()[54] & 0x18, 0x18);
    }
    #[test]
    fn full_bluetooth_passthrough_preserves_all_motion_axes_and_sensor_time() {
        let mut bt = vec![0; 78];
        bt[0] = 0x31;
        bt[1] = 1;
        for (offset, value) in [
            (17, 1234i16),
            (19, -2345),
            (21, 3456),
            (23, -4567),
            (25, 5678),
            (27, -6789),
        ] {
            bt[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        bt[29..33].copy_from_slice(&0x12345678u32.to_le_bytes());
        let r = InputReport::new(bt.clone()).unwrap();
        let usb = r.usb_payload().unwrap();
        assert_eq!(&usb[16..32], &bt[17..33]);
        let motion = InputReport::new(usb.to_vec())
            .unwrap()
            .parse()
            .unwrap()
            .motion;
        assert_eq!(motion.gyro, [1234, -2345, 3456]);
        assert_eq!(motion.accel, [-4567, 5678, -6789]);
        assert_eq!(motion.timestamp, 0x12345678);
    }

    #[test]
    fn detect_modes() {
        assert_eq!(Mode::from_input_len(64), Some(Mode::Usb));
        assert_eq!(Mode::from_input_len(78), Some(Mode::Bluetooth));
        assert_eq!(Mode::from_input_len(32), None);
    }

    #[test]
    fn usb_report_passthrough() {
        let mut data = vec![0u8; 64];
        data[0] = 0x01;
        data[2] = 0x80;
        let r = InputReport::new(data).unwrap();
        let p = r.usb_payload().unwrap();
        assert_eq!(p[0], 0x01);
        assert_eq!(p[2], 0x80);
    }

    #[test]
    fn bluetooth_full_report_skips_two_bytes() {
        let mut data = vec![0u8; 78];
        data[0] = 0x31;
        data[1] = 0x01;
        data[2] = 0xAA;
        data[3] = 0xBB;
        let r = InputReport::new(data).unwrap();
        let p = r.usb_payload().unwrap();
        assert_eq!(p[0], 0x01);
        assert_eq!(p[1], 0xAA);
        assert_eq!(p[2], 0xBB);
        assert!(!r.has_audio_payload());
    }

    #[test]
    fn microphone_reports_never_become_gamepad_input() {
        let mut data = vec![0; 78];
        data[0] = 0x31;
        data[1] = 2;
        data[2] = 0xD4;
        data[3..74].fill(0x9a);
        let r = InputReport::new(data).unwrap();
        assert_eq!(r.microphone_frame().unwrap(), &[0x9a; 71]);
        assert!(r.usb_payload().is_none());
        assert!(r.parse().is_none());
    }

    #[test]
    fn bluetooth_compat_maps_buttons_triggers_and_counter() {
        let mut data = vec![0u8; 78];
        data[0] = 0x01;
        data[1..10].copy_from_slice(&[10, 20, 30, 40, 0x28, 0x11, 0x57, 90, 120]);
        let r = InputReport::new(data).unwrap();
        let p = r.usb_payload().unwrap();
        assert_eq!(&p[1..11], &[10, 20, 30, 40, 90, 120, 0x15, 0x28, 0x11, 3]);
        assert_eq!(p[33], 0x80);
        assert_eq!(p[37], 0x80);
        let state = r.parse().unwrap();
        assert_eq!(state.buttons2, 3);
        assert_eq!(state.seq, 0x15);
    }

    #[test]
    fn neutral_report_releases_buttons_without_phantom_touches() {
        let r = InputReport::new(neutral_usb_report().to_vec()).unwrap();
        let s = r.parse().unwrap();
        assert_eq!((s.lx, s.ly, s.rx, s.ry), (128, 128, 128, 128));
        assert_eq!(
            (s.l2, s.r2, s.buttons0, s.buttons1, s.buttons2),
            (0, 0, 8, 0, 0)
        );
        assert!(!s.touch1.active && !s.touch2.active);
    }

    /// 前缀字节推导：`~crc32([0xA2])` 必须等于输出种子。
    #[test]
    fn output_seed_equals_complement_of_a2_prefix_crc() {
        let prefix_crc = crc32(&[0xA2]);
        assert_eq!(!prefix_crc, BT_OUTPUT_CRC_SEED);
    }

    /// 前缀字节推导：`~crc32([0xA1])` 必须等于输入方向的种子。
    #[test]
    fn input_seed_equals_complement_of_a1_prefix_crc() {
        let prefix_crc = crc32(&[0xA1]);
        assert_eq!(!prefix_crc, BT_INPUT_CRC_SEED);
    }

    /// 构造一份报告并验证尾部 CRC 能通过校验。
    #[test]
    fn build_and_verify_output_report() {
        let rpt = build_bluetooth_output(0x31, &[0x02]);
        assert_eq!(rpt.len(), 78);
        let crc = u32::from_le_bytes([rpt[74], rpt[75], rpt[76], rpt[77]]);
        assert_eq!(crc, bt_output_crc32(&rpt[0..74]));
    }

    /// 输入报告 CRC：用实测帧验证（0x31 帧，尾部 0x26B62FD5 对应的种子关系）。
    #[test]
    fn input_crc_uses_a1_prefix_semantics() {
        let mut rpt = vec![0u8; 78];
        rpt[0] = 0x31;
        for (i, b) in rpt.iter_mut().enumerate().skip(1).take(73) {
            *b = (i as u8).wrapping_mul(7);
        }
        let crc = bt_input_crc32(&rpt[0..74]);
        rpt[74..78].copy_from_slice(&crc.to_le_bytes());
        assert!(verify_input_crc(&rpt));
        // 改动一个字节应当失败
        rpt[10] ^= 0xFF;
        assert!(!verify_input_crc(&rpt));
    }

    /// 触摸点位域解析。
    #[test]
    fn touch_point_bitfields() {
        // status=0x01 → 激活，ID=1
        // X = ((0x52 & 0x0F) << 8) | 0x34 = 0x234 = 564
        // Y = (0x01 << 4) | (0x52 >> 4) = 0x15 = 21
        let p = TouchPoint::from_bytes(&[0x01, 0x34, 0x52, 0x01]);
        assert!(p.active);
        assert_eq!(p.tracking_id, 1);
        assert_eq!(p.x, 564);
        assert_eq!(p.y, 21);

        // status=0x80 → 未激活
        let p = TouchPoint::from_bytes(&[0x80, 0, 0, 0]);
        assert!(!p.active);
        assert_eq!(p.tracking_id, 0);
    }

    /// 完整负载解析：合成一帧 78 字节蓝牙报告，逐字段校验。
    #[test]
    fn parse_full_payload_bluetooth() {
        let mut rpt = vec![0u8; 78];
        rpt[0] = 0x31;
        rpt[1] = 0x21; // seq
                       // 负载从 offset 2 开始
        rpt[2] = 128; // LX
        rpt[3] = 130; // LY
        rpt[4] = 127; // RX
        rpt[5] = 129; // RY
        rpt[6] = 200; // L2
        rpt[7] = 50; // R2
        rpt[8] = 0x01; // seq
        rpt[9] = 0x08; // buttons0 = dpad 中立
        rpt[10] = 0x11; // buttons1 = L1 | Create
        rpt[11] = 0x03; // buttons2 = PS | Touchpad
                        // IMU
        rpt[2 + 15..2 + 17].copy_from_slice(&100i16.to_le_bytes());
        rpt[2 + 23..2 + 25].copy_from_slice(&(-200i16).to_le_bytes());
        rpt[2 + 31] = 35;
        // 触摸点 1（激活）
        rpt[2 + 32..2 + 36].copy_from_slice(&[0x01, 0x34, 0x52, 0x01]);
        // 触摸点 2（未激活：status 的 bit7 置位）
        rpt[2 + 36..2 + 40].copy_from_slice(&[0x80, 0x00, 0x00, 0x00]);
        // 电量 65%（低 4 位 5 → 50%~60% 区间），状态 1（充电）
        rpt[2 + 52] = 0x15;
        rpt[2 + 53] = 0x0C;

        let parsed = InputReport::new(rpt).unwrap().parse().unwrap();
        assert_eq!(parsed.lx, 128);
        assert_eq!(parsed.ly, 130);
        assert_eq!(parsed.l2, 200);
        assert_eq!(parsed.r2, 50);
        assert_eq!(parsed.buttons0, 0x08);
        assert_eq!(parsed.buttons1, 0x11);
        assert_eq!(parsed.buttons2, 0x03);
        assert_eq!(parsed.motion.gyro[0], 100);
        assert_eq!(parsed.motion.accel[1], -200);
        assert_eq!(parsed.motion.temperature, 35);
        assert!(parsed.touch1.active);
        assert_eq!(parsed.touch1.x, 564);
        assert!(!parsed.touch2.active);
        assert_eq!(parsed.touch2.tracking_id, 0);
        assert_eq!(parsed.battery_level, 5);
        assert_eq!(parsed.battery_status, 1);
        assert_eq!(parsed.connection, 0x0C);

        // 按键位判定
        assert!(parsed.buttons1 & buttons::L1 != 0);
        assert!(parsed.buttons1 & buttons::CREATE != 0);
        assert!(parsed.buttons2 & buttons::PS != 0);
        assert!(parsed.buttons2 & buttons::TOUCHPAD != 0);
        assert_eq!(parsed.buttons0 & buttons::DPAD_MASK, buttons::DPAD_NEUTRAL);
    }
}
