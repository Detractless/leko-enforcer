use chrono::{Datelike, Duration, Timelike};

use crate::clock::Moment;
use crate::models::{QueuedEdit, Seal, Window};

#[derive(Clone, Debug, Default)]
pub struct SealProof {
    pub typing_ok: bool,
    pub task_ok: bool,
    pub password_ok: bool,
    pub verifier_satisfied: bool,
    pub edit_window_ok: bool,
}

fn due_at(requested_at: &str, delay_seconds: u64, moment: &Moment) -> bool {
    let Ok(requested) = Moment::parse(requested_at, moment.zone) else {
        return false;
    };
    moment.utc >= requested.utc + Duration::seconds(delay_seconds as i64)
}

pub fn queued_edits_due(seal: &Seal, moment: &Moment) -> Vec<QueuedEdit> {
    match seal {
        Seal::QueuedDelaySeal {
            delay_seconds,
            pending_edits,
        } => pending_edits
            .iter()
            .filter(|edit| !edit.cancelled && due_at(&edit.requested_at, *delay_seconds, moment))
            .cloned()
            .collect(),
        _ => Vec::new(),
    }
}

fn in_windows(days: &[u32], windows: &[Window], moment: &Moment) -> bool {
    let local = moment.utc.with_timezone(&moment.zone);
    let weekday = local.weekday().num_days_from_monday() + 1;
    if !days.contains(&weekday) {
        return false;
    }
    let minute = local.hour() * 60 + local.minute();
    windows.iter().any(|window| {
        if window.end_minute >= window.start_minute {
            minute >= window.start_minute && minute < window.end_minute
        } else {
            minute >= window.start_minute || minute < window.end_minute
        }
    })
}

pub fn seal_satisfied(seal: &Seal, moment: &Moment, proof: &SealProof) -> bool {
    match seal {
        Seal::NoSeal => true,
        Seal::CountdownSeal {
            wait_seconds,
            unseal_requested_at,
        } => unseal_requested_at
            .as_ref()
            .map(|requested| due_at(requested, *wait_seconds, moment))
            .unwrap_or(false),
        Seal::QueuedDelaySeal { .. } => !queued_edits_due(seal, moment).is_empty(),
        Seal::TypingSeal { .. } => proof.typing_ok,
        Seal::TaskSeal { .. } => proof.task_ok,
        Seal::EditWindowSeal { days, windows } => {
            proof.edit_window_ok && in_windows(days, windows, moment)
        }
        Seal::ActiveScheduleSeal { .. } => proof.edit_window_ok,
        Seal::PartnerPasswordSeal { .. } => proof.password_ok,
        Seal::CustomVerifier { fallback_seal, .. } => {
            let result = if proof.verifier_satisfied {
                crate::services::verifier::Verification::Satisfied
            } else {
                crate::services::verifier::Verification::NotSatisfied
            };
            crate::services::verifier::apply_fallback(
                result,
                fallback_seal.as_deref(),
                moment,
                proof,
            )
        }
    }
}
