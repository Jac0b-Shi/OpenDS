//! 可选静态 libopus；与动态后端使用相同的 CBR、帧长和复杂度参数。
use crate::audio::{OpusDecoder, OpusEncoder, OPUS_BYTES, OPUS_FRAMES, SAMPLE_RATE};
use opusic_sys as ffi;
pub fn is_available() -> bool {
    true
}

pub struct LibOpusEncoder {
    ptr: *mut ffi::OpusEncoder,
}
impl LibOpusEncoder {
    pub fn new() -> Option<Self> {
        Self::with_complexity(0)
    }
    pub fn with_complexity(complexity: u8) -> Option<Self> {
        unsafe {
            let mut error = 0;
            let ptr = ffi::opus_encoder_create(SAMPLE_RATE as i32, 2, 2049, &mut error);
            if ptr.is_null() || error != 0 {
                return None;
            }
            let enc = Self { ptr };
            for (request, value) in [
                (4002, 160_000),
                (4006, 0),
                (4010, complexity.min(10) as i32),
                (4040, 5003),
            ] {
                if ffi::opus_encoder_ctl(ptr, request, value) != 0 {
                    return None;
                }
            }
            Some(enc)
        }
    }
}
impl Drop for LibOpusEncoder {
    fn drop(&mut self) {
        unsafe {
            ffi::opus_encoder_destroy(self.ptr);
        }
    }
}
impl OpusEncoder for LibOpusEncoder {
    fn encode(&mut self, pcm: &[i16], out: &mut [u8; OPUS_BYTES]) -> usize {
        if pcm.len() < OPUS_FRAMES * 2 {
            return 0;
        }
        unsafe {
            ffi::opus_encode(
                self.ptr,
                pcm.as_ptr(),
                OPUS_FRAMES as i32,
                out.as_mut_ptr(),
                out.len() as i32,
            )
            .max(0) as usize
        }
    }
    fn reset(&mut self) {
        unsafe {
            ffi::opus_encoder_ctl(self.ptr, 4028);
        }
    }
}
pub struct LibOpusDecoder {
    ptr: *mut ffi::OpusDecoder,
}
impl LibOpusDecoder {
    pub fn new() -> Option<Self> {
        unsafe {
            let mut error = 0;
            let ptr = ffi::opus_decoder_create(SAMPLE_RATE as i32, 1, &mut error);
            if ptr.is_null() || error != 0 {
                None
            } else {
                Some(Self { ptr })
            }
        }
    }
}
impl Drop for LibOpusDecoder {
    fn drop(&mut self) {
        unsafe {
            ffi::opus_decoder_destroy(self.ptr);
        }
    }
}
impl OpusDecoder for LibOpusDecoder {
    fn decode(&mut self, packet: &[u8], out: &mut [i16; OPUS_FRAMES]) -> usize {
        unsafe {
            ffi::opus_decode(
                self.ptr,
                packet.as_ptr(),
                packet.len() as i32,
                out.as_mut_ptr(),
                OPUS_FRAMES as i32,
                0,
            )
            .max(0) as usize
        }
    }
    fn reset(&mut self) {
        unsafe {
            ffi::opus_decoder_ctl(self.ptr, 4028);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_backend_honors_complexity_and_produces_fixed_cbr() {
        for complexity in [0, 4, 10] {
            let mut enc = LibOpusEncoder::with_complexity(complexity).unwrap();
            let mut value = -1i32;
            assert_eq!(
                unsafe { ffi::opus_encoder_ctl(enc.ptr, 4011, &mut value) },
                0
            );
            assert_eq!(value, complexity as i32);
            let mut packet = [0; OPUS_BYTES];
            assert_eq!(enc.encode(&[0; OPUS_FRAMES * 2], &mut packet), OPUS_BYTES);
            let mut dec = LibOpusDecoder::new().unwrap();
            assert_eq!(dec.decode(&packet, &mut [0; OPUS_FRAMES]), OPUS_FRAMES);
            enc.reset();
            dec.reset();
        }
    }
}
