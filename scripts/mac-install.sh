#!/bin/sh
# Build HubDeck.app, sign it with the local "HubDeck Local" certificate when this Mac has one (so
# macOS keeps its Accessibility and Automation permissions across versions; a private, self-signed
# certificate, not for releases), and install it into /Applications. The running copy is quit and
# the new one starts in the background, in the menu bar: it never takes the owner's focus.
# Usage: scripts/mac-install.sh [--no-build]
set -e
cd "$(dirname "$0")/.."
if [ "$1" != "--no-build" ]; then
  (cd app && pnpm tauri build --bundles app > ../logs/mac-bundle.log 2>&1) || { tail -20 logs/mac-bundle.log; exit 1; }
fi
APP=target/release/bundle/macos/HubDeck.app
if security find-identity -p codesigning | grep -q '"HubDeck Local"'; then
  codesign --force --deep --sign "HubDeck Local" "$APP"
  echo "signed with HubDeck Local"
fi
if pgrep -f "/Applications/HubDeck.app/Contents/MacOS/hubdeck" > /dev/null; then
  osascript -e 'tell application "HubDeck" to quit' || true
  for _ in $(seq 1 20); do pgrep -f "/Applications/HubDeck.app/Contents/MacOS/hubdeck" > /dev/null || break; sleep 1; done
fi
rm -rf /Applications/HubDeck.app
cp -R "$APP" /Applications/
open -g /Applications/HubDeck.app --args --hidden
echo "installed and started in the menu bar"
