#!/usr/bin/env bash
set -euo pipefail

record_outcome() {
  status=$?
  trap - EXIT
  case "$status" in
    0) outcome=clean ;;
    1) outcome=drift ;;
    *) outcome=error ;;
  esac
  if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    printf 'outcome=%s\n' "$outcome" >> "$GITHUB_OUTPUT"
    printf 'exit-code=%s\n' "$status" >> "$GITHUB_OUTPUT"
  fi
  exit "$status"
}
trap record_outcome EXIT

mode="${ORDNUNG_ACTION_MODE:-repo-check}"
repository_path="${ORDNUNG_ACTION_PATH:-.}"
fleet="${ORDNUNG_ACTION_FLEET:-}"
repository="${ORDNUNG_ACTION_REPOSITORY:-}"
apply="${ORDNUNG_ACTION_APPLY:-false}"
format="${ORDNUNG_ACTION_FORMAT:-human}"

case "$apply" in
  true|false) ;;
  *)
    printf 'error: apply must be true or false\n' >&2
    exit 2
    ;;
esac

case "$format" in
  human|json) ;;
  *)
    printf 'error: format must be human or json\n' >&2
    exit 2
    ;;
esac

# The platform triple a release asset is named for. An unrecognised platform is
# not an error: it falls back to building from source.
detect_target() {
  case "$(uname -s):$(uname -m)" in
    Linux:x86_64) printf 'x86_64-unknown-linux-gnu' ;;
    Linux:aarch64 | Linux:arm64) printf 'aarch64-unknown-linux-gnu' ;;
    Darwin:x86_64) printf 'x86_64-apple-darwin' ;;
    Darwin:arm64) printf 'aarch64-apple-darwin' ;;
    *) return 1 ;;
  esac
}

# Fetches the released binary matching this Action's ref and platform, printing
# its path. Every failure here is recoverable — the caller builds from source —
# so this reports why and returns non-zero rather than exiting.
download_release_binary() {
  local install_root="$1" ref="${ORDNUNG_ACTION_REF:-}" slug="${ORDNUNG_ACTION_SOURCE:-}"

  # Only a release tag identifies a published binary. A branch or SHA ref means
  # the consumer is tracking source, so source is what they get.
  if [[ ! "$ref" =~ ^v[0-9]+\.[0-9]+\.[0-9]+ ]]; then
    printf 'ordnung: action ref %q is not a release tag; building from source\n' "$ref" >&2
    return 1
  fi
  if [[ -z "$slug" ]]; then
    printf 'ordnung: no source repository to download from; building from source\n' >&2
    return 1
  fi
  if ! command -v gh > /dev/null 2>&1; then
    printf 'ordnung: gh is unavailable; building from source\n' >&2
    return 1
  fi

  local target
  if ! target="$(detect_target)"; then
    printf 'ordnung: no released binary for %s %s; building from source\n' \
      "$(uname -s)" "$(uname -m)" >&2
    return 1
  fi

  local archive="ordnung-${ref}-${target}.tar.gz"
  local staging="${install_root}/download"
  rm -rf "$staging"
  mkdir -p "$staging" "${install_root}/bin"

  if ! gh release download "$ref" \
    --repo "$slug" \
    --pattern "$archive" \
    --pattern "${archive}.sha256" \
    --dir "$staging" > /dev/null 2>&1; then
    printf 'ordnung: no release asset %s; building from source\n' "$archive" >&2
    return 1
  fi

  # A tampered or truncated download must not be executed.
  if ! (cd "$staging" && shasum -a 256 -c "${archive}.sha256" > /dev/null 2>&1); then
    printf 'ordnung: checksum mismatch for %s; building from source\n' "$archive" >&2
    return 1
  fi

  if ! tar -xzf "${staging}/${archive}" -C "$staging"; then
    printf 'ordnung: could not extract %s; building from source\n' "$archive" >&2
    return 1
  fi

  mv "${staging}/ordnung" "${install_root}/bin/ordnung"
  chmod +x "${install_root}/bin/ordnung"
  printf '%s' "${install_root}/bin/ordnung"
}

install_root="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/ordnung-action"
if [[ -n "${ORDNUNG_BIN:-}" ]]; then
  binary="$ORDNUNG_BIN"
elif binary="$(download_release_binary "$install_root")" && [[ -n "$binary" ]]; then
  printf 'ordnung: using the released binary for %s\n' "${ORDNUNG_ACTION_REF:-}" >&2
else
  # No published binary applies. Building takes minutes; ORDNUNG_BIN or a
  # released tag avoids it.
  cargo install \
    --path "$GITHUB_ACTION_PATH/crates/ordnung-cli" \
    --locked \
    --root "$install_root"
  binary="$install_root/bin/ordnung"
fi

arguments=()
case "$mode" in
  repo-check)
    if [[ -z "$repository" ]]; then
      printf 'error: repository is required for repo-check\n' >&2
      exit 2
    fi
    arguments=(repo-check "$repository_path" --repo "$repository")
    ;;
  check)
    arguments=(check "$repository_path")
    ;;
  fix)
    arguments=(fix "$repository_path")
    if [[ "$apply" == true ]]; then
      arguments+=(--apply)
    fi
    ;;
  github-check)
    if [[ -z "$repository" ]]; then
      printf 'error: repository is required for github-check\n' >&2
      exit 2
    fi
    arguments=(github check "$repository" --repo-root "$repository_path")
    ;;
  fleet-check)
    if [[ -z "$fleet" ]]; then
      printf 'error: fleet is required for fleet-check\n' >&2
      exit 2
    fi
    arguments=(fleet check "$fleet")
    ;;
  fleet-sync)
    if [[ -z "$fleet" || -z "$repository" ]]; then
      printf 'error: fleet and repository are required for fleet-sync\n' >&2
      exit 2
    fi
    arguments=(fleet github-sync "$fleet" --repo "$repository")
    if [[ "$apply" == true ]]; then
      arguments+=(--apply)
    fi
    ;;
  fleet-sync-all)
    if [[ -z "$fleet" ]]; then
      printf 'error: fleet is required for fleet-sync-all\n' >&2
      exit 2
    fi
    arguments=(fleet github-sync-all "$fleet")
    if [[ "$apply" == true ]]; then
      arguments+=(--apply)
    fi
    ;;
  *)
    printf 'error: unsupported Ordnung Action mode %q\n' "$mode" >&2
    exit 2
    ;;
esac

if [[ "$format" == json ]]; then
  arguments+=(--json)
fi

set +e
"$binary" "${arguments[@]}"
status=$?
set -e

exit "$status"
