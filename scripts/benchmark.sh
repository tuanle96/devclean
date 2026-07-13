#!/bin/sh
set -eu

runs=${DEVCLEAN_BENCH_RUNS:-5}
case "$runs" in
  ''|*[!0-9]*)
    echo "DEVCLEAN_BENCH_RUNS must be a positive integer" >&2
    exit 2
    ;;
esac
if [ "$runs" -lt 1 ]; then
  echo "DEVCLEAN_BENCH_RUNS must be at least 1" >&2
  exit 2
fi

fixture_root=""
timing_dir=$(mktemp -d "${TMPDIR:-/tmp}/devclean-bench-times.XXXXXX")
cleanup() {
  rm -rf "$timing_dir"
  if [ -n "$fixture_root" ]; then
    rm -rf "$fixture_root"
  fi
}
trap cleanup EXIT INT TERM

if [ -n "${DEVCLEAN_BENCH_ROOT:-}" ]; then
  root=$DEVCLEAN_BENCH_ROOT
  if [ ! -d "$root" ]; then
    echo "DEVCLEAN_BENCH_ROOT is not a directory: $root" >&2
    exit 2
  fi
  fixture_label="real tree: $root"
else
  fixture_root=$(mktemp -d "${TMPDIR:-/tmp}/devclean-bench.XXXXXX")
  root=$fixture_root
  fixture_label="fixture: 500 projects / 1,000 candidates"

  index=1
  while [ "$index" -le 500 ]; do
    project="$root/project-$index"
    mkdir -p "$project/target/debug" "$project/node_modules/package"
    dd if=/dev/zero of="$project/target/debug/object.bin" bs=4096 count=1 2>/dev/null
    dd if=/dev/zero of="$project/node_modules/package/index.js" bs=4096 count=1 2>/dev/null
    index=$((index + 1))
  done
fi

binary=${DEVCLEAN_BENCH_BINARY:-target/release/devclean}
if [ "$binary" = "target/release/devclean" ]; then
  cargo build --release --locked >/dev/null
fi
if [ ! -x "$binary" ]; then
  echo "benchmark binary is not executable: $binary" >&2
  exit 2
fi

echo "$fixture_label"
echo "binary: $binary"
echo "runs: $runs"

index=1
while [ "$index" -le "$runs" ]; do
  timing="$timing_dir/run-$index.time"
  /usr/bin/time -p "$binary" scan --allow-tracked "$root" >/dev/null 2>"$timing"
  real_seconds=$(awk '$1 == "real" { print $2 }' "$timing")
  if [ -z "$real_seconds" ]; then
    echo "run $index did not produce a real-time measurement" >&2
    exit 1
  fi
  printf 'run %s: %ss\n' "$index" "$real_seconds"
  printf '%s\n' "$real_seconds" >>"$timing_dir/reals"
  index=$((index + 1))
done

median=$(sort -n "$timing_dir/reals" | awk '
  { values[NR] = $1 }
  END {
    middle = int((NR + 1) / 2)
    if (NR % 2 == 1) {
      printf "%.2f", values[middle]
    } else {
      printf "%.2f", (values[middle] + values[middle + 1]) / 2
    }
  }
')
printf 'median: %ss\n' "$median"
