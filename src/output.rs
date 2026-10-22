//! 输出报告：游戏 → 虚拟设备 → 真实手柄的反馈通道。
//!
//! 承载自适应扳机、灯光、震动、音量控制。
//!
//! 结构与 DualSenseClient 的 `SetStateData` 一致，并已与实测对齐：
//! - USB 输出报告 `0x02`：48 字节 = 1 字节 ID + **47 字节载荷**
//! - 蓝牙输出报告 `0x31`：78 字节 = `[0]=0x31`、`[1]=seq<<4`、`[2]=0x10`、
//!   载荷从 `[3]` 开始、保留到 `[73]`、`[74..78]` 为 CRC32
//!
//! 两种传输的载荷完全相同，只有封装不同。

use crate::report::bt_output_crc32;

/// 输出载荷长度。
pub const PAYLOAD_SIZE: usize = 47;
/// USB 输出报告 ID。
pub const USB_REPORT_ID: u8 = 0x02;
/// 蓝牙输出报告 ID。
pub const BT_REPORT_ID: u8 = 0x31;
/// 蓝牙封装标志。
pub const BT_FLAGS: u8 = 0x10;
/// 蓝牙报告总长。
pub const BT_REPORT_SIZE: usize = 78;

/// 同 Sony/Linux 驱动：Edge 或普通 DualSense 固件特性版本 >= 2.21 使用 v2。
pub fn uses_vibration_v2(pid: u16, firmware: Option<u16>) -> bool {
    pid == 0x0DF2 || firmware.is_some_and(|v| v >= (2 << 8 | 21))
}

/// 游戏的 v1/v2 选择器必须适配真实手柄，而不是虚拟手柄的固件身份。
pub fn adapt_rumble(state: &mut [u8; 47], vibration_v2: bool) {
    let selected = state[0] & 1 != 0 || state[38] & 4 != 0;
    if selected {
        state[0] |= 2;
        if vibration_v2 {
            state[0] &= !1;
            state[38] |= 4;
        } else {
            state[0] |= 1;
            state[38] &= !4;
        }
    }
}

/// 字节 0 的有效位。
pub mod valid0 {
    pub const ENABLE_RUMBLE_EMULATION: u8 = 1 << 0;
    pub const USE_RUMBLE_NOT_HAPTICS: u8 = 1 << 1;
    pub const ALLOW_RIGHT_TRIGGER_FFB: u8 = 1 << 2;
    pub const ALLOW_LEFT_TRIGGER_FFB: u8 = 1 << 3;
    pub const ALLOW_HEADPHONE_VOLUME: u8 = 1 << 4;
    pub const ALLOW_SPEAKER_VOLUME: u8 = 1 << 5;
    pub const ALLOW_MIC_VOLUME: u8 = 1 << 6;
    pub const ALLOW_AUDIO_CONTROL: u8 = 1 << 7;
}

/// 字节 1 的有效位。
pub mod valid1 {
    pub const ALLOW_MUTE_LIGHT: u8 = 1 << 0;
    pub const ALLOW_AUDIO_MUTE: u8 = 1 << 1;
    pub const ALLOW_LED_COLOR: u8 = 1 << 2;
    pub const RESET_LIGHTS: u8 = 1 << 3;
    pub const ALLOW_PLAYER_INDICATORS: u8 = 1 << 4;
    pub const ALLOW_HAPTIC_LPF: u8 = 1 << 5;
    pub const ALLOW_MOTOR_POWER_LEVEL: u8 = 1 << 6;
    pub const ALLOW_AUDIO_CONTROL2: u8 = 1 << 7;
}

/// 字节 38 的有效位。
pub mod valid2 {
    pub const ALLOW_BRIGHTNESS_CHANGE: u8 = 1 << 0;
    pub const ALLOW_COLOR_FADE_ANIM: u8 = 1 << 1;
    pub const ENABLE_IMPROVED_RUMBLE_EMU: u8 = 1 << 2;
}

/// 自适应扳机效果模式。
pub mod trigger_mode {
    pub const OFF: u8 = 0x00;
    pub const RESISTANCE: u8 = 0x01;
    pub const TRIGGER: u8 = 0x02;
    pub const AUTOMATIC: u8 = 0x06;
    pub const MULTIPLE_POSITION_FEEDBACK: u8 = 0x21;
    pub const MULTIPLE_POSITION_VIBRATION: u8 = 0x26;
}

/// 11 字节的扳机效果块。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TriggerEffect {
    pub mode: u8,
    pub params: [u8; 10],
}

impl TriggerEffect {
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < 11 {
            return None;
        }
        let mut params = [0u8; 10];
        params.copy_from_slice(&b[1..11]);
        Some(Self { mode: b[0], params })
    }

    pub fn write_to(&self, out: &mut [u8]) {
        out[0] = self.mode;
        out[1..11].copy_from_slice(&self.params);
    }

    pub fn is_off(&self) -> bool {
        self.mode == trigger_mode::OFF
    }
}

