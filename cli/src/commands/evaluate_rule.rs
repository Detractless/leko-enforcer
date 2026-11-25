use std::io::Write;

use crate::arguments::CliArgs;
use crate::commands::CommandResult;

pub fn run(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    crate::commands::check_status::run(args, out)
}
