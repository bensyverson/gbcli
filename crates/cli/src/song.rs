//! `song`: argv to `gbcli-music`, and its results as a response, so rows
//! and `--json` go through the same renderer as every other verb.

use std::path::{Path, PathBuf};

use gbcli_music::{Loaded, SongError, SongFormat, piano_roll, to_text, write_uge};
use gbcli_protocol::{
    ErrorBody, ErrorCode, Response, ResultBody, SongFileFormat, SongMade, SongOutput,
    SongOutputKind, SongProblem,
};

use crate::argv::Invocation;
use crate::build::Command;
use crate::{Env, Io, render};

/// The response id for a verb answered here rather than by the server.
const ID: &str = "cli";

/// A path as typed, and where it resolves.
#[derive(Debug)]
struct Given {
    shown: String,
    path: PathBuf,
}

/// A parsed `song`.
#[derive(Debug)]
pub struct SongCommand {
    input: Given,
    outputs: Vec<(SongOutputKind, Given)>,
    json: bool,
}

pub fn command(inv: &Invocation, env: &Env) -> Command {
    let given = |shown: &str| Given {
        shown: shown.to_string(),
        path: env.cwd.join(shown),
    };
    let outputs = [
        ("text", SongOutputKind::Text),
        ("uge", SongOutputKind::Uge),
        ("piano", SongOutputKind::Piano),
    ]
    .into_iter()
    .filter_map(|(flag, kind)| inv.one(flag).map(|p| (kind, given(p))))
    .collect();
    Command::Song(Box::new(SongCommand {
        input: given(&inv.positionals[0]),
        outputs,
        json: inv.has("json"),
    }))
}

pub fn song(command: SongCommand, io: Io) -> u8 {
    let response = match made(&command) {
        Ok((made, warnings)) => Response::ok(
            ID,
            None,
            ResultBody {
                song: Some(made),
                ..ResultBody::default()
            },
        )
        .with_warnings(warnings),
        Err(body) => Response::failed(ID, None, *body),
    };
    render::response(&response, command.json, io.out, io.err)
}

/// What the song made, and what of it hUGEDriver never plays.
fn made(command: &SongCommand) -> Result<(SongMade, Vec<String>), Box<ErrorBody>> {
    let input = &command.input;
    let bytes = std::fs::read(&input.path).map_err(|e| {
        Box::new(ErrorBody::new(
            ErrorCode::SongUnreadable,
            format!("{}: {e}", input.shown),
            "check the path; a song file is a hUGETracker .uge or song text",
        ))
    })?;
    let loaded = gbcli_music::read_song(&bytes).map_err(|e| problem(&input.shown, e))?;
    // Everything is made before anything is written, so a failure writes
    // nothing.
    let mut files = Vec::new();
    for (kind, given) in &command.outputs {
        let (bytes, mut output) = match kind {
            SongOutputKind::Text => (to_text(&loaded.song).into_bytes(), output(*kind, given)),
            SongOutputKind::Uge => (write_uge(&loaded.song), output(*kind, given)),
            SongOutputKind::Piano => piano(&loaded, given, &input.shown)?,
        };
        output.bytes = bytes.len() as u64;
        files.push((given, bytes, output));
    }
    let mut wrote = Vec::new();
    for (given, bytes, output) in files {
        write(&given.path, &given.shown, &bytes)?;
        wrote.push(output);
    }
    let song = &loaded.song;
    let warnings = gbcli_music::unplayable(song)
        .iter()
        .map(ToString::to_string)
        .collect();
    let made = SongMade {
        path: input.shown.clone(),
        format: match loaded.format {
            SongFormat::Uge => SongFileFormat::Uge,
            SongFormat::Text => SongFileFormat::Text,
        },
        uge_version: loaded.uge_version,
        name: String::from_utf8_lossy(song.name.bytes()).into_owned(),
        instruments: song
            .instruments
            .iter()
            .enumerate()
            .filter(|(i, ins)| **ins != gbcli_music::Instrument::default_for(*i))
            .count() as u32,
        patterns: song.patterns.len() as u32,
        orders: song.orders.iter().map(Vec::len).max().unwrap_or(0) as u32,
        wrote,
    };
    Ok((made, warnings))
}

fn output(kind: SongOutputKind, given: &Given) -> SongOutput {
    SongOutput {
        kind,
        path: given.shown.clone(),
        bytes: 0,
        width: None,
        height: None,
        notes: None,
        rows: None,
    }
}

fn piano(
    loaded: &Loaded,
    given: &Given,
    input: &str,
) -> Result<(Vec<u8>, SongOutput), Box<ErrorBody>> {
    let made = piano_roll(&loaded.song).map_err(|_| {
        Box::new(ErrorBody::new(
            ErrorCode::BadRequest,
            format!("{input} plays no rows, so its piano roll would be empty"),
            "give the song an orders block that plays a pattern, or drop --piano",
        ))
    })?;
    let png = made.image.png().map_err(|e| {
        Box::new(ErrorBody::new(
            ErrorCode::Internal,
            format!("encoding the piano roll: {e}"),
            ErrorCode::Internal.default_remedy(),
        ))
    })?;
    let output = SongOutput {
        width: Some(made.image.width),
        height: Some(made.image.height),
        notes: Some(made.notes as u32),
        rows: Some(made.rows),
        ..output(SongOutputKind::Piano, given)
    };
    Ok((png, output))
}

fn write(path: &Path, shown: &str, bytes: &[u8]) -> Result<(), Box<ErrorBody>> {
    std::fs::write(path, bytes).map_err(|e| {
        Box::new(ErrorBody::new(
            ErrorCode::IoFailed,
            format!("writing {shown}: {e}"),
            ErrorCode::IoFailed.default_remedy(),
        ))
    })
}

/// A song that didn't load, as `song_unreadable` with the problem located.
fn problem(path: &str, e: SongError) -> Box<ErrorBody> {
    let (line, column, offset) = match &e {
        SongError::Text(t) => (Some(t.line), Some(t.column), None),
        SongError::Uge(u) => (None, None, Some(u.offset as u64)),
    };
    let separator = if line.is_some() { ":" } else { ": " };
    let mut body = ErrorBody::new(
        ErrorCode::SongUnreadable,
        format!("{path}{separator}{e}"),
        e.remedy(),
    );
    let (message, remedy) = match &e {
        SongError::Text(t) => (t.message.clone(), t.remedy.clone()),
        SongError::Uge(u) => (u.message.clone(), u.remedy.clone()),
    };
    body.result = Some(Box::new(ResultBody {
        song_problem: Some(SongProblem {
            path: path.to_string(),
            line,
            column,
            offset,
            message,
            remedy,
        }),
        ..ResultBody::default()
    }));
    Box::new(body)
}
