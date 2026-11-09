//! Indexed art's slots, and how they feed the packer's budget.

use gbcli_protocol::{
    GbcPalette, ImageLimit, NamedPalette, NamedPalettes, PaletteSlot, Rgb555, Severity,
};

use super::{ObjSlots, assigned, by_obp};
use crate::images::pack::{Layer, Source, pack};
use crate::images::pixels::Pixels;

fn named(names: &[&str], assignments: &[(&str, &str)]) -> NamedPalettes {
    NamedPalettes {
        dmg: None,
        palettes: names
            .iter()
            .enumerate()
            .map(|(i, n)| NamedPalette {
                name: n.to_string(),
                colors: GbcPalette([Rgb555::new(i as u8, 0, 0).unwrap(); 4]),
            })
            .collect(),
        assignments: assignments
            .iter()
            .map(|(i, n)| (i.to_string(), n.to_string()))
            .collect(),
    }
}

/// The OBP of a sheet drawn in all three of shades 1-3: each index is its
/// shade, so its slot is the named palette as listed.
const E4: u8 = 0b11_10_01_00;

/// Entry `i` is the color of the shade OBP gives index `i`; index 0, never
/// drawn, takes shade 0's.
#[test]
fn a_sheet_s_slot_is_its_palette_reordered_by_its_obp() {
    let p = GbcPalette([0, 1, 2, 3].map(|i| Rgb555::new(i, 0, 0).unwrap()));
    let at = |shades: [u8; 4]| GbcPalette(shades.map(|i| p.0[usize::from(i)]));
    assert_eq!(by_obp(p, E4), p);
    assert_eq!(by_obp(p, 0b00_11_10_00), at([0, 2, 3, 0]));
    assert_eq!(by_obp(p, 0b11_01_00_00), at([0, 0, 1, 3]));
    assert_eq!(by_obp(p, 0b00_00_11_00), at([0, 3, 0, 0]));
}

/// One palette drawn with two OBPs takes two slots, the lower OBP first;
/// palettes keep their file order.
#[test]
fn a_palette_takes_a_slot_per_obp_in_file_order_then_by_value() {
    let n = named(
        &["forest", "ember"],
        &[
            ("sprites/a.png", "ember"),
            ("sprites/b.png", "ember"),
            ("sprites/c.png", "ember"),
            ("sprites/d.png", "forest"),
        ],
    );
    let slots = ObjSlots::new(
        &n,
        &[
            ("sprites/a.png", 0b00_11_10_00),
            ("sprites/b.png", 0b00_11_00_00),
            ("sprites/c.png", 0b00_11_10_00),
            ("sprites/d.png", E4),
        ],
    );
    assert_eq!(slots.reserved(), 3);
    assert_eq!(slots.slot("forest", E4), PaletteSlot::new(0));
    assert_eq!(slots.slot("ember", 0b00_11_00_00), PaletteSlot::new(1));
    assert_eq!(slots.slot("ember", 0b00_11_10_00), PaletteSlot::new(2));
    let ember = n.palettes[1].colors;
    assert_eq!(
        slots.colors(),
        [
            n.palettes[0].colors,
            by_obp(ember, 0b00_11_00_00),
            by_obp(ember, 0b00_11_10_00)
        ]
    );
}

/// Seven 8x16 objects of three opaque colors each, none shared: seven OBJ
/// palettes.
fn seven_objects() -> Pixels {
    let (w, h) = (56, 16);
    let rgba = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y): (u32, u32)| {
            let n = (x / 8) * 3 + (x + y) % 3;
            let [r, g, b] = Rgb555::new((n % 32) as u8, (n / 32) as u8 + 1, 0)
                .unwrap()
                .channels();
            [r << 3, g << 3, b << 3, 0xFF]
        })
        .collect();
    Pixels {
        width: w,
        height: h,
        rgba,
    }
}

