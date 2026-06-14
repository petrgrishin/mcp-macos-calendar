#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/coverage.sh [OPTIONS]

Options:
  --update-coverage-md   Replace the Current Summary table in COVERAGE.md.
  -h, --help             Show this help message.
EOF
}

UPDATE_COVERAGE_MD=false
while (($#)); do
  case "$1" in
    --update-coverage-md)
      UPDATE_COVERAGE_MD=true
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown option: %s\n\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_ID="$(date +%Y%m%d%H%M%S)"
COVERAGE_ROOT="$ROOT/target/coverage"
PROFILE_DIR="$COVERAGE_ROOT/profiles-$RUN_ID"
RUN_DIR="$COVERAGE_ROOT/$RUN_ID"
TARGET_DIR="$ROOT/target/coverage-target"
PROFDATA="$RUN_DIR/mcp-macos-calendar.profdata"
REPORT="$RUN_DIR/coverage-report.txt"
SUMMARY="$RUN_DIR/coverage-summary.md"

mkdir -p "$PROFILE_DIR" "$RUN_DIR"

(
  cd "$ROOT"
  CARGO_INCREMENTAL=0 \
    CARGO_TARGET_DIR="$TARGET_DIR" \
    RUSTFLAGS="-Cinstrument-coverage" \
    LLVM_PROFILE_FILE="$PROFILE_DIR/mcp-macos-calendar-%p-%m.profraw" \
    cargo test
)

xcrun llvm-profdata merge -sparse "$PROFILE_DIR"/*.profraw -o "$PROFDATA"

objects=()
while IFS= read -r candidate; do
  if file "$candidate" | grep -q "Mach-O 64-bit executable" &&
    LLVM_PROFILE_FILE="$RUN_DIR/list-%p-%m.profraw" "$candidate" --list >/dev/null 2>&1; then
    objects+=("--object=$candidate")
  fi
done < <(
  find "$TARGET_DIR/debug/deps" \
    -maxdepth 1 \
    -type f \
    -perm -111 \
    -name 'mcp_macos_calendar-*' \
    -print
)

if ((${#objects[@]} == 0)); then
  printf 'No test executables found under %s\n' "$TARGET_DIR/debug/deps" >&2
  exit 1
fi

sources=()
while IFS= read -r source; do
  sources+=("$source")
done < <(
  cd "$ROOT"
  rg --files src
)

(
  cd "$ROOT"
  xcrun llvm-cov report "${objects[0]#--object=}" \
    --instr-profile="$PROFDATA" \
    "${objects[@]:1}" \
    "${sources[@]}"
) | tee "$REPORT"

printf '\nCoverage report written to %s\n' "$REPORT"

if [[ "$UPDATE_COVERAGE_MD" == true ]]; then
  awk '
    BEGIN {
      print "| File | Line coverage | Function coverage | Region coverage |"
      print "| --- | ---: | ---: | ---: |"
    }
    /^Filename[[:space:]]/ {
      in_table = 1
      next
    }
    /^-+$/ {
      next
    }
    in_table && $1 == "TOTAL" {
      printf "| **Total** | **%s** | **%s** | **%s** |\n", $10, $7, $4
      next
    }
    in_table && NF >= 10 {
      printf "| `src/%s` | %s | %s | %s |\n", $1, $10, $7, $4
    }
  ' "$REPORT" > "$SUMMARY"

  if ! grep -q '<!-- coverage-summary:start -->' "$ROOT/COVERAGE.md" ||
    ! grep -q '<!-- coverage-summary:end -->' "$ROOT/COVERAGE.md"; then
    printf 'COVERAGE.md is missing coverage-summary markers.\n' >&2
    exit 1
  fi

  awk -v summary="$SUMMARY" '
    BEGIN {
      while ((getline line < summary) > 0) {
        replacement = replacement line "\n"
      }
      close(summary)
    }
    /<!-- coverage-summary:start -->/ {
      print
      printf "%s", replacement
      in_block = 1
      next
    }
    /<!-- coverage-summary:end -->/ {
      in_block = 0
      print
      next
    }
    !in_block {
      print
    }
  ' "$ROOT/COVERAGE.md" > "$RUN_DIR/COVERAGE.md"

  mv "$RUN_DIR/COVERAGE.md" "$ROOT/COVERAGE.md"
  printf 'COVERAGE.md summary table updated.\n'
fi
