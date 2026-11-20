//! The limits the images break together, counted in joint tiles under
//! `--gbc` (each tile one only when both versions match, plan question 1).

use gbcli_protocol::{ImageLimit, ImageProblem, ObjectSize, Severity};

use super::{Checked, Plan, census, shared, sheet};

/// VRAM's tiles on the DMG, shared by the background and objects.
const VRAM_TILES: u32 = 384;

/// The limits the images break together: each background with every sheet
/// must fit VRAM's 384 tiles (one background on screen at a time, all
/// sheets loaded), and the sheets the 256 tiles objects address. A stored
/// sheet is never loaded whole, so neither count includes it.
pub(super) fn vram(plans: &[Checked]) -> Vec<ImageProblem> {
    let sheets: Vec<(&str, u32)> = plans
        .iter()
        .filter(|c| matches!(c.plan, Plan::Sheet(_)) && c.s.resident)
        .map(|c| (c.image.as_str(), c.plan.tiles()))
        .collect();
    let sprite_tiles: u32 = sheets.iter().map(|(_, n)| n).sum();
    let listed = sheets
        .iter()
        .map(|(image, n)| format!("{image} {n}"))
        .collect::<Vec<_>>()
        .join(", ");
    // Color art counts joint tiles, so a total can pass the DMG versions'.
    let joint = if plans.iter().any(|c| c.color.is_some()) {
        ", counting tiles alike on the DMG but distinct in color as two"
    } else {
        ""
    };
    let mut problems = Vec::new();
    let biggest = plans
        .iter()
        .filter(|c| matches!(c.plan, Plan::Bg(_)))
        .max_by_key(|c| c.plan.tiles());
    if let Some(Checked {
        image, plan: bg, ..
    }) = biggest
        && bg.tiles() + sprite_tiles > VRAM_TILES
    {
        let total = bg.tiles() + sprite_tiles;
        problems.push(ImageProblem {
            image: None,
            ..census::error(
                image,
                ImageLimit::VramTiles,
                total as usize,
                VRAM_TILES as usize,
                format!(
                    "{image} uses {} tiles and the sprite sheets {sprite_tiles} ({listed}): \
                     {total} of the {VRAM_TILES} tiles VRAM holds for both{joint}",
                    bg.tiles()
                ),
                "cut unique tiles from the background or the sheets (flipped copies of an \
                 object are free), or load them at different times as separate images",
            )
        });
    }
    if sheets.len() > 1 && sprite_tiles > sheet::OBJ_TILES as u32 {
        problems.push(ImageProblem {
            image: None,
            ..census::error(
                "",
                ImageLimit::ObjTiles,
                sprite_tiles as usize,
                sheet::OBJ_TILES,
                format!(
                    "the sprite sheets need {sprite_tiles} tiles ({listed}{joint}); objects \
                     address {}",
                    sheet::OBJ_TILES
                ),
                "reuse units across frames, or split the sheets so fewer load at once",
            )
        });
    }
    problems
}

/// With no project object size, sheets cut in both sizes: a warning, not
/// an error, since LCDC bit 2 shows one size at a time but a game may
/// switch it between scenes (main thread's ruling, 2026-11-09). `add` is
/// the remedy's words for setting the size (`PaletteSource::add_objects`).
pub(super) fn object_sizes(plans: &[Checked], add: &str) -> Option<ImageProblem> {
    let of = |size: ObjectSize| -> Vec<String> {
        plans
            .iter()
            .filter(|c| matches!(&c.plan, Plan::Sheet(p) if p.object == size))
            .map(|c| c.image.clone())
            .collect()
    };
    let (tall, square) = (of(ObjectSize::Tall), of(ObjectSize::Square));
    if tall.is_empty() || square.is_empty() {
        return None;
    }
    let message = format!(
        "the sheets are cut in two object sizes, and the screen shows one at a time (LCDC bit \
         2): {} objects in {}; {} objects in {}",
        ObjectSize::Tall.name(),
        shared::and(&tall),
        ObjectSize::Square.name(),
        shared::and(&square),
    );
    let remedy = format!(
        "{add} to cut every sheet in one size; or keep both, and switch LCDC bit 2 between \
         scenes that show only one"
    );
    Some(ImageProblem {
        image: None,
        severity: Severity::Warning,
        ..census::error("", ImageLimit::ObjPair8x16, 2, 1, message, &remedy)
    })
}
