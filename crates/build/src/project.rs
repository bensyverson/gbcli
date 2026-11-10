//! `build`: a project built by convention, with no build file. Every `.asm`
//! under the source directories is assembled with the include directories on
//! rgbasm's include path, the objects are linked into one ROM with its `.sym`
//! and `.map`, and rgbfix fixes the header. A header with no MBC that the
//! link spills past bank 1, or past bank 0, or that holds a section bigger
//! than a 16 KB bank, is linked again as one 32 KB bank (`banks`). Each
//! step's outputs are checked to exist and be
//! non-empty (DX 10). The ROM, `.sym` and `.map` are written to a staging
//! directory of this build's own beside the final ones and moved into place
//! only when every step passed, the ROM last, so a failed build never leaves
//! a stale or half-written ROM where a session would load it. Every tool
//! runs with `--color never` and under a time limit.

mod banks;
mod layout;
mod staging;
pub(crate) mod tool;

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use gbcli_protocol::{
    BankOverflow, BuildMade, BuildTarget, Diagnostic, ErrorBody, ErrorCode, OutputKind, OutputMade,
    ResultBody, Severity, Tool,
};

use crate::diagnostics;
use crate::paths::{display, quote};
use crate::toolchain::Toolchain;
use layout::Layout;
pub use layout::{default_rom, default_target_of};
use staging::Staging;

/// What to build, and how paths are printed. Relative paths are relative to
/// `project`.
#[derive(Clone, Debug)]
pub struct BuildOptions {
    pub project: PathBuf,
    /// Source directories; default `src`.
    pub src: Vec<PathBuf>,
    /// Include directories; default `include`, when it exists.
    pub include: Vec<PathBuf>,
    /// The ROM; default `build/<project directory name>.gb`. The `.sym` and
    /// `.map` sit beside it.
    pub out: Option<PathBuf>,
    /// Paths in the result are relative to this when inside it.
    pub display_base: PathBuf,
    /// How long one tool run may take before it is stopped and the build
    /// fails; default [`TOOL_TIMEOUT`].
    pub tool_timeout: Duration,
    /// The machine the ROM is for; default the DMG.
    pub target: BuildTarget,
}

/// How long one rgbasm, rgblink or rgbfix run may take by default. The
/// starter assembles in milliseconds; a run this long is a tool stuck in a
/// loop (a huge `REPT`), and a build server must not hang on it.
pub const TOOL_TIMEOUT: Duration = Duration::from_secs(120);

impl BuildOptions {
    /// The conventional layout of the project at `project`.
    pub fn new(project: PathBuf, display_base: PathBuf) -> BuildOptions {
        BuildOptions {
            project,
            src: Vec::new(),
            include: Vec::new(),
            out: None,
            display_base,
            tool_timeout: TOOL_TIMEOUT,
            target: BuildTarget::Dmg,
        }
    }
}

/// Why a build produced no ROM.
#[derive(Debug)]
pub enum BuildError {
    /// The project, a source directory or an include directory isn't there,
    /// or there is nothing to assemble.
    Layout { message: String, remedy: String },
    /// A tool reported errors or wrote no output; the report says which.
    /// `again` is the command that builds the same way, for the remedy.
    Failed { made: Box<BuildMade>, again: String },
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
    /// A song under `assets/music` doesn't read (`crate::music`).
    Song {
        path: String,
        error: gbcli_music::SongError,
    },
}

