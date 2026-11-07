use gbcli_bundle::{SessionStore, Start};
use gbcli_engine::{LoadOptions, Machine, SaveMeta};
use gbcli_protocol::{
    CheckpointArgs, CheckpointMade, RestoreArgs, Restored, ResultBody, Rev, SessionFile,
};

use super::{Failure, Handler, Need, save_head};
impl Handler {
    pub(super) fn checkpoint(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &CheckpointArgs,
        actor: &Option<String>,
    ) -> Result<ResultBody, Failure> {
        let (machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
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
        let (mut machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let cp = store
            .checkpoint(&args.name)
            .map_err(|e| Failure::from_store(&e))?;
        let load = LoadOptions {
            allow_rom_change: args.allow_rom_change,
            ..LoadOptions::default()
        };
        warnings.extend(
            store
                .restore(&mut machine, &cp, &load)
                .map_err(|e| Failure::from_store(&e))?,
        );
        let frame = checkpoint_frame(&cp);
        save_head(
            store,
            meta,
            &machine,
            actor.clone(),
            Some(args.name.clone()),
            None,
        )?;
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

    pub(super) fn undo(
        &mut self,
        store: &SessionStore,
        meta: &mut SessionFile,
        actor: &Option<String>,
    ) -> Result<ResultBody, Failure> {
        let (mut machine, _) = self.machine_for(store, meta, Need::Read, None, Start::PowerOn)?;
        let cp = store
            .checkpoint_or_history(meta.rev, "~1")
            .map_err(|e| Failure::from_store(&e))?;
        let frame = checkpoint_frame(&cp);
        let load = LoadOptions::default();
        store
            .restore(&mut machine, &cp, &load)
            .map_err(|e| Failure::from_store(&e))?;
        save_head(
            store,
            meta,
            &machine,
            actor.clone(),
            Some("undo".into()),
            None,
        )?;
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
        assert!(run_frames(&mut h, 2).ok);
        let saved = h.handle(&request(Op::Checkpoint(CheckpointArgs {
            name: "f2".into(),
        })));
        assert!(saved.ok);
        assert_eq!(
            saved.rev,
            Some(Rev(1)),
            "a checkpoint doesn't move the head"
        );
        assert!(run_frames(&mut h, 2).ok);

        let restored = h.handle(&request(Op::Restore(RestoreArgs {
            name: "f2".into(),
            allow_rom_change: false,
        })));
        let restored = restored.result.unwrap().restored.unwrap();
        assert_eq!(restored.name.as_deref(), Some("f2"));
        assert_eq!(restored.frame, Some(2));

        let undone = h.handle(&request(Op::Undo));
        let undone = undone.result.unwrap().restored.unwrap();
        assert_eq!(undone.name, None);
        assert_eq!(undone.frame, Some(4));
    }
}
