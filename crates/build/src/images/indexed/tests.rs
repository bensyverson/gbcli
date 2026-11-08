//! Indexed art's slots, and how they feed the packer's budget.

use gbcli_protocol::{
    GbcPalette, ImageLimit, NamedPalette, NamedPalettes, PaletteSlot, Rgb555, Severity,
};

use super::{ObjSlots, assigned};
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
    let slots = ObjSlots::new(&n, &["sprites/a.png", "sprites/b.png", "sprites/c.png"]);
    assert_eq!(
        slots.reserved(),
        2,
        "a background's palette takes no OBJ slot"
    );
    assert_eq!(slots.slot("forest"), PaletteSlot::new(0));
    assert_eq!(slots.slot("ember"), PaletteSlot::new(1));
    assert_eq!(slots.slot("spare"), None);
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
    let slots = ObjSlots::new(&n, &["sprites/a.png", "sprites/b.png"]);
    let pixels = seven_objects();
    let sources = [Source {
        image: "assets/sprites/c.png",
        pixels: &pixels,
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
    let slots = ObjSlots::new(&one, &["sprites/a.png"]);
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
    let sheet_refs: Vec<&str> = sheets.iter().map(String::as_str).collect();
    let slots = ObjSlots::new(&n, &sheet_refs);
    assert_eq!(slots.slot("p8"), None);
    let p = slots.problem("assets/palettes.txt").expect("over eight");
    assert_eq!((p.severity, p.count, p.max), (Severity::Error, 9, 8));
    let eight = ObjSlots::new(&n, &sheet_refs[..8]);
    assert_eq!(eight.problem("assets/palettes.txt"), None);
}