impl BuildError {
    pub fn body(&self) -> ErrorBody {
        match self {
            BuildError::Layout { message, remedy } => {
                ErrorBody::new(ErrorCode::BadRequest, message, remedy)
            }
            BuildError::Failed { made, again } => {
                if let Some(p) = made.images.as_ref().and_then(|i| i.first_error()) {
                    let image = p
                        .image
                        .as_deref()
                        .map_or(String::new(), |i| format!("{i}: "));
                    let mut body = ErrorBody::new(
                        ErrorCode::BuildFailed,
                        format!(
                            "the build failed: {image}{} ({} of {}): {}",
                            p.limit.name(),
                            p.count,
                            p.max,
                            p.message
                        ),
                        p.remedy.clone(),
                    );
                    body.result = Some(Box::new(ResultBody {
                        build: Some((**made).clone()),
                        ..ResultBody::default()
                    }));
                    return body;
                }
                let errors: Vec<&Diagnostic> = made
                    .diagnostics
                    .iter()
                    .filter(|d| d.severity == Severity::Error)
                    .collect();
                // The overflow is the cause only when its row leads: a
                // one-bank link that failed on something else leads with
                // that error instead (`build`), and the overflow, still true
                // of the first link, rides along. The derived row names the
                // section it couldn't place after this lead.
                if let Some(o) = &made.overflow
                    && let Some(d) = errors.first()
                    && d.message.starts_with(&overflow_lead(o))
                {
                    let mut body = ErrorBody::new(
                        ErrorCode::BuildFailed,
                        format!("the build failed: {}", d.message),
                        OVERFLOW_REMEDY,
                    );
                    body.result = Some(Box::new(ResultBody {
                        build: Some((**made).clone()),
                        ..ResultBody::default()
                    }));
                    return body;
                }
                let first = errors.first().map_or_else(
                    || "no diagnostic".to_string(),
                    |d| {
                        let place = match (&d.file, d.line) {
                            (Some(file), Some(line)) => format!("{file}:{line}: "),
                            (Some(file), None) => format!("{file}: "),
                            _ => String::new(),
                        };
                        let times = match (d.count, d.line, d.last_line) {
                            (1, _, _) => String::new(),
                            (n, Some(line), Some(last)) => {
                                format!(", {n} times on lines {line}-{last}")
                            }
                            (n, _, _) => format!(", {n} times"),
                        };
                        format!("{place}{} ({}{times})", d.message, d.tool.name())
                    },
                );
                // A collapsed run counts once: the count is of the places to
                // fix, as the rows show them.
                let count = match errors.len() {
                    1 => "1 error".to_string(),
                    n => format!("{n} errors, the first"),
                };
                let fix = errors
                    .first()
                    .and_then(|d| diagnostics::hints::remedy(d))
                    .unwrap_or_else(|| "fix the first error at its file and line".into());
                let mut body = ErrorBody::new(
                    ErrorCode::BuildFailed,
                    format!("the build failed with {count}: {first}"),
                    format!("{fix}, then `{again}` again"),
                );
                body.result = Some(Box::new(ResultBody {
                    build: Some((**made).clone()),
                    ..ResultBody::default()
                }));
                body
            }
            BuildError::Io { path, error } => {
                ErrorBody::plain(ErrorCode::IoFailed, format!("{}: {error}", path.display()))
            }
            BuildError::Song { path, error } => crate::music::song_problem(path, error),
        }
    }
}

