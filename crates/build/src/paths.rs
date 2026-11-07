//! Paths as the caller reads them.

use std::path::{Component, Path, PathBuf};

/// `path` relative to `base` when it is inside it, else as it is: what a
/// row prints, so the next command can use it from the working directory.
pub fn display(path: &Path, base: &Path) -> String {
    let path = normalize(path);
    match path.strip_prefix(normalize(base)) {
        Ok(rel) if rel.as_os_str().is_empty() => ".".into(),
        Ok(rel) => rel.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}

/// `path` without `.` components or `dir/..` pairs, without touching the
/// filesystem (a path that doesn't exist yet still normalizes).
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir
                if matches!(out.components().next_back(), Some(Component::Normal(_))) =>
            {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// `word` as a POSIX shell reads it back as one word: as it is when every
/// character is one a shell leaves alone, else in single quotes. For the
/// commands a result prints for the caller to run.
pub fn quote(word: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "@%+=:,./_-".contains(c);
    if !word.is_empty() && word.chars().all(plain) {
        return word.to_string();
    }
    format!("'{}'", word.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_is_quoted_only_when_a_shell_would_split_or_expand_it() {
        assert_eq!(quote("demo/build/demo.gb"), "demo/build/demo.gb");
        assert_eq!(quote("my games/demo"), "'my games/demo'");
        assert_eq!(quote("it's"), r"'it'\''s'");
        assert_eq!(quote("$HOME"), "'$HOME'");
        assert_eq!(quote("a*"), "'a*'");
        assert_eq!(quote(""), "''");
    }

    #[test]
    fn inside_the_base_is_relative_and_outside_is_absolute() {
        let base = Path::new("/p/work");
        assert_eq!(
            display(Path::new("/p/work/./demo/build/x.gb"), base),
            "demo/build/x.gb"
        );
        assert_eq!(display(Path::new("/p/work"), base), ".");
        assert_eq!(display(Path::new("/p/other/x.gb"), base), "/p/other/x.gb");
        assert_eq!(display(Path::new("/p/work/a/../b"), base), "b");
    }
}
