//! Breakpoints and watches on a restart: a location is resolved when it is
//! set (design §7), so one set by a symbol would keep the old build's
//! address after a rebuild. A restart resolves each symbol again against the
//! machine's own symbols, so the location follows its label.

use std::fmt;

use gbcli_protocol::Location;

use super::Machine;
use crate::addr::{self, AddrError, RegionSizes};

/// Which kind of debugger entry a [`Rebased`] is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugEntry {
    Breakpoint,
    Watch,
}

/// A breakpoint or watch set by a symbol whose address the restart changed,
/// or couldn't find.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rebased {
    /// The symbol resolves elsewhere in this build; the entry moved with it.
    Moved {
        entry: DebugEntry,
        id: u32,
        symbol: String,
        from: Location,
        to: Location,
    },
    /// The symbol no longer resolves; the entry keeps its old address.
    Lost {
        entry: DebugEntry,
        id: u32,
        symbol: String,
        at: Location,
        error: AddrError,
    },
}

impl Machine {
    /// Resolves every breakpoint and watch set by a symbol again, against
    /// this machine's symbols and sizes, and reports each that changed. One
    /// whose symbol is gone keeps its address. Without sizes (a core that
    /// can't report them) nothing changes.
    pub fn rebase_debugger(&mut self) -> Vec<Rebased> {
        let Ok(sizes) = RegionSizes::of(self.core()) else {
            return Vec::new();
        };
        let symbols = self.symbols.as_ref().map(|s| s.symbols.clone());
        let symbols = symbols.as_deref();
        let mut report = Vec::new();
        let mut rebase = |entry: DebugEntry, id: u32, at: &mut Location| {
            let Some(symbol) = at.symbol.clone() else {
                return;
            };
            match addr::parse_on(&symbol, symbols, &sizes) {
                Ok(to) if to.addr == at.addr && to.bank == at.bank => {}
                Ok(to) => {
                    let from = std::mem::replace(at, to.clone());
                    report.push(Rebased::Moved {
                        entry,
                        id,
                        symbol,
                        from,
                        to,
                    });
                }
                Err(error) => report.push(Rebased::Lost {
                    entry,
                    id,
                    symbol,
                    at: at.clone(),
                    error,
                }),
            }
        };
        let debugger = &mut self.debugger;
        for b in &mut debugger.breakpoints {
            rebase(DebugEntry::Breakpoint, b.id, &mut b.at);
        }
        for w in &mut debugger.watches {
            rebase(DebugEntry::Watch, w.id, &mut w.at);
        }
        report
    }
}

fn place(at: &Location) -> String {
    match at.bank {
        Some(bank) => format!("{bank:02X}:{:04X}", at.addr),
        None => format!("${:04X}", at.addr),
    }
}

impl fmt::Display for DebugEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DebugEntry::Breakpoint => "breakpoint",
            DebugEntry::Watch => "watch",
        })
    }
}

impl fmt::Display for Rebased {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rebased::Moved {
                entry,
                id,
                symbol,
                from,
                to,
            } => write!(
                f,
                "{entry} #{id} follows {symbol} into the ROM on disk: {} -> {}",
                place(from),
                place(to)
            ),
            Rebased::Lost {
                entry,
                id,
                symbol,
                at,
                error,
            } => write!(
                f,
                "{entry} #{id} was set at {symbol}, which the ROM on disk doesn't resolve \
                 ({error}): it stays at {}; `gbcli delete {id}` removes it",
                place(at)
            ),
        }
    }
}
