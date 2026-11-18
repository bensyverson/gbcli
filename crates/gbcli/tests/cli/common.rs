//! A temporary project with the V1 test ROM, the built binary run in it, and
//! golden files for its output.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const V1: &[u8] = include_bytes!("../../../engine/tests/roms/rebuild_v1.gb");
pub const V1_SYM: &[u8] = include_bytes!("../../../engine/tests/roms/rebuild_v1.sym");
pub const V2: &[u8] = include_bytes!("../../../engine/tests/roms/rebuild_v2.gb");
/// A file from the pinned test corpus (`$GBCLI_TEST_ROMS`, else
/// `tests/corpus/fetched/` at the repo root) by its path inside it, naming
/// the suite to fetch when it is missing. Fails loudly, never skips.
pub fn corpus_file(path: &str) -> Vec<u8> {
    let dir = match std::env::var_os("GBCLI_TEST_ROMS") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/fetched"),
    };
    let suite = path.split('/').next().unwrap_or(path);
    let full = dir.join(path);
    fs::read(&full).unwrap_or_else(|err| {
        panic!(
            "corpus file {} is missing ({err}). Fetch it from the repo root with:\n    \
             python3 -I scripts/corpus/fetch.py --only {suite}\n\
             (or point GBCLI_TEST_ROMS at a directory holding {path})",
            full.display()
        )
    })
}

/// A PNG's width and its pixels as RGB.
pub fn read_png(path: &Path) -> (u32, Vec<[u8; 3]>) {
    let mut decoder = png::Decoder::new(fs::File::open(path).unwrap());
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    let channels = info.color_type.samples();
    let pixels = buf[..info.buffer_size()]
        .chunks(channels)
        .map(|p| {
            if channels < 3 {
                [p[0]; 3]
            } else {
                [p[0], p[1], p[2]]
            }
        })
        .collect();
    (info.width, pixels)
}

pub const V2_SYM: &[u8] = include_bytes!("../../../engine/tests/roms/rebuild_v2.sym");

/// A project directory holding `demo.gb` and `demo.sym`, removed on drop.
pub struct Project {
    pub dir: PathBuf,
}

