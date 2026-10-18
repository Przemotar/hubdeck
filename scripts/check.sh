#!/usr/bin/env bash
# Every check a commit needs, stopping at the first that fails (2026-10-18: three commits went out
# that day with a failing clippy, because `clippy | tail` hid its exit code).
#   scripts/check.sh && git commit ...
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
cargo fmt --check -p hubdeck
cargo clippy -p hubdeck --all-targets -q -- -D warnings
app/node_modules/.bin/tsc --noEmit -p app
(cd app && pnpm -s test >/dev/null)
if ! out=$(cargo test -q -p mh-protocol -p mh-device -p hubdeck 2>&1); then
  echo "$out" | grep -E "FAILED|panicked|failures:" -A3 | head -40
  exit 1
fi
echo "$out" | grep -E "^test result" | grep -v "ok. 0 passed"
echo "checks passed"
