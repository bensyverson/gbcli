//! The limits the images break together, counted in joint tiles under
//! `--gbc` (each tile one only when both versions match, plan question 1).

use gbcli_protocol::{ImageLimit, ImageProblem};

use super::{Checked, Plan, census, sheet};

/// VRAM's tiles on the DMG, shared by the background and objects.
const VRAM_TILES: u32 = 384;

/// The limits the images break together: each background with every sheet
/// must fit VRAM's 384 tiles (one background on screen at a time, all
/// sheets loaded), and the sheets the 256 tiles objects address.
pub(super) fn vram(plans: &[Checked]) -> Vec<ImageProblem> {
    let sheets: Vec<(&str, u32)> = plans
        .iter()
        .filter(|c| matches!(c.plan, Plan::Sheet(_)))
        .map(|c| (c.image.as_str(), c.plan.tiles()))
        .collect();
    let sprite_tiles: u32 = sheets.iter().map(|(_, n)| n).sum();
    let listed = sheets
        .iter()
        .map(|(image, n)| format!("{image} {n}"))
        .collect::<Vec<_>>()
        .join(", ");
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
                     {total} of the {VRAM_TILES} tiles VRAM holds for both",
                    bg.tiles()
                ),
                "cut unique tiles from the background or the sheets (flipped copies of an \
                 8x16 object are free), or load them at different times as separate images",
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
                    "the sprite sheets need {sprite_tiles} tiles ({listed}); objects address {}",
                    sheet::OBJ_TILES
                ),
                "reuse units across frames, or split the sheets so fewer load at once",
            )
        });
    }
    problems
}
