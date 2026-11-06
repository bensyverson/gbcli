//! A session: one or more machines and the named checkpoints they share.
//! Persisting them to `.gbcli/` is the bundle store's job, not this one's.

use std::collections::BTreeMap;

use gbcli_core::CoreError;

use crate::checkpoint::{Checkpoint, LoadOptions, SaveMeta};
use crate::checkpoint_error::{CheckpointError, LoadWarning};
use crate::machine::Machine;

/// A machine's index in its session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MachineId(usize);

#[derive(Default)]
pub struct Session {
    machines: Vec<Machine>,
    checkpoints: BTreeMap<String, Checkpoint>,
}

impl Session {
    pub fn new() -> Session {
        Session::default()
    }

    pub fn add(&mut self, machine: Machine) -> MachineId {
        self.machines.push(machine);
        MachineId(self.machines.len() - 1)
    }

    pub fn machine(&self, id: MachineId) -> &Machine {
        &self.machines[id.0]
    }

    pub fn machine_mut(&mut self, id: MachineId) -> &mut Machine {
        &mut self.machines[id.0]
    }

    /// A copy of a machine in the same state, for A/B experiments.
    pub fn clone_machine(&mut self, id: MachineId) -> Result<MachineId, CoreError> {
        let copy = self.machines[id.0].try_clone()?;
        Ok(self.add(copy))
    }

    /// Saves a machine's state under `name`, replacing any checkpoint of that name.
    pub fn save(&mut self, id: MachineId, name: &str, meta: SaveMeta) -> &Checkpoint {
        let cp = self.machines[id.0].capture(meta);
        self.checkpoints.insert(name.to_string(), cp);
        &self.checkpoints[name]
    }

    pub fn checkpoint(&self, name: &str) -> Option<&Checkpoint> {
        self.checkpoints.get(name)
    }

    /// Checkpoint names, sorted.
    pub fn checkpoint_names(&self) -> Vec<&str> {
        self.checkpoints.keys().map(String::as_str).collect()
    }

    pub fn remove(&mut self, name: &str) -> Option<Checkpoint> {
        self.checkpoints.remove(name)
    }

    /// Restores a named checkpoint into a machine.
    pub fn restore(
        &mut self,
        id: MachineId,
        name: &str,
        opts: &LoadOptions,
    ) -> Result<Vec<LoadWarning>, CheckpointError> {
        let cp = self
            .checkpoints
            .get(name)
            .ok_or_else(|| CheckpointError::Unknown {
                name: name.to_string(),
                known: self.checkpoints.keys().cloned().collect(),
            })?;
        self.machines[id.0].restore(cp, opts)
    }

    /// Adds a checkpoint from envelope bytes. Only the envelope is checked
    /// here; the rest is verified when it's restored.
    pub fn import(&mut self, name: &str, bytes: &[u8]) -> Result<(), CheckpointError> {
        let cp = Checkpoint::decode(bytes)?;
        self.checkpoints.insert(name.to_string(), cp);
        Ok(())
    }
}
