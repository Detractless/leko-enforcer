use leko_audio::EchoProcessor;

#[test]
fn rejects_frames_that_are_not_ten_milliseconds() {
    let aec = EchoProcessor::new(16_000).unwrap();
    let err = aec.process_pair(&[0.0; 80], &[0.0; 160]).unwrap_err();
    assert!(err.contains("160 samples"));
}

#[test]
fn reduces_a_delayed_speaker_echo_after_adaptation() {
    let aec = EchoProcessor::new(16_000).unwrap();
    let frame_len = 160;
    let delay_samples = 320;
    let frames = 600;
    let mut render = vec![0.0_f32; frames * frame_len];
    let mut state = 0x1234_5678_u32;
    for sample in &mut render {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *sample = (state as i32 as f32 / i32::MAX as f32) * 0.25;
    }

    let mut residual_energy = 0.0;
    let mut input_energy = 0.0;
    let mut measured = 0;
    for frame_index in 0..frames {
        let start = frame_index * frame_len;
        let render_frame = &render[start..start + frame_len];
        let capture_frame: Vec<f32> = (0..frame_len)
            .map(|i| {
                let absolute = start + i;
                if absolute >= delay_samples {
                    render[absolute - delay_samples] * 0.6
                } else {
                    0.0
                }
            })
            .collect();

        let cleaned = aec.process_pair(render_frame, &capture_frame).unwrap();
        if frame_index >= 400 {
            residual_energy += cleaned.iter().map(|x| x * x).sum::<f32>();
            input_energy += capture_frame.iter().map(|x| x * x).sum::<f32>();
            measured += cleaned.len();
        }
    }

    let residual_rms = (residual_energy / measured as f32).sqrt();
    let input_rms = (input_energy / measured as f32).sqrt();
    assert!(
        residual_rms < input_rms * 0.5,
        "AEC residual {residual_rms:.4} should be below half of echo input {input_rms:.4}"
    );
}
