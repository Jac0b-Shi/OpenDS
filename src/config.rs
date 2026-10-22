//! 配置结构与默认值。
//!
//! 解析 UTF-8 INI 的 `[opends]` 段（兼容旧 `[ds5]`），再做范围钳制。

use std::{
    fmt,
    path::{Path, PathBuf},
};

/// 配置项，对应配置结构布局。
#[derive(Debug, Clone)]
pub struct Config {
    /// 本机 USB/IP 监听端口，默认 3241，避免占用其他 USB/IP 服务。
    pub port: u16,
    /// 音频缓冲上限（512 帧/块），默认 4，范围 1..=128。
    pub buffer: u8,
    /// 扬声器增益，0..=7。
    pub speaker_gain: u8,
    /// 触觉增益，10..=20（除以 10 后是 1.0..2.0）。
    pub haptics_gain: u8,
    /// 握把马达增益，10..=20。
    pub rumble_gain: u8,
    /// libopus 复杂度，0..=10，默认 0。
    pub opus_complexity: u8,
    /// 启动后自动 attach。
    pub auto_attach: bool,
    /// 监听 DSX UDP 协议。
    pub dsx_udp: bool,
    /// 启动同目录的 DSX.exe。
    pub dsx_exe: bool,
    /// 把 Edge 呈现为普通 DualSense。
    pub edge_as_ds: bool,
    /// USB 外观：0 = DualSense，1 = Xbox One，2 = PS4。
    pub usb_face: u8,
    pub auto_start: bool,
    pub reconnect: bool,
    pub hide_device: bool,
    pub audio_enabled: bool,
    pub haptics_enabled: bool,
    pub microphone_enabled: bool,
    /// 0 自动（优先游戏路由，其次耳机插入状态），1 扬声器，2 耳机。
    pub audio_route: u8,
    pub speaker_volume: u8,
    pub headphone_volume: u8,
    /// 空串表示自动选择；否则只连接指定物理 HID 路径。
    pub device_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 3241,
            buffer: 4,
            speaker_gain: 0,
            haptics_gain: 10,
            rumble_gain: 10,
            opus_complexity: 0,
            auto_attach: true,
            dsx_udp: false,
            dsx_exe: false,
            edge_as_ds: false,
            usb_face: 0,
            auto_start: false,
            reconnect: true,
            hide_device: true,
            audio_enabled: true,
            haptics_enabled: true,
            microphone_enabled: false,
            audio_route: 0,
            speaker_volume: 255,
            headphone_volume: 63,
            device_path: String::new(),
        }
    }
}

impl Config {
    /// 按原始实现的规则钳制所有字段。
    pub fn clamp(&mut self) {
        if self.port == 0 {
            self.port = 3241;
        }
        self.buffer = self.buffer.clamp(1, 128);
        self.speaker_gain = self.speaker_gain.min(7);
        self.haptics_gain = self.haptics_gain.clamp(10, 20);
        self.rumble_gain = self.rumble_gain.clamp(10, 20);
        self.opus_complexity = self.opus_complexity.min(10);
        if self.usb_face > 2 {
            self.usb_face = 0;
        }
        self.audio_route = self.audio_route.min(2);
    }

    /// 触觉增益的实际倍率。
    pub fn haptics_scale(&self) -> f32 {
        self.haptics_gain as f32 / 10.0
    }

