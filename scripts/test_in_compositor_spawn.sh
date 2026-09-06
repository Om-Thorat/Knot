#!/usr/bin/env bash
set -e

export PATH="$HOME/.cargo/bin:$PATH"
export RUST_BACKTRACE=1

echo "🧪 Starting knot-core in background..."
./target/debug/knot-core --socket wayland-knot-test-0 --port 19447 &
CORE_PID=$!

sleep 1

echo "🚀 Spawning Ptyxis Terminal directly via knot-run on wayland-knot-test-0..."
./target/debug/knot-run --parent-socket wayland-knot-test-0 ptyxis -s &
APP_PID=$!

sleep 3

echo "🔍 Checking if processes are alive..."
ps aux | grep knot-

kill $APP_PID || true
kill $CORE_PID || true

echo "Done test."
