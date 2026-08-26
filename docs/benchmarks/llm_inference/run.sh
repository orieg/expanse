#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

echo "=== Expanse LLM Inference & Speculative Decoding Benchmark Suite ==="

# 1. Ensure expanse-capi release library is compiled
if [ ! -f "$REPO_ROOT/target/release/libexpanse.dylib" ] && [ ! -f "$REPO_ROOT/target/release/libexpanse.so" ]; then
    echo "==> Building expanse-capi release library..."
    (cd "$REPO_ROOT" && cargo build --release -p expanse-capi)
fi

# 2. Build native C++ harness
echo "==> Compiling native C++ harnesses..."
make -C "$SCRIPT_DIR"

# 3. Execute master benchmark runner
echo "==> Running benchmarks..."
python3 "$SCRIPT_DIR/scripts/run_all.py" "$@"