    pub fn path() -> PathBuf {
        // 兼容已有便携配置；新配置统一放在用户可写目录。
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let portable = dir.join("opends.ini");
                if portable.is_file() {
                    return portable;
                }
            }
        }
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("OpenDS")
            .join("opends.ini")
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(s) => Self::parse(&s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("读取配置失败：{e}")),
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut cfg = Self::default();
        let mut section_ok = true;
        for (i, line) in text.lines().enumerate() {
            let line = line.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with([';', '#']) {
                continue;
            }
            if line.starts_with('[') {
                section_ok =
                    line.eq_ignore_ascii_case("[ds5]") || line.eq_ignore_ascii_case("[opends]");
                continue;
            }
            if !section_ok {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("配置第 {} 行缺少 =", i + 1))?;
            let value = value.trim();
            macro_rules! number {
                ($field:ident) => {
                    cfg.$field = value
                        .parse()
                        .map_err(|_| format!("配置 {} 无效：{}", key.trim(), value))?
                };
            }
            macro_rules! boolean { ($field:ident) => { cfg.$field = match value { "1" | "true" => true, "0" | "false" => false, _ => return Err(format!("配置 {} 必须为 0/1 或 true/false", key.trim())) } }; }
            match key.trim() {
                "port" => number!(port),
                "buffer" => number!(buffer),
                "speaker_gain" => number!(speaker_gain),
                "haptics_gain" => number!(haptics_gain),
                "rumble_gain" => number!(rumble_gain),
                "opus_complexity" => number!(opus_complexity),
                "auto_attach" => boolean!(auto_attach),
                "dsx_udp" => boolean!(dsx_udp),
                "dsx_exe" => boolean!(dsx_exe),
                "edge_as_ds" => boolean!(edge_as_ds),
                "usb_face" => number!(usb_face),
                "auto_start" => boolean!(auto_start),
                "reconnect" => boolean!(reconnect),
                "hide_device" => boolean!(hide_device),
                "audio_enabled" => boolean!(audio_enabled),
                "haptics_enabled" => boolean!(haptics_enabled),
                "microphone_enabled" => boolean!(microphone_enabled),
                "audio_route" => number!(audio_route),
                "speaker_volume" => number!(speaker_volume),
                "headphone_volume" => number!(headphone_volume),
                "device_path" => cfg.device_path = value.to_owned(),
                _ => {}
            }
        }
        cfg.clamp();
        Ok(cfg)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut s = String::from("[opends]\n");
        macro_rules! fields { ($($field:ident),+) => { $(s.push_str(&format!("{}={}\n", stringify!($field), self.$field));)+ }; }
        fields!(
            port,
            buffer,
            speaker_gain,
            haptics_gain,
            rumble_gain,
            opus_complexity,
            auto_attach,
            dsx_udp,
            dsx_exe,
            edge_as_ds,
            usb_face,
            auto_start,
            reconnect,
            hide_device,
            audio_enabled,
            haptics_enabled,
            microphone_enabled,
            audio_route,
            speaker_volume,
            headphone_volume,
            device_path
        );
        if self.device_path.contains(['\r', '\n']) {
            return Err("设备路径含换行符".into());
        }
        crate::persistence::atomic_write(path, s.as_bytes())
            .map_err(|e| format!("保存配置失败：{e}"))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.dsx_udp || self.dsx_exe || self.usb_face != 0 {
            return Err("DSX UDP/自动启动 DSX/Xbox/DS4 外观尚未支持，请关闭相应配置。".into());
        }
        if !self.auto_attach {
            return Err("本机桥接需要 auto_attach=true。".into());
        }
        Ok(())
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "port={} buffer={} speaker={} haptics={}({:.1}x) rumble={}({:.1}x) opus={} \
             auto_attach={} dsx_udp={} dsx_exe={} edge_as_ds={} usb_face={}",
            self.port,
            self.buffer,
            self.speaker_gain,
            self.haptics_gain,
            self.haptics_scale(),
            self.rumble_gain,
            self.rumble_gain as f32 / 10.0,
            self.opus_complexity,
            self.auto_attach,
            self.dsx_udp,
            self.dsx_exe,
            self.edge_as_ds,
            self.usb_face
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_defaults_use_local_bridge_and_shallow_buffer() {
        let c = Config::default();
        assert_eq!(c.port, 3241);
        assert_eq!(c.buffer, 4);
        assert_eq!(c.opus_complexity, 0);
        assert_eq!(c.haptics_gain, 10);
    }

    #[test]
    fn parse_rejects_bad_values_and_ignores_other_sections() {
        assert!(Config::parse("reconnect=perhaps").is_err());
        assert!(Config::parse("port=abc").is_err());
        let c = Config::parse("[other]\nport=9000\n[opends]\nbuffer=2\nauto_start=1\n").unwrap();
        assert_eq!(c.port, 3241);
        assert_eq!(c.buffer, 2);
        assert!(c.auto_start);
    }

    #[test]
    fn clamp_bounds() {
        let mut c = Config {
            buffer: 200,
            speaker_gain: 99,
            haptics_gain: 0,
            opus_complexity: 42,
            usb_face: 9,
            ..Default::default()
        };
        c.clamp();
        assert_eq!(c.buffer, 128);
        assert_eq!(c.speaker_gain, 7);
        assert_eq!(c.haptics_gain, 10);
        assert_eq!(c.opus_complexity, 10);
        assert_eq!(c.usb_face, 0);
    }
}
