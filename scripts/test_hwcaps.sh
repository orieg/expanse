#!/bin/bash
set -euo pipefail

DIST_DIR="dist"

echo "Running hwcaps validation..."

validate_so() {
    local so_path=$1
    local expected_so="libexpanse.so.1"
    
    if [ ! -f "$so_path" ]; then
        echo "Error: $so_path not found"
        exit 1
    fi
    
    # Check that symlinks exist
    dir=$(dirname "$so_path")
    if [ ! -L "$dir/libexpanse.so.1" ] || [ ! -L "$dir/libexpanse.so" ]; then
        echo "Error: Missing symlinks in $dir"
        exit 1
    fi
}

# 1. Base arch
validate_so "${DIST_DIR}/lib/libexpanse.so.1.0.0"
echo "Validated base architecture"

# 2. hwcaps
for HWCAP in x86-64-v2 x86-64-v3 x86-64-v4; do
    validate_so "${DIST_DIR}/lib/glibc-hwcaps/${HWCAP}/libexpanse.so.1.0.0"
    echo "Validated $HWCAP architecture"
done

echo "HWCAPS validation passed!"
