//! Flag values: whole numbers, bytes, `TARGET=BYTES` writes, step units.

use gbcli_protocol::{Start, StepKind, TimeBase, WriteArg, WriteKind};

use crate::usage::Usage;

/// A whole number for `--flag`.
pub fn count(flag: &str, text: &str) -> Result<u64, Usage> {
    text.parse().map_err(|_| {
        Usage::new(
            format!("--{flag} takes a whole number, not `{text}`"),
            format!("e.g. `--{flag} 120`"),
        )
    })
}

/// One byte: `$91`, `0x91`, `%10010001` or `145`.
fn byte(text: &str) -> Option<u8> {
    let (digits, radix) = if let Some(hex) = text.strip_prefix('$') {
        (hex, 16)
    } else if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        (hex, 16)
    } else if let Some(bin) = text.strip_prefix('%') {
        (bin, 2)
    } else {
        (text, 10)
    };
    u8::from_str_radix(digits, radix).ok()
}

/// `TARGET=BYTES`, where BYTES is one or more comma-separated bytes.
pub fn write(kind: WriteKind, text: &str) -> Result<WriteArg, Usage> {
    let shape = "TARGET=BYTES, e.g. wSpeed=2 or 01:4123=$00,$00";
    let Some((target, bytes)) = text.split_once('=') else {
        return Err(Usage::new(
            format!("`{text}` has no `=`"),
            format!("write {shape}"),
        ));
    };
    if target.is_empty() || bytes.is_empty() {
        return Err(Usage::new(
            format!("`{text}` needs a target and bytes"),
            format!("write {shape}"),
        ));
    }
    let bytes = bytes
        .split(',')
        .map(|b| {
            byte(b.trim()).ok_or_else(|| {
                Usage::new(
                    format!("`{b}` in `{text}` isn't a byte"),
                    "each value is one byte, $00-$FF, 0x00-0xFF, %… or 0-255; \
                     write a 16-bit value low byte first: wX=$34,$12",
                )
            })
        })
        .collect::<Result<Vec<u8>, Usage>>()?;
    Ok(WriteArg {
        kind,
        target: target.to_string(),
        bytes,
    })
}

pub fn start(text: &str) -> Result<Start, Usage> {
    match text {
        "boot" => Ok(Start::Boot),
        "power-on" => Ok(Start::PowerOn),
        other => Err(Usage::new(
            format!("--from takes boot or power-on, not `{other}`"),
            "to start from a checkpoint, use --at NAME",
        )),
    }
}

pub fn since(text: &str) -> TimeBase {
    match text {
        "boot" => TimeBase::Boot,
        "power-on" => TimeBase::PowerOn,
        name => TimeBase::Checkpoint(name.to_string()),
    }
}

/// The step units the CLI offers; `to_address` is `run --to`.
pub fn step_unit(text: &str) -> Result<StepKind, Usage> {
    Ok(match text {
        "instruction" => StepKind::Instruction,
        "over" => StepKind::Over,
        "out" => StepKind::Out,
        "frame" => StepKind::Frame,
        "scanline" => StepKind::Scanline,
        other => {
            return Err(Usage::new(
                format!("`{other}` isn't a step unit"),
                "step instruction, over, out, frame or scanline; to run to an address, \
                 `gbcli run --to ADDR`",
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_take_every_radix_and_keep_order() {
        let w = write(WriteKind::Poke, "wX=$34,0x12,%1,9").unwrap();
        assert_eq!(w.target, "wX");
        assert_eq!(w.bytes, vec![0x34, 0x12, 1, 9]);
        assert_eq!(w.kind, WriteKind::Poke);
        let w = write(WriteKind::Patch, "01:4123=0,0").unwrap();
        assert_eq!(w.target, "01:4123");
    }

    #[test]
    fn a_wide_or_missing_value_is_refused_with_the_shape() {
        let u = write(WriteKind::Poke, "wX=$1234").unwrap_err();
        assert_eq!(u.message, "`$1234` in `wX=$1234` isn't a byte");
        assert!(u.remedy.contains("low byte first"), "{}", u.remedy);
        let u = write(WriteKind::Poke, "wX").unwrap_err();
        assert_eq!(u.message, "`wX` has no `=`");
        assert!(write(WriteKind::Poke, "=1").is_err());
    }

    #[test]
    fn counts_and_units_refuse_strangers() {
        assert_eq!(count("frames", "120").unwrap(), 120);
        assert_eq!(
            count("frames", "ten").unwrap_err().message,
            "--frames takes a whole number, not `ten`"
        );
        assert_eq!(step_unit("over").unwrap(), StepKind::Over);
        assert!(
            step_unit("to_address")
                .unwrap_err()
                .remedy
                .contains("run --to")
        );
        assert_eq!(since("f30"), TimeBase::Checkpoint("f30".into()));
        assert!(start("sideways").is_err());
    }
}