#[test]
fn sheets_assigned_one_palette_share_one_slot_in_file_order() {
    let n = named(
        &["forest", "ember", "spare"],
        &[
            ("sprites/a.png", "ember"),
            ("sprites/b.png", "ember"),
            ("sprites/c.png", "forest"),
            ("bg/town.png", "spare"),
        ],
    );
    let slots = ObjSlots::new(
        &n,
        &[
            ("sprites/a.png", E4),
            ("sprites/b.png", E4),
            ("sprites/c.png", E4),
        ],
    );
    assert_eq!(
        slots.reserved(),
        2,
        "a background's palette takes no OBJ slot"
    );
    assert_eq!(slots.slot("forest", E4), PaletteSlot::new(0));
    assert_eq!(slots.slot("ember", E4), PaletteSlot::new(1));
    assert_eq!(slots.slot("ember", 0), None, "no sheet draws ember so");
    assert_eq!(slots.slot("spare", E4), None);
    assert_eq!(slots.colors(), [n.palettes[0].colors, n.palettes[1].colors]);
    assert_eq!(
        assigned(&n, "bg/town.png").map(|p| p.name.as_str()),
        Some("spare")
    );
    assert_eq!(assigned(&n, "bg/sky.png"), None);
}

/// The assigned slots are the packer's `reserved`: two of them leave six
/// for color sheets, so seven packed palettes overflow, and a fit starts
/// after them.
#[test]
fn the_slot_count_is_the_packers_reserved() {
    let n = named(
        &["forest", "ember"],
        &[("sprites/a.png", "forest"), ("sprites/b.png", "ember")],
    );
    let slots = ObjSlots::new(&n, &[("sprites/a.png", E4), ("sprites/b.png", E4)]);
    let pixels = seven_objects();
    let sources = [Source {
        image: "assets/sprites/c.png",
        pixels: &pixels,
        cell_height: 16,
    }];
    assert!(pack(&sources, Layer::Obj, 0).is_ok(), "seven fit alone");
    let problems = pack(&sources, Layer::Obj, slots.reserved()).unwrap_err();
    assert_eq!(problems[0].limit, ImageLimit::GbcPalettes);
    assert_eq!(problems[0].count, 9);
    assert!(
        problems[0].message.contains("2 assigned"),
        "{}",
        problems[0].message
    );
    let one = named(&["forest"], &[("sprites/a.png", "forest")]);
    let slots = ObjSlots::new(&one, &[("sprites/a.png", E4)]);
    let packing = pack(&sources, Layer::Obj, slots.reserved()).unwrap();
    assert_eq!(packing.first_slot, 1);
    assert_eq!(packing.cell_palettes[0][0], PaletteSlot::new(1));
}

#[test]
fn nine_assigned_palettes_are_one_problem_past_eight() {
    let names: Vec<String> = (0..9).map(|i| format!("p{i}")).collect();
    let sheets: Vec<String> = (0..9).map(|i| format!("sprites/h{i}.png")).collect();
    let pairs: Vec<(&str, &str)> = sheets
        .iter()
        .zip(&names)
        .map(|(s, n)| (s.as_str(), n.as_str()))
        .collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let n = named(&refs, &pairs);
    let sheet_refs: Vec<(&str, u8)> = sheets.iter().map(|s| (s.as_str(), E4)).collect();
    let slots = ObjSlots::new(&n, &sheet_refs);
    assert_eq!(slots.slot("p8", E4), None);
    let p = slots.problem("assets/palettes.txt").expect("over eight");
    assert_eq!((p.severity, p.count, p.max), (Severity::Error, 9, 8));
    let eight = ObjSlots::new(&n, &sheet_refs[..8]);
    assert_eq!(eight.problem("assets/palettes.txt"), None);
    let one: Vec<(&str, u8)> = sheet_refs.iter().map(|&(s, _)| (s, E4)).collect();
    let n = named(
        &["p0"],
        &one.iter().map(|&(s, _)| (s, "p0")).collect::<Vec<_>>(),
    );
    let nine_obps: Vec<(&str, u8)> = one
        .iter()
        .zip(1u8..)
        .map(|(&(s, _), o)| (s, o << 2))
        .collect();
    let p = ObjSlots::new(&n, &nine_obps)
        .problem("assets/palettes.txt")
        .expect("one palette by nine OBPs is nine slots");
    assert_eq!((p.count, p.max), (9, 8));
}
