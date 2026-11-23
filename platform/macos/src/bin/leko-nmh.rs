//! Minimal length-framed native messaging host for Leko browsers.
//! The extension handshake uses this channel to report extension health.

use std::io::{self, Read, Write};

fn main() {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    loop {
        let mut len_bytes = [0u8; 4];
        match input.read_exact(&mut len_bytes) {
            Ok(()) => {}
            Err(_) => break,
        }
        let len = u32::from_le_bytes(len_bytes) as usize;
        if len > 1024 * 1024 {
            break;
        }
        let mut payload = vec![0u8; len];
        if input.read_exact(&mut payload).is_err() {
            break;
        }
        if output.write_all(&len_bytes).is_err() || output.write_all(&payload).is_err() {
            break;
        }
        if output.flush().is_err() {
            break;
        }
    }
}
