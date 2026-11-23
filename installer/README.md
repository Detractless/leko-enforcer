# Leko macOS installer

`install.sh` installs a per-user `KeepAlive` LaunchAgent and native messaging host
manifests for Chrome, Vivaldi, and Edge. It never requests root; the agent runs as
the logged-in user and calls the `leko agent` CLI, which owns enforcement ticks.

## Build and install

```
cargo build --release --workspace
LEKO_EXTENSION_ID=<chrome-extension-id> ./installer/install.sh
```

`LEKO_BIN` and `NMH_BIN` can override the release binary paths. `DRY_RUN=1` prints
every action without writing files, which is also how the acceptance check verifies
idempotence. `uninstall.sh` reverses the install.

## .pkg layout

A distributable package can wrap these files without changing them:

```
leko.pkg/
  com.leko.enforcer.pkg      # payload: target/release/leko, leko-nmh, installer/*
  Distribution              # references the component package
  Resources/
    README.md
```

The post-install script should call `install.sh` with the extension id supplied by
the installer UI. Keep the private key and secrets out of the package.

## Heartbeat wiring

The extension connects to `com.leko.enforcer` over stdio. The host binary
(`leko-nmh`) is a length-framed echo channel; the Rust `ExtensionHandshake`
contract in `enforcers/browser/` interprets the extension state reported through
it. A missing or stale heartbeat fails closed while a block is locked.
