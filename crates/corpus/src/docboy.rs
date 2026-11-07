//! The docboy adapter: runs a case from docboy's own config
//! (`docboy/config/dmg.json`) the way docboy's runner judges it
//! (`tests/testutils/testutils/runners.h` at `01e04d3`), within what the
//! `Core` trait offers. Differences from docboy's runner are named where made.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gbcli_core::{BootRom, FrameRef, Keys, Pixels, Region, VblankKind};
use serde::Deserialize;

use crate::drive::{Driver, Event};
use crate::frame;
use crate::judge::{Factory, boot};
use crate::models::Target;
use crate::rules::Verdict;

/// docboy's T-cycles to our ticks (2^23 Hz).
const TICKS_PER_T: u64 = 2;
/// docboy's defaults (runners.h): check every 100,000 T-cycles, give up at 100M.
const CHECK_INTERVAL_T: u64 = 100_000;
const DEFAULT_MAX_T: u64 = 100_000_000;
const HRAM: u16 = 0xFF80;

#[derive(Clone, Debug, Deserialize)]
pub struct Case {
    pub rom: String,
    pub framebuffer: Option<String>,
    pub palette: Option<DocboyPalette>,
    pub serial: Option<Vec<u8>>,
    pub memory: Option<Vec<MemoryExpectation>>,
    pub check_at_tick: Option<u64>,
    pub check_at_instruction: Option<Instruction>,
    pub max_ticks: Option<u64>,
    #[serde(default)]
    pub inputs: Vec<Input>,
    pub rom2: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocboyPalette {
    Grey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Instruction {
    #[serde(rename = "ld b,b")]
    LdBB,
    #[serde(rename = "ld c,c")]
    LdCC,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct MemoryExpectation {
    pub address: u16,
    pub value: u8,
    pub fail_value: Option<u8>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Input {
    pub trigger_at_tick: Option<u64>,
    pub trigger_at_instruction: Option<Instruction>,
    pub key: Key,
    pub state: KeyState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Key {
    A,
    B,
    Start,
    Select,
    Up,
    Down,
    Left,
    Right,
}

impl Key {
    fn bits(self) -> Keys {
        match self {
            Key::A => Keys::A,
            Key::B => Keys::B,
            Key::Start => Keys::START,
            Key::Select => Keys::SELECT,
            Key::Up => Keys::UP,
            Key::Down => Keys::DOWN,
            Key::Left => Keys::LEFT,
            Key::Right => Keys::RIGHT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyState {
    Pressed,
    Released,
}

pub type Config = HashMap<String, Vec<Case>>;

/// gbcli's boot ROM that hands over in docboy's DMG post-boot state
/// (`roms/docboy_boot.asm`). docboy runs with no boot ROM, starting from that
/// state; SameBoy needs a boot ROM, so this one reproduces it. Test-only: no
/// session uses it.
const POST_BOOT_ROM: &[u8] = include_bytes!("../roms/docboy_boot.bin");

/// [`POST_BOOT_ROM`] as a core's boot ROM.
pub fn post_boot_rom() -> BootRom {
    BootRom::Custom(POST_BOOT_ROM.to_vec())
}

/// docboy's DMG appearances in RGB565: the LCD-off color, then shades 0–3.
const DEFAULT_APPEARANCE: (u16, [u16; 4]) = (0xAD00, [0x84A0, 0x4B40, 0x2AA0, 0x1200]);
const GREY_APPEARANCE: (u16, [u16; 4]) = (0xFFFF, [0xFFFF, 0xAD55, 0x52AA, 0x0000]);

/// A frame as docboy would have drawn it, in RGB565.
pub fn frame_rgb565(frame: &FrameRef<'_>, palette: Option<DocboyPalette>) -> Option<Vec<u16>> {
    let (off, shades) = match palette {
        None => DEFAULT_APPEARANCE,
        Some(DocboyPalette::Grey) => GREY_APPEARANCE,
    };
    let Pixels::Shades(pixels) = frame.pixels else {
        return None;
    };
    let lcd_off = frame.tag.kind == VblankKind::LcdOff;
    Some(
        pixels
            .iter()
            .map(|&s| {
                if lcd_off {
                    off
                } else {
                    shades[usize::from(s & 3)]
                }
            })
            .collect(),
    )
}

/// RGB888 to RGB565 by truncation, as docboy's `convert_image` does.
pub fn rgb565([r, g, b]: [u8; 3]) -> u16 {
    (u16::from(r >> 3) << 11) | (u16::from(g >> 2) << 5) | u16::from(b >> 3)
}

/// docboy's memory check: pass when every byte matches, fatal when any holds its fail value.
pub fn memory_check(read: impl Fn(u16) -> u8, expected: &[MemoryExpectation]) -> Option<Verdict> {
    let mut all = true;
    for m in expected {
        let value = read(m.address);
        if m.fail_value == Some(value) {
            return Some(Verdict::fail(format!(
                "{:#06x} holds the fail value {value:#04x}",
                m.address
            )));
        }
        all &= value == m.value;
    }
    all.then_some(Verdict::Pass)
}

static CONFIGS: Mutex<Option<HashMap<PathBuf, Arc<Config>>>> = Mutex::new(None);

fn load_config(path: &Path) -> Result<Arc<Config>, String> {
    let mut cache = CONFIGS.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(config) = cache.get(path) {
        return Ok(config.clone());
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let config: Arc<Config> =
        Arc::new(serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?);
    cache.insert(path.to_path_buf(), config.clone());
    Ok(config)
}

/// Runs one docboy case; returns the verdict and the ticks it ran, if it ran.
pub fn run(
    corpus: &Path,
    config: &str,
    category: &str,
    index: usize,
    target: &Target,
    factory: Factory,
) -> (Verdict, Option<u64>) {
    match run_case(corpus, config, category, index, target, factory) {
        Ok(result) => result,
        Err(err) => (Verdict::fail(format!("runner error: {err}")), None),
    }
}

fn run_case(
    corpus: &Path,
    config: &str,
    category: &str,
    index: usize,
    target: &Target,
    factory: Factory,
) -> Result<(Verdict, Option<u64>), String> {
    let config_path = corpus.join(config);
    let cases = load_config(&config_path)?;
    let case = cases
        .get(category)
        .and_then(|c| c.get(index))
        .ok_or_else(|| format!("no case {category}[{index}] in {config}"))?;
    if case.enabled == Some(false) {
        return Ok((Verdict::Skip("disabled in docboy's config".into()), None));
    }
    if case.rom2.is_some() {
        return Ok((
            Verdict::Skip("two linked Game Boys: the Core trait has no link cable".into()),
            None,
        ));
    }
    if case
        .inputs
        .iter()
        .any(|i| i.trigger_at_instruction.is_some())
        || case.check_at_instruction == Some(Instruction::LdCC)
    {
        return Ok((
            Verdict::Skip("an input triggered by an instruction: not adapted".into()),
            None,
        ));
    }
    let root = config_path
        .parent()
        .and_then(Path::parent)
        .ok_or("docboy config has no root")?;
    let rom = std::fs::read(root.join("roms/dmg").join(&case.rom))
        .map_err(|e| format!("{}: {e}", case.rom))?;
    let reference = match &case.framebuffer {
        Some(png) => Some(
            frame::load_png(&root.join("results/dmg").join(png))?
                .rgb
                .into_iter()
                .map(rgb565)
                .collect::<Vec<u16>>(),
        ),
        None => None,
    };
    let mut d = boot(&rom, target, factory, post_boot_rom())?;
    let verdict = judge(&mut d, case, reference.as_deref())?;
    Ok((verdict, Some(d.elapsed())))
}

fn check(d: &Driver, case: &Case, reference: Option<&[u16]>) -> Result<Option<Verdict>, String> {
    if let Some(reference) = reference {
        let Some(frame) = d.core.last_complete_frame() else {
            return Ok(None);
        };
        let actual = frame_rgb565(&frame, case.palette).ok_or("docboy cases are DMG only")?;
        return Ok((actual == reference).then_some(Verdict::Pass));
    }
    if let Some(expected) = &case.serial {
        return Ok((&d.serial == expected).then_some(Verdict::Pass));
    }
    if let Some(expected) = &case.memory {
        let hram = d.region_bytes(Region::Hram)?;
        if expected
            .iter()
            .any(|m| m.address < HRAM || m.address > 0xFFFE)
        {
            return Err("docboy memory expectation outside HRAM".into());
        }
        return Ok(memory_check(|a| hram[usize::from(a - HRAM)], expected));
    }
    Err("docboy case with no expectation".into())
}

fn judge(d: &mut Driver, case: &Case, reference: Option<&[u16]>) -> Result<Verdict, String> {
    let max = case.max_ticks.unwrap_or(DEFAULT_MAX_T) * TICKS_PER_T;
    let mut inputs: Vec<(u64, Key, KeyState)> = case
        .inputs
        .iter()
        .filter_map(|i| Some((i.trigger_at_tick? * TICKS_PER_T, i.key, i.state)))
        .collect();
    inputs.sort_by_key(|i| i.0);
    let mut inputs = inputs.into_iter().peekable();
    let mut keys = Keys::NONE;
    let forced_at = case.check_at_tick.map(|t| t / 4 * 4 * TICKS_PER_T);
    let at_instruction = case.check_at_instruction.is_some();
    let mut next_check = CHECK_INTERVAL_T * TICKS_PER_T;
    loop {
        while let Some(&(at, key, state)) = inputs.peek() {
            if at > d.elapsed() {
                break;
            }
            keys = match state {
                KeyState::Pressed => Keys(keys.0 | key.bits().0),
                KeyState::Released => Keys(keys.0 & !key.bits().0),
            };
            d.core.set_keys(keys);
            inputs.next();
        }
        let mut until = max;
        if let Some(&(at, ..)) = inputs.peek() {
            until = until.min(at);
        }
        if !at_instruction {
            until = until.min(forced_at.unwrap_or(u64::MAX)).min(next_check);
        }
        match d.advance(until, at_instruction)? {
            Event::Serial => continue,
            Event::Locked(op) => return Ok(Verdict::fail(format!("the CPU locked on {op:#04x}"))),
            Event::Stopped if inputs.peek().is_none() => {
                return Ok(Verdict::fail("the CPU stopped with no wake source"));
            }
            Event::Stopped => {
                // Waiting in STOP for a scheduled input: skip ahead to it.
                d.idle_until(inputs.peek().map_or(max, |i| i.0));
                continue;
            }
            Event::LdBB => {
                if let Some(v) = check(d, case, reference)? {
                    return Ok(v);
                }
                if reference.is_some() {
                    d.next_frame()?;
                    if let Some(v) = check(d, case, reference)? {
                        return Ok(v);
                    }
                }
                return Ok(Verdict::fail("expectation not met at LD B,B"));
            }
            Event::Budget => {}
        }
        let now = d.elapsed();
        if forced_at == Some(now) {
            return Ok(check(d, case, reference)?
                .unwrap_or_else(|| Verdict::fail("expectation not met at check_at_tick")));
        }
        if now >= next_check {
            next_check += CHECK_INTERVAL_T * TICKS_PER_T;
            if let Some(v) = check(d, case, reference)? {
                return Ok(v);
            }
        }
        if now >= max {
            return Ok(Verdict::fail(format!(
                "expectation not met within {} T-cycles",
                max / TICKS_PER_T
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gbcli_core::{FrameTag, Ticks};

    #[test]
    fn a_case_with_inputs_and_memory_parses() {
        let case: Case = serde_json::from_str(
            r#"{"rom": "x.gb", "memory": [{"address": 65520, "value": 1, "fail_value": 2}],
                "inputs": [{"trigger_at_tick": 8192, "key": "up", "state": "pressed"}],
                "check_at_instruction": "ld b,b", "palette": "grey", "limit_speed": true}"#,
        )
        .unwrap();
        assert_eq!(
            case.memory.unwrap()[0],
            MemoryExpectation {
                address: 0xFFF0,
                value: 1,
                fail_value: Some(2)
            }
        );
        assert_eq!(case.inputs[0].key, Key::Up);
        assert_eq!(case.inputs[0].state, KeyState::Pressed);
        assert_eq!(case.check_at_instruction, Some(Instruction::LdBB));
        assert_eq!(case.palette, Some(DocboyPalette::Grey));
    }

    #[test]
    fn rgb565_truncates_like_docboy() {
        assert_eq!(rgb565([0xFF, 0xFF, 0xFF]), 0xFFFF);
        assert_eq!(rgb565([0xAA, 0xAA, 0xAA]), 0xAD55);
        assert_eq!(rgb565([0x55, 0x55, 0x55]), 0x52AA);
    }

    #[test]
    fn frames_take_docboy_colors_and_the_lcd_off_color() {
        let mut tag = FrameTag {
            number: 1,
            kind: VblankKind::Normal,
            at: Ticks(0),
        };
        let frame = FrameRef {
            tag,
            width: 4,
            height: 1,
            pixels: Pixels::Shades(&[0, 1, 2, 3]),
        };
        assert_eq!(
            frame_rgb565(&frame, None).unwrap(),
            vec![0x84A0, 0x4B40, 0x2AA0, 0x1200]
        );
        assert_eq!(
            frame_rgb565(&frame, Some(DocboyPalette::Grey)).unwrap(),
            vec![0xFFFF, 0xAD55, 0x52AA, 0]
        );
        tag.kind = VblankKind::LcdOff;
        let off = FrameRef { tag, ..frame };
        assert_eq!(frame_rgb565(&off, None).unwrap(), vec![0xAD00; 4]);
    }

    #[test]
    fn memory_check_passes_on_all_matches_and_fails_on_any_fail_value() {
        let expected = [
            MemoryExpectation {
                address: 1,
                value: 1,
                fail_value: Some(2),
            },
            MemoryExpectation {
                address: 2,
                value: 1,
                fail_value: None,
            },
        ];
        assert_eq!(memory_check(|_| 1, &expected), Some(Verdict::Pass));
        assert_eq!(
            memory_check(|a| if a == 1 { 1 } else { 0 }, &expected),
            None
        );
        assert!(matches!(
            memory_check(|_| 2, &expected),
            Some(Verdict::Fail(_))
        ));
    }
}
