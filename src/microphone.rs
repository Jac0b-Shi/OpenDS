//! BT 71B Opus 单声道 → UAC1 每毫秒 48 帧双声道 S16LE。
//! 麦克风帧绝不进入按键输入通路；此模块不自行开启录音。
use crate::{
    audio::{OpusDecoder, OPUS_FRAMES},
    opus::LibOpusDecoder,
};
use std::collections::VecDeque;
pub struct Microphone {
    decoder: LibOpusDecoder,
    pending: VecDeque<i16>,
}
impl Microphone {
    pub fn new() -> Option<Self> {
        Some(Self {
            decoder: LibOpusDecoder::new()?,
            pending: VecDeque::with_capacity(960),
        })
    }
    pub fn decode(&mut self, packet: &[u8], mut send: impl FnMut(&[u8; 192]) -> bool) -> bool {
        if packet.len() != 71 {
            return false;
        }
        let mut pcm = [0; OPUS_FRAMES];
        let n = self.decoder.decode(packet, &mut pcm);
        if n == 0 || n > OPUS_FRAMES {
            return false;
        }
        append_frames(&mut self.pending, &pcm[..n], &mut send);
        true
    }
}
fn append_frames(
    pending: &mut VecDeque<i16>,
    samples: &[i16],
    send: &mut impl FnMut(&[u8; 192]) -> bool,
) {
    if pending.len() + samples.len() > 960 {
        pending.clear();
    }
    pending.extend(samples.iter().copied());
    while pending.len() >= 48 {
        let mut frame = [0; 192];
        for i in 0..48 {
            let v = pending.pop_front().unwrap().to_le_bytes();
            frame[i * 4..i * 4 + 2].copy_from_slice(&v);
            frame[i * 4 + 2..i * 4 + 4].copy_from_slice(&v);
        }
        if !send(&frame) {
            pending.clear();
            break;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_is_split_at_exact_millisecond_boundaries_and_duplicated() {
        let mut q = VecDeque::new();
        let mut frames = vec![];
        append_frames(&mut q, &[1234; 47], &mut |f| {
            frames.push(*f);
            true
        });
        assert!(frames.is_empty());
        append_frames(&mut q, &[1234; 433], &mut |f| {
            frames.push(*f);
            true
        });
        assert_eq!(frames.len(), 10);
        assert!(q.is_empty());
        for f in frames {
            for p in f.chunks_exact(4) {
                assert_eq!(p, &[0xd2, 0x04, 0xd2, 0x04]);
            }
        }
    }
    #[test]
    fn closed_host_microphone_does_not_accumulate_stale_audio() {
        let mut q = VecDeque::new();
        append_frames(&mut q, &[1; 480], &mut |_| false);
        assert!(q.is_empty());
    }
}
