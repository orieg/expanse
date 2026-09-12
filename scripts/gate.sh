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
  # The `occ_stats` counters are compiled out by default, so every test that
  # reads them is absent from or empty in the run above. Same script as the CI
  # `test` job's extra step.
  echo "  + occ-stats tests (feature is off in the default build)"
  bash scripts/test_occ_stats.sh
  # Same as the CI `test` job's ablation step: the Hypothesis D features are
  # off by default, so their tests are absent from the run above.
  echo "  + concurrency ablation tests (features are off in the default build)"
  ABL=ablation-sharded-alloc,ablation-striped-epoch,ablation-striped-freelist
  cargo test -p expanse-trie --lib --features "$ABL" -- ablation_ 2>&1 | tee "${TMPDIR:-/tmp}/gate-ablation-unit.log"
  grep -Eq 'test result: ok\. [1-9][0-9]* passed' "${TMPDIR:-/tmp}/gate-ablation-unit.log" \
    || { echo "the ablation_ filter ran zero unit tests" >&2; exit 1; }
  cargo test -p expanse-trie --features "$ABL" --test test_concurrency_ablations
else
  step "3/6 cargo test — skipped (--quick)"
fi

step "4/6 repository consistency scripts (as in the CI lint job)"
python3 scripts/bump_version.py --check
python3 scripts/check_abi_parity.py
python3 scripts/check_ecosystem_theme.py --local-only
python3 scripts/check_ci_gate.py
python3 scripts/check_ci_filters.py
python3 scripts/check_ci_filters.py --self-test
python3 scripts/check_gate_floor.py --self-test
python3 scripts/ci_job_diff.py --self-test
python3 scripts/check_bench_suites.py
python3 scripts/check_bench_shapes.py
python3 scripts/check_bench_pin.py
python3 scripts/check_readme_tables.py
python3 scripts/check_bench_provenance.py
python3 scripts/check_man_pages.py
python3 scripts/check_deletion_rationale.py
python3 scripts/check_test_floors.py
python3 scripts/check_miri_shards.py
python3 scripts/perf_report.py --self-test
python3 tests/test_perf_report.py
python3 scripts/bench_counters.py --self-test
python3 scripts/pin_exposure.py --self-test
python3 scripts/warmup_ramp.py --self-test
python3 scripts/bench_report.py --self-test
python3 scripts/check_docs_hygiene.py --self-test
python3 scripts/check_ecosystem_theme.py --self-test
python3 scripts/check_bench_suites.py --self-test
python3 scripts/check_bench_shapes.py --self-test
python3 scripts/check_bench_pin.py --self-test
python3 scripts/check_public_api.py --self-test
python3 scripts/check_readme_tables.py --self-test

# Python lint (ruff, pyflakes rules per ruff.toml). CI is the authority: the
# `docs-lint` job installs a pinned ruff and runs the same check. Locally it is
# advisory when ruff is absent, and says so rather than passing silently.
if command -v ruff >/dev/null 2>&1; then
  ruff check .
else
  echo "  (skipping ruff: not installed -- 'pip install ruff'; CI docs-lint runs it pinned)"
fi
python3 scripts/check_bench_provenance.py --self-test
python3 scripts/check_man_pages.py --self-test
python3 scripts/check_miri_shards.py --self-test
python3 scripts/check_man_examples.py --self-test
python3 scripts/check_abi_parity.py --self-test
python3 scripts/check_deletion_rationale.py --self-test
python3 scripts/check_test_floors.py --self-test
python3 scripts/esp32_bench_harvest.py --self-test
python3 scripts/verify_release_registries.py --self-test
python3 scripts/embedded_envelope.py
python3 scripts/density_poisson.py --self-test
python3 scripts/art_envelope.py
python3 scripts/masstree_envelope.py
python3 scripts/set_algebra_bounds.py
python3 scripts/set_domain_bounds.py
python3 scripts/olc_bounds.py --self-test
python3 scripts/fit_usl.py --self-test
python3 scripts/rocksdb_locate_bound.py --self-test

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
  # Keep this list byte-identical to the one in .github/workflows/ci.yml.
  # The cursor entries are substring matches and are here because the strmap
  # cursor walks a raw *mut StrNode path stack; only tests small enough for
  # the interpreter belong in them (the 2,040-key walks run nightly).
  # occ::tests:: covers the OCC lock and epoch primitives; Miri fails weak
  # CASes spuriously, so a single-attempt try-lock must use a strong CAS.
  cargo miri test -p expanse-trie --lib -- leaf:: node:: slot:: alloc:: bits:: types:: \
    blobmap::tests::deferred strmap::tests::deferred bytesmap::tests::deferred strmap::tests::cursor_walks strmap::tests::cursor_edges strmap::tests::cursor_slots map::tests::occ_engine_single_thread_under_miri map::tests::slot_calls_on_a_warm_insert_path set::tests::occ_engine_single_thread_under_miri occ::tests::
  cargo miri test -p expanse-trie --lib --features ablation-sharded-alloc,ablation-striped-epoch,ablation-striped-freelist -- ablation_
else
  step "6/6 Miri — skipped (pass --miri for the Tier-1 filter; CI runs it on every PR)"
fi

printf '\n\033[1;32mAll local gates passed.\033[0m Callgrind / wall-clock gates run in CI and on the reference host.\n'
