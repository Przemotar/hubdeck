#!/bin/sh
# Restart the dev app cleanly (macOS/Linux): ask it to quit through the local API, so the deck is
# handed back (killing it can leave the keys module mid-picture), then start it in the tray again.
# Log: logs/dev.log. Usage: scripts/dev-restart.sh
cd "$(dirname "$0")/.." || exit 1
DATA="$HOME/Library/Application Support/dev.hubdeck.app"
[ -d "$DATA" ] || DATA="${XDG_DATA_HOME:-$HOME/.local/share}/dev.hubdeck.app"
if [ -f "$DATA/api-http-port" ] && pgrep -f "target/debug/hubdeck" >/dev/null; then
  curl -s -m 5 -X POST "http://127.0.0.1:$(cat "$DATA/api-http-port")/api/quit" -H "Authorization: Bearer $(cat "$DATA/api-token")" >/dev/null
  for _ in 1 2 3 4 5 6 7 8 9 10; do pgrep -f "target/debug/hubdeck" >/dev/null || break; sleep 1; done
fi
# (the app writes this itself at exit; an older build may not)
[ -d "$DATA" ] && python3 -c "import time; print(int(time.time()*1000))" > "$DATA/released-at"
pkill -f "tauri dev" 2>/dev/null
# a vite server left from browser tests holds the port the dev app needs
lsof -tiTCP:1420 -sTCP:LISTEN 2>/dev/null | xargs kill 2>/dev/null
sleep 1
mkdir -p logs
(cd app && RUST_LOG="${RUST_LOG:-info}" nohup pnpm tauri dev --no-watch -- -- --hidden > ../logs/dev.log 2>&1 &)
for _ in $(seq 1 120); do grep -qa "connected keys\|init keys\|plugin server on" logs/dev.log 2>/dev/null && break; sleep 2; done
grep -a "error\[\|panicked\|init keys" logs/dev.log | head -5
echo "started"
