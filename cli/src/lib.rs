pub mod arguments;
pub mod clock;
pub mod commands;
pub mod services;

use std::ffi::OsString;
use std::io::Write;

use arguments::{EXIT_OK, EXIT_REFUSED, EXIT_USAGE};

pub fn run(raw: &[OsString], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let args: Vec<String> = raw
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    let parsed = match arguments::parse(&args) {
        Ok(value) => value,
        Err(message) => {
            let _ = writeln!(err, "{}", message);
            return EXIT_USAGE;
        }
    };
    match commands::dispatch(&parsed, out, err) {
        Ok(()) => EXIT_OK,
        Err((message, code)) => {
            let _ = writeln!(err, "{}", message);
            if code == EXIT_USAGE {
                EXIT_USAGE
            } else {
                EXIT_REFUSED
            }
        }
    }
}
