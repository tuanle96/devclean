#!/bin/sh
set -eu

root=$(mktemp -d "${TMPDIR:-/tmp}/devclean-bench.XXXXXX")
trap 'rm -rf "$root"' EXIT INT TERM

index=1
while [ "$index" -le 500 ]; do
  project="$root/project-$index"
  mkdir -p "$project/target/debug" "$project/node_modules/package"
  dd if=/dev/zero of="$project/target/debug/object.bin" bs=4096 count=1 2>/dev/null
  dd if=/dev/zero of="$project/node_modules/package/index.js" bs=4096 count=1 2>/dev/null
  index=$((index + 1))
done

cargo build --release --locked >/dev/null
echo "fixture: 500 projects / 1,000 candidates"
/usr/bin/time -p target/release/devclean scan --all --allow-tracked "$root" >/dev/null
