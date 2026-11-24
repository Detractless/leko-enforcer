use crate::io::snapshot::{AddonSnapshot, SnapshotStore};
use crate::models::CustomVerifierSpec;
use crate::services::nonce::{Nonce, NonceGuard};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verification {
    Satisfied,
    NotSatisfied,
}

pub trait AddonRunner {
    fn run(&self, snapshot: &AddonSnapshot, config: &serde_json::Value) -> Result<bool, String>;
}

pub struct VerifierHook<'a> {
    snapshots: &'a SnapshotStore,
    guard: &'a mut NonceGuard,
}

impl<'a> VerifierHook<'a> {
    pub fn new(snapshots: &'a SnapshotStore, guard: &'a mut NonceGuard) -> Self {
        VerifierHook { snapshots, guard }
    }

    pub fn evaluate(
        &mut self,
        spec: &CustomVerifierSpec,
        runner: Option<&dyn AddonRunner>,
        nonce: &Nonce,
    ) -> Verification {
        if self.guard.consume(nonce).is_err() {
            return Verification::NotSatisfied;
        }
        let Some(snapshot) = self.snapshots.get(&spec.addon_id) else {
            return Verification::NotSatisfied;
        };
        if snapshot.sha256 != spec.sha256 || !self.snapshots.verify(spec) {
            return Verification::NotSatisfied;
        }
        let Some(runner) = runner else {
            return Verification::NotSatisfied;
        };
        match runner.run(snapshot, &spec.config) {
            Ok(true) => Verification::Satisfied,
            _ => Verification::NotSatisfied,
        }
    }
}
