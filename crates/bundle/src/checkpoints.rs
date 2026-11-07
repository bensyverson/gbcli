//! Named checkpoints (design §1): `checkpoints/<name>.gbck`, kept until the
//! session is removed with `--with-checkpoints`.

use std::fs;
use std::path::PathBuf;

use gbcli_engine::Checkpoint;

use crate::error::StoreError;
use crate::io::{atomic_write, ensure_dir};
use crate::store::SessionStore;

impl SessionStore {
    fn checkpoint_path(&self, name: &str) -> PathBuf {
        self.checkpoints_dir().join(format!("{name}.gbck"))
    }

    /// A named checkpoint. The name is checked first, so it can't reach a
    /// file outside `checkpoints/`.
    pub fn checkpoint(&self, name: &str) -> Result<Checkpoint, StoreError> {
        validate_checkpoint_name(name)?;
        let path = self.checkpoint_path(name);
        let bytes = fs::read(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => StoreError::CheckpointMissing {
                name: name.to_string(),
                known: self.checkpoint_names().unwrap_or_default(),
            },
            _ => StoreError::Io {
                path: path.clone(),
                source: e,
            },
        })?;
        Checkpoint::decode(&bytes).map_err(StoreError::Checkpoint)
    }

    pub fn write_checkpoint(&self, name: &str, cp: &Checkpoint) -> Result<(), StoreError> {
        validate_checkpoint_name(name)?;
        ensure_dir(&self.checkpoints_dir())?;
        atomic_write(&self.checkpoint_path(name), &cp.encode())
    }

    /// Checkpoint names, sorted.
    pub fn checkpoint_names(&self) -> Result<Vec<String>, StoreError> {
        let dir = self.checkpoints_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(StoreError::Io { path: dir, source }),
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let path = e.path();
                if path.extension().is_some_and(|x| x == "gbck") {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(str::to_string)
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        Ok(names)
    }
}

/// A checkpoint name: letters, digits, `-`, `_` and `.`, no path parts.
pub fn validate_checkpoint_name(name: &str) -> Result<(), StoreError> {
    let ok = (1..=64).contains(&name.len())
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(())
    } else {
        Err(StoreError::BadCheckpointName {
            name: name.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use gbcli_engine::SaveMeta;

    use super::*;
    use crate::test_fixture::Fixture;

    #[test]
    fn a_checkpoint_name_cannot_escape_its_directory() {
        for bad in ["", ".", "..", "a/b", "a\\b"] {
            assert!(validate_checkpoint_name(bad).is_err(), "{bad:?}");
        }
        assert!(validate_checkpoint_name("f30").is_ok());
    }

    #[test]
    fn reading_a_checkpoint_checks_its_name_before_touching_a_file() {
        let f = Fixture::new("checkpoint-read-name");
        let store = f.demo();
        let machine = store.machine(&store.read_meta().unwrap()).unwrap();
        let cp = machine.capture(SaveMeta::default());
        // A real .gbck outside checkpoints/ that a path could reach.
        fs::create_dir_all(store.history_dir()).unwrap();
        fs::write(store.history_dir().join("s3.gbck"), cp.encode()).unwrap();
        let outside = f.root.join("loose.gbck");
        fs::write(&outside, cp.encode()).unwrap();
        for bad in [
            "../history/s3",
            outside.with_extension("").to_str().unwrap(),
        ] {
            let err = store.checkpoint(bad).unwrap_err();
            assert!(
                matches!(err, StoreError::BadCheckpointName { .. }),
                "{bad}: {err:?}"
            );
        }
    }

    #[test]
    fn an_unreadable_checkpoint_is_an_io_failure_not_a_missing_one() {
        use std::os::unix::fs::PermissionsExt;
        let f = Fixture::new("checkpoint-eacces");
        let store = f.demo();
        let machine = store.machine(&store.read_meta().unwrap()).unwrap();
        store
            .write_checkpoint("a", &machine.capture(SaveMeta::default()))
            .unwrap();
        let path = store.checkpoint_path("a");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        let err = store.checkpoint("a").unwrap_err();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(err.code(), gbcli_protocol::ErrorCode::IoFailed, "{err:?}");
        assert!(matches!(
            store.checkpoint("b"),
            Err(StoreError::CheckpointMissing { .. })
        ));
    }
}
