//! A symbol a saved state depends on that a rebuild moved: what a
//! `rom_changed` refusal lists, every one in its error's `moved`.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::checkpoint::SymbolRole;

/// A bank and address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolAt {
    pub bank: u16,
    pub addr: u16,
}

impl fmt::Display for SymbolAt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02X}:{:04X}", self.bank, self.addr)
    }
}

/// A symbol whose address differs between the saved and the current `.sym`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovedSymbol {
    pub name: String,
    pub role: SymbolRole,
    pub old: SymbolAt,
    /// `None` when the symbol is gone.
    pub new: Option<SymbolAt>,
}

impl fmt::Display for MovedSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let role = match self.role {
            SymbolRole::Pc => "PC is in",
            SymbolRole::Ram => "RAM symbol",
            SymbolRole::Stack => "return address on the stack is in",
        };
        match self.new {
            Some(new) => write!(f, "{role} {}: {} -> {new}", self.name, self.old),
            None => write!(f, "{role} {}: {} -> gone", self.name, self.old),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moved_symbol_reads_as_text_and_as_json() {
        let moved = MovedSymbol {
            name: "wFrame".into(),
            role: SymbolRole::Ram,
            old: SymbolAt {
                bank: 0,
                addr: 0xC000,
            },
            new: None,
        };
        assert_eq!(moved.to_string(), "RAM symbol wFrame: 00:C000 -> gone");
        let json = serde_json::to_value(&moved).unwrap();
        assert_eq!(json["role"], "ram");
        assert_eq!(json["old"]["addr"], 0xC000);
        assert_eq!(serde_json::from_value::<MovedSymbol>(json).unwrap(), moved);
    }
}
