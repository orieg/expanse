#!/bin/bash
set -euo pipefail

# Validate the glibc-hwcaps layout that `scripts/build_hwcaps.sh` produces and
# `scripts/package_deb.sh` / `package_rpm.sh` stage.
#
# Usage: scripts/test_hwcaps.sh [dist-dir]
#
# **This checks that the variants are distinct builds, not just present.** The
# earlier version validated file existence and symlinks only, so three copies of
# the baseline would have passed it -- and since nothing in CI ever called the
# builder, "the variants ship" could have been believed on the strength of a
# check that could not tell (#762). A variant identical to the baseline is now
# a failure.

DIST_DIR="${1:-dist}"
BASE="${DIST_DIR}/lib/libexpanse.so.1.0.0"

fail() { echo "test_hwcaps.sh: $*" >&2; exit 1; }

validate_so() {
    local dir=$1
    [ -f "${dir}/libexpanse.so.1.0.0" ] || fail "${dir}/libexpanse.so.1.0.0 not found"
    [ -L "${dir}/libexpanse.so.1" ] || fail "missing symlink ${dir}/libexpanse.so.1"
    [ -L "${dir}/libexpanse.so" ]   || fail "missing symlink ${dir}/libexpanse.so"
}

[ -f "$BASE" ] || fail "$BASE not found — the baseline must be staged before the variants"
validate_so "${DIST_DIR}/lib"
echo "validated the baseline"

base_sum=$(sha256sum "$BASE" | cut -d' ' -f1)

for LEVEL in x86-64-v2 x86-64-v3 x86-64-v4; do
    dir="${DIST_DIR}/lib/glibc-hwcaps/${LEVEL}"
    validate_so "$dir"

    # A variant byte-identical to the baseline means the target-cpu flag did not
    # reach the compiler, which is the failure this file exists to catch.
    sum=$(sha256sum "${dir}/libexpanse.so.1.0.0" | cut -d' ' -f1)
    [ "$sum" != "$base_sum" ] \
        || fail "${LEVEL} is byte-identical to the baseline — the -C target-cpu flag did not take"

    # And distinct from every variant already checked, for the same reason.
    for seen in "${seen_sums[@]:-}"; do
        [ -n "$seen" ] || continue
        [ "$sum" != "$seen" ] \
            || fail "${LEVEL} is byte-identical to another variant — the levels are not distinct builds"
    done
    seen_sums+=("$sum")

    echo "validated ${LEVEL} (distinct build)"
done

echo "hwcaps validation passed: baseline plus 3 distinct variants under ${DIST_DIR}/lib/glibc-hwcaps/"
