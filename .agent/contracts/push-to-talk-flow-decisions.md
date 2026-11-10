# Push-to-talk flow decisions

Resolved: the user accepted the recommended defaults for the interaction, and approved a text-free visible UI.

1. **Interrupt while Maeve is responding:** Yes. Holding the circle or Space during model generation or speech playback cancels that response and begins a new recording.
2. **Remove the bottom Mute and Stop controls:** Yes. The hold gesture controls capture; Escape cancels audio that has not been submitted.
3. **Support both circle hold and Space:** Yes. Mouse and touch hold the circle; Space works when the app is focused and no editable field has focus.
4. **Visible text:** None. Screen-reader labels and announcements remain, and pending-action details stay available through speech and screen readers.

Current-code note: the existing four-second value is the acoustic continuation window after detected speech; source inspection found no automatic idle “Are you there?” prompt. The proposed two-second release grace replaces that acoustic endpoint in push-to-talk mode.
