use gbcli_bundle::{SessionStore, Start};
use gbcli_core::Region;
use gbcli_engine::{Machine, addr, mem};
use gbcli_protocol::{
    ErrorBody, ErrorCode, ResultBody, SessionFile, WriteArg, WriteArgs, WriteKind, WriteMade,
};

use super::{Failure, Handler, Need, save_head};
use crate::convert::{self};
impl Handler {
    pub(super) fn write(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &WriteArgs,
        actor: &Option<String>,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let writes = apply_writes(&mut machine, &args.writes)?;
        save_head(store, meta, &machine, actor.clone(), None, None)?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            writes: Some(writes),
            ..ResultBody::default()
        })
    }
}

/// Applies the three kinds of write in order (design §4).
pub(super) fn apply_writes(
    machine: &mut Machine,
    writes: &[WriteArg],
) -> Result<Vec<WriteMade>, Failure> {
    let mut made = Vec::new();
    for write in writes {
        match write.kind {
            WriteKind::Bus => {
                let at = addr::parse(&write.target, machine.symbols())
                    .map_err(|e| Failure::new(convert::addr(&e)))?;
                let base = at.addr;
                for (i, &byte) in write.bytes.iter().enumerate() {
                    machine
                        .core_mut()
                        .bus_write(base.wrapping_add(i as u16), byte)
                        .map_err(|e| Failure::new(convert::core_error(&e)))?;
                }
                made.push(WriteMade {
                    kind: write.kind,
                    target: write.target.clone(),
                    bytes: write.bytes.clone(),
                    addr: Some(base),
                });
            }
            WriteKind::Poke => {
                let target = addr::parse_write(&write.target, machine.symbols())
                    .map_err(|e| Failure::new(convert::addr(&e)))?;
                let gbcli_engine::addr::WriteTarget::Cpu(at) = target else {
                    return Err(Failure::new(ErrorBody::new(
                        ErrorCode::Refused,
                        "a poke can't target a ROM file offset",
                        "use the patch kind; ROM changes are recorded",
                    )));
                };
                let base = at.addr;
                for (i, &byte) in write.bytes.iter().enumerate() {
                    let addr = base.wrapping_add(i as u16);
                    let target = mem::target(machine.core(), addr)
                        .map_err(|e| Failure::new(convert::core_error(&e)))?;
                    let (region, offset) = match target {
                        mem::Target::Backing { region, offset } => (region, offset),
                        mem::Target::Io(reg) => (Region::Io, usize::from(reg.0)),
                    };
                    machine
                        .core_mut()
                        .poke(region, offset as u32, &[byte])
                        .map_err(|e| Failure::new(convert::core_error(&e)))?;
                }
                made.push(WriteMade {
                    kind: write.kind,
                    target: write.target.clone(),
                    bytes: write.bytes.clone(),
                    addr: Some(base),
                });
            }
            WriteKind::Patch => {
                let offset = rom_offset(machine, &write.target)?;
                machine
                    .patch(offset, &write.bytes)
                    .map_err(|e| Failure::new(convert::core_error(&e)))?;
                made.push(WriteMade {
                    kind: write.kind,
                    target: write.target.clone(),
                    bytes: write.bytes.clone(),
                    addr: None,
                });
            }
        }
    }
    Ok(made)
}

/// A patch target as a ROM file offset (design §7).
fn rom_offset(machine: &Machine, text: &str) -> Result<u32, Failure> {
    match addr::parse_write(text, machine.symbols()).map_err(|e| Failure::new(convert::addr(&e)))? {
        addr::WriteTarget::RomOffset(offset) => Ok(offset),
        addr::WriteTarget::Cpu(at) => {
            let bank = at
                .bank
                .unwrap_or_else(|| mem::bank_at(machine.core(), at.addr));
            match mem::banked_target(bank, at.addr)
                .map_err(|e| Failure::new(convert::core_error(&e)))?
            {
                mem::Target::Backing {
                    region: Region::Rom,
                    offset,
                } => Ok(offset as u32),
                _ => Err(Failure::new(ErrorBody::new(
                    ErrorCode::BadAddress,
                    format!("{text} isn't in ROM"),
                    "a patch targets ROM; use a poke for RAM or IO",
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use crate::handle::test_support::{handler, request};
    use gbcli_protocol::{Op, ReadArgs};

    #[test]
    fn a_poke_is_backing_memory_and_the_next_read_sees_it() {
        let f = Fixture::new("poke");
        let mut h = handler(&f);
        let written = h.handle(&request(Op::Write(WriteArgs {
            writes: vec![WriteArg {
                kind: WriteKind::Poke,
                target: "$C002".into(),
                bytes: vec![2],
            }],
        })));
        assert!(written.ok, "{:?}", written.error);
        let read = h.handle(&request(Op::Read(ReadArgs {
            reads: vec!["[$C002]".into()],
            since: None,
        })));
        let reads = read.result.unwrap().reads.unwrap();
        assert_eq!(reads[0].value, 2);
    }
}
