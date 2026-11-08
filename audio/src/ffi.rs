//! Small C interface used by the Python voice frontend.

use crate::EchoProcessor;
use std::{ptr, slice};

/// Creates a 16 kHz or 48 kHz echo processor; returns null on initialization failure.
#[unsafe(no_mangle)]
pub extern "C" fn leko_aec_create(sample_rate_hz: u32) -> *mut EchoProcessor {
    EchoProcessor::new(sample_rate_hz)
        .map(Box::new)
        .map(Box::into_raw)
        .unwrap_or(ptr::null_mut())
}

/// Releases a handle previously returned by `leko_aec_create`.
///
/// # Safety
/// `handle` must be null or a live pointer returned by `leko_aec_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn leko_aec_destroy(handle: *mut EchoProcessor) {
    if !handle.is_null() {
        unsafe { drop(Box::from_raw(handle)) };
    }
}

/// Adds one exact 10 ms speaker frame to the echo reference.
///
/// # Safety
/// `handle` must be live and `samples` must point to `len` readable floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn leko_aec_render(
    handle: *const EchoProcessor,
    samples: *const f32,
    len: usize,
) -> i32 {
    let Some(processor) = (unsafe { handle.as_ref() }) else {
        return -1;
    };
    if samples.is_null() {
        return -1;
    }
    let frame = unsafe { slice::from_raw_parts(samples, len) };
    processor.process_render(frame).map_or(-1, |_| 0)
}

/// Cancels speaker echo in one exact 10 ms microphone frame.
///
/// # Safety
/// `handle` must be live; `input` must be readable and `output` writable for `len` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn leko_aec_capture(
    handle: *const EchoProcessor,
    input: *const f32,
    output: *mut f32,
    len: usize,
) -> i32 {
    let Some(processor) = (unsafe { handle.as_ref() }) else {
        return -1;
    };
    if input.is_null() || output.is_null() {
        return -1;
    }
    let frame = unsafe { slice::from_raw_parts(input, len) };
    let Ok(cleaned) = processor.process_capture(frame) else {
        return -1;
    };
    unsafe { ptr::copy_nonoverlapping(cleaned.as_ptr(), output, len) };
    0
}
