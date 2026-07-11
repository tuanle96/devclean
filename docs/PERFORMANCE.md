# Performance

Performance changes are benchmark-driven. Run:

```bash
scripts/benchmark.sh
```

The deterministic fixture creates 500 projects containing 1,000 generated candidates, builds `devclean` in release mode, and scans the fixture on one filesystem.

Baseline on an Apple Silicon development machine on 2026-07-11:

```text
real 0.51s
user 0.00s
sys  0.08s
```

The scan is filesystem-bound and already completes this fixture well below one second. Parallel traversal is therefore intentionally not the default: saturating an SSD would add complexity and I/O contention without a measured user-visible benefit at this scale. Revisit only with representative multi-million-entry fixtures and a repeatable improvement above 5%.
