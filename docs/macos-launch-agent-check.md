# Real Mac check

Run `cargo test -p leko-cli --test real_launch_agent` in a logged-in macOS GUI session with the Swift command-line tools installed. The test writes a disposable plist outside `~/Library/LaunchAgents`, registers a uniquely named agent in the current user's GUI domain, launches a disposable app, and loads a rule that closes it. It unloads the test agent and verifies it is absent afterward; cleanup also runs on test failure.

The test passed on the development Mac on 2026-11-06. It does not install Leko's persistent agent or test logging out and back in.

To preview a persistent agent, run `leko agent install --executable /absolute/path/to/leko --settings /absolute/path/to/settings.zip`. Add `--write` only when ready to install and start it. Use `leko agent status` to inspect it and `leko agent uninstall` to preview removal; add `--write` to remove it. A per-user agent does not protect itself against the account owner or an administrator.
