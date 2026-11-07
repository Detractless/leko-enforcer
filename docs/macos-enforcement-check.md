# Real Mac check

Run `cargo test -p leko-cli --test real_enforcement` on a logged-in Mac with the Swift command-line tools installed. The test builds a disposable app with its own bundle ID, starts `leko enforce` with an empty settings zip, launches the app, and checks that it stays running. It then replaces the zip with a session rule that closes that app and waits for the process to exit. The test cleans up the disposable app and enforcement process.

On 2026-11-06 this check passed on the development Mac. It exercises normal AppKit termination; an app that refuses the request remains open and is retried. The loop tests simulate a delayed wakeup and forward and backward clock changes; they do not put the Mac to sleep or change its system clock. Check cadence is targeted at 900 ms to allow for overhead, but macOS scheduling or slow operations can cause intervals over one second. Slow checks and long intervals measured by the monotonic clock are reported; reporting a real sleep interval has not been verified. No strict one-second upper bound is guaranteed.

With `LEKO_REAL_SLEEP_CHECK=1`, the test also requests real Mac sleep and checks the power log for a sleep event. On 2026-11-06 that check passed: the enforcement process remained alive, then loaded a timed rule and terminated the disposable app after wake.

With `LEKO_SLEEP_BOUNDARY_CHECK=1`, the test starts an app under an open-hours rule, requests sleep before the hours end, checks the power log for sleep before and wake after the boundary, and checks that the app exits after wake. This passed on 2026-11-06; the power log recorded sleep at 18:48:51 and wake at 18:50:55 local time. This does not prove sleep intervals are reported.
