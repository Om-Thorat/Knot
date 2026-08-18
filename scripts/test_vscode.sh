#!/usr/bin/env bash
set -e

# Project Knot: VS Code Multiplayer Island Test Script
# Launches knot-core compositor and opens VS Code + Terminal in isolated Knot Islands

export PATH="$HOME/.cargo/bin:$PATH"
export RUST_BACKTRACE=1
export RUST_LOG=debug

echo "======================================================="
echo "  🚀 LAUNCHING PROJECT KNOT WITH VS CODE & TERMINAL   "
echo "======================================================="

# Build binaries
cargo build --bin knot-core --bin knot-island --bin knot-run

# Cleanup old processes
pkill -f knot-core || true
pkill -f knot-island || true
pkill -f knot-run || true

SOCKET="wayland-knot-0"
SOCKET_PATH="${XDG_RUNTIME_DIR:-/run/user/$UID}/$SOCKET"
rm -f "$SOCKET_PATH"

echo "🌟 Starting Knot Compositor..."
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
echo "  🏝️ LAUNCHING VS CODE VIA KNOT-RUN...                "
echo "  • Window 1: VS Code (code)                           "
echo "  • Window 2: Terminal (ptyxis)                        "
echo "                                                       "
echo "  Controls inside Knot:                                "
echo "  - [Tab]: Toggle active seat (Alice 💖 / Bob 💙)      "
echo "  - Test typing code in VS Code simultaneously!        "
echo "======================================================="
echo ""

# Ensure dedicated user data directory so VS Code spawns a new standalone process
mkdir -p /tmp/knot-vscode

# Launch VS Code inside Knot Island
./target/debug/knot-run --parent-socket "$SOCKET" code --ozone-platform=wayland --user-data-dir /tmp/knot-vscode . &

# Launch Terminal alongside it
./target/debug/knot-run --parent-socket "$SOCKET" ptyxis -- -s &

# Wait for core
wait $CORE_PID
