//! Named checkpoints (design §1): `checkpoints/<name>.gbck`, kept until the
//! session is removed with `--with-checkpoints`.

use std::io;

use gbcli_engine::Checkpoint;

use crate::error::StoreError;
use crate::project_store::Key;
use crate::store::SessionStore;

impl SessionStore {
    fn checkpoint_key(&self, name: &str) -> Key {
        self.checkpoints_key().join(&format!("{name}.gbck"))
    }

    #[cfg(test)]
    fn checkpoint_path(&self, name: &str) -> std::path::PathBuf {
        self.files().locate(&self.checkpoint_key(name))
    }

    /// A named checkpoint. The name is checked first, so it can't reach a
    /// key outside `checkpoints/`.
    pub fn checkpoint(&self, name: &str) -> Result<Checkpoint, StoreError> {
        validate_checkpoint_name(name)?;
        let key = self.checkpoint_key(name);
        let bytes = self.files().get(&key).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => StoreError::CheckpointMissing {
                name: name.to_string(),
                known: self.checkpoint_names().unwrap_or_default(),
            },
            _ => self.bundle().io_error(&key, e),
        })?;
        Checkpoint::decode(&bytes).map_err(StoreError::Checkpoint)
    }

    pub fn write_checkpoint(&self, name: &str, cp: &Checkpoint) -> Result<(), StoreError> {
        validate_checkpoint_name(name)?;
        let dir = self.checkpoints_key();
        self.files()
            .make_dir(&dir)
            .map_err(self.bundle().io_at(&dir))?;
        let key = self.checkpoint_key(name);
        self.files()
            .put(&key, &cp.encode())
            .map_err(self.bundle().io_at(&key))
    }

    /// Checkpoint names, sorted.
    pub fn checkpoint_names(&self) -> Result<Vec<String>, StoreError> {
        let dir = self.checkpoints_key();
        let entries = match self.files().list(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(self.bundle().io_error(&dir, source)),
        };
        let mut names: Vec<String> = entries
            .iter()
            .filter_map(|e| checkpoint_stem(&e.name).map(str::to_string))
            .collect();
        names.sort();
        Ok(names)
    }
}

/// A `.gbck` file's name without the extension, as `Path::file_stem` takes
/// it: a name that is only `.gbck` has no extension, so no stem.
fn checkpoint_stem(name: &str) -> Option<&str> {
    name.strip_suffix(".gbck").filter(|stem| !stem.is_empty())
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
    use std::fs;

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
        let mut machine = store.machine(&store.read_meta().unwrap()).unwrap();
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
        let mut machine = store.machine(&store.read_meta().unwrap()).unwrap();
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
