//! The cartridge header against the file it heads: the cart type ($0147),
//! ROM size ($0148) and RAM size ($0149) a real cart and other emulators go
//! by. A disagreement loads anyway (SameBoy sizes the cart from the header
//! and the file), with a warning naming the fix.

use std::fmt;

const CART_TYPE: usize = 0x147;
const ROM_SIZE: usize = 0x148;
const RAM_SIZE: usize = 0x149;
const KIB: usize = 1024;

/// The mapper a cart type names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mapper {
    None,
    Mbc1,
    Mbc2,
    Mmm01,
    Mbc3,
    Mbc5,
    Mbc6,
    Mbc7,
    Camera,
    Tama5,
    HuC3,
    HuC1,
}

/// What a cart type byte says the cart has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CartType {
    mapper: Mapper,
    name: &'static str,
    ram: bool,
}

fn cart_type(byte: u8) -> Option<CartType> {
    use Mapper::*;
    let (mapper, name, ram) = match byte {
        0x00 => (None, "ROM ONLY", false),
        0x01 => (Mbc1, "MBC1", false),
        0x02 => (Mbc1, "MBC1+RAM", true),
        0x03 => (Mbc1, "MBC1+RAM+BATTERY", true),
        0x05 => (Mbc2, "MBC2", false),
        0x06 => (Mbc2, "MBC2+BATTERY", false),
        0x08 => (None, "ROM+RAM", true),
        0x09 => (None, "ROM+RAM+BATTERY", true),
        0x0B => (Mmm01, "MMM01", false),
        0x0C => (Mmm01, "MMM01+RAM", true),
        0x0D => (Mmm01, "MMM01+RAM+BATTERY", true),
        0x0F => (Mbc3, "MBC3+TIMER+BATTERY", false),
        0x10 => (Mbc3, "MBC3+TIMER+RAM+BATTERY", true),
        0x11 => (Mbc3, "MBC3", false),
        0x12 => (Mbc3, "MBC3+RAM", true),
        0x13 => (Mbc3, "MBC3+RAM+BATTERY", true),
        0x19 => (Mbc5, "MBC5", false),
        0x1A => (Mbc5, "MBC5+RAM", true),
        0x1B => (Mbc5, "MBC5+RAM+BATTERY", true),
        0x1C => (Mbc5, "MBC5+RUMBLE", false),
        0x1D => (Mbc5, "MBC5+RUMBLE+RAM", true),
        0x1E => (Mbc5, "MBC5+RUMBLE+RAM+BATTERY", true),
        0x20 => (Mbc6, "MBC6", true),
        0x22 => (Mbc7, "MBC7+SENSOR+RUMBLE+RAM+BATTERY", true),
        0xFC => (Camera, "POCKET CAMERA", true),
        0xFD => (Tama5, "BANDAI TAMA5", true),
        0xFE => (HuC3, "HuC3", true),
        0xFF => (HuC1, "HuC1+RAM+BATTERY", true),
        _ => return Option::None,
    };
    Some(CartType { mapper, name, ram })
}

impl Mapper {
    /// The largest ROM the mapper can address, where the header's own limit
    /// (8 MiB) isn't already the bound.
    fn max_rom(self) -> Option<usize> {
        match self {
            Mapper::None => Some(32 * KIB),
            Mapper::Mbc1 => Some(2048 * KIB),
            Mapper::Mbc2 => Some(256 * KIB),
            // MBC30, which SameBoy emulates, reaches 4 MiB.
            Mapper::Mbc3 => Some(4096 * KIB),
            _ => Option::None,
        }
    }

    /// The most RAM the mapper can bank, for the mappers games are built on.
    fn max_ram(self) -> Option<usize> {
        match self {
            Mapper::None => Some(8 * KIB),
            Mapper::Mbc1 => Some(32 * KIB),
            // MBC30 again.
            Mapper::Mbc3 => Some(64 * KIB),
            Mapper::Mbc5 => Some(128 * KIB),
            _ => Option::None,
        }
    }
}

/// One way the header disagrees with the file or with itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeaderMismatch {
    /// The file is shorter than a header.
    NoHeader { len: usize },
    /// $0147 names no cart type.
    UnknownCartType { byte: u8 },
    /// $0148 names no ROM size.
    UnknownRomSize { byte: u8 },
    /// $0148's size isn't the file's.
    RomSize {
        byte: u8,
        declared: usize,
        file: usize,
    },
    /// The file is bigger than the cart type's mapper can address.
    RomTooBigForMapper {
        cart: &'static str,
        max: usize,
        file: usize,
    },
    /// $0149 names no RAM size.
    UnknownRamSize { byte: u8 },
    /// $0149 declares RAM on a cart type without RAM.
    RamWithoutRamType { cart: &'static str, declared: usize },
    /// The cart type has RAM but $0149 declares none.
    RamTypeWithoutRam { cart: &'static str },
    /// MBC2's RAM is built in, so $0149 must be 0.
    Mbc2RamSize { declared: usize },
    /// More RAM than the mapper can bank.
    RamTooBigForMapper {
        cart: &'static str,
        max: usize,
        declared: usize,
    },
}

impl HeaderMismatch {
    /// The next step: the `rgbfix` flags that make the header true.
    pub fn remedy(&self) -> &'static str {
        match self {
            HeaderMismatch::NoHeader { .. } => {
                "a ROM needs at least 0x150 bytes; build it with RGBDS (`gbcli build`)"
            }
            HeaderMismatch::UnknownCartType { .. }
            | HeaderMismatch::RomTooBigForMapper { .. }
            | HeaderMismatch::RamWithoutRamType { .. } => {
                "set a cart type that fits the ROM, e.g. `rgbfix -m MBC5+RAM+BATTERY -p 0xFF game.gb`"
            }
            HeaderMismatch::UnknownRomSize { .. } | HeaderMismatch::RomSize { .. } => {
                "pad the ROM to a valid size and set $0148 to match: `rgbfix -p 0xFF game.gb`"
            }
            HeaderMismatch::UnknownRamSize { .. }
            | HeaderMismatch::RamTypeWithoutRam { .. }
            | HeaderMismatch::RamTooBigForMapper { .. } => {
                "set the RAM size: `rgbfix -r 2` for 8 KiB, `-r 3` for 32 KiB, `-r 4` for 128 KiB"
            }
            HeaderMismatch::Mbc2RamSize { .. } => "clear the RAM size: `rgbfix -r 0 game.gb`",
        }
    }
}

