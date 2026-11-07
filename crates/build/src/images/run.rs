//! rgbgfx, run on an image that passed its checks, and the data it wrote.
//! A background is converted with `-u -t` (unique tiles and a tilemap); a
//! sprite sheet with `-Z` and no `-u`, so each 8x16 unit's two tiles stay
//! adjacent, and gbcli keeps the units its dedup chose. A background run
//! passes `-N`, so a count past the map's limit is fatal rather than a
//! warning; a sheet run doesn't, because without `-u` rgbgfx counts the
//! sheet's raw tiles, blanks and repeats included, while the limit applies
//! to the units gbcli keeps (`sheet::check`). Every run passes `-c` with the
//! image's own colors at the indices of their shades. What rgbgfx wrote is
//! then compared with gbcli's plan (`agree`); a difference stops the build.

use std::fs;
use std::path::Path;
use std::time::Duration;

use gbcli_protocol::{Diagnostic, ImageProblem, Severity, TileAt, Tool};

use super::agree::{self, Disagreement};
use super::bg::{BgPlan, MAP_TILES};
use super::sheet::SheetPlan;
use crate::diagnostics;
use crate::project::BuildError;
use crate::project::tool;

/// One run's context: where, with what, and what it reported.
pub(crate) struct Rgbgfx<'a> {
    pub path: &'a Path,
    pub project: &'a Path,
    pub timeout: Duration,
    pub diagnostics: Vec<Diagnostic>,
    /// Conversions that disagree with gbcli's plan (`agree`).
    pub problems: Vec<ImageProblem>,
}

/// A converted background: unique tiles (2bpp) and its tilemap.
pub(crate) struct BgData {
    pub tiles: Vec<u8>,
    pub map: Vec<u8>,
}

impl Rgbgfx<'_> {
    /// Converts a background; `out` is the data's path without extension,
    /// relative to the project. `None` when rgbgfx failed (its diagnostics
    /// say why).
    pub fn background(
        &mut self,
        image: &str,
        source: &Path,
        plan: &BgPlan,
        out: &str,
    ) -> Result<Option<BgData>, BuildError> {
        let (tiles, map) = (format!("{out}.2bpp"), format!("{out}.tilemap"));
        let args = [
            "-u",
            "-N",
            &MAP_TILES.to_string(),
            "-c",
            &plan.spec,
            "-o",
            &tiles,
            "-t",
            &map,
        ];
        if !self.run(image, &args, source)? {
            return Ok(None);
        }
        let data = BgData {
            tiles: self.read(&tiles)?,
            map: self.read(&map)?,
        };
        let expected = (
            plan.tiles as usize * 16,
            (plan.width * plan.height) as usize,
        );
        if (data.tiles.len(), data.map.len()) != expected {
            self.error(
                image,
                format!(
                    "rgbgfx wrote {} tiles and a {}-byte map; gbcli's check counted {} tiles of a \
                     {}x{} map",
                    data.tiles.len() / 16,
                    data.map.len(),
                    plan.tiles,
                    plan.width,
                    plan.height
                ),
            );
            return Ok(None);
        }
        let mut off = Vec::new();
        for (i, &unique) in plan.cells.iter().enumerate() {
            let tile = TileAt {
                x: i as u32 % plan.width,
                y: i as u32 / plan.width,
            };
            let id = usize::from(data.map[i]) * 16;
            match data.tiles.get(id..id + 16) {
                None => off.push(Disagreement { tile, pixel: None }),
                Some(wrote) => {
                    if let Some(d) = agree::first_difference(&plan.unique[unique], wrote) {
                        off.push(Disagreement {
                            tile,
                            pixel: Some(d),
                        });
                    }
                }
            }
        }
        if !off.is_empty() {
            self.problems.push(agree::problem(image, &off));
            return Ok(None);
        }
        Ok(Some(data))
    }

    /// Converts a sprite sheet and keeps the plan's units: each 32 bytes,
    /// top tile then bottom.
    pub fn sheet(
        &mut self,
        image: &str,
        source: &Path,
        plan: &SheetPlan,
        out: &str,
    ) -> Result<Option<Vec<u8>>, BuildError> {
        if plan.kept.is_empty() {
            return Ok(Some(Vec::new()));
        }
        let all = format!("{out}.sheet.2bpp");
        let args = ["-Z", "-c", &plan.spec, "-o", &all];
        if !self.run(image, &args, source)? {
            return Ok(None);
        }
        let raw = self.read(&all)?;
        let _ = fs::remove_file(self.project.join(&all));
        let rows = plan.height as usize;
        if raw.len() != plan.width as usize * rows * 16 {
            self.error(
                image,
                format!(
                    "rgbgfx wrote {} tiles for a {}x{}-tile sheet",
                    raw.len() / 16,
                    plan.width,
                    plan.height
                ),
            );
            return Ok(None);
        }
        let mut kept = Vec::with_capacity(plan.kept.len() * 32);
        let mut off = Vec::new();
        for (&(ux, uy), planned) in plan.kept.iter().zip(&plan.drawn) {
            // Column-major: down each 8-pixel column, top to bottom.
            let index = (ux / 8) as usize * rows + (uy / 8) as usize;
            let unit = &raw[index * 16..index * 16 + 32];
            if let Some(d) = agree::first_difference(planned, unit) {
                off.push(Disagreement {
                    tile: TileAt {
                        x: ux / 8,
                        y: (uy + d.y) / 8,
                    },
                    pixel: Some(agree::Difference { y: d.y % 8, ..d }),
                });
            }
            kept.extend_from_slice(unit);
        }
        if !off.is_empty() {
            self.problems.push(agree::problem(image, &off));
            return Ok(None);
        }
        Ok(Some(kept))
    }

    /// Runs rgbgfx on `source`; true when it exited 0.
    fn run(&mut self, image: &str, args: &[&str], source: &Path) -> Result<bool, BuildError> {
        let mut all: Vec<String> = vec!["--color".into(), "never".into()];
        all.extend(args.iter().map(|a| a.to_string()));
        all.push(source.display().to_string());
        let ran = tool::run(self.path, &all, self.project, self.timeout).map_err(|error| {
            BuildError::Io {
                path: self.path.to_path_buf(),
                error,
            }
        })?;
        let mut found = diagnostics::parse(Tool::Rgbgfx, &ran.stderr);
        for d in &mut found {
            d.file.get_or_insert_with(|| image.to_string());
        }
        let reported = found.iter().any(|d| d.severity == Severity::Error);
        self.diagnostics.extend(found);
        match ran.status {
            Some(status) if status.success() => Ok(true),
            Some(status) => {
                if !reported {
                    let said = ran.stderr.trim();
                    self.error(image, format!("rgbgfx exited with {status}: {said}"));
                }
                Ok(false)
            }
            None => {
                let limit = self.timeout.as_secs_f64();
                self.error(
                    image,
                    format!("rgbgfx ran past {limit:.1} s and was stopped"),
                );
                Ok(false)
            }
        }
    }

    fn read(&self, rel: &str) -> Result<Vec<u8>, BuildError> {
        let path = self.project.join(rel);
        fs::read(&path).map_err(|error| BuildError::Io { path, error })
    }

    fn error(&mut self, image: &str, message: String) {
        self.diagnostics.push(Diagnostic {
            file: Some(image.to_string()),
            ..Diagnostic::new(Tool::Rgbgfx, Severity::Error, message)
        });
    }
}
