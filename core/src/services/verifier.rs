use serde_json::Value;

use crate::clock::Moment;
use crate::domain::seal_eval::seal_satisfied;
use crate::domain::SealProof;
use crate::io::snapshot::{AddonSnapshot, SnapshotStore};
use crate::models::{CustomVerifierSpec, Seal};
use crate::services::nonce::{Nonce, NonceGuard};
use crate::services::runner::Challenge;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verification {
    Satisfied,
    NotSatisfied,
}

pub trait AddonRunner {
    fn run(&self, snapshot: &AddonSnapshot, config: &Value, challenge: &Challenge)
        -> Result<bool, String>;
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
        let challenge = Challenge {
            challenge_id: format!("challenge-{}", nonce.0),
            nonce: nonce.0.clone(),
        };
        match runner.run(snapshot, &spec.config, &challenge) {
            Ok(true) => Verification::Satisfied,
            _ => Verification::NotSatisfied,
        }
    }
}

pub fn apply_fallback(
    result: Verification,
    fallback: Option<&Seal>,
    moment: &Moment,
    proof: &SealProof,
) -> bool {
    match result {
        Verification::Satisfied => true,
        Verification::NotSatisfied => fallback
            .map(|seal| seal_satisfied(seal, moment, proof))
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_pass_and_seal_share_the_fail_closed_hook() {
        let satisfied = apply_fallback(Verification::Satisfied, None,
            &Moment::parse("2026-01-01T00:00:00Z", chrono_tz::UTC).unwrap(), &SealProof::default());
        assert!(satisfied);
        let locked = apply_fallback(Verification::NotSatisfied, None,
            &Moment::parse("2026-01-01T00:00:00Z", chrono_tz::UTC).unwrap(), &SealProof::default());
        assert!(!locked);
        let fallback = Seal::NoSeal;
        let escaped = apply_fallback(Verification::NotSatisfied, Some(&fallback),
            &Moment::parse("2026-01-01T00:00:00Z", chrono_tz::UTC).unwrap(), &SealProof::default());
        assert!(escaped);
    }

    #[test]
    fn fallback_seal_is_used_only_when_present() {
        let moment = Moment::parse("2026-01-01T00:00:00Z", chrono_tz::UTC).unwrap();
        let proof = SealProof::default();
        assert!(!apply_fallback(Verification::NotSatisfied, None, &moment, &proof));
        assert!(apply_fallback(Verification::NotSatisfied, Some(&Seal::NoSeal), &moment, &proof));
    }
}
