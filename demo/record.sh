#!/usr/bin/env bash
# Record the hop demo GIF: seed an isolated sandbox, build the release binary
# if needed, then drive the TUI with VHS. Output: demo/hop.gif
set -euo pipefail

cd "$(dirname "$0")/.."

command -v vhs >/dev/null || { echo "error: vhs not found (brew install vhs)" >&2; exit 1; }

echo "==> seeding sandbox ($PWD/demo/.demo-home)"
python3 demo/seed.py demo/.demo-home

if [ ! -x target/release/hop ]; then
  echo "==> building release binary"
  cargo build --release
fi

echo "==> recording demo/demo.tape"
vhs demo/demo.tape

echo "==> wrote demo/hop.gif"
