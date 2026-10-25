//! Battery telemetry is independent of capture queues and audio resets.
use std::{
    sync::atomic::{
        AtomicBool, AtomicU64,
        Ordering::{AcqRel, Acquire, Release},
    },
    time::{Duration, Instant},
};

const VALID: u64 = 1 << 8;
const CONNECTED: u64 = 1 << 9;
const TIME_SHIFT: u32 = 10;
pub const DISPLAY_INTERVAL: Duration = Duration::from_secs(10);
pub const STALE_AFTER: Duration = Duration::from_secs(30);

pub struct Telemetry {
    origin: Instant,
    epoch: AtomicU64,
    value: AtomicU64,
    enabled: AtomicBool,
}
impl Default for Telemetry {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            epoch: AtomicU64::new(0),
            value: AtomicU64::new(0),
            enabled: AtomicBool::new(true),
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Snapshot {
    pub session: u64,
    pub connected: bool,
    pub raw: Option<u8>,
    pub seen: Duration,
}
impl Telemetry {
    // Called before starting a worker, after its predecessor has been joined.
    pub fn resume(&self) {
        self.transition(false);
        self.enabled.store(true, Release);
    }
    // UI cancellation masks readings immediately, even if device cleanup is slow.
    pub fn suspend(&self) {
        self.enabled.store(false, Release);
    }
    pub fn now(&self) -> Duration {
        self.origin.elapsed()
    }
    // Lifecycle changes and reports have a single writer: the bridge worker.
    fn transition(&self, connected: bool) {
        self.epoch.fetch_add(1, AcqRel);
        self.value
            .store(if connected { CONNECTED } else { 0 }, Release);
        self.epoch.fetch_add(1, Release);
    }
    pub fn connect(&self) {
        self.transition(true);
    }
    pub fn disconnect(&self) {
        self.transition(false);
    }
    pub fn observe(&self, raw: u8) {
        self.observe_at(raw, self.now());
    }
    fn observe_at(&self, raw: u8, now: Duration) {
        if !self.enabled.load(Acquire) || self.value.load(Acquire) & CONNECTED == 0 {
            return;
        }
        let millis = now.as_millis().min((u64::MAX >> TIME_SHIFT) as u128) as u64;
        self.value.store(
            (millis << TIME_SHIFT) | CONNECTED | VALID | raw as u64,
            Release,
        );
    }
    pub fn snapshot(&self) -> Snapshot {
        if !self.enabled.load(Acquire) {
            return Snapshot::default();
        }
        for _ in 0..3 {
            let session = self.epoch.load(Acquire);
            if session & 1 != 0 {
                continue;
            }
            let value = self.value.load(Acquire);
            if self.epoch.load(Acquire) == session && self.enabled.load(Acquire) {
                return Snapshot {
                    session,
                    connected: value & CONNECTED != 0,
                    raw: (value & VALID != 0).then_some(value as u8),
                    seen: Duration::from_millis(value >> TIME_SHIFT),
                };
            }
        }
        Snapshot::default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Disconnected,
    Awaiting,
    Discharging,
    Charging,
    Full,
    Fault(u8),
    Unknown,
    Expired,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub percent: Option<u8>,
    pub status: Status,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Green,
    Yellow,
    Red,
    Unknown,
}
impl View {
    pub fn band(self) -> Band {
        match self.percent {
            Some(0..=20) => Band::Red,
            Some(21..=50) => Band::Yellow,
            Some(_) => Band::Green,
            None => Band::Unknown,
        }
    }
    pub fn number(self) -> String {
        self.percent
            .map_or_else(|| "--%".into(), |p| format!("{p}%"))
    }
    pub fn caption(self) -> &'static str {
        match self.status {
            Status::Disconnected => crate::tr!(battery_disconnected),
            Status::Awaiting => crate::tr!(battery_awaiting),
            Status::Charging => crate::tr!(battery_charging),
            Status::Full => crate::tr!(battery_full),
            Status::Fault(_) => crate::tr!(battery_fault),
            Status::Unknown => crate::tr!(battery_unknown),
            Status::Expired => crate::tr!(battery_expired),
            Status::Discharging => crate::tr!(battery_discharging),
        }
    }
    pub fn tooltip(self) -> String {
        match (self.status, self.percent) {
            (Status::Fault(code), _) => {
                crate::tr!(battery_tooltip_fault, code = format!("0x{code:X}"))
            }
            (Status::Charging, Some(percent)) => {
                crate::tr!(battery_tooltip_charging, percent = percent)
            }
            (_, Some(percent)) => crate::tr!(battery_tooltip_level, percent = percent),
            _ => self.caption().into(),
        }
    }
}
impl Snapshot {
    pub fn view(self, now: Duration) -> View {
        let status = if !self.connected {
            Status::Disconnected
        } else if self.raw.is_none() {
            Status::Awaiting
        } else if now.saturating_sub(self.seen) >= STALE_AFTER {
            Status::Expired
        } else {
            match self.raw.unwrap() >> 4 {
                0 => Status::Discharging,
                1 => Status::Charging,
                2 => Status::Full,
                code @ (0xa | 0xb | 0xf) => Status::Fault(code),
                _ => Status::Unknown,
            }
        };
        let percent = match status {
            Status::Full => Some(100),
            Status::Discharging | Status::Charging => self.raw.and_then(|raw| {
                let level = raw & 15;
                (level <= 10).then_some((level * 10 + 5).min(100))
            }),
            _ => None,
        };
        View {
            percent,
            status: if percent.is_none() && matches!(status, Status::Discharging | Status::Charging)
            {
                Status::Unknown
            } else {
                status
            },
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Display {
    pub view: View,
    session: u64,
    next: Duration,
}
impl Display {
    pub fn update(&mut self, snapshot: Snapshot, now: Duration, force: bool) -> bool {
        let view = snapshot.view(now);
        let critical = (view.status != self.view.status && view.status != Status::Expired)
            || (view.band() == Band::Red && self.view.band() != Band::Red);
        if !force && self.session == snapshot.session && now < self.next && !critical {
            return false;
        }
        self.session = snapshot.session;
        self.next = now + DISPLAY_INTERVAL;
        let changed = self.view != view;
        self.view = view;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(raw: u8, seconds: u64) -> Snapshot {
        Snapshot {
            session: 2,
            connected: true,
            raw: Some(raw),
            seen: Duration::from_secs(seconds),
        }
    }
    #[test]
    fn conversion_preserves_estimates_full_faults_and_invalid_levels() {
        for level in 0..=10 {
            for charging in [0, 0x10] {
                let view = sample(charging | level, 0).view(Duration::ZERO);
                assert_eq!(view.percent, Some((level * 10 + 5).min(100)));
            }
        }
        assert_eq!(sample(0x20, 0).view(Duration::ZERO).percent, Some(100));
        for raw in [0xa0, 0xb5, 0xff] {
            assert_eq!(
                sample(raw, 0).view(Duration::ZERO),
                View {
                    percent: None,
                    status: Status::Fault(raw >> 4)
                }
            );
        }
        for raw in [0x0b, 0x1f, 0x30] {
            assert_eq!(sample(raw, 0).view(Duration::ZERO).status, Status::Unknown);
        }
        for (percent, band) in [
            (0, Band::Red),
            (20, Band::Red),
            (21, Band::Yellow),
            (50, Band::Yellow),
            (51, Band::Green),
            (100, Band::Green),
        ] {
            assert_eq!(
                View {
                    percent: Some(percent),
                    status: Status::Discharging
                }
                .band(),
                band
            );
        }
    }
    #[test]
    fn ordinary_changes_wait_ten_seconds_and_events_bypass_throttle() {
        let mut display = Display::default();
        assert!(display.update(sample(8, 0), Duration::ZERO, false));
        assert!(!display.update(sample(7, 1), Duration::from_secs(1), false));
        assert_eq!(display.view.percent, Some(85));
        assert!(display.update(sample(7, 10), Duration::from_secs(10), false));
        assert!(display.update(sample(1, 11), Duration::from_secs(11), false));
        assert_eq!(display.view.band(), Band::Red);
        assert!(display.update(sample(0x11, 12), Duration::from_secs(12), false));
        assert_eq!(display.view.status, Status::Charging);
        assert!(display.update(sample(0x15, 13), Duration::from_secs(13), true));
        assert_eq!(display.view.percent, Some(55));
        assert!(display.update(
            Snapshot {
                session: 4,
                connected: true,
                ..Default::default()
            },
            Duration::from_secs(14),
            false
        ));
        assert_eq!(display.view.status, Status::Awaiting);
        assert!(display.update(Snapshot::default(), Duration::from_secs(15), false));
        assert_eq!(display.view.status, Status::Disconnected);
    }
    #[test]
    fn freshness_and_session_changes_never_reuse_previous_device_data() {
        let telemetry = Telemetry::default();
        telemetry.observe(8);
        assert!(telemetry.snapshot().raw.is_none());
        telemetry.connect();
        let session = telemetry.snapshot().session;
        telemetry.observe_at(8, Duration::ZERO);
        assert_eq!(
            telemetry.snapshot().view(Duration::from_secs(29)).percent,
            Some(85)
        );
        assert_eq!(
            telemetry.snapshot().view(STALE_AFTER).status,
            Status::Expired
        );
        telemetry.disconnect();
        telemetry.connect();
        assert_ne!(telemetry.snapshot().session, session);
        assert_eq!(
            telemetry.snapshot().view(Duration::ZERO).status,
            Status::Awaiting
        );
    }
    #[test]
    fn expired_reading_changes_only_at_next_regular_check() {
        let mut display = Display::default();
        display.update(sample(8, 0), Duration::from_secs(25), false);
        assert!(!display.update(sample(8, 0), Duration::from_secs(30), false));
        assert!(display.update(sample(8, 0), Duration::from_secs(35), false));
        assert_eq!(display.view.status, Status::Expired);
        assert!(display.update(sample(7, 36), Duration::from_secs(36), false));
    }
    #[test]
    fn unchanged_values_do_not_repaint_or_disable_the_periodic_throttle() {
        let mut display = Display::default();
        assert!(display.update(sample(8, 0), Duration::ZERO, false));
        assert!(!display.update(sample(8, 10), Duration::from_secs(10), false));
        assert!(!display.update(sample(7, 11), Duration::from_secs(11), false));
        assert!(display.update(sample(7, 20), Duration::from_secs(20), false));
    }
    #[test]
    fn stop_masks_input_before_slow_cleanup_and_restart_requires_fresh_data() {
        let telemetry = Telemetry::default();
        telemetry.connect();
        telemetry.observe(8);
        telemetry.suspend();
        telemetry.observe(7);
        assert_eq!(
            telemetry.snapshot().view(telemetry.now()).status,
            Status::Disconnected
        );
        telemetry.disconnect();
        telemetry.resume();
        telemetry.connect();
        assert_eq!(
            telemetry.snapshot().view(telemetry.now()).status,
            Status::Awaiting
        );
    }
}
