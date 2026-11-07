//! WAV export: 16-bit stereo PCM at the capture rate.

use gbcli_core::{AudioCapture, StereoSample};

const CHANNELS: u16 = 2;
const BITS: u16 = 16;
const BLOCK_ALIGN: u16 = CHANNELS * BITS / 8;
/// The WAVE format tag for integer PCM.
const PCM: u16 = 1;

/// `samples` as a RIFF/WAVE file: 16-bit signed little-endian PCM, two
/// channels, [`AudioCapture::SAMPLE_RATE`] Hz, a 44-byte header and nothing
/// after the data.
pub fn wav(samples: &[StereoSample]) -> Vec<u8> {
    let data_len = u32::try_from(samples.len() * usize::from(BLOCK_ALIGN))
        .expect("a WAV holds at most 4 GiB of data");
    let rate = AudioCapture::SAMPLE_RATE;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&PCM.to_le_bytes());
    out.extend_from_slice(&CHANNELS.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * u32::from(BLOCK_ALIGN)).to_le_bytes());
    out.extend_from_slice(&BLOCK_ALIGN.to_le_bytes());
    out.extend_from_slice(&BITS.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.left.to_le_bytes());
        out.extend_from_slice(&s.right.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_says_16_bit_stereo_at_the_capture_rate() {
        let bytes = wav(&[StereoSample { left: 1, right: -2 }]);
        assert_eq!(bytes.len(), 48);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 40);
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 2);
        assert_eq!(
            u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            32_768
        );
        assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 16);
        assert_eq!(&bytes[44..], &[1, 0, 0xFE, 0xFF]);
    }
}
