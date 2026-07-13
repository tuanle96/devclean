#!/usr/bin/env bash
set -euo pipefail

validator="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/validate-release-tag.sh"

assert_accepts() {
  local channel="$1"
  local tag="$2"
  local expected="$3"
  local actual
  actual="$($validator "$channel" "$tag" "$expected")"
  test "$actual" = "$expected"
}

assert_rejects() {
  if "$validator" "$@" >/dev/null 2>&1; then
    echo "expected tag policy to reject: $*" >&2
    exit 1
  fi
}

assert_accepts core v0.5.0 0.5.0
assert_accepts core v12.34.56 12.34.56
assert_accepts vscode vscode-v0.1.0 0.1.0

assert_rejects core vscode-v0.1.0
assert_rejects core v0.5
assert_rejects core v0.5.0-beta.1
assert_rejects core version-v0.5.0
assert_rejects vscode v0.1.0
assert_rejects vscode vscode-v0.1
assert_rejects vscode vscode-v0.1.0-beta.1
assert_rejects vscode vscode-v0.1.0 0.2.0

echo "release tag policy tests passed"
