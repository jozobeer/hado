#!/usr/bin/env bash
# Idempotent bootstrap for the hado compiler dev environment.
# Prepares: Boehm GC dev headers, the pinned Rust toolchain, the offline
# crate cache, and a warm debug build. Safe to re-run.
set -euo pipefail

# Rust version required by Cranelift 0.135 (README: 1.95+, 1.98 verified).
RUST_VERSION="1.98.0"

echo "==> Ensuring system packages (cc, libgc-dev)"
if ! dpkg -s libgc-dev >/dev/null 2>&1; then
  sudo apt-get update -qq
  sudo apt-get install -y --no-install-recommends libgc-dev
fi

echo "==> Ensuring Rust ${RUST_VERSION}"
if ! rustup toolchain list | grep -q "^${RUST_VERSION}-"; then
  rustup toolchain install "${RUST_VERSION}" --profile minimal
fi
rustup default "${RUST_VERSION}"

# AGENTS.md documents CARGO_TARGET_DIR pointing at ~/.cargo/target so agents can
# call `cargo run` without hunting for the binary path. The environment schema
# has no env field, so persist it in the shell rc (guarded, idempotent).
BASHRC="${HOME}/.bashrc"
if ! grep -q 'CARGO_TARGET_DIR' "${BASHRC}" 2>/dev/null; then
  echo 'export CARGO_TARGET_DIR="$HOME/.cargo/target"' >> "${BASHRC}"
fi
export CARGO_TARGET_DIR="${HOME}/.cargo/target"

echo "==> Populating offline crate cache"
cargo fetch --locked

echo "==> Warming a debug build (offline)"
cargo build --offline

echo "==> hado environment ready"
