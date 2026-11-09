//! `font` (project/2026-11-09-pixel-fonts-design.md): a `.yaff` font read
//! through `gbcli-font`, summarized, a sample set by the layout rules
//! (`gbcli help fonts`), a preview written, and `--fit` checked.

use std::path::Path;

use gbcli_protocol::{
    ErrorBody, ErrorCode, FontArgs, FontMade, FontPreview, FontProblem, FontSample, ResultBody,
};

const FORMAT: &str = "check the path; a font is a .yaff file (gbcli help fonts)";

/// Reads the font `args` names, sets its sample and writes its preview;
/// paths start at `cwd`.
pub fn font(cwd: &Path, args: &FontArgs) -> Result<FontMade, Box<ErrorBody>> {
    if args.fit.is_some() && args.sample.is_none() {
        return Err(Box::new(ErrorBody::new(
            ErrorCode::BadRequest,
            "--fit measures a sample, but nothing gives one",
            "add --sample TEXT, or drop --fit",
        )));
    }
    let shown = &args.file;
    let path = cwd.join(shown);
    let unreadable = |line: Option<u32>, column: Option<u32>, message: String, remedy: &str| {
        let at = line.map_or(String::new(), |l| match column {
            Some(c) => format!(":{l}:{c}"),
            None => format!(":{l}"),
        });
        let mut body = ErrorBody::new(
            ErrorCode::FontUnreadable,
            format!("{shown}{at}: {message}"),
            remedy,
        );
        body.result = Some(Box::new(ResultBody {
            font_problem: Some(FontProblem {
                path: shown.clone(),
                line,
                column,
                message,
                remedy: remedy.to_string(),
            }),
            ..ResultBody::default()
        }));
        Box::new(body)
    };
    let bytes = std::fs::read(&path).map_err(|e| unreadable(None, None, e.to_string(), FORMAT))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        let line = bytes[..e.valid_up_to()]
            .iter()
            .filter(|b| **b == b'\n')
            .count() as u32
            + 1;
        unreadable(
            Some(line),
            None,
            "this line is not UTF-8; a YAFF file is UTF-8 text".into(),
            FORMAT,
        )
    })?;
    let font = gbcli_font::read(text)
        .map_err(|e| unreadable(Some(e.line), Some(e.column), e.message, &e.remedy))?;
    let line = args.sample.as_ref().map(|t| (t, gbcli_font::set(&font, t)));
    let preview = match &args.preview {
        Some(out) => {
            let png = match &line {
                Some((_, l)) => gbcli_font::preview_line(l),
                None => gbcli_font::preview_font(&font),
            };
            std::fs::write(cwd.join(out), &png).map_err(|e| {
                Box::new(ErrorBody::new(
                    ErrorCode::IoFailed,
                    format!("writing {out}: {e}"),
                    ErrorCode::IoFailed.default_remedy(),
                ))
            })?;
            Some(FontPreview {
                path: out.clone(),
                bytes: png.len() as u64,
            })
        }
        None => None,
    };
    let widths = font.widths();
    let made = FontMade {
        path: shown.clone(),
        glyphs: font.glyphs.len() as u32,
        fixed: font.fixed(),
        min_width: widths.map(|w| w.0),
        max_width: widths.map(|w| w.1),
        sample: line.map(|(text, l)| FontSample {
            text: text.clone(),
            width: l.width,
            tiles: l.tiles,
            missing: l.missing,
            data: l.data,
            fit: args.fit,
        }),
        preview,
    };
    if let (Some(fit), Some(sample)) = (args.fit, &made.sample)
        && sample.width > fit
    {
        let mut body = ErrorBody::new(
            ErrorCode::PredicateFalse,
            format!(
                "the sample is {} px wide, past --fit {fit} by {}",
                sample.width,
                sample.width - fit
            ),
            "shorten the text, break it into lines, or use narrower glyphs",
        );
        body.result = Some(Box::new(ResultBody {
            font: Some(made),
            ..ResultBody::default()
        }));
        return Err(Box::new(body));
    }
    Ok(made)
}
