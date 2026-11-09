//! Breakpoints and watches across a rebuild: a location is resolved when it
//! is set, and again whenever the session's debugger meets a machine (design
//! §7), so one set by a symbol follows its label into the ROM on disk rather
//! than keeping the old build's address, and one whose label the ROM on disk
//! doesn't resolve is disarmed until a build has the label again. One set by
//! an address never moves and is never disarmed.

use std::fmt;

use gbcli_protocol::{Arming, DebuggerConfig, Location};

use super::Machine;
use crate::addr::{self, AddrError, RegionSizes};

/// Which kind of debugger entry a [`Rebased`] is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugEntry {
    Breakpoint,
    Watch,
}

/// A breakpoint or watch set by a symbol whose address the ROM on disk
/// changed, or couldn't find, or found again.
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
    /// The symbol no longer resolves; the entry is disarmed, keeping its
    /// old address only as a record.
    Lost {
        entry: DebugEntry,
        id: u32,
        symbol: String,
        at: Location,
        error: AddrError,
    },
    /// The symbol of a disarmed entry resolves again; the entry is armed at
    /// its address in this build.
    Rearmed {
        entry: DebugEntry,
        id: u32,
        symbol: String,
        at: Location,
    },
}

impl Machine {
    /// Gives this machine a session's breakpoints and watches, each set by a
    /// symbol resolved again against this machine's symbols, and reports
    /// each that moved, no longer resolves (and is disarmed), or resolves
    /// again (and is re-armed). Resolving is idempotent, so
    /// this holds on every load: the session's addresses may have been
    /// resolved against another build (a head or checkpoint that loads
    /// across a rebuild, a restart, a power cycle), and only the symbols on
    /// disk say where each label is now.
    pub fn attach_debugger(&mut self, config: DebuggerConfig) -> Vec<Rebased> {
        self.debugger = config;
        self.rebase_debugger()
    }

    /// Resolves every breakpoint and watch set by a symbol again, against
    /// this machine's symbols and sizes, and reports each that changed. One
    /// whose symbol is gone is disarmed, keeping its address; one disarmed
    /// whose symbol is back is armed where the symbol now is. Without sizes
    /// (a core that can't report them) nothing changes.
    pub fn rebase_debugger(&mut self) -> Vec<Rebased> {
        let Ok(sizes) = RegionSizes::of(self.core()) else {
            return Vec::new();
        };
        let symbols = self.symbols.as_ref().map(|s| s.symbols.clone());
        let symbols = symbols.as_deref();
        let mut report = Vec::new();
        let mut rebase = |entry: DebugEntry, id: u32, at: &mut Location, arming: &mut Arming| {
            let Some(symbol) = at.symbol.clone() else {
                return;
            };
            match addr::parse_on(&symbol, symbols, &sizes) {
                Ok(to) => {
                    let was = std::mem::replace(arming, Arming::Armed);
                    if was == Arming::Disarmed {
                        *at = to.clone();
                        report.push(Rebased::Rearmed {
                            entry,
                            id,
                            symbol,
                            at: to,
                        });
                    } else if (at.addr, at.bank) != (to.addr, to.bank) {
                        let from = std::mem::replace(at, to.clone());
                        report.push(Rebased::Moved {
                            entry,
                            id,
                            symbol,
                            from,
                            to,
                        });
                    }
                }
                Err(error) => {
                    *arming = Arming::Disarmed;
                    report.push(Rebased::Lost {
                        entry,
                        id,
                        symbol,
                        at: at.clone(),
                        error,
                    });
                }
            }
        };
        let debugger = &mut self.debugger;
        for b in &mut debugger.breakpoints {
            rebase(DebugEntry::Breakpoint, b.id, &mut b.at, &mut b.arming);
        }
        for w in &mut debugger.watches {
            rebase(DebugEntry::Watch, w.id, &mut w.at, &mut w.arming);
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
                 ({error}): it is disarmed, stopping nothing at its old {} or anywhere, \
                 until a build has {symbol} again; `gbcli delete {id}` removes it",
                place(at)
            ),
            Rebased::Rearmed {
                entry,
                id,
                symbol,
                at,
            } => write!(
                f,
                "{entry} #{id} is armed again: the ROM on disk has {symbol} again, at {}",
                place(at)
            ),
        }
    }
}
