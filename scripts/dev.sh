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

# Git looks in .git/hooks by default, which nothing tracks. The fleet's hooks
# are committed under .githooks, so they only run once a checkout is pointed at
# them; doing it here means a clone that ran this script is a clone that has
# them.
echo "==> pointing Git at the committed hooks"
git config core.hooksPath .githooks

echo "Development environment ready."