/// Builds the project. Warnings ride in the result's diagnostics.
pub fn build(tools: &Toolchain, options: &BuildOptions) -> Result<BuildMade, BuildError> {
    let base = fs::canonicalize(&options.display_base).unwrap_or(options.display_base.clone());
    let shown = |p: &Path| display(p, &base);
    let project = fs::canonicalize(&options.project).map_err(|_| BuildError::Layout {
        message: format!("no project directory at {}", shown(&options.project)),
        remedy: "pass the project directory, or make one with `gbcli new DIR`".into(),
    })?;
    let mut layout = Layout::of(&project, options, &shown)?;
    // Images first: the program includes what they become.
    let images = crate::images::convert(&project, &shown, options.target)?;
    if let Some(generated) = images.as_ref().and_then(|i| i.generated.as_ref()) {
        layout.sources.push(generated.source.clone());
        layout.include.push(generated.include_dir.clone());
    }
    let songs = crate::music::convert(&project, &layout.include, &shown)?;
    layout.sources.extend(songs.sources);
    let mut run = Run {
        tools,
        project: &project,
        shown: &shown,
        timeout: options.tool_timeout,
        again: again(&shown(&project), options),
        made: BuildMade {
            rgbds: tools.version().to_string(),
            target: options.target,
            sources: layout.sources.iter().map(|s| shown(s)).collect(),
            outputs: Vec::new(),
            diagnostics: Vec::new(),
            images: None,
            songs: songs.made,
            overflow: None,
        },
    };
    if let Some(images) = images {
        run.made.diagnostics.extend(images.diagnostics);
        run.made.images = Some(images.made);
        if images.stopped {
            return Err(run.failed());
        }
    }

    let mut objects = Vec::new();
    let mut ok = true;
    for source in &layout.sources {
        let rel = source.strip_prefix(&project).unwrap_or(source);
        // A source outside the project (an absolute --src) still gets its
        // object under build/obj/, never beside the source.
        let inside: PathBuf = rel
            .components()
            .filter(|c| matches!(c, Component::Normal(_)))
            .collect();
        let object = layout.objects.join(inside).with_extension("o");
        create_dir(object.parent().expect("an object has a directory"))?;
        let mut args: Vec<String> = Vec::new();
        for dir in &layout.include {
            args.push("-I".into());
            args.push(dir.display().to_string());
        }
        if options.target == BuildTarget::Gbc {
            // Every source can guard its color code with IF DEF(GBCLI_GBC),
            // even in a project with no images and so no images.inc.
            args.extend(["-D".into(), "GBCLI_GBC=1".into()]);
        }
        args.extend(["-o".into(), object.display().to_string()]);
        args.push(rel.display().to_string());
        ok &= run.step(Tool::Rgbasm, &args, Some(source), &[&object])?;
        objects.push(object);
    }
    if !ok {
        return Err(run.failed());
    }

    let out_dir = layout.out.parent().expect("the ROM has a directory");
    create_dir(out_dir)?;
    let staging = Staging::new(out_dir)?;
    let finals = [
        (OutputKind::Rom, layout.out.clone()),
        (OutputKind::Sym, layout.out.with_extension("sym")),
        (OutputKind::Map, layout.out.with_extension("map")),
    ];
    let staged: Vec<PathBuf> = finals
        .iter()
        .map(|(_, p)| staging.path().join(p.file_name().expect("a file name")))
        .collect();
    let before_link = run.made.diagnostics.len();
    let linked = run.link(&objects, &staged, options.target, Banks::Split)?;
    let spill = if linked {
        run.overflow(&staged[0], &staged[2])?.map(Spill::PastBank1)
    } else {
        run.made.diagnostics[before_link..]
            .iter()
            .any(|d| banks::bank_0_full(d) || banks::outgrows_a_bank(d))
            .then_some(Spill::NoRom)
    };
    match spill {
        None if !linked => return Err(run.failed()),
        None => {}
        Some(spill) => {
            // A cartridge with no MBC maps one flat 32 KB, which is how
            // `--tiny` links. Only a build that spilled pays this second
            // link, so a build that fits is linked as it always was.
            //
            // When the one-bank link succeeds, its ROM's header decides
            // whether the build is accepted. When it fails after the first
            // link wrote a ROM, its rows lead: the first link stopped short
            // of them. After a failed first link (a full bank 0, or a section
            // bigger than a bank) there is no ROM, so the header is unknown:
            // only a row true whatever the layout can lead there. A full bank
            // 0 under an MBC still pays a one-bank link it discards: the
            // header byte is in the objects, and nothing short of parsing
            // them reads it without a ROM.
            let first = run.made.diagnostics.split_off(before_link);
            let one_bank = run.link(&objects, &staged, options.target, Banks::Tiny)?;
            if one_bank && (matches!(spill, Spill::PastBank1(_)) || run.names_no_mbc(&staged[0])?) {
                run.made.diagnostics.push(Diagnostic::new(
                    Tool::Rgblink,
                    Severity::Warning,
                    tiny_message(&spill),
                ));
            } else {
                let tiny = run.made.diagnostics.split_off(before_link);
                match spill {
                    Spill::PastBank1(overflow) => {
                        // Linked as one bank, rgblink calls every section,
                        // ROMX too, a ROM0 section. One it can't place
                        // anywhere may have no room because 32 KB is full or
                        // because a pinned section splits the space, so the
                        // derived row names the section and keeps rgblink's
                        // row rather than asserting the total size.
                        match tiny.iter().find_map(banks::unplaced) {
                            Some(unplaced) => {
                                run.error(
                                    Tool::Rgblink,
                                    None,
                                    overflow_message(&overflow, unplaced),
                                );
                                run.made.diagnostics.extend(tiny);
                            }
                            None => {
                                // Any other failure (a ROMX bank past 1, an
                                // overlap, an undefined symbol) is the real
                                // cause, and it leads: the first link stopped
                                // short of it.
                                run.made.diagnostics.extend(tiny);
                                run.made.diagnostics.push(Diagnostic::new(
                                    Tool::Rgblink,
                                    Severity::Warning,
                                    tiny_failed_message(&overflow),
                                ));
                            }
                        }
                        run.made.diagnostics.extend(first);
                        run.made.overflow = Some(overflow);
                    }
                    Spill::NoRom => {
                        // The first link wrote no ROM, so the header is
                        // unknown. A one-bank error true whatever the layout
                        // (an undefined symbol) leads: the first link
                        // stopped short of it. Then the first link's rows.
                        // Every other one-bank row holds only for a header
                        // with no MBC, so it follows as a warning that says
                        // so, never leading and never lending its hint. A
                        // one-bank link that passed under an MBC header said
                        // nothing about this cartridge.
                        let (any_layout, one_bank_only) = if one_bank {
                            (Vec::new(), Vec::new())
                        } else {
                            tiny.into_iter().partition(banks::true_in_any_layout)
                        };
                        run.made.diagnostics.extend(any_layout);
                        run.made.diagnostics.extend(first);
                        run.made
                            .diagnostics
                            .extend(one_bank_only.into_iter().map(only_without_mbc));
                    }
                }
                return Err(run.failed());
            }
        }
    }
    // The `.sym` and `.map` first and the ROM last: the ROM's arrival marks
    // a complete build, so whatever loads it finds the `.sym` it was linked
    // with.
    let mut outputs = Vec::new();
    for i in [1, 2, 0] {
        let ((kind, path), from) = (&finals[i], &staged[i]);
        fs::rename(from, path).map_err(|error| BuildError::Io {
            path: path.clone(),
            error,
        })?;
        let bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        outputs.push(OutputMade {
            kind: *kind,
            path: shown(path),
            bytes,
        });
    }
    outputs.rotate_right(1);
    run.made.outputs = outputs;
    Ok(run.made)
}

