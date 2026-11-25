use std::io::Write;

use leko_core::domain::Decision;
use leko_core::models::{
    Pass, Penalty, Schedule, Seal, Target, TimeBasedSchedule, UsageLimit,
};
use leko_core::TechnologyManagementRule;

pub fn target_name(target: &Target) -> &'static str {
    match target {
        Target::SpecificApps { .. } => "Specific Apps",
        Target::AllowOnlyApps { .. } => "Allow Only Apps",
        Target::SpecificWebsites { .. } => "Specific Websites",
        Target::TopicDetectedWebsites { .. } => "Topic Detected Websites",
        Target::AllowOnlyWebsites { .. } => "Allow Only Websites",
        Target::WholeDevice { .. } => "Whole Device",
    }
}

pub fn schedule_name(schedule: &Schedule) -> &'static str {
    match &schedule.time_based {
        TimeBasedSchedule::AlwaysOnSchedule => "Always On Schedule",
        TimeBasedSchedule::OneTimeSchedule { .. } => "One Time Schedule",
        TimeBasedSchedule::RoutineSchedule { .. } => "Routine Schedule",
    }
}

pub fn usage_name(usage: &Option<UsageLimit>) -> &'static str {
    match usage {
        None => "No Usage Limit",
        Some(UsageLimit::TimeLimit { .. }) => "Time Limit",
        Some(UsageLimit::SessionLimit { .. }) => "Session Limit",
    }
}

pub fn penalty_name(penalty: &Option<Penalty>) -> &'static str {
    match penalty {
        None => "No Penalty",
        Some(Penalty::ForceClosePenalty) => "Force Close Penalty",
    }
}

pub fn pass_name(pass: &Pass) -> &'static str {
    match pass {
        Pass::NoPass => "No Pass",
        Pass::EarnPass { .. } => "Earn Pass",
    }
}

pub fn seal_name(seal: &Seal) -> &'static str {
    match seal {
        Seal::NoSeal => "No Seal",
        Seal::CountdownSeal { .. } => "Countdown Seal",
        Seal::QueuedDelaySeal { .. } => "Queued Delay Seal",
        Seal::TypingSeal { .. } => "Typing Seal",
        Seal::TaskSeal { .. } => "Task Seal",
        Seal::EditWindowSeal { .. } => "Edit Window Seal",
        Seal::ActiveScheduleSeal { .. } => "Active Schedule Seal",
        Seal::PartnerPasswordSeal { .. } => "Partner Password Seal",
        Seal::CustomVerifier { .. } => "Custom Verifier",
    }
}

pub fn rule(rule: &TechnologyManagementRule, out: &mut dyn Write) -> Result<(), String> {
    writeln!(out, "Rule: {}", rule.name).map_err(|e| e.to_string())?;
    writeln!(out, "Id: {}", rule.id).map_err(|e| e.to_string())?;
    writeln!(out, "Enabled: {}", rule.enabled).map_err(|e| e.to_string())?;
    writeln!(out, "Target: {}", target_name(&rule.target)).map_err(|e| e.to_string())?;
    writeln!(out, "Schedule: {}", schedule_name(&rule.schedule)).map_err(|e| e.to_string())?;
    writeln!(out, "Usage: {}", usage_name(&rule.schedule.usage_based)).map_err(|e| e.to_string())?;
    writeln!(out, "Penalty: {}", penalty_name(&rule.penalty)).map_err(|e| e.to_string())?;
    writeln!(out, "Pass: {}", pass_name(&rule.pass)).map_err(|e| e.to_string())?;
    writeln!(out, "Seal: {}", seal_name(&rule.seal)).map_err(|e| e.to_string())?;
    writeln!(out, "Created At: {}", rule.created_at).map_err(|e| e.to_string())?;
    writeln!(out, "Updated At: {}", rule.updated_at).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn decision(decision: &Decision, out: &mut dyn Write) -> Result<(), String> {
    writeln!(out, "Enforced: {}", decision.enforced).map_err(|e| e.to_string())?;
    writeln!(out, "Reason: {:?}", decision.reason).map_err(|e| e.to_string())
}

pub fn message(text: &str, out: &mut dyn Write) -> Result<(), String> {
    writeln!(out, "{}", text).map_err(|e| e.to_string())
}
