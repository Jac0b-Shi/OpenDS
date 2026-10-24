//! 配置结构与默认值。
//!
//! 解析 UTF-8 INI 的 `[opends]` 段（兼容旧 `[ds5]`），再做范围钳制。

use std::{
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CloseAction {
    #[default]
    Ask,
    Tray,
    Exit,
}

impl fmt::Display for CloseAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ask => "ask",
            Self::Tray => "tray",
            Self::Exit => "exit",
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}
impl ThemeMode {
    pub fn parse(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Light,
            2 => Self::Dark,
            _ => Self::System,
        }
    }
}
impl fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        })
    }
}

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
    pub language: String,
    pub theme: ThemeMode,
    pub close_action: CloseAction,
    pub reconnect: bool,
    pub hide_device: bool,
    pub audio_enabled: bool,
    pub haptics_enabled: bool,
    pub microphone_enabled: bool,
    pub microphone_gain_db: u8,
    pub microphone_buffer_ms: u16,
    /// 0 自动（优先游戏路由，其次耳机插入状态），1 扬声器，2 耳机。
    pub audio_route: u8,
    pub speaker_volume: u8,
    pub headphone_volume: u8,
    /// 空串表示自动选择；否则只连接指定物理 HID 路径。
    pub device_path: String,
    /// Empty selects automatic discovery; otherwise use this installation directory.
    pub usbip_directory: String,
    pub hidhide_directory: String,
    pub monitor_sample_hz: u32,
    pub monitor_fps: u32,
    pub monitor_seconds: u32,
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
            language: "auto".into(),
            theme: ThemeMode::System,
            close_action: CloseAction::Ask,
            reconnect: true,
            hide_device: true,
            audio_enabled: true,
            haptics_enabled: true,
            microphone_enabled: false,
            microphone_gain_db: 0,
            microphone_buffer_ms: 128,
            audio_route: 0,
            speaker_volume: 255,
            headphone_volume: 63,
            device_path: String::new(),
            usbip_directory: String::new(),
            hidhide_directory: String::new(),
            monitor_sample_hz: 100,
            monitor_fps: 60,
            monitor_seconds: 60,
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
        self.microphone_gain_db = self.microphone_gain_db.min(50);
        self.microphone_buffer_ms = self.microphone_buffer_ms.clamp(16, 512);
        if ![20, 50, 100, 200].contains(&self.monitor_sample_hz) {
            self.monitor_sample_hz = 100;
        }
        if ![15, 30, 60].contains(&self.monitor_fps) {
            self.monitor_fps = 60;
        }
        if ![30, 60, 120, 300, 600, 900].contains(&self.monitor_seconds) {
            self.monitor_seconds = 60;
        }
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
            Err(e) => Err(crate::tr!(
                config_could_not_read_configuration,
                error = crate::i18n_arg!(e)
            )),
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
            let (key, value) = line.split_once('=').ok_or_else(|| {
                crate::tr!(
                    config_configuration_line_is_missing,
                    line = crate::i18n_arg!(i + 1)
                )
            })?;
            let value = value.trim();
            macro_rules! number {
                ($field:ident) => {
                    cfg.$field = value.parse().map_err(|_| {
                        crate::tr!(
                            config_invalid_configuration,
                            key = crate::i18n_arg!(key.trim()),
                            value = crate::i18n_arg!(value)
                        )
                    })?
                };
            }
            macro_rules! boolean {
                ($field:ident) => {
                    cfg.$field = match value {
                        "1" | "true" => true,
                        "0" | "false" => false,
                        _ => {
                            return Err(crate::tr!(
                                config_configuration_must_be_0_1_or_true,
                                key = crate::i18n_arg!(key.trim())
                            ))
                        }
                    }
                };
            }
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
                "language" => cfg.language = value.to_owned(),
                "theme" => cfg.theme = ThemeMode::parse(value),
                "close_action" => {
                    cfg.close_action = match value {
                        "tray" => CloseAction::Tray,
                        "exit" => CloseAction::Exit,
                        _ => CloseAction::Ask,
                    }
                }
                "reconnect" => boolean!(reconnect),
                "hide_device" => boolean!(hide_device),
                "audio_enabled" => boolean!(audio_enabled),
                "haptics_enabled" => boolean!(haptics_enabled),
                "microphone_enabled" => boolean!(microphone_enabled),
                "microphone_gain_db" => number!(microphone_gain_db),
                "microphone_buffer_ms" => number!(microphone_buffer_ms),
                "audio_route" => number!(audio_route),
                "speaker_volume" => number!(speaker_volume),
                "headphone_volume" => number!(headphone_volume),
                "device_path" => cfg.device_path = value.to_owned(),
                "usbip_directory" => cfg.usbip_directory = value.to_owned(),
                "hidhide_directory" => cfg.hidhide_directory = value.to_owned(),
                "monitor_sample_hz" => number!(monitor_sample_hz),
                "monitor_fps" => number!(monitor_fps),
                "monitor_seconds" => number!(monitor_seconds),
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
            language,
            theme,
            close_action,
            reconnect,
            hide_device,
            audio_enabled,
            haptics_enabled,
            microphone_enabled,
            microphone_gain_db,
            microphone_buffer_ms,
            audio_route,
            speaker_volume,
            headphone_volume,
            device_path,
            usbip_directory,
            hidhide_directory,
            monitor_sample_hz,
            monitor_fps,
            monitor_seconds
        );
        if [
            &self.device_path,
            &self.language,
            &self.usbip_directory,
            &self.hidhide_directory,
        ]
        .iter()
        .any(|value| value.contains(['\r', '\n', '\0']))
        {
            return Err(
                crate::tr!(config_configuration_values_must_not_contain_line_breaks).into(),
            );
        }
        crate::persistence::atomic_write(path, s.as_bytes()).map_err(|e| {
            crate::tr!(
                config_could_not_save_configuration,
                error = crate::i18n_arg!(e)
            )
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.dsx_udp || self.dsx_exe || self.usb_face != 0 {
            return Err(crate::tr!(config_dsx_udp_automatic_dsx_launch_and_xbox).into());
        }
        if !self.auto_attach {
            return Err(crate::tr!(config_local_bridging_requires_auto_attach_true).into());
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
    fn dependency_directories_round_trip_reset_and_reject_ini_injection() {
        let path = std::env::temp_dir().join(format!(
            "opends-directory-config-{}.ini",
            std::process::id()
        ));
        let mut cfg = Config {
            usbip_directory: r"D:\软件 空格\USB=ip".into(),
            hidhide_directory: r"D:\软件\HidHide".into(),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.usbip_directory, cfg.usbip_directory);
        assert_eq!(loaded.hidhide_directory, cfg.hidhide_directory);
        for invalid in ["bad\nport=1", "bad\rport=1", "bad\0path"] {
            cfg.usbip_directory = invalid.into();
            assert!(cfg.save(&path).is_err());
            assert_eq!(
                Config::load(&path).unwrap().usbip_directory,
                loaded.usbip_directory
            );
            cfg.usbip_directory.clear();
            cfg.hidhide_directory = invalid.into();
            assert!(cfg.save(&path).is_err());
        }
        cfg.hidhide_directory.clear();
        cfg.save(&path).unwrap();
        let reset = Config::load(&path).unwrap();
        assert!(reset.usbip_directory.is_empty() && reset.hidhide_directory.is_empty());
        assert!(Config::parse("[opends]\nport=3241")
            .unwrap()
            .usbip_directory
            .is_empty());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn language_defaults_to_system_and_round_trips_unknown_preferences() {
        assert_eq!(Config::default().language, "auto");
        let path =
            std::env::temp_dir().join(format!("opends-language-config-{}.ini", std::process::id()));
        for code in ["auto", "en-US", "zh-CN", "de-DE"] {
            let cfg = Config {
                language: code.into(),
                microphone_gain_db: 29,
                ..Default::default()
            };
            cfg.save(&path).unwrap();
            let loaded = Config::load(&path).unwrap();
            assert_eq!(loaded.language, code);
            assert_eq!(loaded.microphone_gain_db, 29);
        }
        let cfg = Config {
            language: "de-DE\nport=1".into(),
            ..Default::default()
        };
        assert!(cfg.save(&path).is_err());
        assert_eq!(Config::load(&path).unwrap().language, "de-DE");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn close_action_defaults_to_asking_and_round_trips() {
        assert_eq!(Config::default().close_action, CloseAction::Ask);
        assert_eq!(
            Config::parse("close_action=unknown").unwrap().close_action,
            CloseAction::Ask
        );
        let path =
            std::env::temp_dir().join(format!("opends-close-action-{}.ini", std::process::id()));
        for action in [CloseAction::Tray, CloseAction::Exit, CloseAction::Ask] {
            let cfg = Config {
                close_action: action,
                microphone_gain_db: 29,
                ..Default::default()
            };
            cfg.save(&path).unwrap();
            let saved = Config::load(&path).unwrap();
            assert_eq!(saved.close_action, action);
            assert_eq!(saved.microphone_gain_db, 29);
        }
        std::fs::remove_file(path).unwrap();
    }

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
    #[test]
    fn microphone_buffer_is_separate_clamped_and_persisted() {
        assert_eq!(Config::default().microphone_buffer_ms, 128);
        assert_eq!(
            Config::parse("microphone_buffer_ms=0")
                .unwrap()
                .microphone_buffer_ms,
            16
        );
        assert_eq!(
            Config::parse("microphone_buffer_ms=999")
                .unwrap()
                .microphone_buffer_ms,
            512
        );
        let c = Config::parse("microphone_buffer_ms=160\nbuffer=4").unwrap();
        let path =
            std::env::temp_dir().join(format!("opends-mic-buffer-{}.ini", std::process::id()));
        c.save(&path).unwrap();
        let saved = Config::load(&path).unwrap();
        assert_eq!(saved.microphone_buffer_ms, 160);
        assert_eq!(saved.buffer, 4);
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn microphone_gain_is_opt_in_clamped_and_round_trips() {
        assert_eq!(Config::default().microphone_gain_db, 0);
        assert_eq!(
            Config::parse("microphone_gain_db=255")
                .unwrap()
                .microphone_gain_db,
            50
        );
        assert!(Config::parse("microphone_gain_db=-1").is_err());
        let path = std::env::temp_dir().join(format!("opends-gain-{}.ini", std::process::id()));
        let cfg = Config {
            microphone_gain_db: 24,
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap().microphone_gain_db, 24);
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod theme_tests {
    use super::*;
    #[test]
    fn themes_migrate_and_values_do_not_depend_on_labels() {
        for (raw, expected) in [
            ("", ThemeMode::System),
            ("theme=broken", ThemeMode::System),
            ("theme=light", ThemeMode::Light),
            ("theme=dark", ThemeMode::Dark),
        ] {
            let c = Config::parse(&format!("[opends]\n{raw}\n")).unwrap();
            assert_eq!(c.theme, expected);
            assert_eq!(ThemeMode::from_index(c.theme.index()), expected);
            assert_eq!(ThemeMode::parse(&c.theme.to_string()), expected);
        }
    }
}
