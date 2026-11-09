//! A line into words, each with the column it starts at, up to its comment.

/// A word and where it starts.
#[derive(Clone, Debug)]
pub struct Word<'a> {
    pub text: &'a str,
    /// 1-based, in characters.
    pub column: u32,
}

/// What a line holds.
pub enum Line<'a> {
    /// Nothing but whitespace: it ends a block.
    Blank,
    /// Only a comment: skipped, even inside a block.
    Comment,
    Words(Vec<Word<'a>>),
}

/// The words of `line` before its comment: `#` alone as a word (followed
/// by whitespace or the line's end) starts one, as in `palettes.txt`.
pub fn words(line: &str) -> Line<'_> {
    if line.trim().is_empty() {
        return Line::Blank;
    }
    let mut all = Vec::new();
    let mut start: Option<(usize, u32)> = None;
    for (column, (i, c)) in (1..).zip(line.char_indices()) {
        if c == ' ' || c == '\t' {
            if let Some((from, col)) = start.take() {
                all.push(Word {
                    text: &line[from..i],
                    column: col,
                });
            }
        } else if start.is_none() {
            start = Some((i, column));
        }
    }
    if let Some((from, col)) = start {
        all.push(Word {
            text: &line[from..],
            column: col,
        });
    }
    let words: Vec<Word> = all.into_iter().take_while(|w| w.text != "#").collect();
    if words.is_empty() {
        Line::Comment
    } else {
        Line::Words(words)
    }
}
