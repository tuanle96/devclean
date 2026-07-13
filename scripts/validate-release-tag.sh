#!/usr/bin/env bash
set -euo pipefail

channel="${1:-}"
tag="${2:-}"
expected_version="${3:-}"

case "$channel" in
  core)
    pattern='^v([0-9]+\.[0-9]+\.[0-9]+)$'
    ;;
  vscode)
    pattern='^vscode-v([0-9]+\.[0-9]+\.[0-9]+)$'
    ;;
  *)
    echo "usage: $0 <core|vscode> <tag> [expected-version]" >&2
    exit 2
    ;;
esac

if [[ ! "$tag" =~ $pattern ]]; then
  echo "invalid $channel release tag: $tag" >&2
  exit 1
fi

version="${BASH_REMATCH[1]}"
if [[ -n "$expected_version" && "$version" != "$expected_version" ]]; then
  echo "tag version $version does not match package version $expected_version" >&2
  exit 1
fi

printf '%s\n' "$version"