/// What one invocation printed, and its exit code.
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Project {
    pub fn new(name: &str) -> Project {
        let dir = std::env::temp_dir().join(format!("gbcli-cli-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("demo.gb"), V1).unwrap();
        fs::write(dir.join("demo.sym"), V1_SYM).unwrap();
        Project { dir }
    }

    /// Runs `gbcli args…` in the project, with no gbcli variables inherited.
    pub fn gbcli(&self, args: &[&str]) -> Run {
        self.gbcli_with(args, &[], None)
    }

    pub fn gbcli_with(&self, args: &[&str], env: &[(&str, &str)], stdin: Option<&str>) -> Run {
        self.gbcli_at(&self.dir, args, env, stdin)
    }

    /// `gbcli args…` from `cwd` (a directory inside the project), as a user
    /// who has `cd`'d there would run it.
    pub fn gbcli_in(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Run {
        self.gbcli_at(cwd, args, env, None)
    }

    fn gbcli_at(
        &self,
        cwd: &Path,
        args: &[&str],
        env: &[(&str, &str)],
        stdin: Option<&str>,
    ) -> Run {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gbcli"));
        command
            .args(args)
            .current_dir(cwd)
            .env_remove("GBCLI_DIR")
            .env_remove("GBCLI_SESSION")
            .env_remove("GBCLI_AS")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            command.env(k, v);
        }
        let mut child = command.spawn().expect("spawn gbcli");
        let input = stdin.unwrap_or("").to_string();
        let mut pipe = child.stdin.take().unwrap();
        std::thread::spawn(move || {
            use std::io::Write;
            let _ = pipe.write_all(input.as_bytes());
        });
        let output = child.wait_with_output().unwrap();
        Run {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8(output.stdout).unwrap(),
            stderr: String::from_utf8(output.stderr).unwrap(),
        }
    }

    /// A session named `demo` over `demo.gb`, at power-on (nothing run).
    pub fn with_session(name: &str) -> Project {
        let project = Project::new(name);
        let made = project.gbcli(&["show", "--session", "demo", "--rom", "demo.gb"]);
        assert_eq!(made.code, 0, "{}", made.stderr);
        project
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

impl Run {
    /// One transcript entry: the command, its exit code and both streams.
    pub fn transcript(&self, args: &[&str], dir: &Path) -> String {
        let mut out = format!("$ gbcli {}\nexit {}\n", args.join(" "), self.code);
        out += &self.stdout;
        if !self.stderr.is_empty() {
            out += "--- stderr\n";
            out += &self.stderr;
        }
        // The process's working directory is the canonical path (on macOS,
        // /private/var/… for the temp dir's /var/…), and paths built from it,
        // such as session_created's bundle, name that one.
        if let Ok(canonical) = dir.canonicalize() {
            out = out.replace(&canonical.display().to_string(), "<project>");
        }
        mask(&out.replace(&dir.display().to_string(), "<project>"))
    }

    /// Asserts a failure: its exit code, the code's name, and a remedy line
    /// containing `remedy`.
    pub fn fails(&self, exit: i32, code: &str, remedy: &str) {
        assert_eq!(
            self.code, exit,
            "stdout:\n{}stderr:\n{}",
            self.stdout, self.stderr
        );
        let mut lines = self.stderr.lines().filter(|l| !l.starts_with("warning: "));
        let first = lines.next().unwrap_or_default();
        assert!(
            first.starts_with(&format!("gbcli: {code}: ")),
            "{}",
            self.stderr
        );
        let second = lines.next().unwrap_or_default();
        assert!(second.starts_with("remedy: "), "{}", self.stderr);
        assert!(
            second.contains(remedy),
            "remedy {second:?} lacks {remedy:?}"
        );
    }
}

/// Host time and sizes that depend on it are the documented exception to
/// byte-exact output (cli-design ▸ Discoverability); they are masked.
fn mask(text: &str) -> String {
    let mut out = text.to_string();
    for key in ["last_used=", "at=", "size_bytes="] {
        out = mask_after(&out, &format!(" {key}"), |c| c == ' ' || c == '\n');
    }
    for key in ["\"last_used\":", "\"at\":", "\"size_bytes\":"] {
        out = mask_after(&out, key, |c| c == ',' || c == '}');
    }
    out
}

fn mask_after(text: &str, key: &str, end: impl Fn(char) -> bool) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find(key) {
        out += &rest[..i + key.len()];
        rest = &rest[i + key.len()..];
        // Only a number or a date: `"at":{…}` is a breakpoint's location.
        if rest.starts_with(|c: char| c.is_ascii_digit()) {
            let stop = rest.find(&end).unwrap_or(rest.len());
            out += "*";
            rest = &rest[stop..];
        }
    }
    out + rest
}

/// Compares `actual` with `tests/cli/golden/<name>`; `GBCLI_BLESS=1` writes it.
pub fn golden(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/cli/golden")
        .join(name);
    if std::env::var_os("GBCLI_BLESS").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, actual).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "no golden {}; write it with GBCLI_BLESS=1 and review it",
            path.display()
        )
    });
    if expected != actual {
        let first = expected
            .lines()
            .zip(actual.lines())
            .position(|(e, a)| e != a)
            .unwrap_or(expected.lines().count().min(actual.lines().count()));
        panic!(
            "{} differs from line {}:\n--- expected\n{}\n--- actual\n{}",
            path.display(),
            first + 1,
            expected
                .lines()
                .skip(first)
                .take(5)
                .collect::<Vec<_>>()
                .join("\n"),
            actual
                .lines()
                .skip(first)
                .take(5)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
}

/// `actual` against the PNG golden `name`; `GBCLI_BLESS=1` writes it.
pub fn png_golden(actual: &[u8], name: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/cli/golden")
        .join(name);
    if std::env::var_os("GBCLI_BLESS").is_some() {
        fs::write(&path, actual).unwrap();
        return;
    }
    let expected = fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "no golden {}; write it with GBCLI_BLESS=1 and look at it",
            path.display()
        )
    });
    if expected != actual {
        let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        fs::write(&out, actual).unwrap();
        panic!(
            "{} differs; the actual image is {}",
            path.display(),
            out.display()
        );
    }
}
