//! The backend's state blob: SameBoy's save state plus what SameBoy doesn't
//! save and the wrapper owns (emulated time, the vblank count, the last
//! complete frame). Little-endian throughout.

use gbcli_core::{CoreError, FrameTag, Ticks, VblankKind};

use crate::hooks::FrameCopy;

const MAGIC: &[u8; 8] = b"GBCLISB\0";
const VERSION: u32 = 2;

/// Everything in a blob, decoded.
#[derive(Clone, Debug, Default)]
pub(crate) struct Blob {
    pub model: u32,
    pub ticks: u64,
    pub vblanks: u64,
    pub frame: FrameCopy,
    /// SameBoy's live, partial framebuffer, which its save state leaves out:
    /// without it the first frame after a mid-frame load keeps stale lines.
    pub screen: Vec<u32>,
    /// Never-written bitmaps (`Tracking::words`); empty with tracking off.
    pub written: Vec<u64>,
    pub sameboy: Vec<u8>,
}

pub(crate) fn encode(blob: &Blob) -> Vec<u8> {
    let frame = &blob.frame;
    let mut out =
        Vec::with_capacity(64 + frame.shades.len() + frame.rgb.len() * 4 + blob.sameboy.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&blob.model.to_le_bytes());
    out.extend_from_slice(&blob.ticks.to_le_bytes());
    out.extend_from_slice(&blob.vblanks.to_le_bytes());
    match frame.tag {
        None => out.push(0),
        Some(tag) => {
            out.push(1);
            out.push(kind_code(tag.kind));
            out.extend_from_slice(&tag.number.to_le_bytes());
            out.extend_from_slice(&tag.at.0.to_le_bytes());
        }
    }
    out.extend_from_slice(&frame.width.to_le_bytes());
    out.extend_from_slice(&frame.height.to_le_bytes());
    out.extend_from_slice(&(frame.shades.len() as u64).to_le_bytes());
    out.extend_from_slice(&frame.shades);
    put_words(&mut out, &frame.rgb);
    put_words(&mut out, &blob.screen);
    out.extend_from_slice(&(blob.written.len() as u64).to_le_bytes());
    for word in &blob.written {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.extend_from_slice(&(blob.sameboy.len() as u64).to_le_bytes());
    out.extend_from_slice(&blob.sameboy);
    out
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Blob, CoreError> {
    let mut r = Reader(bytes);
    if r.take(MAGIC.len())? != MAGIC {
        return Err(invalid("wrong magic"));
    }
    if r.u32()? != VERSION {
        return Err(invalid("unknown version"));
    }
    let model = r.u32()?;
    let ticks = r.u64()?;
    let vblanks = r.u64()?;
    let tag = match r.u8()? {
        0 => None,
        1 => {
            let kind = code_kind(r.u8()?)?;
            let number = r.u64()?;
            let at = Ticks(r.u64()?);
            Some(FrameTag { number, kind, at })
        }
        _ => return Err(invalid("bad frame flag")),
    };
    let width = r.u32()?;
    let height = r.u32()?;
    let n = r.len()?;
    let shades = r.take(n)?.to_vec();
    let rgb = r.words()?;
    let screen = r.words()?;
    let n = r.len()?;
    let written = (0..n).map(|_| r.u64()).collect::<Result<_, _>>()?;
    let n = r.len()?;
    let sameboy = r.take(n)?.to_vec();
    if !r.0.is_empty() {
        return Err(invalid("trailing bytes"));
    }
    Ok(Blob {
        model,
        ticks,
        vblanks,
        frame: FrameCopy {
            tag,
            width,
            height,
            shades,
            rgb,
            // Layer tags are an observation of a frame, not part of the state:
            // a load leaves none until the next complete frame.
            layers: Vec::new(),
        },
        screen,
        written,
        sameboy,
    })
}

fn put_words(out: &mut Vec<u8>, words: &[u32]) {
    out.extend_from_slice(&(words.len() as u64).to_le_bytes());
    for word in words {
        out.extend_from_slice(&word.to_le_bytes());
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], CoreError> {
        if n > self.0.len() {
            return Err(invalid("truncated"));
        }
        let (head, tail) = self.0.split_at(n);
        self.0 = tail;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8, CoreError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, CoreError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, CoreError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn len(&mut self) -> Result<usize, CoreError> {
        usize::try_from(self.u64()?).map_err(|_| invalid("length overflow"))
    }
    fn words(&mut self) -> Result<Vec<u32>, CoreError> {
        let n = self.len()?;
        let bytes = self.take(n.checked_mul(4).ok_or_else(|| invalid("length overflow"))?)?;
        Ok(bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&c| u32::from_le_bytes(c))
            .collect())
    }
}

fn kind_code(kind: VblankKind) -> u8 {
    match kind {
        VblankKind::Normal => 0,
        VblankKind::LcdOff => 1,
        VblankKind::Artificial => 2,
    }
}

fn code_kind(code: u8) -> Result<VblankKind, CoreError> {
    match code {
        0 => Ok(VblankKind::Normal),
        1 => Ok(VblankKind::LcdOff),
        2 => Ok(VblankKind::Artificial),
        _ => Err(invalid("unknown frame kind")),
    }
}

fn invalid(what: &str) -> CoreError {
    CoreError::InvalidState {
        reason: format!("not a gbcli SameBoy state blob: {what}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(rgb: bool) -> Blob {
        let mut frame = FrameCopy {
            tag: Some(FrameTag {
                number: 85,
                kind: VblankKind::LcdOff,
                at: Ticks(12_345_678),
            }),
            width: 2,
            height: 2,
            ..FrameCopy::default()
        };
        if rgb {
            frame.rgb = vec![0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0];
        } else {
            frame.shades = vec![0, 1, 2, 3];
        }
        Blob {
            model: 0x002,
            ticks: 1 << 40,
            vblanks: 86,
            frame,
            screen: vec![0x0000_0003; 4],
            written: vec![1, u64::MAX],
            sameboy: vec![9, 8, 7, 6, 5],
        }
    }

    fn assert_same(a: &Blob, b: &Blob) {
        assert_eq!(a.model, b.model);
        assert_eq!(a.ticks, b.ticks);
        assert_eq!(a.vblanks, b.vblanks);
        assert_eq!(a.frame.tag, b.frame.tag);
        assert_eq!(
            (a.frame.width, a.frame.height),
            (b.frame.width, b.frame.height)
        );
        assert_eq!(a.frame.shades, b.frame.shades);
        assert_eq!(a.frame.rgb, b.frame.rgb);
        assert_eq!(a.screen, b.screen);
        assert_eq!(a.written, b.written);
        assert_eq!(a.sameboy, b.sameboy);
    }

    #[test]
    fn a_blob_with_shades_round_trips() {
        let blob = sample(false);
        assert_same(&decode(&encode(&blob)).unwrap(), &blob);
    }

    #[test]
    fn a_blob_with_rgb_round_trips() {
        let blob = sample(true);
        assert_same(&decode(&encode(&blob)).unwrap(), &blob);
    }

    #[test]
    fn a_blob_without_a_frame_round_trips() {
        let blob = Blob {
            sameboy: vec![1],
            ..Blob::default()
        };
        assert_same(&decode(&encode(&blob)).unwrap(), &blob);
    }

    #[test]
    fn a_foreign_blob_is_refused() {
        let err = decode(b"SAME\0\0\0\0 not ours").unwrap_err();
        assert!(matches!(err, CoreError::InvalidState { .. }), "{err:?}");
    }

    #[test]
    fn every_truncation_is_refused_without_panicking() {
        let bytes = encode(&sample(true));
        for len in 0..bytes.len() {
            assert!(decode(&bytes[..len]).is_err(), "accepted {len} bytes");
        }
    }

    #[test]
    fn trailing_bytes_are_refused() {
        let mut bytes = encode(&sample(false));
        bytes.push(0);
        assert!(decode(&bytes).is_err());
    }
}
