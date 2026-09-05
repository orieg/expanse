#!/usr/bin/env bash
# The AGENTS.md §5 "Mandatory Local Gates", as ONE command, mirroring what the
# `lint` and `test` CI jobs actually run — so "did you run the gates?" has a
# checkable answer. CI (the `ci-gate` rollup) remains the authority.
#
#   scripts/gate.sh                 # fmt, clippy, workspace tests (PROPTEST_CASES=500), repo scripts, docs hygiene
#   scripts/gate.sh --quick         # fmt, clippy, repo scripts, docs hygiene only (no cargo test)
#   scripts/gate.sh --miri          # additionally run the Tier-1 Miri filter CI runs (never the full suite)
#   scripts/gate.sh --with-bindings # also test expanse-php / expanse-py (see below)
#
# Scope note — this is NOT byte-identical to CI. The test step excludes
# `expanse-php` (needs PHP headers, as CI does) and `expanse-py` (a PyO3
# extension module whose test binary needs libpythonX.Y on the rpath, which a
# plain `cargo test` on a dev machine usually lacks). CI's `test` job runs
# expanse-py on its runners, and `test-php` / `php-judy-*` cover PHP. Pass
# --with-bindings to include both locally once your toolchain is set up.
#
# Exit code is non-zero on the first failing gate. No benchmark is run here —
# instruction-count and wall-clock gates need the CI runners / reference host.
set -euo pipefail

QUICK=0; MIRI=0; WITH_BINDINGS=0
for arg in "$@"; do
  case "$arg" in
    --quick) QUICK=1 ;;
    --miri) MIRI=1 ;;
    --with-bindings) WITH_BINDINGS=1 ;;
    -h|--help) sed -n '2,21p' "$0"; exit 0 ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

cd "$(git rev-parse --show-toplevel)"
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"
export PROPTEST_CASES="${PROPTEST_CASES:-500}"

step() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }

step "1/6 cargo fmt --all --check"
cargo fmt --all --check

step "2/6 cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings

if [ "$QUICK" -eq 0 ]; then
  EXCLUDE=(--exclude expanse-php --exclude expanse-py)
  [ "$WITH_BINDINGS" -eq 1 ] && EXCLUDE=()
  step "3/6 cargo test --workspace ${EXCLUDE[*]:-} (PROPTEST_CASES=$PROPTEST_CASES)"
  cargo test --workspace "${EXCLUDE[@]}"
else
  step "3/6 cargo test — skipped (--quick)"
fi

step "4/6 repository consistency scripts (as in the CI lint job)"
python3 scripts/bump_version.py --check
python3 scripts/check_abi_parity.py
python3 scripts/check_ecosystem_theme.py --local-only
python3 scripts/check_ci_gate.py
python3 scripts/check_bench_suites.py
python3 scripts/check_bench_shapes.py
python3 scripts/check_bench_pin.py
python3 scripts/check_man_pages.py
python3 scripts/check_deletion_rationale.py
python3 scripts/check_test_floors.py
python3 scripts/perf_report.py --self-test
python3 scripts/pin_exposure.py --self-test
python3 scripts/warmup_ramp.py --self-test
python3 scripts/bench_report.py --self-test
python3 scripts/check_docs_hygiene.py --self-test
python3 scripts/check_ecosystem_theme.py --self-test
python3 scripts/check_bench_suites.py --self-test
python3 scripts/check_bench_shapes.py --self-test
python3 scripts/check_bench_pin.py --self-test
python3 scripts/check_man_pages.py --self-test
python3 scripts/check_man_examples.py --self-test
python3 scripts/check_abi_parity.py --self-test
python3 scripts/check_deletion_rationale.py --self-test
python3 scripts/check_test_floors.py --self-test
python3 scripts/esp32_bench_harvest.py --self-test
python3 scripts/verify_release_registries.py --self-test
python3 scripts/embedded_envelope.py
python3 scripts/art_envelope.py
python3 scripts/set_algebra_bounds.py
python3 scripts/set_domain_bounds.py

# Verifying the documented example output needs libexpanse built; the CI
# man-examples job always runs it. Locally it is opt-in, so `gate.sh` stays
# fast and does not force a release build.
if [ -n "$(ls target/release/libexpanse.* 2>/dev/null)" ]; then
  python3 scripts/check_man_examples.py
else
  echo "  (skipping man-page example run: build with 'cargo build --release -p expanse-capi' to enable)"
fi

step "5/6 docs hygiene (time estimates, PII, provenance advisory)"
python3 scripts/check_docs_hygiene.py

if [ "$MIRI" -eq 1 ]; then
  step "6/6 Tier-1 Miri filter (the per-PR CI scope; the full suite runs nightly in CI only)"
  cargo miri test -p expanse-trie --lib -- leaf:: node:: slot:: alloc:: bits:: types:: \
    blobmap::tests::deferred strmap::tests::deferred bytesmap::tests::deferred
else
  step "6/6 Miri — skipped (pass --miri for the Tier-1 filter; CI runs it on every PR)"
fi

printf '\n\033[1;32mAll local gates passed.\033[0m Callgrind / wall-clock gates run in CI and on the reference host.\n'
