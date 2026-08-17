#!/usr/bin/env bash
set -e

echo "======================================================="
echo "  PROJECT KNOT: STEP 1 DUAL-SEAT COMPOSITOR LAUNCHER   "
echo "======================================================="

# Build latest binary
cargo build --bin knot-core

# Socket name
KNOT_SOCK="wayland-knot-0"

# Remove any stale socket
rm -f "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/${KNOT_SOCK}"*

# Launch knot-core in background with explicit socket (manual interactive mode)
echo "🚀 Starting Knot Multi-Seat Compositor on WAYLAND_DISPLAY=${KNOT_SOCK}..."
./target/debug/knot-core --socket "${KNOT_SOCK}" &
KNOT_PID=$!

# Wait until socket is created
for i in {1..30}; do
    if [ -e "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/${KNOT_SOCK}" ]; then
        break
    fi
    sleep 0.1
done

echo "🌟 Knot Socket Ready: WAYLAND_DISPLAY=${KNOT_SOCK}"
echo "🚀 Spawning 2 standalone terminal windows into Knot..."

# Launch two standalone terminal instances with full logging (no suppression)
WAYLAND_DISPLAY="${KNOT_SOCK}" GDK_BACKEND=wayland GSK_RENDERER=gl ptyxis -s &
sleep 0.6
WAYLAND_DISPLAY="${KNOT_SOCK}" GDK_BACKEND=wayland GSK_RENDERER=gl ptyxis -s &

echo ""
echo "======================================================="
echo "  TWO TERMINAL WINDOWS ARE NOW RUNNING IN KNOT!        "
echo "  • Manual input is now ACTIVE!                        "
echo "                                                       "
echo "  Try these controls:                                  "
echo "  1. Click Terminal 1 -> Type as Alice (Pink 💖)       "
echo "  2. Press [Tab] -> Switch active seat to Bob (Cyan 💙)"
echo "  3. Click Terminal 2 -> Type as Bob                   "
echo "  4. Press [F2] -> Toggle automated concurrent typing  "
echo "======================================================="

wait $KNOT_PID
