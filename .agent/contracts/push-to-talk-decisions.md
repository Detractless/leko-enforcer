# Push-to-talk decisions

Resolved: user accepted all recommended answers below.

No visible UI text; hidden screen-reader labels and announcements remain, and pending action details stay available through speech and screen readers.

1. **Interrupt while Maeve is responding:** Yes. Holding the circle or Space during model generation or speech playback cancels that response and starts a new recording.
2. **Remove the bottom Mute and Stop buttons:** Yes. The hold gesture controls capture, releasing ends the hold, and Escape cancels audio that has not been submitted.
3. **Support both circle hold and Space:** Yes. Mouse and touch hold the circle; Space works while the app is focused and no editable field has focus.

Current-code note: the four-second value is the acoustic continuation window after detected speech; I found no automatic idle “Are you there?” prompt. The proposed two-second release grace replaces that acoustic endpoint in push-to-talk mode.
