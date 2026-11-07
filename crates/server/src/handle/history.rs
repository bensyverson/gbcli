use gbcli_bundle::{SessionStore, Start};
use gbcli_engine::{Machine, SaveMeta};
use gbcli_protocol::{
    CheckpointArgs, CheckpointMade, RestoreArgs, Restored, ResultBody, Rev, SessionFile,
};

use super::{Failure, Handler, HeadChange, Need, save_head};
impl Handler {
    /// A checkpoint before the first run is the boot handoff, where a run
    /// would start, not power-on.
    pub(super) fn checkpoint(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &CheckpointArgs,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let need = Need::Start(Start::Boot);
        let (machine, loaded) = self.machine_for(store, meta, need, None)?;
        warnings.extend(loaded);
        let made = write_checkpoint(store, &machine, &args.name, actor, meta.rev)?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            checkpoint: Some(made),
            ..ResultBody::default()
        })
    }

    pub(super) fn restore(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        args: &RestoreArgs,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let load = self.load.clone();
        let cp = store
            .checkpoint(&args.name)
            .map_err(|e| Failure::from_store(&e))?;
        let (mut machine, _) = self.machine_for(store, meta, Need::Replace, None)?;
        warnings.extend(
            store
                .restore(&mut machine, &cp, &load)
                .map_err(|e| Failure::from_store(&e))?,
        );
        let frame = checkpoint_frame(&cp);
        let change = HeadChange {
            actor: actor.clone(),
            parent: Some(args.name.clone()),
            ..HeadChange::default()
        };
        save_head(store, meta, &machine, change)?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            restored: Some(Restored {
                name: Some(args.name.clone()),
                rev: meta.rev,
                frame: Some(frame),
            }),
            ..ResultBody::default()
        })
    }

    /// Consecutive undos walk back through history (design §10).
    pub(super) fn undo(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        actor: &Option<String>,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        let (cp, walk) = store
            .undo_target(meta)
            .map_err(|e| Failure::from_store(&e))?;
        let load = self.load.clone();
        let (mut machine, _) = self.machine_for(store, meta, Need::Replace, None)?;
        let frame = checkpoint_frame(&cp);
        warnings.extend(
            store
                .restore(&mut machine, &cp, &load)
                .map_err(|e| Failure::from_store(&e))?,
        );
        let change = HeadChange {
            actor: actor.clone(),
            parent: Some("undo".into()),
            walk: Some(walk),
            ..HeadChange::default()
        };
        save_head(store, meta, &machine, change)?;
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            restored: Some(Restored {
                name: None,
                rev: meta.rev,
                frame: Some(frame),
            }),
            ..ResultBody::default()
        })
    }
}

/// A checkpoint's frame since the boot handoff when it has one, else since
/// power-on: the number `checkpoint` and `restore` report (design §12).
fn checkpoint_frame(cp: &gbcli_engine::Checkpoint) -> u64 {
    let base = cp.header.boot.map_or(0, |boot| boot.frame);
    cp.header.frame.saturating_sub(base)
}

pub(super) fn write_checkpoint(
    store: &SessionStore,
    machine: &Machine,
    name: &str,
    actor: &Option<String>,
    rev: Rev,
) -> Result<CheckpointMade, Failure> {
    let cp = machine.capture(SaveMeta {
        created_by: actor.clone(),
        parent: None,
        rev: Some(rev),
    });
    let frame = checkpoint_frame(&cp);
    store
        .write_checkpoint(name, &cp)
        .map_err(|e| Failure::from_store(&e))?;
    Ok(CheckpointMade {
        name: name.to_string(),
        path: store.display_path(&store.dir().join("checkpoints").join(format!("{name}.gbck"))),
        frame: Some(frame),
        rev,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Fixture;
    use crate::handle::test_support::{handler, request, run_frames};
    use gbcli_protocol::Op;

    #[test]
    fn checkpoint_restore_and_undo_move_the_head_back_and_forth() {
        let f = Fixture::new("undo");
        let mut h = handler(&f);
        assert!(run_frames(&mut h, 2).is_ok());
        let saved = h.handle(&request(Op::Checkpoint(CheckpointArgs {
            name: "f2".into(),
        })));
        assert!(saved.is_ok());
        assert_eq!(
            saved.rev,
            Some(Rev(1)),
            "a checkpoint doesn't move the head"
        );
        assert!(run_frames(&mut h, 2).is_ok());

        let restored = h.handle(&request(Op::Restore(RestoreArgs { name: "f2".into() })));
        let restored = restored.outcome.unwrap().restored.unwrap();
        assert_eq!(restored.name.as_deref(), Some("f2"));
        assert_eq!(restored.frame, Some(2));

        let undone = h.handle(&request(Op::Undo));
        let undone = undone.outcome.unwrap().restored.unwrap();
        assert_eq!(undone.name, None);
        assert_eq!(undone.frame, Some(4));
    }

    /// The frame an undo restored, or its error.
    fn undo(h: &mut Handler) -> Result<u64, Box<gbcli_protocol::ErrorBody>> {
        let response = h.handle(&request(Op::Undo));
        match response.outcome {
            Err(error) => Err(Box::new(error)),
            Ok(result) => Ok(result.restored.unwrap().frame.unwrap()),
        }
    }

    #[test]
    fn consecutive_undos_walk_back_and_stop_at_the_oldest_entry() {
        let f = Fixture::new("undo-walk");
        let mut h = handler(&f);
        for _ in 0..4 {
            assert!(run_frames(&mut h, 1).is_ok());
        }
        assert_eq!(undo(&mut h), Ok(3));
        assert_eq!(undo(&mut h), Ok(2));
        assert_eq!(undo(&mut h), Ok(1));
        let error = undo(&mut h).expect_err("rev 0 has no entry to restore");
        assert_eq!(error.code, gbcli_protocol::ErrorCode::BadRequest);
        assert!(error.remedy.contains("s1"), "{}", error.remedy);
    }

    #[test]
    fn a_run_after_an_undo_resets_the_walk() {
        let f = Fixture::new("undo-reset");
        let mut h = handler(&f);
        for _ in 0..3 {
            assert!(run_frames(&mut h, 1).is_ok());
        }
        assert_eq!(undo(&mut h), Ok(2));
        assert!(run_frames(&mut h, 1).is_ok());
        assert_eq!(undo(&mut h), Ok(2), "the run's ~1 is the undone state");
    }

    #[test]
    fn an_undo_walk_reaches_eight_back_from_where_it_began() {
        let f = Fixture::new("undo-full");
        let mut h = handler(&f);
        for _ in 0..12 {
            assert!(run_frames(&mut h, 1).is_ok());
        }
        for back in 1..=8 {
            assert_eq!(undo(&mut h), Ok(12 - back), "undo {back}");
        }
        let error = undo(&mut h).expect_err("s4 is the oldest kept");
        assert!(error.remedy.contains("s4"), "{}", error.remedy);
    }
}
