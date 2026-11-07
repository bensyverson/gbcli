//! `equiv [ROM_B] --frames N …` → the protocol's `EquivArgs`.

use gbcli_protocol::{AudioCompareName, EquivArgs, EquivSide, RamRangeArg, WriteKind};

use crate::Env;
use crate::argv::Invocation;
use crate::usage::Usage;
use crate::values;

pub fn equiv(inv: &Invocation, env: &Env) -> Result<EquivArgs, Usage> {
    let path = |p: &str| env.cwd.join(p).to_string_lossy().into_owned();
    let Some(frames) = inv.one("frames") else {
        return Err(Usage::new(
            "equiv needs --frames N: how many frames to compare",
            "e.g. `gbcli equiv fast.gb --frames 600`",
        ));
    };
    Ok(EquivArgs {
        b: EquivSide {
            rom: inv.positionals.first().map(|p| path(p)),
            sym: inv.one("sym-b").map(path),
            writes: inv
                .all("patch")
                .map(|w| values::write(WriteKind::Patch, w))
                .collect::<Result<_, _>>()?,
        },
        at: inv.one("at").map(str::to_string),
        from: inv.one("from").map(values::start).transpose()?,
        frames: values::count("frames", frames)?,
        keys: super::video::keys(inv, env)?,
        ram: inv.all("ram").map(ram).collect::<Result<_, _>>()?,
        audio: inv.one("audio").map(audio).transpose()?,
    })
}

/// `ADDR` or `ADDR,LEN` (one byte by default).
fn ram(text: &str) -> Result<RamRangeArg, Usage> {
    let (at, len) = match text.rsplit_once(',') {
        Some((at, len)) => (at, values::count("ram", len)?),
        None => (text, 1),
    };
    let len = u16::try_from(len)
        .ok()
        .filter(|&n| n > 0 && !at.is_empty())
        .ok_or_else(|| {
            Usage::new(
                format!("`{text}` is not ADDR[,LEN]"),
                "e.g. --ram wPlayerX or --ram wOAMBuffer,160 (LEN 1 to 65535)",
            )
        })?;
    Ok(RamRangeArg {
        at: at.to_string(),
        len,
    })
}

fn audio(text: &str) -> Result<AudioCompareName, Usage> {
    match text {
        "writes" => Ok(AudioCompareName::Writes),
        "timed" => Ok(AudioCompareName::Timed),
        "off" => Ok(AudioCompareName::Off),
        _ => Err(Usage::new(
            format!("--audio takes writes, timed or off, not `{text}`"),
            "writes compares each frame's sound writes in order; timed also their times",
        )),
    }
}