impl fmt::Display for HeaderMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeaderMismatch::NoHeader { len } => {
                write!(f, "the file is {len} bytes, too short for a header")
            }
            HeaderMismatch::UnknownCartType { byte } => {
                write!(
                    f,
                    "the header's cart type ${byte:02X} ($0147) isn't one a cart has"
                )
            }
            HeaderMismatch::UnknownRomSize { byte } => {
                write!(
                    f,
                    "the header's ROM size ${byte:02X} ($0148) isn't a valid code"
                )
            }
            HeaderMismatch::RomSize {
                byte,
                declared,
                file,
            } => write!(
                f,
                "the header declares a {} ROM (${byte:02X} at $0148) but the file is {}",
                size(*declared),
                size(*file)
            ),
            HeaderMismatch::RomTooBigForMapper { cart, max, file } => write!(
                f,
                "the cart type {cart} reaches {} of ROM but the file is {}; the rest is unreachable",
                size(*max),
                size(*file)
            ),
            HeaderMismatch::UnknownRamSize { byte } => {
                write!(
                    f,
                    "the header's RAM size ${byte:02X} ($0149) isn't a valid code"
                )
            }
            HeaderMismatch::RamWithoutRamType { cart, declared } => write!(
                f,
                "the header declares {} of cart RAM ($0149) but the cart type {cart} has none",
                size(*declared)
            ),
            HeaderMismatch::RamTypeWithoutRam { cart } => write!(
                f,
                "the cart type {cart} has RAM but the header's RAM size ($0149) is 0, so the cart \
                 gets none"
            ),
            HeaderMismatch::Mbc2RamSize { declared } => write!(
                f,
                "MBC2 has its RAM built in, but the header's RAM size ($0149) declares {}",
                size(*declared)
            ),
            HeaderMismatch::RamTooBigForMapper {
                cart,
                max,
                declared,
            } => write!(
                f,
                "the header declares {} of cart RAM but the cart type {cart} banks at most {}",
                size(*declared),
                size(*max)
            ),
        }
    }
}

fn size(bytes: usize) -> String {
    if bytes >= KIB && bytes.is_multiple_of(KIB) {
        format!("{} KiB", bytes / KIB)
    } else {
        format!("{bytes}-byte")
    }
}

fn ram_size(byte: u8) -> Option<usize> {
    Some(match byte {
        0x00 => 0,
        0x01 => 2 * KIB,
        0x02 => 8 * KIB,
        0x03 => 32 * KIB,
        0x04 => 128 * KIB,
        0x05 => 64 * KIB,
        _ => return None,
    })
}

/// Every way `rom`'s header disagrees with the file or itself, in header
/// order. Empty for a well-formed ROM.
pub fn check_header(rom: &[u8]) -> Vec<HeaderMismatch> {
    if rom.len() < 0x150 {
        return vec![HeaderMismatch::NoHeader { len: rom.len() }];
    }
    let mut found = Vec::new();
    let kind = cart_type(rom[CART_TYPE]);
    if kind.is_none() {
        found.push(HeaderMismatch::UnknownCartType {
            byte: rom[CART_TYPE],
        });
    }
    let rom_byte = rom[ROM_SIZE];
    if rom_byte <= 0x08 {
        let declared = (32 * KIB) << rom_byte;
        if declared != rom.len() {
            found.push(HeaderMismatch::RomSize {
                byte: rom_byte,
                declared,
                file: rom.len(),
            });
        }
    } else {
        found.push(HeaderMismatch::UnknownRomSize { byte: rom_byte });
    }
    if let Some(kind) = kind
        && let Some(max) = kind.mapper.max_rom()
        && rom.len() > max
    {
        found.push(HeaderMismatch::RomTooBigForMapper {
            cart: kind.name,
            max,
            file: rom.len(),
        });
    }
    let Some(declared) = ram_size(rom[RAM_SIZE]) else {
        found.push(HeaderMismatch::UnknownRamSize {
            byte: rom[RAM_SIZE],
        });
        return found;
    };
    let Some(kind) = kind else {
        return found;
    };
    if kind.mapper == Mapper::Mbc2 {
        if declared != 0 {
            found.push(HeaderMismatch::Mbc2RamSize { declared });
        }
    } else if !kind.ram && declared != 0 {
        found.push(HeaderMismatch::RamWithoutRamType {
            cart: kind.name,
            declared,
        });
    } else if kind.ram && declared == 0 && kind.mapper != Mapper::Mbc7 {
        found.push(HeaderMismatch::RamTypeWithoutRam { cart: kind.name });
    } else if let Some(max) = kind.mapper.max_ram()
        && declared > max
    {
        found.push(HeaderMismatch::RamTooBigForMapper {
            cart: kind.name,
            max,
            declared,
        });
    }
    found
}

#[cfg(test)]
#[path = "cart_tests.rs"]
mod tests;
