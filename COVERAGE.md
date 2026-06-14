# Test Coverage Report

Generated on 2026-06-14 for `mcp-macos-calendar` 0.1.0.

## Test Run

```text
cargo test
90 unit tests passed
5 integration tests passed
0 failed
```

Coverage is generated with Rust LLVM instrumentation and Xcode Command Line Tools:

```bash
CARGO_INCREMENTAL=0 \
  CARGO_TARGET_DIR="$PWD/target/coverage-target" \
  RUSTFLAGS="-Cinstrument-coverage" \
  LLVM_PROFILE_FILE="$PWD/target/coverage/profiles/mcp-macos-calendar-%p-%m.profraw" \
  cargo test

xcrun llvm-profdata merge -sparse \
  target/coverage/profiles/*.profraw \
  -o target/coverage/mcp-macos-calendar.profdata

xcrun llvm-cov report <unit-test-binary> \
  --instr-profile=target/coverage/mcp-macos-calendar.profdata \
  $(rg --files src)
```

For repeatable local runs without manually resolving test binary hashes:

```bash
just coverage
just coverage --update-coverage-md
```

The generated artifacts are written under `target/coverage` and `target/coverage-target`, both covered by the existing `/target` gitignore rule.

## Current Summary

<!-- coverage-summary:start -->
| File | Line coverage | Function coverage | Region coverage |
| --- | ---: | ---: | ---: |
| `src/services/mod.rs` | 92.31% | 100.00% | 94.74% |
| `src/services/event_service.rs` | 45.43% | 71.88% | 53.24% |
| `src/services/calendar_service.rs` | 20.62% | 38.46% | 13.82% |
| `src/sse_transport.rs` | 0.00% | 0.00% | 0.00% |
| `src/main.rs` | 45.02% | 48.15% | 45.34% |
| `src/server.rs` | 37.26% | 42.86% | 50.23% |
| `src/models.rs` | 100.00% | 100.00% | 100.00% |
| `src/bridge/eventkit.rs` | 39.82% | 38.46% | 40.66% |
| `src/config.rs` | 100.00% | 100.00% | 100.00% |
| `src/tools/calendar.rs` | 100.00% | 100.00% | 100.00% |
| `src/tools/event.rs` | 100.00% | 100.00% | 100.00% |
| **Total** | **48.39%** | **53.44%** | **54.23%** |
<!-- coverage-summary:end -->

## Notes

- The report is scoped to files under `src`.
