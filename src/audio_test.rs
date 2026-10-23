use crate::audio_test_source::{Captured, SourceCapture};
use std::{
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, Ordering::*},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use winapi::{
    shared::mmreg::WAVEFORMATEX,
    um::{mmeapi::*, mmsystem::*},
};

const RATE: usize = 48000;
const SECONDS: usize = 5;
#[derive(Clone, Default)]
pub struct Status {
    pub message: String,
    pub busy: bool,
    pub recorded: bool,
    pub source_recorded: bool,
}
struct State {
    status: Status,
    pcm: Arc<[i16]>,
    source_pcm: Arc<[i16]>,
}
pub struct Test {
    shared: Arc<Mutex<State>>,
    cancel: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    source: Arc<SourceCapture>,
}
impl Default for Test {
    fn default() -> Self {
        Self::with_source(Arc::new(SourceCapture::default()))
    }
}
impl Test {
    pub fn with_source(source: Arc<SourceCapture>) -> Self {
        Self {
            shared: Arc::new(Mutex::new(State {
                status: Status {
                    message: crate::tr!(audio_test_click_record_to_record_5_seconds_after).into(),
                    ..Default::default()
                },
                pcm: Arc::from([]),
                source_pcm: Arc::from([]),
            })),
            cancel: Arc::new(AtomicBool::new(false)),
            worker: None,
            source,
        }
    }
}
fn status(shared: &Mutex<State>, message: impl Into<String>, busy: bool) {
    if let Ok(mut s) = shared.lock() {
        s.status.message = message.into();
        s.status.busy = busy;
        s.status.recorded = !s.pcm.is_empty();
        s.status.source_recorded = !s.source_pcm.is_empty();
    }
}
fn countdown(shared: &Mutex<State>, cancel: &AtomicBool) -> bool {
    for second in (1..=3).rev() {
        status(
            shared,
            crate::tr!(audio_test_starting_in, second = format!("{}", second)),
            true,
        );
        for _ in 0..50 {
            if cancel.load(Acquire) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    !cancel.load(Acquire)
}
impl Test {
    pub fn snapshot(&self) -> Status {
        self.shared
            .try_lock()
            .map(|s| s.status.clone())
            .unwrap_or(Status {
                message: crate::tr!(audio_test_working).into(),
                busy: true,
                recorded: false,
                source_recorded: false,
            })
    }
    pub fn cancel(&mut self, clear: bool) {
        self.cancel.store(true, Release);
        if let Ok(mut s) = self.shared.lock() {
            if clear {
                s.pcm = Arc::from([]);
                s.source_pcm = Arc::from([]);
                s.status.recorded = false;
                s.status.source_recorded = false;
            }
        }
        if self.worker.as_ref().is_none_or(|w| w.is_finished()) {
            if let Some(w) = self.worker.take() {
                let _ = w.join();
            }
            status(
                &self.shared,
                crate::tr!(audio_test_cancelled_ready_to_record_again),
                false,
            );
        }
    }
    pub fn start(&mut self, record: bool) -> Result<(), String> {
        self.start_job(record, false)
    }
    pub fn play_source(&mut self) -> Result<(), String> {
        self.start_job(false, true)
    }
    fn start_job(&mut self, record: bool, source_replay: bool) -> Result<(), String> {
        if self.worker.as_ref().is_some_and(|w| !w.is_finished()) {
            return Err(crate::tr!(audio_test_cancel_the_current_test_and_wait_for).into());
        }
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let shared = self.shared.clone();
        let source = self.source.clone();
        let replay = if record {
            if let Ok(mut s) = shared.lock() {
                s.pcm = Arc::from([]);
                s.source_pcm = Arc::from([]);
            }
            None
        } else {
            let s = shared
                .lock()
                .map_err(|_| crate::tr!(audio_test_recording_cleared))?;
            Some(if source_replay {
                s.source_pcm.clone()
            } else {
                s.pcm.clone()
            })
        };
        if replay.as_ref().is_some_and(|p| p.is_empty()) {
            return Err(crate::tr!(audio_test_record_some_audio_first).into());
        }
        status(
            &shared,
            if record {
                crate::tr!(audio_test_starting_in_3)
            } else if source_replay {
                crate::tr!(audio_test_playing_bluetooth_source_through_the_default_playback)
            } else {
                crate::tr!(audio_test_playing_usb_capture_through_the_default_playback)
            },
            true,
        );
        self.worker = Some(
            std::thread::Builder::new()
                .name("opends-audio-test".into())
                .spawn(move || {
                    let result = if record {
                        if countdown(&shared, &cancel) {
                            capture(&shared, &cancel, &source).map(Some)
                        } else {
                            Ok(None)
                        }
                    } else {
                        playback(replay.unwrap(), &cancel).map(|_| None)
                    };
                    if cancel.load(Acquire) {
                        status(&shared, crate::tr!(audio_test_cancelled), false);
                        return;
                    }
                    match result {
                        Ok(Some((pcm, source))) => {
                            let (zero_frames, longest, interior) = zero_runs(&pcm);
                            crate::logging::record_message(crate::i18n::MessageId::audio_test_microphone_test_captured_frames_stereo_zero_frames, &crate::tr!(audio_test_microphone_test_captured_frames_stereo_zero_frames, frames = format!("{}", pcm.len() / 2), silent = format!("{}", zero_frames), longest = format!("{}", longest), bursts = format!("{}", interior)));
                            crate::logging::record_message(crate::i18n::MessageId::audio_test_microphone_source_decoded_frames_longest_packet_gap, &crate::tr!(audio_test_microphone_source_decoded_frames_longest_packet_gap, frames = format!("{}", source.pcm.len() / 2), gap = format!("{:.1}", source.longest_gap_ms), gaps = format!("{}", source.dropped), device = format!("{}", pcm.len() / 2)));
                            let peak = pcm.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
                            let seconds = pcm.len() as f32 / (RATE * 2) as f32;
                            if let Ok(mut s) = shared.lock() {
                                if cancel.load(Acquire) {
                                    return;
                                }
                                s.pcm = pcm.into();
                                s.source_pcm = source.pcm.into();
                                s.status = Status {
                                    message: crate::tr!(audio_test_recorded_s_peak, seconds = format!("{:.1}", seconds), peak = format!("{:.1}", peak as f32 / 327.68), hint = format!("{}", if peak == 0 {
                                            crate::tr!(audio_test_no_audible_signal_recorded_check_mute_and)
                                        } else {
                                            crate::tr!(audio_test_ready_to_play_back)
                                        })),
                                    busy: false,
                                    recorded: !s.pcm.is_empty(),
                                    source_recorded: !s.source_pcm.is_empty(),
                                };
                            }
                        }
                        Ok(None) => status(&shared, crate::tr!(audio_test_playback_finished), false),
                        Err(e) => status(&shared, e, false),
                    }
                })
                .map_err(|e| {
                    status(&self.shared, crate::tr!(audio_test_could_not_start_the_audio_test), false);
                    e.to_string()
                })?,
        );
        Ok(())
    }
}
fn zero_runs(pcm: &[i16]) -> (usize, usize, usize) {
    let mut total = 0;
    let mut longest = 0;
    let mut run = 0;
    let mut interior = 0;
    let mut started = false;
    for pair in pcm.chunks_exact(2) {
        if pair == [0, 0] {
            total += 1;
            run += 1;
            longest = longest.max(run);
        } else {
            if started && run >= 48 {
                interior += 1;
            }
            started = true;
            run = 0;
        }
    }
    (total, longest, interior)
}
impl Drop for Test {
    fn drop(&mut self) {
        self.cancel.store(true, Release);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}
fn format() -> WAVEFORMATEX {
    WAVEFORMATEX {
        wFormatTag: 1,
        nChannels: 2,
        nSamplesPerSec: RATE as u32,
        nAvgBytesPerSec: (RATE * 4) as u32,
        nBlockAlign: 4,
        wBitsPerSample: 16,
        cbSize: 0,
    }
}
fn check(code: u32, operation: &str) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        let mut message = [0u16; 256];
        unsafe {
            waveInGetErrorTextW(code, message.as_mut_ptr(), message.len() as u32);
        }
        let end = message
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(message.len());
        Err(crate::tr!(
            audio_test_failed,
            operation = format!("{}", operation),
            error = format!("{}", String::from_utf16_lossy(&message[..end])),
            code = format!("{}", code)
        ))
    }
}
fn capture(
    shared: &Mutex<State>,
    cancel: &AtomicBool,
    source: &SourceCapture,
) -> Result<(Vec<i16>, Captured), String> {
    unsafe {
        let mut ids = Vec::new();
        for id in 0..waveInGetNumDevs() {
            let mut caps: WAVEINCAPSW = std::mem::zeroed();
            if waveInGetDevCapsW(
                id as usize,
                &mut caps,
                std::mem::size_of::<WAVEINCAPSW>() as u32,
            ) == 0
            {
                let name = caps.szPname;
                let name = String::from_utf16_lossy(
                    &name[..name.iter().position(|c| *c == 0).unwrap_or(name.len())],
                );
                if name.contains("DualSense") {
                    ids.push(id);
                }
            }
        }
        if ids.len() != 1 {
            return Err(if ids.is_empty() {
                crate::tr!(audio_test_no_dualsense_microphone_found_connect_the_controller)
            } else {
                crate::tr!(audio_test_multiple_dualsense_microphones_found_disconnect_the_other)
            }
            .into());
        }
        let mut device = Wave::new(true, vec![0; RATE * SECONDS * 2]);
        let mut handle = null_mut();
        check(
            waveInOpen(&mut handle, ids[0], &format(), 0, 0, 0),
            crate::tr!(audio_test_open_dualsense_microphone),
        )?;
        device.input = handle;
        check(
            waveInPrepareHeader(handle, &mut *device.header, device.header_size()),
            crate::tr!(audio_test_prepare_recording_buffer),
        )?;
        device.prepared = true;
        check(
            waveInAddBuffer(handle, &mut *device.header, device.header_size()),
            crate::tr!(audio_test_submit_recording_buffer),
        )?;
        let recording = source.begin();
        check(waveInStart(handle), crate::tr!(audio_test_start_recording))?;
        let started = Instant::now();
        while !device.done() && !cancel.load(Acquire) {
            if started.elapsed() > Duration::from_secs(SECONDS as u64 + 2) {
                return Err(
                    crate::tr!(audio_test_recording_timed_out_no_complete_usb_microphone).into(),
                );
            }
            status(
                shared,
                crate::tr!(
                    audio_test_recording_seconds_remaining,
                    seconds = format!(
                        "{}",
                        SECONDS.saturating_sub(started.elapsed().as_secs() as usize)
                    )
                ),
                true,
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        check(waveInReset(handle), crate::tr!(audio_test_finish_recording))?;
        crate::logging::record_message(
            crate::i18n::MessageId::audio_test_microphone_recording_duration_s,
            &crate::tr!(
                audio_test_microphone_recording_duration_s,
                seconds = format!("{:.3}", started.elapsed().as_secs_f64())
            ),
        );
        let count = (device.header.dwBytesRecorded as usize / 2).min(device.buffer.len());
        device.finish()?;
        let mut pcm = Vec::new();
        std::mem::swap(&mut pcm, &mut device.buffer);
        pcm.truncate(count);
        // Header is unprepared before the backing allocation can be released.
        drop(device);
        Ok((pcm, recording.finish()))
    }
}
fn playback(pcm: Arc<[i16]>, cancel: &AtomicBool) -> Result<(), String> {
    unsafe {
        let mut device = Wave::new(false, pcm.to_vec());
        let mut handle = null_mut();
        check(
            waveOutOpen(&mut handle, WAVE_MAPPER, &format(), 0, 0, 0),
            crate::tr!(audio_test_open_default_playback_device),
        )?;
        device.output = handle;
        check(
            waveOutPrepareHeader(handle, &mut *device.header, device.header_size()),
            crate::tr!(audio_test_prepare_playback_buffer),
        )?;
        device.prepared = true;
        check(
            waveOutWrite(handle, &mut *device.header, device.header_size()),
            crate::tr!(audio_test_start_playback),
        )?;
        let started = Instant::now();
        while !device.done() && !cancel.load(Acquire) {
            if started.elapsed() > Duration::from_secs(8) {
                return Err(crate::tr!(audio_test_playback_timed_out).into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }
}
struct Wave {
    input: HWAVEIN,
    output: HWAVEOUT,
    header: Box<WAVEHDR>,
    buffer: Vec<i16>,
    prepared: bool,
}
impl Wave {
    fn new(_: bool, mut buffer: Vec<i16>) -> Self {
        let mut header: Box<WAVEHDR> = Box::new(unsafe { std::mem::zeroed() });
        header.lpData = buffer.as_mut_ptr() as _;
        header.dwBufferLength = (buffer.len() * 2) as u32;
        Self {
            input: null_mut(),
            output: null_mut(),
            header,
            buffer,
            prepared: false,
        }
    }
    fn header_size(&self) -> u32 {
        std::mem::size_of::<WAVEHDR>() as u32
    }
    fn done(&self) -> bool {
        unsafe { std::ptr::read_volatile(std::ptr::addr_of!(self.header.dwFlags)) & 1 != 0 }
    } // WHDR_DONE
    fn finish(&mut self) -> Result<(), String> {
        unsafe {
            if !self.input.is_null() {
                check(
                    waveInReset(self.input),
                    crate::tr!(audio_test_stop_recording),
                )?;
                if self.prepared {
                    check(
                        waveInUnprepareHeader(self.input, &mut *self.header, self.header_size()),
                        crate::tr!(audio_test_release_recording_buffer),
                    )?;
                    self.prepared = false;
                }
                check(
                    waveInClose(self.input),
                    crate::tr!(audio_test_close_microphone),
                )?;
                self.input = null_mut();
            }
            if !self.output.is_null() {
                check(
                    waveOutReset(self.output),
                    crate::tr!(audio_test_stop_playback),
                )?;
                if self.prepared {
                    check(
                        waveOutUnprepareHeader(self.output, &mut *self.header, self.header_size()),
                        crate::tr!(audio_test_release_playback_buffer),
                    )?;
                    self.prepared = false;
                }
                check(
                    waveOutClose(self.output),
                    crate::tr!(audio_test_close_playback_device),
                )?;
                self.output = null_mut();
            }
        }
        Ok(())
    }
}
impl Drop for Wave {
    fn drop(&mut self) {
        for _ in 0..3 {
            match self.finish() {
                Ok(()) => return,
                Err(e) => crate::logging::record(&e),
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // A broken driver must never keep a pointer to freed Rust memory.
        // Retain only a still-prepared buffer in this exceptional failure case.
        if self.prepared {
            let header =
                std::mem::replace(&mut self.header, Box::new(unsafe { std::mem::zeroed() }));
            std::mem::forget(header);
            std::mem::forget(std::mem::take(&mut self.buffer));
            crate::logging::record_message(
                crate::i18n::MessageId::audio_test_the_audio_driver_did_not_return_the,
                &crate::tr!(audio_test_the_audio_driver_did_not_return_the),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn construction_never_opens_microphone_and_empty_replay_is_rejected() {
        let mut test = Test::default();
        assert!(test.worker.is_none());
        assert!(!test.snapshot().recorded);
        assert!(test.start(false).is_err());
        assert!(test.play_source().is_err());
        assert!(test.worker.is_none());
    }
    #[test]
    fn cancelling_countdown_prevents_any_recording() {
        let test = Test::default();
        let cancelled = AtomicBool::new(true);
        assert!(!countdown(&test.shared, &cancelled));
        assert!(test.shared.lock().unwrap().pcm.is_empty());
        assert!(test.shared.lock().unwrap().source_pcm.is_empty());
    }
}