fn read(path: &Path) -> Result<Vec<u8>, BuildError> {
    fs::read(path).map_err(|error| BuildError::Io {
        path: path.to_path_buf(),
        error,
    })
}

fn create_dir(dir: &Path) -> Result<(), BuildError> {
    fs::create_dir_all(dir).map_err(|error| BuildError::Io {
        path: dir.to_path_buf(),
        error,
    })
}

/// One build's tool runs and what they reported.
struct Run<'a> {
    tools: &'a Toolchain,
    project: &'a Path,
    shown: &'a dyn Fn(&Path) -> String,
    timeout: Duration,
    /// The command that builds the same way, for a failure's remedy.
    again: String,
    made: BuildMade,
}

impl Run<'_> {
    /// Runs `tool` in the project and records its diagnostics. A step passes
    /// when the tool exits 0 and every one of `outputs` is non-empty;
    /// otherwise it records an error (on `source`, when it has one) unless
    /// the tool already reported one.
    fn step(
        &mut self,
        tool: Tool,
        args: &[String],
        source: Option<&Path>,
        outputs: &[&PathBuf],
    ) -> Result<bool, BuildError> {
        let path = self.tools.path(tool);
        // RGBDS colors its diagnostics when FORCE_COLOR is set, even into a
        // pipe, and colored text doesn't parse.
        let args = [&["--color".to_string(), "never".to_string()], args].concat();
        let ran =
            tool::run(path, &args, self.project, self.timeout).map_err(|error| BuildError::Io {
                path: path.to_path_buf(),
                error,
            })?;
        let stderr = ran.stderr;
        let mut found = diagnostics::parse(tool, &stderr);
        for d in &mut found {
            if let Some(file) = &d.file {
                d.file = Some(self.show(file));
            }
            for at in d.chain.iter_mut().chain(d.also.iter_mut().flatten()) {
                at.file = self.show(&at.file);
            }
        }
        let reported = found.iter().any(|d| d.severity == Severity::Error);
        self.made.diagnostics.extend(found);
        let file = source.map(|s| (self.shown)(s));
        let Some(status) = ran.status else {
            let message = format!(
                "{} ran past {:.1} s and was stopped",
                tool.name(),
                self.timeout.as_secs_f64()
            );
            self.error(tool, file, message);
            return Ok(false);
        };
        if !status.success() {
            if !reported {
                let status = status.code().map_or("was killed by a signal".into(), |c| {
                    format!("exited with status {c}")
                });
                let said = stderr.trim();
                let said = if said.is_empty() {
                    "it printed nothing".to_string()
                } else {
                    said.lines().rev().take(3).collect::<Vec<_>>().join(" / ")
                };
                self.error(tool, file, format!("{} {status}: {said}", tool.name()));
            }
            return Ok(false);
        }
        for out in outputs {
            let name = tool.name();
            let shown = (self.shown)(out);
            let message = match fs::metadata(out).map(|m| m.len()) {
                Ok(0) => format!("{name} exited 0 but left {shown} empty"),
                Ok(_) => continue,
                Err(_) => format!("{name} exited 0 but didn't write {shown}"),
            };
            self.error(tool, file.clone(), message);
            return Ok(false);
        }
        Ok(true)
    }

    /// Links `objects` into `staged` (the ROM, `.sym` and `.map`) and fixes
    /// the ROM's header; whether both steps passed.
    fn link(
        &mut self,
        objects: &[PathBuf],
        staged: &[PathBuf],
        target: BuildTarget,
        banks: Banks,
    ) -> Result<bool, BuildError> {
        // `-p 0xFF`: every byte no section holds (gaps, unused banks) is $FF,
        // as erased flash is, not rgblink's $00; rgbfix pads the tail the
        // same. A flash save carried across a rebuild relies on it: it is
        // kept in a sector the build leaves all $FF
        // (`FlashChip::revert_over_code`).
        let mut args = ["-p", "0xFF"].map(String::from).to_vec();
        if banks == Banks::Tiny {
            // rgblink's tiny mode: ROMX sections are placed as ROM0.
            args.push("-t".into());
        }
        for (flag, path) in ["-o", "-n", "-m"].iter().zip(staged) {
            args.extend([flag.to_string(), path.display().to_string()]);
        }
        args.extend(objects.iter().map(|o| o.display().to_string()));
        if !self.step(
            Tool::Rgblink,
            &args,
            None,
            &staged.iter().collect::<Vec<_>>(),
        )? {
            return Ok(false);
        }
        let mut args = ["-v", "-p", "0xFF"].map(String::from).to_vec();
        if target == BuildTarget::Gbc {
            // Dual mode: $0143 = $80. `-C` (GBC only, $C0) isn't offered.
            args.push("-c".into());
        }
        let rom = staged[0].display().to_string();
        self.step(
            Tool::Rgbfix,
            &[args, vec![rom]].concat(),
            None,
            &[&staged[0]],
        )
    }

    /// The overflow, when the header names no MBC and the link at `rom`
    /// (its map at `map`) went past bank 1.
    fn overflow(&self, rom: &Path, map: &Path) -> Result<Option<BankOverflow>, BuildError> {
        let map_text = String::from_utf8_lossy(&read(map)?).into_owned();
        Ok(banks::overflow(&read(rom)?, &map_text))
    }

    /// Whether the header of the ROM at `rom` names no MBC.
    fn names_no_mbc(&self, rom: &Path) -> Result<bool, BuildError> {
        Ok(banks::names_no_mbc(&read(rom)?))
    }

    /// A file as a tool printed it (relative to the project, where it ran),
    /// as the caller reads it.
    fn show(&self, file: &str) -> String {
        (self.shown)(&self.project.join(file))
    }

    fn error(&mut self, tool: Tool, file: Option<String>, message: String) {
        self.made.diagnostics.push(Diagnostic {
            file,
            ..Diagnostic::new(tool, Severity::Error, message)
        });
    }

    fn failed(self) -> BuildError {
        BuildError::Failed {
            again: self.again,
            made: Box::new(self.made),
        }
    }
}

