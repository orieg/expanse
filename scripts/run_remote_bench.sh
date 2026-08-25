#!/usr/bin/env bash
set -euo pipefail

# Standalone CLI script to run remote bare-metal Callgrind benchmarks
# Requires: BENCH_HOST and BENCH_REPO to be set in the environment.

if [ -z "${BENCH_HOST:-}" ]; then
  echo "Error: BENCH_HOST is not set." >&2
  exit 1
fi

if [ -z "${BENCH_REPO:-}" ]; then
  echo "Error: BENCH_REPO is not set." >&2
  exit 1
fi

BENCHMARK_SUITE=${1:-"all"}

echo "Syncing workspace to $BENCH_HOST:$BENCH_REPO..."
rsync -az --exclude 'target' --exclude '.git' ./ "$BENCH_HOST:$BENCH_REPO/"

echo "Running benchmarks on remote host..."
ssh "$BENCH_HOST" "
  export PATH=\$HOME/.cargo/bin:\$HOME/.local/bin:\$PATH;
  export LD_LIBRARY_PATH=\$HOME/.local/lib:\$LD_LIBRARY_PATH;
  export LIBRARY_PATH=\$HOME/.local/lib:\$LIBRARY_PATH;
  export C_INCLUDE_PATH=\$HOME/.local/include:\$C_INCLUDE_PATH;
  cd \"$BENCH_REPO\" && \
  cargo test --workspace && \
  cargo build --release -p expanse-capi && \
  export EXPANSE_CDYLIB=\$PWD/target/release/libexpanse.so && \
  if [ \"$BENCHMARK_SUITE\" = \"all\" ] || [ \"$BENCHMARK_SUITE\" = \"vs_stock\" ]; then
    cargo bench --bench vs_stock -p expanse-capi
  fi && \
  if [ \"$BENCHMARK_SUITE\" = \"all\" ] || [ \"$BENCHMARK_SUITE\" = \"instructions\" ]; then
    cargo bench --bench instructions -p expanse-trie
  fi && \
  if [ \"$BENCHMARK_SUITE\" = \"all\" ] || [ \"$BENCHMARK_SUITE\" = \"comparative\" ]; then
    cargo bench --bench comparative -p expanse-trie
  fi
"

echo "Remote benchmark execution completed."
