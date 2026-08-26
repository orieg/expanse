#!/usr/bin/env bash
# ==============================================================================
# 1-command reproduction runner for the Redis ZSET (sorted set) engine suite.
# Compares the Expanse single-trie design (composite-key ExpanseMap +
# member->score ExpanseMap) against a Redis-style span skip list + dict.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "========================================================================"
echo " Redis ZSET Engine Benchmark Suite — Expanse vs SkipList + Dict (#330)"
echo "========================================================================"

python3 "${SCRIPT_DIR}/scripts/run_all.py" "$@"

echo ""
echo " Results and charts written to:"
echo "   docs/benchmarks/redis_zset_engine/results/"
echo "========================================================================"
