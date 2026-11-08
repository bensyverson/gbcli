//! rgbgfx, run on an image that passed its checks, and the data it wrote.
//! Every image is converted without `-u`, every tile as drawn, and gbcli
//! keeps the tiles its dedup chose: a background's unique tiles, each from
//! its first cell, with gbcli's map; a sheet's units, with `-Z` so each
//! 8x16 unit's two tiles stay adjacent. gbcli's dedup is the one that
//! counts, since under `--gbc` a tile is one only when its DMG and GBC
//! versions both match (plan question 1), which rgbgfx can't see from one
//! version; its limits are gbcli's checks (`bg::check`, `sheet::check`).
//! Every run passes `-c` with the image's own colors at the indices of
//! their shades. What rgbgfx wrote is then compared with gbcli's plan, cell
//! by cell (`agree`); a difference stops the build. A GBC version is
//! converted beside it (`gbc`).

use std::fs;
use std::path::Path;
use std::time::Duration;

use gbcli_protocol::{Diagnostic, ImageProblem, Severity, TileAt, Tool};

use super::agree::{self, Disagreement};
use super::bg::BgPlan;
use super::sheet::SheetPlan;
use crate::diagnostics;
use crate::project::BuildError;
use crate::project::tool;

/// One run's context: where, with what, and what it reported.
pub(crate) struct Rgbgfx<'a> {
    pub path: &'a Path,
    /// The working directory, which the data's paths are relative to.
    pub root: &'a Path,
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
    /// Converts a background (or a tileset) and keeps the plan's unique
    /// tiles, each from its first cell, and writes them: `out` is the
    /// data's path without extension, relative to `root`. The plan's map
    /// comes back for the caller to write, since a tileset has none of its
    /// own. `None` when rgbgfx failed (its diagnostics say why) or
    /// disagreed.
    pub fn background(
        &mut self,
        image: &str,
        source: &Path,
        plan: &BgPlan,
        out: &str,
    ) -> Result<Option<BgData>, BuildError> {
        let raw_path = format!("{out}.raw.2bpp");
        let args = ["-c", &plan.spec, "-o", &raw_path];
        let ran = self.run(image, &args, source)?;
        let raw = ran.then(|| self.read(&raw_path)).transpose();
        let _ = fs::remove_file(self.root.join(&raw_path));
        let Some(raw) = raw? else {
            return Ok(None);
        };
        let cells = plan.cells.len();
        if raw.len() != cells * 16 {
            self.error(
                image,
                format!(
                    "rgbgfx wrote {} tiles for a {}x{}-tile background",
                    raw.len() / 16,
                    plan.width,
                    plan.height
                ),
            );
            return Ok(None);
        }
        let mut off = Vec::new();
        let mut firsts: Vec<Option<usize>> = vec![None; plan.unique.len()];
        for (i, &unique) in plan.cells.iter().enumerate() {
            firsts[unique].get_or_insert(i);
            let tile = TileAt {
                x: i as u32 % plan.width,
                y: i as u32 / plan.width,
            };
            let wrote = &raw[i * 16..i * 16 + 16];
            if let Some(d) = agree::first_difference(&plan.unique[unique], wrote) {
                off.push(Disagreement {
                    tile,
                    pixel: Some(d),
                });
            }
        }
        if !off.is_empty() {
            self.problems.push(agree::problem(image, &off));
            return Ok(None);
        }
        let tiles: Vec<u8> = firsts
            .iter()
            .flat_map(|c| {
                let c = c.expect("every unique tile has a cell");
                raw[c * 16..c * 16 + 16].iter().copied()
            })
            .collect();
        let map: Vec<u8> = plan.cells.iter().map(|&u| u as u8).collect();
        self.write(&format!("{out}.2bpp"), &tiles)?;
        Ok(Some(BgData { tiles, map }))
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
        let _ = fs::remove_file(self.root.join(&all));
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
    pub(super) fn run(
        &mut self,
        image: &str,
        args: &[&str],
        source: &Path,
    ) -> Result<bool, BuildError> {
        let mut all: Vec<String> = vec!["--color".into(), "never".into()];
        all.extend(args.iter().map(|a| a.to_string()));
        all.push(source.display().to_string());
        let ran = tool::run(self.path, &all, self.root, self.timeout).map_err(|error| {
            BuildError::Io {
                path: self.path.to_path_buf(),
                error,
            }
        })?;
        // rgbgfx names the file it read, which may be the transient merged
        // copy (`shared`); the user drew, and should hear of, the source.
        let given = source.display().to_string();
        let stderr = ran.stderr.replace(&given, image);
        let mut found = diagnostics::parse(Tool::Rgbgfx, &stderr);
        for d in &mut found {
            d.file.get_or_insert_with(|| image.to_string());
        }
        let reported = found.iter().any(|d| d.severity == Severity::Error);
        self.diagnostics.extend(found);
        match ran.status {
            Some(status) if status.success() => Ok(true),
            Some(status) => {
                if !reported {
                    let said = stderr.trim();
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

    fn write(&self, rel: &str, data: &[u8]) -> Result<(), BuildError> {
        let path = self.root.join(rel);
        fs::write(&path, data).map_err(|error| BuildError::Io { path, error })
    }

    pub(super) fn read(&self, rel: &str) -> Result<Vec<u8>, BuildError> {
        let path = self.root.join(rel);
        fs::read(&path).map_err(|error| BuildError::Io { path, error })
    }

    pub(super) fn error(&mut self, image: &str, message: String) {
        self.diagnostics.push(Diagnostic {
            file: Some(image.to_string()),
            ..Diagnostic::new(Tool::Rgbgfx, Severity::Error, message)
        });
    }
}
