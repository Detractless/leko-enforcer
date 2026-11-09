//! The `leko` command: makes a timed session (a session rule and a moment seal that end together)
//! from a typed length or from heard words, and shows it, says whether one app or site is closed
//! now (`leko status`), and drafts a new block into a settings zip and, when told to, saves it
//! (`leko add-block`), switches a block off at once or parks the change (`leko switch-off`) and lands
//! the parked changes that are due (`leko settle`), takes a parked change back (`leko cancel`), switches a block back on at once (`leko switch-on`) and lists the
//! parked changes that touch what `leko status` is asked about. Only `--write` changes a file. The clock
//! is read only by the program's `main.rs`; everything here is given the moment as an argument.

pub mod session {
    pub mod run_command;
    pub mod session_arguments;
    pub mod session_length;
    pub mod session_report;
    pub mod voice_phrase;
}

pub mod add_block {
    pub mod add_block_arguments;
    pub mod add_block_report;
    pub mod file_replace;
    pub mod run_add_block;
}

pub mod edit_schedule {
    pub mod run_edit_schedule;
    pub mod schedule_arguments;
}

pub mod edit_block {
    pub mod edit_arguments;
    pub mod run_edit_block;
}

pub mod delayed_changes {
    pub mod delay_arguments;
    pub mod request_files;
    pub mod run_settle;
    pub mod run_switch_off;
    pub mod settle_report;
    pub mod switch_off_report;
    pub mod waiting_file_disk;
}

pub mod waiting_changes {
    pub mod cancel_arguments;
    pub mod cancel_report;
    pub mod run_cancel;
    pub mod status_waiting;
}

pub mod switch_on {
    pub mod run_switch_on;
    pub mod switch_on_arguments;
    pub mod switch_on_report;
}

pub mod status {
    pub mod run_status;
    pub mod settings_file;
    pub mod status_arguments;
    pub mod status_report;
    pub mod zone_choice;
}

pub mod enforcement {
    pub mod enforcement_loop;
    pub mod macos_applications;
    pub mod macos_device_lock;
}

pub mod launch_agent {
    pub mod agent_command;
    pub mod agent_control;
    pub mod agent_plist;
}

pub use session::run_command::run;
