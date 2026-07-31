#!/usr/bin/env bash
# Install and build the local dependencies needed to work on Ordnung.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> building Rust workspace"
if [[ -f ../entl/crates/entl-codebase/Cargo.toml ]] &&
    [[ -f ../entl/crates/entl-github/Cargo.toml ]]; then
    echo "==> using sibling Entl checkout"
    ./scripts/local_dev build --workspace
else
    cargo build --workspace
fi

echo "==> installing documentation dependencies"
(
    cd site
    bun install --frozen-lockfile
)

echo "Development environment ready."
