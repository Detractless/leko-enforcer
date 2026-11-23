#!/usr/bin/env bash
# Install Leko's per-user KeepAlive agent and browser native messaging hosts.
# Safe to run repeatedly. DRY_RUN=1 prints actions without writing anything.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
HOME_DIR="${HOME:-/Users/$(whoami)}"
LEKO_BIN="${LEKO_BIN:-$HERE/../target/release/leko}"
NMH_BIN="${NMH_BIN:-$HERE/../target/release/leko-nmh}"
EXTENSION_ID="${LEKO_EXTENSION_ID:-}"
DRY_RUN="${DRY_RUN:-0}"

if [[ -z "$EXTENSION_ID" ]]; then
  echo "LEKO_EXTENSION_ID is required (chrome-extension id without the scheme)." >&2
  exit 2
fi

PLIST_SRC="$HERE/com.leko.enforcer.plist"
PLIST_DST="$HOME_DIR/Library/LaunchAgents/com.leko.enforcer.plist"
HOST_NAME="com.leko.enforcer.json"

BROWSERS=(
  "$HOME_DIR/Library/Application Support/Google/Chrome/NativeMessagingHosts"
  "$HOME_DIR/Library/Application Support/Vivaldi/NativeMessagingHosts"
  "$HOME_DIR/Library/Application Support/Microsoft Edge/NativeMessagingHosts"
)

write_file() {
  local dst="$1" src="$2"
  if [[ "$DRY_RUN" == "1" ]]; then
    echo "DRY_RUN write $dst from $src"
    return
  fi
  mkdir -p "$(dirname "$dst")"
  sed -e "s|__LEKO_BIN__|$LEKO_BIN|g" "$src" > "$dst"
  echo "installed $dst"
}

write_host() {
  local dir="$1"
  local dst="$dir/$HOST_NAME"
  if [[ "$DRY_RUN" == "1" ]]; then
    echo "DRY_RUN write $dst (path=$NMH_BIN origin=$EXTENSION_ID)"
    return
  fi
  mkdir -p "$dir"
  cat > "$dst" <<JSON
{
  "name": "com.leko.enforcer",
  "description": "Leko browser extension heartbeat host",
  "path": "$NMH_BIN",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$EXTENSION_ID/"]
}
JSON
  echo "installed $dst"
}

write_file "$PLIST_DST" "$PLIST_SRC"
for dir in "${BROWSERS[@]}"; do
  write_host "$dir"
done

if [[ "$DRY_RUN" == "1" ]]; then
  echo "DRY_RUN complete: 1 LaunchAgent, ${#BROWSERS[@]} native messaging hosts"
  exit 0
fi

launchctl unload "$PLIST_DST" 2>/dev/null || true
launchctl load "$PLIST_DST"
echo "Leko agent loaded (KeepAlive). Installer is idempotent."