/// How rgblink lays out ROMX: in 16 KB banks from bank 1 on, or with
/// `--tiny`, as one flat 32 KB bank (ROMX sections placed as ROM0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Banks {
    Split,
    Tiny,
}

/// The sections `o` places past bank 1, each with where and how big.
fn past_bank_1(o: &BankOverflow) -> String {
    let sections: Vec<String> = o
        .sections
        .iter()
        .map(|s| {
            format!(
                "\"{}\" in bank {} (${:04X}, {} bytes)",
                s.name, s.bank, s.start, s.bytes
            )
        })
        .collect();
    format!(
        "{} past bank 1: {}",
        match sections.len() {
            1 => "a section".to_string(),
            n => format!("{n} sections"),
        },
        sections.join(", ")
    )
}

/// The start of the derived row for a one-bank link that couldn't place a
/// section. It names the section after this, so `body` recognizes the row it
/// wrote by the lead rather than the whole message.
fn overflow_lead(o: &BankOverflow) -> String {
    format!(
        "the header names no MBC (cartridge type ${:02X}), so the cartridge maps banks 0 and 1 \
         (32 KB) only; linked as one 32 KB bank (rgblink -t), rgblink could not place",
        o.cart_type
    )
}

/// Why a build whose header names no MBC failed: the one-bank link had
/// nowhere for `unplaced`. The 32 KB may be full or split by a pinned
/// section, so the row names the section and claims no total size.
fn overflow_message(o: &BankOverflow, unplaced: &str) -> String {
    format!(
        "{} \"{unplaced}\" anywhere; linked in 16 KB banks, the link put {}",
        overflow_lead(o),
        past_bank_1(o)
    )
}

