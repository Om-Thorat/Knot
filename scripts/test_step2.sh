#!/usr/bin/env bash
set -e

# Project Knot: Step 2 Verification Script
# Launches knot-core multi-seat compositor and tests single-seat app isolation via knot-run

export PATH="$HOME/.cargo/bin:$PATH"
export RUST_BACKTRACE=1
export RUST_LOG=debug

echo "======================================================="
echo "  🚀 BUILDING & LAUNCHING KNOT STEP 2 (ISLAND PROXY)  "
echo "======================================================="

# Build all 3 binaries: knot-core, knot-island, knot-run
cargo build --bin knot-core --bin knot-island --bin knot-run

# Cleanup old processes
pkill -f knot-core || true
pkill -f knot-island || true
pkill -f knot-run || true

SOCKET="wayland-knot-0"
SOCKET_PATH="${XDG_RUNTIME_DIR:-/run/user/$UID}/$SOCKET"
rm -f "$SOCKET_PATH"

echo "🌟 Launching Knot Multi-Seat Core Compositor..."
./target/debug/knot-core --socket "$SOCKET" &
CORE_PID=$!

echo "⏳ Waiting for Knot socket ($SOCKET_PATH)..."
for i in {1..50}; do
    if [ -e "$SOCKET_PATH" ]; then
        echo "✅ Knot core socket is ready!"
        break
    fi
    sleep 0.1
done

if [ ! -e "$SOCKET_PATH" ]; then
    echo "❌ Error: Knot core socket failed to appear!"
    exit 1
fi

sleep 0.5

echo ""
echo "======================================================="
echo "  🏝️ SPAWNING APPS INSIDE ISOLATED KNOT ISLANDS...    "
echo "  • Window 1: Terminal (ptyxis) via knot-run           "
echo "  • Window 2: Text Editor (gnome-text-editor) via knot-run"
echo "                                                       "
echo "  Controls inside Knot:                                "
echo "  - [Tab]: Toggle active interactive seat (Alice/Bob)  "
echo "  - Clicks & inputs are multiplexed per island!        "
echo "======================================================="
echo ""

# Launch App 1 inside Knot Island #1
./target/debug/knot-run --parent-socket "$SOCKET" ptyxis -- -s &

# Launch App 2 inside Knot Island #2
./target/debug/knot-run --parent-socket "$SOCKET" gnome-text-editor &

# Wait for core
wait $CORE_PID
