#!/usr/bin/env bash
set -e

# Project Knot: Test Suite Runner
export PATH="$HOME/.cargo/bin:$PATH"
export RUST_BACKTRACE=1

echo "======================================================="
echo "  🧪 RUNNING PROJECT KNOT TEST SUITE                  "
echo "======================================================="

echo "📦 1. Building all binaries (knot-core, knot-island, knot-run, knot-client)..."
cargo build --bin knot-core --bin knot-island --bin knot-run --bin knot-client

echo "🧪 2. Running unit & integration tests..."
cargo test --all -- --nocapture

echo "======================================================="
echo "  ✅ ALL TESTS PASSED SUCCESSFULLY!                   "
echo "======================================================="
