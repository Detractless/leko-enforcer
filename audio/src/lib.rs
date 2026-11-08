//! Portable, device-independent echo cancellation for synchronized PCM frames.

use webrtc_audio_processing::{
    Processor,
    config::{Config, EchoCanceller},
};

/// WebRTC echo cancellation operating on 10 ms mono PCM frames.
pub struct EchoProcessor {
    processor: Processor,
    sample_rate_hz: u32,
}

impl EchoProcessor {
    /// Creates an echo canceller at 16 or 48 kHz.
    pub fn new(sample_rate_hz: u32) -> Result<Self, String> {
        if !matches!(sample_rate_hz, 16_000 | 48_000) {
            return Err("sample rate must be 16000 or 48000 Hz".into());
        }

        let processor = Processor::new(sample_rate_hz).map_err(|e| e.to_string())?;
        processor.set_config(Config {
            echo_canceller: Some(EchoCanceller::default()),
            ..Config::default()
        });

        Ok(Self {
            processor,
            sample_rate_hz,
        })
    }

    /// Cancels speaker echo from one microphone frame using the matching render frame.
    /// Both frames must contain exactly 10 ms of mono float PCM in the range [-1, 1].
    pub fn process_pair(&self, render: &[f32], capture: &[f32]) -> Result<Vec<f32>, String> {
        self.process_render(render)?;
        self.process_capture(capture)
    }

    /// Supplies one 10 ms frame of the exact audio sent to the speaker.
    pub fn process_render(&self, render: &[f32]) -> Result<(), String> {
        self.validate_frame(render)?;
        let mut frame = render.to_vec();
        self.processor
            .process_render_frame([frame.as_mut_slice()])
            .map_err(|e| e.to_string())
    }

    /// Removes the supplied speaker echo from one 10 ms microphone frame.
    pub fn process_capture(&self, capture: &[f32]) -> Result<Vec<f32>, String> {
        self.validate_frame(capture)?;
        let mut frame = capture.to_vec();
        self.processor
            .process_capture_frame([frame.as_mut_slice()])
            .map_err(|e| e.to_string())?;
        Ok(frame)
    }

    fn validate_frame(&self, samples: &[f32]) -> Result<(), String> {
        let expected = self.sample_rate_hz as usize / 100;
        if samples.len() == expected {
            Ok(())
        } else {
            Err(format!("expected {expected} samples per frame"))
        }
    }
}

mod ffi;
