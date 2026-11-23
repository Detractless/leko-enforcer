#!/usr/bin/env bash
# Remove the Leko LaunchAgent and browser native messaging hosts.
set -euo pipefail

HOME_DIR="${HOME:-/Users/$(whoami)}"
PLIST="$HOME_DIR/Library/LaunchAgents/com.leko.enforcer.plist"
HOST_NAME="com.leko.enforcer.json"

if [[ -f "$PLIST" ]]; then
  launchctl unload "$PLIST" 2>/dev/null || true
  rm -f "$PLIST"
  echo "removed $PLIST"
fi

for dir in \
  "$HOME_DIR/Library/Application Support/Google/Chrome/NativeMessagingHosts" \
  "$HOME_DIR/Library/Application Support/Vivaldi/NativeMessagingHosts" \
  "$HOME_DIR/Library/Application Support/Microsoft Edge/NativeMessagingHosts"; do
  if [[ -f "$dir/$HOST_NAME" ]]; then
    rm -f "$dir/$HOST_NAME"
    echo "removed $dir/$HOST_NAME"
  fi
done

echo "Leko agent and hosts removed."
