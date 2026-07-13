# Performance

Performance changes are benchmark-driven. The default command creates a deterministic fixture and reports the median of five scans:

```bash
scripts/benchmark.sh
```

The deterministic fixture creates 500 projects containing 1,000 generated candidates, builds `devclean` in release mode, and scans the fixture on one filesystem. Override the run count, root, or executable when comparing real trees and release baselines:

```bash
DEVCLEAN_BENCH_RUNS=7 DEVCLEAN_BENCH_ROOT="$HOME/Dev" scripts/benchmark.sh
DEVCLEAN_BENCH_RUNS=7 DEVCLEAN_BENCH_ROOT="$HOME/Dev" \
  DEVCLEAN_BENCH_BINARY=/path/to/released/devclean scripts/benchmark.sh
```

Alternate baseline and candidate runs on the same machine. Record at least five runs per binary and compare medians; do not infer an improvement from a single scan. Filesystem cache, APFS state, power mode, and background I/O can dominate wall time on a large tree.

Example deterministic-fixture result on an Apple Silicon development machine on 2026-07-11, after parallel size measurement landed:

```text
run 1: 0.08s
...
median: 0.04s
```

Directory traversal and classification stay single-threaded: they are pruning-heavy and keep the safety-relevant control flow easy to review. Size and modification-time measurement runs afterwards on a bounded pool of at most eight worker threads, preserving input order and per-candidate error reporting. On a real development tree with 83 candidates totaling about 241 GiB, observed single-run times ranged from roughly 2m23s to 4m26s during review; that variance is why repeated medians are the evidence boundary.

The remaining sequential costs include the classification walk and the per-candidate Git tracked-file guard. Keep a performance change only when alternating repeated runs show a median improvement above 5% on a representative multi-million-entry tree without weakening safety checks.
