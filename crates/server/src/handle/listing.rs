//! The `sym` and `checkpoints` read verbs (design §12). Neither changes the
//! head.

use gbcli_bundle::SessionStore;
use gbcli_engine::Area;
use gbcli_protocol::{ResultBody, SessionFile, SymArgs, SymbolArea, SymbolRow};

use super::{Failure, Handler, Need};

impl Handler {
    pub(super) fn sym(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &SymArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let (machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
        let rows = match machine.symbols() {
            Some(symbols) => symbols
                .matching(&args.pattern)
                .into_iter()
                .map(|s| SymbolRow {
                    name: s.name.clone(),
                    bank: s.bank,
                    addr: s.addr,
                    area: area(Area::of(s.addr)),
                })
                .collect(),
            None => {
                warnings.push(
                    "the session loaded no .sym; build with `rgblink -n game.sym` beside the ROM"
                        .into(),
                );
                Vec::new()
            }
        };
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            symbols: Some(rows),
            ..ResultBody::default()
        })
    }

    pub(super) fn checkpoints(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
    ) -> Result<ResultBody, Failure> {
        let rows = store
            .checkpoint_rows(meta)
            .map_err(|e| Failure::from_store(&e))?;
        Ok(ResultBody {
            checkpoints: Some(rows),
            ..ResultBody::default()
        })
    }
}

fn area(area: Area) -> SymbolArea {
    match area {
        Area::Rom0 => SymbolArea::Rom0,
        Area::RomX => SymbolArea::Romx,
        Area::Vram => SymbolArea::Vram,
        Area::Sram => SymbolArea::Sram,
        Area::Wram0 => SymbolArea::Wram0,
        Area::WramX => SymbolArea::Wramx,
        Area::Echo => SymbolArea::Echo,
        Area::Oam => SymbolArea::Oam,
        Area::Unusable => SymbolArea::Unusable,
        Area::Io => SymbolArea::Io,
        Area::Hram => SymbolArea::Hram,
        Area::Ie => SymbolArea::Ie,
    }
}
