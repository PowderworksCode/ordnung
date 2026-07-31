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

if [[ -n "${ORDNUNG_BIN:-}" ]]; then
  binary="$ORDNUNG_BIN"
else
  install_root="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/ordnung-action"
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
