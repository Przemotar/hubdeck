#!/bin/sh
# Build HubDeck.app, sign it with the local "HubDeck Local" certificate when this Mac has one (so
# macOS keeps its Accessibility and Automation permissions across versions; a private, self-signed
# certificate, not for releases), and install it into /Applications. The running copy is quit and
# the new one starts in the background, in the menu bar: it never takes the owner's focus.
# Usage: scripts/mac-install.sh [--no-build | --no-install] [--test]
#   --test   a test build for the owner's deck, as scripts/test-build.ps1 on Windows: thin LTO,
#            incremental, half the cores at low priority, its own target folder
set -e
cd "$(dirname "$0")/.."
BUILD=1
INSTALL=1
TARGET=target
for arg in "$@"; do
  case "$arg" in
    --no-build) BUILD=0 ;;
    --no-install) INSTALL=0 ;;
    --test)
      TARGET=target/test-build
      HALF=$(( $(sysctl -n hw.ncpu) / 2 )); [ "$HALF" -lt 2 ] && HALF=2
      export CARGO_TARGET_DIR="$PWD/$TARGET"
      export CARGO_PROFILE_RELEASE_LTO=thin
      export CARGO_BUILD_JOBS=$HALF
      export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=$HALF
      export CARGO_PROFILE_RELEASE_INCREMENTAL=true
      # only a test build reads shaders from the folder named in `shader-dir.txt` (canvas.rs)
      export HUBDECK_TEST_BUILD=1
      ;;
  esac
done
if [ "$BUILD" = 1 ]; then
  # every shader compiles before the build (scripts/wgsl-check.py; needs `wgpu` in .venv)
  if [ -x .venv/bin/python ] && .venv/bin/python -I -c "import wgpu" 2>/dev/null; then
    .venv/bin/python -I scripts/wgsl-check.py | grep -v '^ok' || true
  fi
  started=$(date +%s)
  (cd app && nice -n 15 pnpm tauri build --bundles app > ../logs/mac-bundle.log 2>&1) || { tail -20 logs/mac-bundle.log; exit 1; }
  echo "built in $(( ($(date +%s) - started) / 60 )) min"
fi
[ "$INSTALL" = 1 ] || exit 0
APP=$TARGET/release/bundle/macos/HubDeck.app
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
