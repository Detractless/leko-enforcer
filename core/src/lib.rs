//! Leko Enforcer core: reads Enforcer settings (format 2) and judges which changes land now and
//! which must wait. It makes no operating system calls and never reads the clock.

pub mod io {
    pub mod zip_check;
    pub mod zip_reader;
    pub mod zip_writer;
}

pub mod blocks {
    pub mod add_block;
    pub mod block_id;
    pub mod block_request;
    pub mod draft_block;
}

pub mod edit_schedule {
    pub mod rule_edit;
    pub mod schedule_plan;
    pub mod schedule_safety;
}

pub mod edit_block {
    pub mod coverage;
    pub mod edit_plan;
}

pub mod delayed_changes {
    pub mod block_off_zip;
    pub mod settle_plan;
    pub mod switch_off_plan;
    pub mod waiting_change;
    pub mod waiting_file;
}

pub mod waiting_changes {
    pub mod cancel_waiting;
    pub mod waiting_for_target;
}

pub mod switch_on {
    pub mod block_on_zip;
    pub mod plan_switch_on;
}

pub mod domain {
    pub mod canonical;
    pub mod extended_schema;
    pub mod extension_fields;
    pub mod json_same;
    pub mod key_derivation;
    pub mod schema_fields;
    pub mod set_rules;
    pub mod stored_state;
}

pub mod config_diff {
    pub mod diff_parts;
    pub mod door_compare;
    pub mod item_diff;
    pub mod lock_compare;
    pub mod loosening_table;
    pub mod rule_compare;
    pub mod small_item_diff;
}

pub mod config_time {
    pub mod due_time;
    pub mod stretch_overlap;
    pub mod window_mask;
    pub mod zone_time;
}

pub mod decision {
    pub mod block_cover;
    pub mod decide_target;
    pub mod device_action;
    pub mod next_change;
    pub mod rule_closes;
}

pub mod status {
    pub mod next_flip;
}

pub mod models {
    pub mod specs;
}

pub mod sessions {
    pub mod session_compare;
    pub mod session_rule;
    pub mod timed_session;
    pub mod utc_moment;
}

pub mod sites {
    pub mod site_match;
    pub mod site_name;
}

pub mod config {
    pub mod read_basics;
    pub mod read_block;
    pub mod read_deck;
    pub mod read_door;
    pub mod read_key_lock;
    pub mod read_practice;
    pub mod read_routine;
    pub mod strict_fields;
}

pub use io::zip_reader::{open_zip, OpenedZip};
