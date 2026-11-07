//! The structured OAM forms (design §7): `oam[N]` is entry N (0–39), and
//! `oam[N].y`, `.x`, `.tile` and `.attr` are its four bytes, each one byte
//! of the OAM region wherever an expression takes a value.

use gbcli_core::Region;

use super::lex::{Tok, lex};
use super::{Expr, ExprError, Place, Width};

/// The name that starts the form.
pub const NAME: &str = "oam";
/// OAM holds 40 entries of four bytes.
const ENTRIES: i64 = 40;
const FIELDS: [&str; 4] = ["y", "x", "tile", "attr"];

/// The entry number inside `oam[...]`: a §8 integer from 0 to 39. `field`
/// is the text after the entry, for the corrected form a hint shows.
pub fn index(at: usize, tok: &Tok, field: &str) -> Result<u8, ExprError> {
    let value = match tok {
        Tok::Num { value: Some(v), .. } => Some(*v),
        _ => None,
    };
    match value {
        Some(n) if (0..ENTRIES).contains(&n) => Ok(n as u8),
        Some(n) if n >= ENTRIES => Err(ExprError::Syntax {
            at,
            message: format!("oam[{n}] is past the last entry: OAM holds 40, oam[0] to oam[39]"),
            hint: Some(format!("oam[39]{field}")),
        }),
        _ => Err(ExprError::syntax(
            at,
            format!("expected an entry number in oam[...], oam[0] to oam[39]; got {tok:?}"),
        )),
    }
}

/// The byte `.field` names in entry `index`, as a read of the OAM region.
pub fn field(at: usize, index: u8, field: &str) -> Result<Expr, ExprError> {
    let name = field.strip_prefix('.').unwrap_or(field);
    let Some(byte) = FIELDS.iter().position(|f| *f == name) else {
        return Err(ExprError::Syntax {
            at,
            message: format!("oam[{index}] has no field .{name}"),
            hint: Some(field_list(index)),
        });
    };
    Ok(Expr::Read {
        width: Width::U8,
        place: Place::Region {
            region: Region::Oam,
            offset: u32::from(index) * 4 + byte as u32,
        },
    })
}

/// A whole entry where a value belongs.
pub fn whole_entry(at: usize, index: u8) -> ExprError {
    ExprError::Syntax {
        at,
        message: format!(
            "oam[{index}] is a whole entry, four bytes; an expression takes one of them \
             (only a read prints the whole entry, as fields)"
        ),
        hint: Some(field_list(index)),
    }
}

fn field_list(index: u8) -> String {
    format!("oam[{index}].y, oam[{index}].x, oam[{index}].tile or oam[{index}].attr")
}

/// Entry N when `text` is `oam[N]` alone.
pub fn entry(text: &str) -> Result<Option<u8>, ExprError> {
    let Ok(toks) = lex(text) else {
        return Ok(None);
    };
    match toks.as_slice() {
        [
            (_, Tok::Ident(name)),
            (_, Tok::LBracket),
            (at, n),
            (_, Tok::RBracket),
        ] if name == NAME => index(*at, n, "").map(Some),
        _ => Ok(None),
    }
}
