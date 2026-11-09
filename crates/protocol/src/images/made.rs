//! Reading an [`ImagesMade`]: what ties its images to what they share on
//! the GBC. Each image's own map is checked as it is read (`asset`); here,
//! an image with a GBC version needs the shared `gbc` beside it, and a
//! sheet's slots must be among its OBJ palettes, as the build makes them.

use serde::Deserialize;

use crate::font::FontAsset;

use super::{GbcImagesMade, ImageAsset, ImageKind, ImageProblem, ImagesMade, ShadeChoice};

/// [`ImagesMade`] as sent, before its images are checked against `gbc`.
#[derive(Deserialize)]
pub(super) struct Unchecked {
    #[serde(default)]
    include: Option<String>,
    images: Vec<ImageAsset>,
    #[serde(default)]
    problems: Vec<ImageProblem>,
    #[serde(default)]
    shades: Option<ShadeChoice>,
    #[serde(default)]
    gbc: Option<GbcImagesMade>,
    #[serde(default)]
    fonts: Vec<FontAsset>,
    #[serde(default)]
    font_overlaps: Vec<crate::font_author::FontOverlap>,
}

impl TryFrom<Unchecked> for ImagesMade {
    type Error = String;

    fn try_from(u: Unchecked) -> Result<ImagesMade, String> {
        let obj = u.gbc.as_ref().map(|g| g.obj_palettes.len());
        for image in &u.images {
            if image.gbc.is_none() {
                continue;
            }
            let Some(obj) = obj else {
                return Err(format!(
                    "{} has a GBC version, but the images have no gbc section",
                    image.source
                ));
            };
            if image.kind != ImageKind::Sprites {
                continue;
            }
            let past = image
                .tile_map
                .iter()
                .flatten()
                .filter_map(|e| e.palette)
                .find(|s| usize::from(s.get()) >= obj);
            if let Some(slot) = past {
                return Err(format!(
                    "{} names OBJ palette {}, but gbc.obj_palettes lists {obj}",
                    image.source,
                    slot.get()
                ));
            }
        }
        Ok(ImagesMade {
            include: u.include,
            images: u.images,
            problems: u.problems,
            shades: u.shades,
            gbc: u.gbc,
            fonts: u.fonts,
            font_overlaps: u.font_overlaps,
        })
    }
}
