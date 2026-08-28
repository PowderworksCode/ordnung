#!/usr/bin/env bash
# Regenerate site/content/checks.json from the binary.
#
# The manifest is what the documentation is checked against, so it has to come
# out of the binary rather than be maintained beside it. A check added, renamed
# or withdrawn without regenerating the manifest fails the docs workflow rather
# than reaching the website.
set -euo pipefail

cd "$(dirname "$0")/.."

OUT=site/content/checks.json
CHECK=0
[[ ${1:-} == "--check" ]] && CHECK=1

cargo build --release --quiet
./target/release/ordnung --list-checks --json >"${OUT}.new"

if [[ $CHECK -eq 1 ]]; then
    if ! diff -u "$OUT" "${OUT}.new"; then
        rm -f "${OUT}.new"
        echo "checks-manifest: $OUT is stale. Run scripts/checks-manifest.sh and commit the result." >&2
        exit 1
    fi
    rm -f "${OUT}.new"
    echo "checks-manifest: $OUT matches the binary"
else
    mv "${OUT}.new" "$OUT"
    echo "checks-manifest: wrote $OUT"
fi
