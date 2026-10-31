//! Event-based, bounded input history. No rendering or device I/O.
use crate::report::ParsedState;
use crossbeam_queue::ArrayQueue;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::*},
        Mutex,
    },
};

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    pub right: bool,
    pub source: u8,
    pub rows: usize,
    pub overlay_rows: usize,
    pub overlay_auto_rows: bool,
    pub separators: bool,
    pub separator_alpha: u8,
    pub separator_style: u8,
    pub separator: u32,
    pub dark_separator: u32,
    pub content_size: u16,
    pub display_min_ms: f64,
    pub record_min_ms: f64,
    pub stick: [[f32; 3]; 2],
    pub trigger: [[u8; 2]; 2],
    pub trigger_deadzone: [u8; 2],
    pub ignore_strength: [bool; 4],
    pub unified_colors: [u32; 4],
    pub dark_unified_colors: [u32; 4],
    pub colors: [[u32; 3]; 4],
    pub dark_colors: [[u32; 3]; 4],
    pub dark_neutral: u32,
    pub dark_dpad: u32,
    pub dark_button: u32,
    pub dark_connection: u32,
    pub neutral: u32,
    pub dpad: u32,
    pub button: u32,
    pub connection: u32,
    pub background: [u32; 2],
    pub text: [u32; 2],
    pub auto_colors: bool,
    pub theme: u8,
    pub bg_alpha: u8,
    pub fg_alpha: u8,
    pub topmost: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            right: false,
            source: 2,
            rows: 8,
            overlay_rows: 12,
            overlay_auto_rows: true,
            separators: true,
            separator_alpha: 55,
            separator_style: 1,
            separator: 0x808080,
            dark_separator: 0x808080,
            content_size: 100,
            display_min_ms: 16.67,
            record_min_ms: 0.,
            stick: [[12., 51., 98.]; 2],
            trigger: [[128, 250]; 2],
            trigger_deadzone: [0; 2],
            ignore_strength: [false; 4],
            unified_colors: [0xE54747, 0x36AC65, 0xE54747, 0xE54747],
            dark_unified_colors: [0xE54747, 0x36AC65, 0xE54747, 0xE54747],
            colors: [
                [0x9DDCF5, 0xE5B900, 0xE54747],
                [0xBFE8C5, 0x83D69B, 0x36AC65],
                [0x9DDCF5, 0xE5B900, 0xE54747],
                [0x9DDCF5, 0xE5B900, 0xE54747],
            ],
            dark_colors: [
                [0x9DDCF5, 0xE5B900, 0xE54747],
                [0xBFE8C5, 0x83D69B, 0x36AC65],
                [0x9DDCF5, 0xE5B900, 0xE54747],
                [0x9DDCF5, 0xE5B900, 0xE54747],
            ],
            dark_neutral: 0x808080,
            dark_dpad: 0x808080,
            dark_button: 0xffffff,
            dark_connection: 0xffffff,
            neutral: 0x808080,
            dpad: 0x808080,
            button: 0,
            connection: 0,
            background: [0xffffff, 0],
            text: [0, 0xffffff],
            auto_colors: true,
            theme: 0,
            bg_alpha: 55,
            fg_alpha: 100,
            topmost: true,
            x: 100,
            y: 100,
            width: 560,
            height: 430,
        }
    }
}
impl Settings {
    // Stable INI keys; RGB is stored in human-readable RRGGBB order.
    pub fn fields(&self) -> Vec<(String, String)> {
        let mut v = vec![];
        macro_rules! val {($($f:ident),*)=>{$(v.push((stringify!($f).into(),self.$f.to_string()));)*}}
        val!(
            enabled,
            right,
            source,
            rows,
            overlay_rows,
            overlay_auto_rows,
            separators,
            separator_alpha,
            separator_style,
            content_size,
            display_min_ms,
            record_min_ms,
            neutral,
            dpad,
            button,
            connection,
            auto_colors,
            theme,
            bg_alpha,
            fg_alpha,
            topmost,
            x,
            y,
            width,
            height
        );
        for i in 0..4 {
            v.push((format!("ignore_{i}"), self.ignore_strength[i].to_string()));
            v.push((
                format!("unified_{i}"),
                format!("#{:06X}", self.unified_colors[i]),
            ));
            v.push((
                format!("darkunified_{i}"),
                format!("#{:06X}", self.dark_unified_colors[i]),
            ));
        }
        for i in 0..2 {
            v.push((
                format!("triggerdead_{i}"),
                self.trigger_deadzone[i].to_string(),
            ));
        }
        for (i, a) in self.stick.iter().enumerate() {
            for (j, n) in a.iter().enumerate() {
                v.push((format!("stick_{i}_{j}"), n.to_string()));
            }
        }
        for (i, a) in self.trigger.iter().enumerate() {
            for (j, n) in a.iter().enumerate() {
                v.push((format!("trigger_{i}_{j}"), n.to_string()));
            }
        }
        for (i, a) in self.colors.iter().enumerate() {
            for (j, n) in a.iter().enumerate() {
                v.push((format!("color_{i}_{j}"), format!("#{n:06X}")));
            }
        }
        for (i, a) in self.dark_colors.iter().enumerate() {
            for (j, n) in a.iter().enumerate() {
                v.push((format!("darkcolor_{i}_{j}"), format!("#{n:06X}")));
            }
        }
        for (k, n) in [
            ("dark_neutral", self.dark_neutral),
            ("dark_dpad", self.dark_dpad),
            ("dark_button", self.dark_button),
            ("dark_connection", self.dark_connection),
            ("separator", self.separator),
            ("dark_separator", self.dark_separator),
        ] {
            v.push((k.into(), format!("#{n:06X}")));
        }
        for (i, n) in self.background.iter().enumerate() {
            v.push((format!("background_{i}"), format!("#{n:06X}")));
        }
        for (i, n) in self.text.iter().enumerate() {
            v.push((format!("text_{i}"), format!("#{n:06X}")));
        }
        for (k, n) in [
            ("neutral", self.neutral),
            ("dpad", self.dpad),
            ("button", self.button),
            ("connection", self.connection),
        ] {
            v.iter_mut().find(|(key, _)| key == k).unwrap().1 = format!("#{n:06X}");
        }
        v
    }
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let value = if matches!(
            key,
            "enabled" | "right" | "auto_colors" | "topmost" | "overlay_auto_rows" | "separators"
        ) {
            match value {
                "1" => "true",
                "0" => "false",
                v => v,
            }
        } else {
            value
        };
        fn number<T: std::str::FromStr>(s: &str) -> Result<T, String> {
            s.parse().map_err(|_| format!("Invalid value: {s}"))
        }
        fn rgb(s: &str) -> Result<u32, String> {
            let s = s.strip_prefix('#').unwrap_or(s);
            if s.len() != 6 {
                return Err("Expected #RRGGBB".into());
            }
            u32::from_str_radix(s, 16).map_err(|_| "Expected #RRGGBB".into())
        }
        macro_rules! fields {($($f:ident),*)=>{match key{$(stringify!($f)=>{self.$f=number(value)?;return Ok(())},)*_=>{}}}}
        fields!(
            enabled,
            right,
            source,
            rows,
            overlay_rows,
            overlay_auto_rows,
            separators,
            separator_alpha,
            separator_style,
            content_size,
            display_min_ms,
            record_min_ms,
            auto_colors,
            theme,
            bg_alpha,
            fg_alpha,
            topmost,
            x,
            y,
            width,
            height
        );
        match key {
            "separator" => self.separator = rgb(value)?,
            "dark_separator" => self.dark_separator = rgb(value)?,
            "dark_neutral" => self.dark_neutral = rgb(value)?,
            "dark_dpad" => self.dark_dpad = rgb(value)?,
            "dark_button" => self.dark_button = rgb(value)?,
            "dark_connection" => self.dark_connection = rgb(value)?,
            "neutral" => self.neutral = rgb(value)?,
            "dpad" => self.dpad = rgb(value)?,
            "button" => self.button = rgb(value)?,
            "connection" => self.connection = rgb(value)?,
            _ => {
                let parts: Vec<_> = key.split('_').collect();
                let i = parts
                    .get(1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .ok_or("Unknown command setting")?;
                match parts[0] {
                    "ignore" if i < 4 => self.ignore_strength[i] = number(value)?,
                    "unified" if i < 4 => self.unified_colors[i] = rgb(value)?,
                    "darkunified" if i < 4 => self.dark_unified_colors[i] = rgb(value)?,
                    "triggerdead" if i < 2 => self.trigger_deadzone[i] = number(value)?,
                    "background" if i < 2 => self.background[i] = rgb(value)?,
                    "text" if i < 2 => self.text[i] = rgb(value)?,
                    "stick" | "trigger" | "color" | "darkcolor" => {
                        let j = parts
                            .get(2)
                            .and_then(|s| s.parse::<usize>().ok())
                            .ok_or("Invalid index")?;
                        match parts[0] {
                            "stick" if i < 2 && j < 3 => self.stick[i][j] = number(value)?,
                            "trigger" if i < 2 && j < 2 => self.trigger[i][j] = number(value)?,
                            "color" if i < 4 && j < 3 => self.colors[i][j] = rgb(value)?,
                            "darkcolor" if i < 4 && j < 3 => self.dark_colors[i][j] = rgb(value)?,
                            _ => return Err("Invalid index".into()),
                        }
                    }
                    _ => return Err("Unknown command setting".into()),
                }
            }
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=64).contains(&self.rows)
            || !(1..=64).contains(&self.overlay_rows)
            || !(50..=500).contains(&self.content_size)
            || [self.display_min_ms, self.record_min_ms]
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=60_000.).contains(v))
            || self.separator_alpha > 100
            || self.separator_style > 2
            || self.source > 2
            || self.theme > 2
            || self.bg_alpha > 100
            || self.fg_alpha > 100
            || !(320..=2048).contains(&self.width)
            || !(100..=2048).contains(&self.height)
        {
            return Err("Invalid command display setting".into());
        }
        if self.stick.iter().enumerate().any(|(i, a)| {
            a.iter().any(|v| !v.is_finite())
                || a[0] < 0.
                || a[0] >= 100.
                || (!self.ignore_strength[i] && (a[0] >= a[1] || a[1] >= a[2] || a[2] > 100.))
        }) || self.trigger.iter().any(|a| a[0] == 0 || a[0] >= a[1])
        {
            return Err("Thresholds must increase: dead zone < medium < full".into());
        }
        Ok(())
    }
    /// Controls which existing editor fields are relevant without erasing them.
    pub fn field_enabled(&self, key: &str) -> bool {
        let a = key.split('_').collect::<Vec<_>>();
        let i = a
            .get(1)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(usize::MAX);
        let level = a.get(2).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        match a[0] {
            "stick" if i < 2 && level > 0 => !self.ignore_strength[i],
            "trigger" if i < 2 => !self.ignore_strength[i + 2],
            "color" | "darkcolor" if i < 4 => !self.ignore_strength[i],
            _ => true,
        }
    }
    pub fn overlay_row_limit(&self) -> usize {
        if self.overlay_auto_rows {
            64
        } else {
            self.overlay_rows
        }
    }
    pub fn content_scale(&self) -> f32 {
        self.content_size as f32 / 100.
    }
    pub fn classification_eq(&self, b: &Self) -> bool {
        self.enabled == b.enabled
            && self.right == b.right
            && self.stick == b.stick
            && self.trigger == b.trigger
            && self.trigger_deadzone == b.trigger_deadzone
            && self.ignore_strength == b.ignore_strength
    }
    pub fn color(&self, color: u32, dark: bool, semantic: bool) -> u32 {
        if self.auto_colors && dark && !semantic {
            match color {
                0 => 0xffffff,
                0xffffff => 0,
                _ => color,
            }
        } else {
            color
        }
    }
    pub fn palette(&self, dark: bool) -> (u32, u32) {
        let i = if self.auto_colors { 0 } else { dark as usize };
        (
            self.color(self.background[i], dark, false),
            self.color(self.text[i], dark, false),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Normalised stick reading: eight directions plus a strength tier.
///
/// `direction` is 1–8 clockwise from up and 0 when centred; `strength` is 0
/// when inside the first threshold, 1..=3 for tiers, or 4 for ungraded active input. Keeping the tier rather
/// than the raw axis value is what makes the history read as "pushed" instead of
/// a stream of numbers.
pub struct Axis {
    pub direction: u8,
    pub strength: u8,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub sticks: [Axis; 2],
    pub triggers: [u8; 2],
    pub dpad: u8,
    pub buttons: u32,
}
pub const ARROWS: [&str; 9] = ["N", "↑", "↗", "→", "↘", "↓", "↙", "←", "↖"];
/// Collapse raw axis bytes into a direction and strength tier.
///
/// Both edges carry hysteresis so a stick resting near a boundary does not
/// produce a burst of one-frame events: a tier already reached is released 2
/// percentage points lower (`t - 2.`), and a direction stays put while the angle
/// is within 27.5° of the previous one's centre.
fn stick(x: u8, y: u8, old: Axis, threshold: [f32; 3]) -> Axis {
    let axis = |v: u8| {
        if v < 128 {
            (v as f32 - 128.) / 128.
        } else {
            (v as f32 - 128.) / 127.
        }
    };
    let (x, y) = (axis(x), -axis(y));
    let radius = (x * x + y * y).sqrt().min(1.) * 100.;
    if radius == 0. {
        return Axis::default();
    }
    let mut strength = 0;
    for (i, t) in threshold.iter().enumerate() {
        if radius
            >= if old.strength as usize > i {
                t - 2.
            } else {
                *t
            }
        {
            strength = i as u8 + 1;
        }
    }
    if strength == 0 {
        return Axis::default();
    }
    let angle = x.atan2(y).to_degrees().rem_euclid(360.);
    let mut direction = (((angle + 22.5) / 45.).floor() as u8) % 8 + 1;
    if old.direction > 0 {
        let center = (old.direction - 1) as f32 * 45.;
        let distance = (angle - center + 180.).rem_euclid(360.) - 180.;
        if distance.abs() < 27.5 {
            direction = old.direction;
        }
    }
    Axis {
        direction,
        strength,
    }
}
/// Map a trigger byte to 0–3, with the same 2-point release hysteresis as the
/// sticks. Any non-zero reading is already tier 1, so a light touch registers.
fn trigger(v: u8, old: u8, threshold: [u8; 2]) -> u8 {
    if v == 0 {
        return 0;
    }
    let mut s = 1;
    for (i, t) in threshold.iter().enumerate() {
        if v >= if old as usize > i + 1 {
            t.saturating_sub(2)
        } else {
            *t
        } {
            s = i as u8 + 2;
        }
    }
    s
}
fn command_stick(x: u8, y: u8, old: Axis, cfg: &Settings, i: usize) -> Axis {
    if !cfg.ignore_strength[i] {
        return stick(x, y, old, cfg.stick[i]);
    }
    let mut baseline = old;
    baseline.strength = u8::from(old.strength > 0);
    let mut axis = stick(x, y, baseline, [cfg.stick[i][0], 101., 102.]);
    if axis.strength > 0 {
        axis.strength = 4;
    }
    axis
}
fn command_trigger(v: u8, old: u8, cfg: &Settings, i: usize) -> u8 {
    if v <= cfg.trigger_deadzone[i] {
        0
    } else if cfg.ignore_strength[i + 2] {
        4
    } else {
        trigger(v, old, cfg.trigger[i])
    }
}
/// Reduce parsed input to the fields the history displays.
///
/// `session` and `sequence` in [`History`] are assigned by the caller, not here;
/// this only decides what counts as a change.
pub fn classify(p: ParsedState, old: State, cfg: &Settings) -> State {
    let mut buttons = (p.buttons0 as u32 & 0xf0)
        | ((p.buttons1 as u32 & !0x0c) << 8)
        | ((p.buttons2 as u32) << 16);
    // Edge reserved bits aren't inputs; mute/function/paddle bits are preserved.
    buttons &= 0x00f7_fff0;
    State {
        sticks: [
            command_stick(p.lx, p.ly, old.sticks[0], cfg, 0),
            if cfg.right {
                command_stick(p.rx, p.ry, old.sticks[1], cfg, 1)
            } else {
                Axis::default()
            },
        ],
        triggers: [
            command_trigger(p.l2, old.triggers[0], cfg, 0),
            command_trigger(p.r2, old.triggers[1], cfg, 1),
        ],
        dpad: crate::monitor::dpad_bits(p.buttons0),
        buttons,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Input,
    Connected,
    Disconnected,
    Gap,
    Settings,
    Clear,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Gap => "gap",
            Self::Settings => "settings",
            Self::Clear => "clear",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Event {
    pub session: u64,
    pub sequence: u64,
    pub time: u64,
    pub delta: Option<u64>,
    pub kind: Kind,
    pub state: State,
    pub raw: [u8; 9],
}
/// Bounded dwell filter: one undecided state and one last published state.
/// Boundaries always pass; short excursions returning to the prior state merge.
#[derive(Default)]
pub struct DurationFilter {
    /// Candidate state and the dwell it must reach before it is published.
    /// Replaced outright when a different state arrives, so two short-lived
    /// states in a row are both dropped rather than summed.
    pending: Option<(Event, u64)>,
    /// Last published event, used to merge returns to the previous state and to
    /// compute each published event's delta.
    last: Option<Event>,
}
impl DurationFilter {
    fn publish(&mut self, mut event: Event) -> Event {
        event.delta = self
            .last
            .as_ref()
            .filter(|last| last.session == event.session)
            .map(|last| event.time.saturating_sub(last.time));
        self.last = Some(event.clone());
        event
    }
    pub fn advance(&mut self, now: u64) -> Option<Event> {
        if self
            .pending
            .as_ref()
            .is_some_and(|(e, threshold)| now.saturating_sub(e.time) >= *threshold)
        {
            let (event, _) = self.pending.take().unwrap();
            return Some(self.publish(event));
        }
        None
    }
    pub fn receive(&mut self, event: Event, minimum_ms: f64) -> [Option<Event>; 2] {
        if self
            .last
            .as_ref()
            .is_some_and(|last| last.session != event.session)
        {
            *self = Self::default();
        }
        let completed = self.advance(event.time);
        self.pending = None;
        if event.kind != Kind::Input {
            let boundary = self.publish(event);
            return [completed, Some(boundary)];
        }
        if self
            .last
            .as_ref()
            .is_some_and(|last| last.kind == Kind::Input && last.state == event.state)
        {
            return [completed, None];
        }
        self.pending = Some((event, (minimum_ms * 1_000_000.).round() as u64));
        [
            completed,
            self.advance(self.pending.as_ref().unwrap().0.time),
        ]
    }
}

#[derive(Clone, Copy)]
pub enum Message {
    Input(ParsedState),
    Connection(bool),
}
/// Rolling window of input changes for the views to render.
///
/// `session` increments on every start/stop, and `sequence` numbers events
/// within one session. A consumer that sees a gap in either knows events were
/// lost rather than merely scrolled away, which is what surfaces as 输入记录缺口.
pub struct History {
    pub settings: Settings,
    pub events: VecDeque<Event>,
    pub revision: u64,
    pub latest: Option<Event>,
    pub session: u64,
    sequence: u64,
    previous: Option<State>,
    time: Option<u64>,
    connected: bool,
    display: DurationFilter,
}
impl Default for History {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            events: VecDeque::new(),
            revision: 0,
            latest: None,
            session: 0,
            sequence: 0,
            previous: None,
            time: None,
            connected: false,
            display: DurationFilter::default(),
        }
    }
}
impl History {
    pub fn start(&mut self, now: u64, connected: bool, capacity: usize) {
        self.session += 1;
        self.sequence = 0;
        self.events.clear();
        self.display = DurationFilter::default();
        self.latest = None;
        self.previous = None;
        self.time = None;
        self.connected = connected;
        if !self.settings.enabled {
            self.revision += 1;
            return;
        }
        self.push(
            now,
            if connected {
                Kind::Connected
            } else {
                Kind::Disconnected
            },
            State::default(),
            [0; 9],
            capacity,
        );
    }
    fn push(
        &mut self,
        time: u64,
        kind: Kind,
        state: State,
        raw: [u8; 9],
        capacity: usize,
    ) -> Event {
        let time = time.max(self.time.unwrap_or(time));
        self.sequence += 1;
        let e = Event {
            session: self.session,
            sequence: self.sequence,
            time,
            delta: self.time.map(|t| time.saturating_sub(t)),
            kind,
            state,
            raw,
        };
        self.time = Some(time);
        self.latest = Some(e.clone());
        for visible in self
            .display
            .receive(e.clone(), self.settings.display_min_ms)
            .into_iter()
            .flatten()
        {
            self.events.push_front(visible);
        }
        self.trim(capacity);
        self.revision = self.revision.wrapping_add(1);
        e
    }
    pub fn visible_latest(&self) -> Option<&Event> {
        self.display.last.as_ref()
    }
    pub fn advance_display(&mut self, now: u64, capacity: usize) {
        if let Some(event) = self.display.advance(now) {
            self.events.push_front(event);
            self.trim(capacity);
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub fn trim(&mut self, capacity: usize) {
        let previous = self.events.len();
        self.events.truncate(capacity.min(64));
        if previous != self.events.len() {
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub fn receive(&mut self, time: u64, message: Message, capacity: usize) -> Option<Event> {
        if !self.settings.enabled {
            return None;
        }
        match message {
            Message::Connection(c) => {
                if c == self.connected {
                    return None;
                }
                self.connected = c;
                self.previous = None;
                Some(self.push(
                    time,
                    if c {
                        Kind::Connected
                    } else {
                        Kind::Disconnected
                    },
                    State::default(),
                    [0; 9],
                    capacity,
                ))
            }
            Message::Input(p) => {
                if !self.connected {
                    return None;
                }
                let s = classify(p, self.previous.unwrap_or_default(), &self.settings);
                if self.previous == Some(s) {
                    return None;
                }
                self.previous = Some(s);
                Some(self.push(
                    time,
                    Kind::Input,
                    s,
                    [
                        p.lx, p.ly, p.rx, p.ry, p.l2, p.r2, p.buttons0, p.buttons1, p.buttons2,
                    ],
                    capacity,
                ))
            }
        }
    }
    pub fn boundary(&mut self, time: u64, kind: Kind, capacity: usize) -> Event {
        self.previous = None;
        self.push(time, kind, State::default(), [0; 9], capacity)
    }
    pub fn clear(&mut self, time: u64) -> Event {
        let e = self.boundary(time, Kind::Clear, 0);
        self.latest = None;
        self.display = DurationFilter::default();
        self.time = None;
        self.revision += 1;
        e
    }
}
/// Shared input-history state.
///
/// Threading: the bridge's input thread calls [`Commands::submit`], which only
/// pushes onto a bounded lock-free queue. A worker drains it into [`History`]
/// under a mutex; views take the same mutex with `try_lock` and render whatever
/// is there. Nothing on the input path blocks, and a full queue is counted in
/// `dropped` rather than waited on.
pub struct Commands {
    queue: ArrayQueue<(u64, Message)>,
    /// Messages lost because the queue was full. Non-zero means the history has
    /// gaps that are visible to the user as 输入记录缺口.
    pub dropped: AtomicU64,
    pub enabled: AtomicBool,
    connection_state: AtomicBool,
    running: AtomicBool,
    /// Row counts currently visible in each view, capped at 64. History keeps
    /// the largest of the two and trims the rest, so shrinking a window drops
    /// events that a later resize cannot bring back.
    pub main_capacity: AtomicUsize,
    pub overlay_capacity: AtomicUsize,
    pub history: Mutex<History>,
    pub recording: crate::command_recording::Recording,
}
impl Default for Commands {
    fn default() -> Self {
        Self {
            queue: ArrayQueue::new(4096),
            dropped: AtomicU64::new(0),
            enabled: AtomicBool::new(true),
            connection_state: AtomicBool::new(false),
            running: AtomicBool::new(false),
            main_capacity: AtomicUsize::new(0),
            overlay_capacity: AtomicUsize::new(0),
            history: Mutex::new(History::default()),
            recording: Default::default(),
        }
    }
}
impl Commands {
    pub fn visible_capacity(&self, overlay: bool, capacity: usize) {
        if overlay {
            self.overlay_capacity.store(capacity.min(64), Relaxed);
        } else {
            self.main_capacity.store(capacity.min(64), Relaxed);
        }
        // Views may be resized or hidden after the collection worker has stopped.
        if let Ok(mut history) = self.history.try_lock() {
            history.trim(self.capacity());
        }
    }
    pub fn allocated_bytes(&self) -> usize {
        4096 * (std::mem::size_of::<(u64, Message)>() + std::mem::size_of::<usize>())
            + 64 * std::mem::size_of::<Event>()
            + self.recording.allocated_bytes()
    }
    pub fn capacity(&self) -> usize {
        self.main_capacity
            .load(Relaxed)
            .max(self.overlay_capacity.load(Relaxed))
            .min(64)
    }
    pub fn submit(&self, time: u64, message: Message) {
        if let Message::Connection(c) = message {
            self.connection_state.store(c, Release);
        }
        if self.running.load(Acquire)
            && self.enabled.load(Relaxed)
            && self.queue.push((time, message)).is_err()
        {
            self.dropped.fetch_add(1, Relaxed);
        }
    }
    pub fn pause(&self) {
        self.running.store(false, Release);
        if let Ok(mut h) = self.history.lock() {
            h.display.pending = None;
        }
    }
    pub fn start(&self, time: u64, connected: bool) {
        self.running.store(true, Release);
        self.connection_state.store(connected, Release);
        while self.queue.pop().is_some() {}
        self.dropped.store(0, Relaxed);
        if let Ok(mut h) = self.history.lock() {
            h.start(time, connected, self.capacity());
        }
    }
    pub fn work(&self, now: u64) {
        if let Ok(mut h) = self.history.lock() {
            if self.dropped.swap(0, AcqRel) > 0 {
                h.connected = self.connection_state.load(Acquire);
                let e = h.boundary(now, Kind::Gap, self.capacity());
                self.recording.submit(&e, &h.settings);
                while self.queue.pop().is_some() {}
                return;
            }
            for _ in 0..4096 {
                let Some((t, m)) = self.queue.pop() else {
                    break;
                };
                if let Some(e) = h.receive(t, m, self.capacity()) {
                    self.recording.submit(&e, &h.settings);
                }
            }
            if self.running.load(Acquire) {
                h.advance_display(now, self.capacity());
                self.recording.advance(now);
            }
            h.trim(self.capacity());
        }
    }
    pub fn configure(&self, settings: Settings, now: u64) {
        self.work(now);
        if let Ok(mut h) = self.history.lock() {
            let changed = !h.settings.classification_eq(&settings)
                || h.settings.display_min_ms != settings.display_min_ms
                || h.settings.record_min_ms != settings.record_min_ms;
            h.settings = settings;
            self.enabled.store(h.settings.enabled, Release);
            if changed && self.running.load(Acquire) {
                h.connected = self.connection_state.load(Acquire);
                let e = h.boundary(now, Kind::Settings, self.capacity());
                self.recording.submit(&e, &h.settings);
            }
            h.revision += 1;
        }
    }
    pub fn clear(&self, now: u64) {
        if let Ok(mut h) = self.history.lock() {
            let e = h.clear(now);
            self.recording.submit(&e, &h.settings);
        }
    }
}
/// Visible events are newest first. A row lasts until the next newer event;
/// only the current row advances with the session clock. CSV deltas stay unchanged.
pub fn state_duration(events: &[Event], index: usize, now: u64) -> u64 {
    let end = if index == 0 {
        now
    } else {
        events[index - 1].time
    };
    end.saturating_sub(events[index].time)
}

/// Format a duration for display, trading precision for width as it grows:
/// whole milliseconds under a second, then two decimals, then one, then capped
/// at `99+ s` so a long-held state cannot push the row layout around.
pub fn duration(ns: u64) -> String {
    let ms = ns / 1_000_000;
    if ms >= 99_000 {
        "99+ s".into()
    } else if ms >= 10_000 {
        format!("{}.{:01} s", ms / 1000, ms % 1000 / 100)
    } else if ms >= 1000 {
        format!("{}.{:02} s", ms / 1000, ms % 1000 / 10)
    } else {
        format!("{ms} ms")
    }
}
/// Flatten a state into the one-line description stored in CSV rows and used by
/// tests: sticks, d-pad, face buttons, shoulders and specials, in that order.
pub fn combination(s: State, right: bool) -> String {
    let mut parts = vec![format!("LS {}", ARROWS[s.sticks[0].direction as usize])];
    if right {
        parts.push(format!("RS {}", ARROWS[s.sticks[1].direction as usize]));
    }
    for (i, a) in ["↑", "→", "↓", "←"].iter().enumerate() {
        if s.dpad & (1 << i) != 0 {
            parts.push(format!("DP {a}"));
        }
    }
    for (i, label) in ["□", "×", "○", "△"].iter().enumerate() {
        if s.buttons & (1 << (i + 4)) != 0 {
            parts.push(label.to_string());
        }
    }
    for (bit, label) in [
        (8, "L1"),
        (9, "R1"),
        (12, "Create"),
        (13, "Options"),
        (14, "L3"),
        (15, "R3"),
        (16, "PS"),
        (17, "Touchpad"),
        (18, "Mute"),
        (20, "FnL"),
        (21, "FnR"),
        (22, "P1"),
        (23, "P2"),
    ] {
        if s.buttons & (1 << bit) != 0 {
            parts.push(label.into());
        }
    }
    for (i, label) in ["L2", "R2"].iter().enumerate() {
        if s.triggers[i] > 0 {
            parts.push(if s.triggers[i] == 4 {
                label.to_string()
            } else {
                format!("{label}:{}", s.triggers[i])
            });
        }
    }
    parts.join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(time: u64, kind: Kind, pressed: bool) -> Event {
        let mut state = State::default();
        state.buttons = if pressed { 16 } else { 0 };
        Event {
            session: 1,
            sequence: time + 1,
            time,
            delta: None,
            kind,
            state,
            raw: [0; 9],
        }
    }
    #[test]
    fn ungraded_inputs_keep_dead_zones_and_direction_without_pressure_events() {
        let mut cfg = Settings::default();
        assert_eq!(cfg.ignore_strength, [false; 4]);
        assert_eq!(cfg.trigger_deadzone, [0; 2]);
        cfg.ignore_strength = [true; 4];
        cfg.right = true;
        cfg.trigger_deadzone = [10, 20];
        let mut p = p();
        p.lx = 150;
        p.rx = 150;
        p.l2 = 11;
        p.r2 = 21;
        let initial = classify(p, State::default(), &cfg);
        assert_eq!(
            initial.sticks[0],
            Axis {
                direction: 3,
                strength: 4
            }
        );
        assert_eq!(initial.triggers, [4; 2]);
        p.lx = 255;
        p.rx = 255;
        p.l2 = 255;
        p.r2 = 255;
        assert_eq!(classify(p, initial, &cfg), initial);
        p.ly = 0;
        assert_ne!(
            classify(p, initial, &cfg).sticks[0].direction,
            initial.sticks[0].direction
        );
        p = crate::monitor::neutral();
        p.l2 = 10;
        p.r2 = 20;
        assert_eq!(classify(p, initial, &cfg), State::default());
        cfg.ignore_strength[1] = false;
        p.rx = 255;
        assert_eq!(classify(p, State::default(), &cfg).sticks[1].strength, 3);
        assert!(cfg.field_enabled("stick_0_0"));
        assert!(!cfg.field_enabled("stick_0_1"));
        assert!(!cfg.field_enabled("trigger_0_1"));
        assert!(cfg.field_enabled("triggerdead_0"));
        assert!(!cfg.field_enabled("color_0_0"));
        assert!(cfg.field_enabled("unified_0"));
    }
    #[test]
    fn legacy_max_colors_seed_new_colors_and_explicit_choices_round_trip() {
        let cfg = crate::config::Config::parse(
            "command_color_0_2=#123456\ncommand_darkcolor_0_2=#ABCDEF\n",
        )
        .unwrap();
        assert_eq!(cfg.commands.unified_colors[0], 0x123456);
        assert_eq!(cfg.commands.dark_unified_colors[0], 0xabcdef);
        let cfg =
            crate::config::Config::parse("command_color_0_2=#123456\ncommand_unified_0=#112233\n")
                .unwrap();
        assert_eq!(cfg.commands.unified_colors[0], 0x112233);
        let mut settings = cfg.commands;
        settings.ignore_strength[3] = true;
        settings.trigger_deadzone[1] = 255;
        let mut restored = Settings::default();
        for (k, v) in settings.fields() {
            restored.set(&k, &v).unwrap();
        }
        assert_eq!(restored, settings);
    }
    #[test]
    fn separators_and_auto_rows_round_trip_without_changing_inputs() {
        let defaults = Settings::default();
        assert!(defaults.overlay_auto_rows && defaults.separators);
        assert_eq!(defaults.overlay_row_limit(), 64);
        let mut cfg = defaults.clone();
        cfg.overlay_auto_rows = false;
        cfg.overlay_rows = 7;
        cfg.separator_alpha = 35;
        cfg.separator_style = 2;
        cfg.separator = 0x123456;
        cfg.dark_separator = 0xabcdef;
        assert_eq!(cfg.overlay_row_limit(), 7);
        assert!(cfg.classification_eq(&defaults));
        let mut restored = Settings::default();
        for (key, value) in cfg.fields() {
            restored.set(&key, &value).unwrap();
        }
        assert_eq!(cfg, restored);
        cfg.separator_style = 3;
        assert!(cfg.validate().is_err());
        cfg.separator_style = 0;
        cfg.separator_alpha = 101;
        assert!(cfg.validate().is_err());
    }
    #[test]
    fn dwell_filter_has_exact_boundary_and_retains_original_timestamps() {
        let mut filter = DurationFilter::default();
        assert!(filter
            .receive(event(100, Kind::Input, true), 16.67)
            .iter()
            .all(Option::is_none));
        assert!(filter.advance(16_670_099).is_none());
        let held = filter.advance(16_670_100).unwrap();
        assert_eq!(held.time, 100);
        assert!(filter.advance(99_000_000).is_none());
        let result = filter.receive(event(99_000_000, Kind::Disconnected, false), 16.67);
        assert_eq!(result[1].as_ref().unwrap().kind, Kind::Disconnected);
    }
    #[test]
    fn short_excursions_merge_and_boundaries_always_pass() {
        let mut filter = DurationFilter::default();
        filter.receive(event(0, Kind::Input, false), 16.67);
        filter.advance(20_000_000).unwrap();
        filter.receive(event(100_000_000, Kind::Input, true), 16.67);
        assert!(filter
            .receive(event(110_000_000, Kind::Input, false), 16.67)
            .iter()
            .all(Option::is_none));
        assert!(filter.advance(200_000_000).is_none());
        assert_eq!(filter.last.as_ref().unwrap().time, 0);
        for kind in [
            Kind::Gap,
            Kind::Settings,
            Kind::Clear,
            Kind::Disconnected,
            Kind::Connected,
        ] {
            let rows = filter.receive(event(300_000_000, kind, false), 60_000.);
            assert_eq!(rows[1].as_ref().unwrap().kind, kind);
        }
    }
    #[test]
    fn display_filter_defaults_and_invalid_thresholds() {
        let cfg = Settings::default();
        assert_eq!((cfg.display_min_ms, cfg.record_min_ms), (16.67, 0.));
        for value in [0., 0.001, 16.67, 60_000.] {
            let mut configured = cfg.clone();
            configured.display_min_ms = value;
            configured.record_min_ms = value;
            configured.validate().unwrap();
            let mut restored = Settings::default();
            for (key, value) in configured.fields() {
                restored.set(&key, &value).unwrap();
            }
            assert_eq!(restored, configured);
        }
        for value in [-1., f64::NAN, f64::INFINITY, 60_001.] {
            let mut configured = cfg.clone();
            configured.display_min_ms = value;
            assert!(configured.validate().is_err());
        }
    }
    #[test]
    fn visible_pending_state_is_bounded_and_not_revived_after_pause() {
        let c = Commands::default();
        c.visible_capacity(false, 8);
        c.start(0, true);
        let mut pressed = p();
        pressed.buttons0 |= 16;
        c.submit(1, Message::Input(pressed));
        c.work(10);
        assert_eq!(
            c.history.lock().unwrap().visible_latest().unwrap().kind,
            Kind::Connected
        );
        c.pause();
        c.work(100_000_000);
        assert_eq!(
            c.history.lock().unwrap().visible_latest().unwrap().kind,
            Kind::Connected
        );
        c.start(200_000_000, true);
        c.submit(200_000_001, Message::Input(pressed));
        c.work(216_670_001);
        assert_eq!(
            c.history.lock().unwrap().visible_latest().unwrap().time,
            200_000_001
        );
        c.visible_capacity(false, 0);
        assert!(c.history.lock().unwrap().events.is_empty());
    }
    #[test]
    fn content_size_defaults_validates_and_round_trips_without_reclassification() {
        let initial = Settings::default();
        assert_eq!(initial.content_size, 100);
        for size in [50, 100, 200, 300, 500] {
            let mut cfg = initial.clone();
            cfg.set("content_size", &size.to_string()).unwrap();
            cfg.validate().unwrap();
            assert!(cfg.classification_eq(&initial));
            let mut restored = Settings::default();
            for (key, value) in cfg.fields() {
                restored.set(&key, &value).unwrap();
            }
            assert_eq!(restored, cfg);
        }
        for size in [0, 49, 501] {
            let mut cfg = initial.clone();
            cfg.content_size = size;
            assert!(cfg.validate().is_err());
        }
    }
    #[test]
    fn manual_theme_palettes_are_independent_and_do_not_invert_colors() {
        let mut cfg = Settings {
            auto_colors: false,
            ..Settings::default()
        };
        assert_eq!(cfg.palette(false), (0xffffff, 0));
        assert_eq!(cfg.palette(true), (0, 0xffffff));
        assert_eq!(cfg.button, 0);
        assert_eq!(cfg.dark_button, 0xffffff);
        cfg.set("darkcolor_0_2", "#123456").unwrap();
        cfg.set("dark_button", "#ABCDEF").unwrap();
        assert_eq!(cfg.colors[0][2], 0xE54747);
        assert_eq!(cfg.dark_colors[0][2], 0x123456);
        assert_eq!(cfg.color(cfg.dark_button, true, false), 0xABCDEF);
        let mut restored = Settings::default();
        for (k, v) in cfg.fields() {
            restored.set(&k, &v).unwrap();
        }
        assert_eq!(restored, cfg);
    }
    #[test]
    fn changing_classification_while_paused_does_not_advance_history() {
        let c = Commands::default();
        c.start(10, true);
        c.pause();
        let previous = c.history.lock().unwrap().latest.clone().unwrap();
        let mut cfg = Settings::default();
        cfg.right = true;
        c.configure(cfg, 1000);
        c.submit(2000, Message::Input(p()));
        c.work(2000);
        let h = c.history.lock().unwrap();
        let current = h.latest.as_ref().unwrap();
        assert_eq!(
            (current.sequence, current.time),
            (previous.sequence, previous.time)
        );
        assert!(h.settings.right);
    }
    #[test]
    fn shrinking_a_paused_view_discards_rows_permanently() {
        let c = Commands::default();
        c.history.lock().unwrap().settings.display_min_ms = 0.;
        c.visible_capacity(false, 8);
        c.start(0, true);
        for time in 1..8 {
            let mut p = p();
            p.buttons0 |= (time % 2) as u8 * 16;
            c.submit(time, Message::Input(p));
        }
        c.work(10);
        c.visible_capacity(false, 2);
        assert_eq!(c.history.lock().unwrap().events.len(), 2);
        c.visible_capacity(false, 8);
        assert_eq!(c.history.lock().unwrap().events.len(), 2);
        c.visible_capacity(false, 0);
        let h = c.history.lock().unwrap();
        assert!(h.events.is_empty());
        assert!(h.latest.is_some());
    }
    #[test]
    fn overflow_reestablishes_connection_and_input_baseline() {
        let c = Commands::default();
        c.main_capacity.store(8, Relaxed);
        c.start(0, false);
        for i in 0..4096 {
            c.submit(i, Message::Input(p()));
        }
        c.submit(4096, Message::Connection(true));
        c.work(5000);
        assert_eq!(
            c.history.lock().unwrap().latest.as_ref().unwrap().kind,
            Kind::Gap
        );
        c.submit(6000, Message::Input(p()));
        c.work(7000);
        assert_eq!(
            c.history.lock().unwrap().latest.as_ref().unwrap().kind,
            Kind::Input
        );
    }
    #[test]
    fn input_pipeline_retains_a_press_released_before_the_ui_refreshes() {
        let m = crate::monitor::Monitor::new();
        m.commands.history.lock().unwrap().settings.display_min_ms = 0.;
        m.commands.main_capacity.store(8, Relaxed);
        m.start(20).unwrap();
        m.connection(true);
        let mut pressed = p();
        pressed.buttons0 |= 16;
        m.input(pressed);
        m.input(p());
        m.stop();
        let h = m.commands.history.lock().unwrap();
        assert!(h
            .events
            .iter()
            .any(|e| e.kind == Kind::Input && e.state.buttons & 16 != 0));
        assert_eq!(h.latest.as_ref().unwrap().state.buttons & 16, 0);
    }
    #[test]
    fn geometry_changes_do_not_create_classification_events() {
        let c = Commands::default();
        c.start(0, true);
        let sequence = c.history.lock().unwrap().latest.as_ref().unwrap().sequence;
        let mut cfg = Settings::default();
        assert_eq!(cfg.source, 2);
        cfg.width = 640;
        cfg.source = 0;
        cfg.theme = 2;
        c.configure(cfg.clone(), 10);
        assert_eq!(
            c.history.lock().unwrap().latest.as_ref().unwrap().sequence,
            sequence
        );
        cfg.right = true;
        c.configure(cfg, 20);
        assert_eq!(
            c.history.lock().unwrap().latest.as_ref().unwrap().kind,
            Kind::Settings
        );
    }
    #[test]
    fn both_sticks_and_all_trigger_levels_are_distinct() {
        let mut cfg = Settings::default();
        cfg.right = true;
        for (v, level) in [(1, 1), (128, 2), (250, 3)] {
            let mut p = p();
            p.l2 = v;
            p.r2 = v;
            p.rx = 255;
            let s = classify(p, State::default(), &cfg);
            assert_eq!(s.triggers, [level; 2]);
            assert_eq!(s.sticks[1].direction, 3);
        }
    }
    fn p() -> ParsedState {
        crate::monitor::neutral()
    }
    #[test]
    fn clock_boundaries() {
        for (ms, want) in [
            (0, "0 ms"),
            (999, "999 ms"),
            (1000, "1.00 s"),
            (9999, "9.99 s"),
            (10000, "10.0 s"),
            (98999, "98.9 s"),
            (99000, "99+ s"),
            (9999999, "99+ s"),
        ] {
            assert_eq!(duration(ms * 1_000_000), want);
        }
    }
    #[test]
    fn rows_measure_their_own_state_duration() {
        let ms = 1_000_000;
        let mut h = History::default();
        h.settings.display_min_ms = 0.;
        h.start(0, true, 8);
        h.receive(0, Message::Input(p()), 8).unwrap();
        let mut pressed = p();
        pressed.buttons0 |= 0x10;
        h.receive(58_700 * ms, Message::Input(pressed), 8).unwrap();
        h.receive(58_811 * ms, Message::Input(p()), 8).unwrap();
        let events = h.events.iter().cloned().collect::<Vec<_>>();
        for now in [67_981 * ms, 68_981 * ms] {
            assert_eq!(state_duration(&events, 0, now), now - 58_811 * ms);
            assert_eq!(duration(state_duration(&events, 1, now)), "111 ms");
            assert_eq!(duration(state_duration(&events, 2, now)), "58.7 s");
        }
        // The file continues to retain arrival-to-arrival intervals.
        assert_eq!(events[0].delta, Some(111 * ms));
        assert_eq!(events[1].delta, Some(58_700 * ms));
        h.receive(70_000 * ms, Message::Connection(false), 8)
            .unwrap();
        h.receive(180_000 * ms, Message::Connection(true), 8)
            .unwrap();
        let events = h.events.iter().cloned().collect::<Vec<_>>();
        assert_eq!(duration(state_duration(&events, 1, 181_000 * ms)), "99+ s");
        assert_eq!(duration(state_duration(&events, 0, 181_000 * ms)), "1.00 s");
    }

    #[test]
    fn transitions_and_short_press() {
        let mut h = History::default();
        h.settings.display_min_ms = 0.;
        h.start(0, true, 8);
        assert!(h.receive(1, Message::Connection(true), 8).is_none());
        h.receive(2, Message::Input(p()), 8).unwrap();
        let mut p = p();
        p.buttons0 |= 0x10;
        h.receive(3, Message::Input(p), 8).unwrap();
        p.buttons0 = 8;
        h.receive(4, Message::Input(p), 8).unwrap();
        assert!(h.receive(5, Message::Input(p), 8).is_none());
        assert_eq!(h.events.len(), 4);
        h.receive(6, Message::Connection(false), 8);
        assert!(h.receive(7, Message::Input(p), 8).is_none());
        let e = h
            .receive(100_000_000_006, Message::Connection(true), 8)
            .unwrap();
        assert_eq!(e.delta, Some(100_000_000_000));
        assert!(h.receive(100_000_000_007, Message::Input(p), 8).is_some());
    }
    #[test]
    fn directions_and_touch_ignored() {
        let cfg = Settings::default();
        let mut p = p();
        let base = classify(p, State::default(), &cfg);
        p.touch1.active = true;
        p.touch1.x = 99;
        assert_eq!(base, classify(p, base, &cfg));
        p.rx = 255;
        assert_eq!(base, classify(p, base, &cfg));
        p.buttons0 = 1;
        assert_eq!(classify(p, base, &cfg).dpad, 3);
        for (x, y, d) in [
            (128, 0, 1),
            (255, 0, 2),
            (255, 128, 3),
            (255, 255, 4),
            (128, 255, 5),
            (0, 255, 6),
            (0, 128, 7),
            (0, 0, 8),
        ] {
            assert_eq!(stick(x, y, Axis::default(), cfg.stick[0]).direction, d);
        }
    }
    #[test]
    fn bounded_and_hidden() {
        let mut h = History::default();
        h.settings.display_min_ms = 0.;
        h.start(0, true, 3);
        for t in 1..100 {
            let mut p = p();
            p.buttons0 |= ((t % 2) as u8) * 16;
            h.receive(t, Message::Input(p), 3);
        }
        assert_eq!(h.events.len(), 3);
        h.trim(0);
        assert!(h.events.is_empty());
        assert!(h.latest.is_some());
        h.trim(64);
        assert!(h.events.is_empty());
        h.clear(100);
        assert!(h.time.is_none());
    }
    #[test]
    fn trigger_and_hysteresis() {
        assert_eq!(trigger(1, 0, [128, 250]), 1);
        assert_eq!(trigger(128, 1, [128, 250]), 2);
        assert_eq!(trigger(127, 2, [128, 250]), 2);
        assert_eq!(trigger(125, 2, [128, 250]), 1);
        assert_eq!(trigger(0, 3, [128, 250]), 0);
        let a = stick(128, 0, Axis::default(), [12., 51., 98.]);
        assert_eq!(a.strength, 3);
    }
    #[test]
    fn settings_roundtrip_and_colors() {
        let s = Settings::default();
        let mut t = s.clone();
        for (k, v) in s.fields() {
            t.set(&k, &v).unwrap();
        }
        assert_eq!(s, t);
        assert!(t.validate().is_ok());
        let before = t.clone();
        for _ in 0..20 {
            assert_eq!(t.palette(true), (0, 0xffffff));
            assert_eq!(t.palette(false), (0xffffff, 0));
        }
        assert_eq!(t, before);
        assert_eq!(t.color(0, true, true), 0);
    }
}