/// 47 字节输出载荷。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetStateData {
    pub valid0: u8,
    pub valid1: u8,
    pub rumble_right: u8,
    pub rumble_left: u8,
    pub headphone_volume: u8,
    pub speaker_volume: u8,
    pub mic_volume: u8,
    pub audio_control: u8,
    pub mute_led_mode: u8,
    pub power_save_control: u8,
    pub r2_effect: TriggerEffect,
    pub l2_effect: TriggerEffect,
    pub host_timestamp: u32,
    pub motor_power_reduction: u8,
    pub audio_control2: u8,
    pub valid2: u8,
    pub haptic_low_pass_filter: u8,
    pub light_fade_animation: u8,
    pub light_brightness: u8,
    pub player_leds: u8,
    pub led_red: u8,
    pub led_green: u8,
    pub led_blue: u8,
}

impl Default for SetStateData {
    fn default() -> Self {
        Self {
            valid0: 0,
            valid1: 0,
            rumble_right: 0,
            rumble_left: 0,
            headphone_volume: 0,
            speaker_volume: 0,
            mic_volume: 0,
            audio_control: 0,
            mute_led_mode: 0,
            power_save_control: 0,
            r2_effect: TriggerEffect::default(),
            l2_effect: TriggerEffect::default(),
            host_timestamp: 0,
            motor_power_reduction: 0,
            audio_control2: 0,
            valid2: 0,
            haptic_low_pass_filter: 0,
            light_fade_animation: 0,
            light_brightness: 0,
            player_leds: 0,
            led_red: 0,
            led_green: 0,
            led_blue: 0,
        }
    }
}

impl SetStateData {
    /// 从 47 字节载荷解析。
    pub fn parse(p: &[u8]) -> Option<Self> {
        if p.len() < PAYLOAD_SIZE {
            return None;
        }
        Some(Self {
            valid0: p[0],
            valid1: p[1],
            rumble_right: p[2],
            rumble_left: p[3],
            headphone_volume: p[4],
            speaker_volume: p[5],
            mic_volume: p[6],
            audio_control: p[7],
            mute_led_mode: p[8],
            power_save_control: p[9],
            r2_effect: TriggerEffect::from_bytes(&p[10..21])?,
            l2_effect: TriggerEffect::from_bytes(&p[21..32])?,
            host_timestamp: u32::from_le_bytes([p[32], p[33], p[34], p[35]]),
            motor_power_reduction: p[36],
            audio_control2: p[37],
            valid2: p[38],
            haptic_low_pass_filter: p[39],
            // p[40] 保留
            light_fade_animation: p[41],
            light_brightness: p[42],
            player_leds: p[43],
            led_red: p[44],
            led_green: p[45],
            led_blue: p[46],
        })
    }

    /// 写回 47 字节载荷。
    pub fn write_to(&self, p: &mut [u8]) {
        p[0] = self.valid0;
        p[1] = self.valid1;
        p[2] = self.rumble_right;
        p[3] = self.rumble_left;
        p[4] = self.headphone_volume;
        p[5] = self.speaker_volume;
        p[6] = self.mic_volume;
        p[7] = self.audio_control;
        p[8] = self.mute_led_mode;
        p[9] = self.power_save_control;
        self.r2_effect.write_to(&mut p[10..21]);
        self.l2_effect.write_to(&mut p[21..32]);
        p[32..36].copy_from_slice(&self.host_timestamp.to_le_bytes());
        p[36] = self.motor_power_reduction;
        p[37] = self.audio_control2;
        p[38] = self.valid2;
        p[39] = self.haptic_low_pass_filter;
        p[40] = 0;
        p[41] = self.light_fade_animation;
        p[42] = self.light_brightness;
        p[43] = self.player_leds;
        p[44] = self.led_red;
        p[45] = self.led_green;
        p[46] = self.led_blue;
    }

    /// 灯条颜色是否被本帧允许更新。
    pub fn allows_led_color(&self) -> bool {
        self.valid1 & valid1::ALLOW_LED_COLOR != 0
    }

    /// 是否有任一扳机效果不是 Off。
    pub fn has_trigger_effect(&self) -> bool {
        !self.r2_effect.is_off() || !self.l2_effect.is_off()
    }

    pub fn rumble(&self) -> (u8, u8) {
        (self.rumble_right, self.rumble_left)
    }
}

/// 从 USB 输出报告（0x02，48 字节）解析载荷。
pub fn parse_usb_output(report: &[u8]) -> Option<SetStateData> {
    if report.len() < 1 + PAYLOAD_SIZE || report[0] != USB_REPORT_ID {
        return None;
    }
    SetStateData::parse(&report[1..1 + PAYLOAD_SIZE])
}

