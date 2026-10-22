//! BT 71B Opus 单声道 → UAC1 每毫秒 48 帧双声道 S16LE。
//! 麦克风帧绝不进入按键输入通路；此模块不自行开启录音。
use crate::{
    audio::{OpusDecoder, OPUS_FRAMES},
    opus::LibOpusDecoder,
};
use std::collections::VecDeque;
pub struct Microphone<D = LibOpusDecoder> {
    decoder: D,
    pending: VecDeque<i16>,
}
impl Microphone {
    pub fn new() -> Option<Self> {
        Some(Self {
            decoder: LibOpusDecoder::new()?,
            pending: VecDeque::with_capacity(960),
        })
    }
}
impl<D: OpusDecoder> Microphone<D> {
    pub fn decode(&mut self, packet: &[u8], mut send: impl FnMut(&[u8; 192]) -> bool) -> bool {
        self.decode_observed(packet, false, |_, _| {}, &mut send)
    }
    pub fn decode_observed(
        &mut self,
        packet: &[u8],
        observe_forwarded: bool,
        mut observe: impl FnMut(&[i16], bool),
        mut send: impl FnMut(&[u8; 192]) -> bool,
    ) -> bool {
        if packet.len() != 71 {
            return false;
        }
        let mut pcm = [0; OPUS_FRAMES];
        let n = self.decoder.decode(packet, &mut pcm);
        if n == 0 || n > OPUS_FRAMES {
            return false;
        }
        observe(&pcm[..n], false);
        let mut forwarded = [0i16; OPUS_FRAMES + 48];
        let mut count = 0;
        append_frames(&mut self.pending, &pcm[..n], &mut |frame| {
            let ok = send(frame);
            if ok && observe_forwarded {
                for sample in frame.chunks_exact(4) {
                    if count < forwarded.len() {
                        forwarded[count] = i16::from_le_bytes([sample[0], sample[1]]);
                        count += 1;
                    }
                }
            }
            ok
        });
        if count > 0 {
            observe(&forwarded[..count], true);
        }
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
    struct Decoder;
    impl OpusDecoder for Decoder {
        fn decode(&mut self, _: &[u8], pcm: &mut [i16; OPUS_FRAMES]) -> usize {
            for (i, s) in pcm.iter_mut().enumerate() {
                *s = i as i16 + 1;
            }
            OPUS_FRAMES
        }
        fn reset(&mut self) {}
    }
    #[test]
    fn observer_reports_decoded_source_and_only_successful_usb_samples() {
        let mut mic = Microphone {
            decoder: Decoder,
            pending: VecDeque::new(),
        };
        let mut observed = Vec::new();
        let mut sends = 0;
        assert!(mic.decode_observed(
            &[0; 71],
            true,
            |samples, forwarded| observed.push((samples.to_vec(), forwarded)),
            |frame| {
                for sample in frame.chunks_exact(4) {
                    assert_eq!(&sample[..2], &sample[2..]);
                }
                sends += 1;
                sends <= 5
            }
        ));
        assert_eq!(observed.len(), 2);
        assert!(!observed[0].1);
        assert_eq!(observed[0].0.len(), 480);
        assert!(observed[1].1);
        assert_eq!(observed[1].0.len(), 240);
        assert_eq!(&observed[0].0[..240], &observed[1].0);
        assert!(mic.pending.is_empty());
    }
    #[test]
    fn source_only_monitor_does_not_copy_or_publish_forwarded_microphone() {
        let mut mic = Microphone {
            decoder: Decoder,
            pending: VecDeque::new(),
        };
        let mut calls = 0;
        assert!(mic.decode_observed(
            &[0; 71],
            false,
            |_, forwarded| {
                assert!(!forwarded);
                calls += 1;
            },
            |_| true
        ));
        assert_eq!(calls, 1);
    }
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
