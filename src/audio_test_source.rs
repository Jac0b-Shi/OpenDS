//! Opt-in, bounded diagnostic tap after microphone gain and before USB framing.
//! The bridge never takes a lock or allocates here. Only a manual test arms it.
use crossbeam_queue::ArrayQueue;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering::*},
    OnceLock,
};
use std::time::Instant;

const SAMPLES: usize = 480;
const CAPACITY: usize = 1024;
const LIMIT: usize = 48000 * 5;
struct Chunk {
    epoch: u64,
    at: Instant,
    len: usize,
    pcm: [i16; SAMPLES],
}
#[derive(Default)]
pub struct SourceCapture {
    queue: OnceLock<ArrayQueue<Chunk>>,
    next_epoch: AtomicU64,
    active: AtomicU64,
    writers: AtomicUsize,
    dropped: AtomicU64,
}
pub struct Recording<'a> {
    source: &'a SourceCapture,
    epoch: u64,
}
#[derive(Default)]
pub struct Captured {
    pub pcm: Vec<i16>, // Stereo L=R, matching the USB format and replay rate.
    pub dropped: u64,
    pub longest_gap_ms: f64,
}
impl SourceCapture {
    pub fn push(&self, pcm: &[i16]) {
        let epoch = self.active.load(SeqCst);
        if epoch == 0 || pcm.is_empty() || pcm.len() > SAMPLES {
            return;
        }
        self.writers.fetch_add(1, SeqCst);
        if self.active.load(SeqCst) == epoch {
            let mut chunk = Chunk {
                epoch,
                at: Instant::now(),
                len: pcm.len(),
                pcm: [0; SAMPLES],
            };
            chunk.pcm[..pcm.len()].copy_from_slice(pcm);
            if self.queue.get().unwrap().push(chunk).is_err() {
                self.dropped.fetch_add(1, Relaxed);
            }
        }
        self.writers.fetch_sub(1, SeqCst);
    }
    pub fn begin(&self) -> Recording<'_> {
        // Test serializes recording jobs; this also drains a cancelled session.
        self.stop();
        let queue = self.queue.get_or_init(|| ArrayQueue::new(CAPACITY));
        while queue.pop().is_some() {}
        self.dropped.store(0, Relaxed);
        let epoch = self.next_epoch.fetch_add(1, Relaxed) + 1;
        self.active.store(epoch, SeqCst);
        Recording {
            source: self,
            epoch,
        }
    }
    fn stop(&self) {
        self.active.store(0, SeqCst);
        // Only the test worker waits. The real-time producer never waits.
        while self.writers.load(SeqCst) != 0 {
            std::thread::yield_now();
        }
    }
}
impl Recording<'_> {
    pub fn finish(self) -> Captured {
        self.source.stop();
        let mut result = Captured {
            dropped: self.source.dropped.load(Relaxed),
            ..Default::default()
        };
        let mut previous = None;
        while let Some(chunk) = self.source.queue.get().unwrap().pop() {
            if chunk.epoch != self.epoch {
                continue;
            }
            if let Some(at) = previous {
                result.longest_gap_ms = result
                    .longest_gap_ms
                    .max(chunk.at.saturating_duration_since(at).as_secs_f64() * 1000.);
            }
            previous = Some(chunk.at);
            let remaining = LIMIT.saturating_sub(result.pcm.len() / 2);
            for &sample in &chunk.pcm[..chunk.len.min(remaining)] {
                result.pcm.extend_from_slice(&[sample, sample]);
            }
        }
        result
    }
}
impl Drop for Recording<'_> {
    fn drop(&mut self) {
        self.source.stop();
        while self.source.queue.get().unwrap().pop().is_some() {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dormant_tap_does_not_allocate_and_cancel_discards_audio() {
        let source = SourceCapture::default();
        source.push(&[12; SAMPLES]);
        assert!(source.queue.get().is_none());
        {
            let _recording = source.begin();
            source.push(&[12; SAMPLES]);
        }
        assert_eq!(source.active.load(Acquire), 0);
        assert!(source.queue.get().unwrap().is_empty());
        assert!(source.begin().finish().pcm.is_empty());
    }
    #[test]
    fn owned_copy_is_bounded_and_new_sessions_never_replay_old_audio() {
        let source = SourceCapture::default();
        let recording = source.begin();
        let mut samples = [123; SAMPLES];
        source.push(&samples);
        samples.fill(456);
        for _ in 0..CAPACITY {
            source.push(&samples);
        }
        let captured = recording.finish();
        assert_eq!(captured.dropped, 1);
        assert_eq!(captured.pcm.len(), LIMIT * 2);
        assert_eq!(&captured.pcm[..SAMPLES * 2], &[123; SAMPLES * 2]);
        assert_eq!(captured.pcm[SAMPLES * 2], 456);
        source.push(&[789; SAMPLES]);
        assert!(source.begin().finish().pcm.is_empty());
    }
    #[test]
    fn cancelling_with_a_live_producer_leaves_no_queued_audio() {
        use std::sync::{atomic::AtomicBool, Arc};
        let source = Arc::new(SourceCapture::default());
        let stop = Arc::new(AtomicBool::new(false));
        let producer = {
            let source = source.clone();
            let stop = stop.clone();
            std::thread::spawn(move || {
                while !stop.load(Acquire) {
                    source.push(&[321; SAMPLES]);
                    std::thread::yield_now();
                }
            })
        };
        for _ in 0..100 {
            let recording = source.begin();
            std::thread::yield_now();
            drop(recording);
            assert!(source.queue.get().unwrap().is_empty());
        }
        stop.store(true, Release);
        producer.join().unwrap();
        assert!(source.begin().finish().pcm.is_empty());
    }
}