/// 把 USB 输出报告转换成蓝牙 0x31 报告（78 字节，含 CRC）。
///
/// `seq` 只取低 4 位，写入 `[1]` 的高半字节。
pub fn usb_to_bluetooth(usb: &[u8], seq: u8) -> Option<[u8; BT_REPORT_SIZE]> {
    let state = parse_usb_output(usb)?;
    let mut payload = [0u8; PAYLOAD_SIZE];
    state.write_to(&mut payload);

    let mut out = [0u8; BT_REPORT_SIZE];
    out[0] = BT_REPORT_ID;
    out[1] = (seq & 0x0F) << 4;
    out[2] = BT_FLAGS;
    out[3..3 + PAYLOAD_SIZE].copy_from_slice(&payload);
    // 50..74 保留为 0
    let crc = bt_output_crc32(&out[0..74]);
    out[74..78].copy_from_slice(&crc.to_le_bytes());
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rumble_protocol_follows_real_firmware_and_preserves_other_output() {
        assert!(!uses_vibration_v2(0x0CE6, None));
        assert!(!uses_vibration_v2(0x0CE6, Some(0x0214)));
        assert!(uses_vibration_v2(0x0CE6, Some(0x0215)));
        assert!(uses_vibration_v2(0x0DF2, None));
        let mut v1 = [0; 47];
        v1[0] = 0x3F;
        v1[2] = 77;
        v1[3] = 123;
        v1[10] = 0x21;
        v1[44] = 200;
        let original = v1;
        adapt_rumble(&mut v1, true);
        assert_eq!(v1[0] & 3, 2);
        assert_eq!(v1[38] & 4, 4);
        assert_eq!((v1[2], v1[3], v1[10], v1[44]), (77, 123, 0x21, 200));
        adapt_rumble(&mut v1, false);
        assert_eq!(v1, original);
        let mut leds = [0; 47];
        leds[1] = 4;
        leds[44] = 255;
        let expected = leds;
        adapt_rumble(&mut leds, true);
        assert_eq!(leds, expected);
    }

    fn sample_payload() -> [u8; PAYLOAD_SIZE] {
        let mut p = [0u8; PAYLOAD_SIZE];
        p[0] = valid0::ALLOW_RIGHT_TRIGGER_FFB | valid0::ALLOW_LEFT_TRIGGER_FFB;
        p[1] = valid1::ALLOW_LED_COLOR | valid1::ALLOW_PLAYER_INDICATORS;
        p[2] = 0x40; // rumble right
        p[3] = 0x80; // rumble left
        p[10] = trigger_mode::RESISTANCE;
        p[11] = 0xAA;
        p[21] = trigger_mode::TRIGGER;
        p[22] = 0xBB;
        p[32..36].copy_from_slice(&12345u32.to_le_bytes());
        p[42] = 0x01;
        p[43] = 0b00100; // player LED 中间
        p[44] = 0xFF;
        p[45] = 0x00;
        p[46] = 0x10;
        p
    }

    #[test]
    fn parse_and_write_roundtrip() {
        let p = sample_payload();
        let s = SetStateData::parse(&p).unwrap();
        assert_eq!(s.rumble_right, 0x40);
        assert_eq!(s.rumble_left, 0x80);
        assert_eq!(s.r2_effect.mode, trigger_mode::RESISTANCE);
        assert_eq!(s.r2_effect.params[0], 0xAA);
        assert_eq!(s.l2_effect.mode, trigger_mode::TRIGGER);
        assert_eq!(s.host_timestamp, 12345);
        assert_eq!(s.light_brightness, 0x01);
        assert_eq!(s.player_leds, 0b00100);
        assert_eq!((s.led_red, s.led_green, s.led_blue), (0xFF, 0x00, 0x10));
        assert!(s.allows_led_color());
        assert!(s.has_trigger_effect());

        let mut out = [0u8; PAYLOAD_SIZE];
        s.write_to(&mut out);
        assert_eq!(out, p);
    }

    #[test]
    fn usb_output_rejects_wrong_id() {
        let mut r = [0u8; 48];
        r[0] = 0x31;
        assert!(parse_usb_output(&r).is_none());
    }

    #[test]
    fn usb_to_bt_framing_and_crc() {
        let mut usb = [0u8; 48];
        usb[0] = USB_REPORT_ID;
        usb[1..].copy_from_slice(&sample_payload());

        let bt = usb_to_bluetooth(&usb, 0x0A).unwrap();
        assert_eq!(bt.len(), BT_REPORT_SIZE);
        assert_eq!(bt[0], BT_REPORT_ID);
        assert_eq!(bt[1], 0xA0); // seq 0xA << 4
        assert_eq!(bt[2], BT_FLAGS);
        // 载荷从 [3] 开始，与原 47 字节一致
        assert_eq!(&bt[3..50], &sample_payload()[..]);
        // 50..74 保留为 0
        assert!(bt[50..74].iter().all(|&b| b == 0));
        // CRC 自校验
        let crc = u32::from_le_bytes([bt[74], bt[75], bt[76], bt[77]]);
        assert_eq!(crc, bt_output_crc32(&bt[0..74]));
    }

    #[test]
    fn seq_masked_to_low_nibble() {
        let mut usb = [0u8; 48];
        usb[0] = USB_REPORT_ID;
        let bt = usb_to_bluetooth(&usb, 0xFF).unwrap();
        assert_eq!(bt[1], 0xF0);
    }

    #[test]
    fn off_trigger_has_no_effect() {
        let s = SetStateData::default();
        assert!(!s.has_trigger_effect());
        assert!(!s.allows_led_color());
        assert_eq!(s.rumble(), (0, 0));
    }
}