/// The row after the errors of a one-bank link that failed on something
/// other than size, saying why the build linked as one bank.
fn tiny_failed_message(o: &BankOverflow) -> String {
    format!(
        "the header names no MBC (cartridge type ${:02X}), so the cartridge maps banks 0 and 1 \
         (32 KB) only, and in 16 KB banks the link put {}; linked again as one 32 KB bank \
         (rgblink -t), it failed on the error above",
        o.cart_type,
        past_bank_1(o)
    )
}

/// Why the first link of a build whose header may name no MBC is linked
/// again as one bank.
enum Spill {
    /// The header names no MBC and rgblink placed sections past bank 1.
    PastBank1(BankOverflow),
    /// rgblink found no room in bank 0 for a ROM0 section, or a section was
    /// bigger than a 16 KB bank, so it wrote no ROM.
    NoRom,
}

/// The warning row for a build linked again as one bank.
fn tiny_message(spill: &Spill) -> String {
    let why = match spill {
        Spill::PastBank1(o) => format!(
            "the header names no MBC (cartridge type ${:02X}) and in 16 KB banks the link put {}",
            o.cart_type,
            past_bank_1(o)
        ),
        Spill::NoRom => "the header names no MBC and in 16 KB banks the link found no room in \
                         bank 0 or a section bigger than a bank"
            .to_string(),
    };
    format!(
        "linked as one 32 KB bank (rgblink -t), since {why}; ROM0 and ROMX sections alike \
         may sit anywhere in $0000-$7FFF"
    )
}

/// `d`, from a one-bank link made when the header is unknown, as a warning
/// that says it holds only for a header with no MBC. It keeps `d`'s tool and
/// place, so an assert's row still names its file and line.
fn only_without_mbc(d: Diagnostic) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message: format!(
            "only if the header names no MBC (unknown: the first link wrote no ROM), which \
             links as one 32 KB bank (rgblink -t): {}",
            d.message
        ),
        ..d
    }
}

/// What to do about a one-bank link that couldn't place a section on a
/// cartridge with no MBC. Its 32 KB may be full or split by a pinned section,
/// so moving a pin is an option beside shrinking.
const OVERFLOW_REMEDY: &str = "move the section the one-bank link couldn't place, or a section \
     pinned to a fixed address that splits the space (a `ROMX[$4000]` section, say), or shrink the \
     sections until they fit in 32 KB (banks 0 and 1), or name an MBC in the header (the byte at \
     $0147, e.g. $01 for MBC1) and switch banks (write the bank number to $2000) around the code \
     that reads each section past bank 1";

/// The command that builds the project shown as `dir` with `options`' layout
/// again, each word quoted for a shell.
fn again(dir: &str, options: &BuildOptions) -> String {
    let mut words = vec!["gbcli".to_string(), "build".to_string()];
    if dir != "." {
        words.push(quote(dir));
    }
    let flags = [
        ("--src", options.src.iter().collect::<Vec<_>>()),
        ("--include", options.include.iter().collect()),
        ("--out", options.out.iter().collect()),
    ];
    for (flag, paths) in flags {
        for path in paths {
            words.push(flag.to_string());
            words.push(quote(&path.display().to_string()));
        }
    }
    if options.target == BuildTarget::Gbc {
        words.push("--gbc".into());
    }
    words.join(" ")
}
