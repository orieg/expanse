#!/bin/bash
set -euo pipefail

# Build Multi-Architecture Dynamic Libraries for Expanse
# utilizing glibc-hwcaps (x86-64-v2, x86-64-v3, x86-64-v4)

# Create dist directories
mkdir -p dist/lib/glibc-hwcaps/x86-64-v2
mkdir -p dist/lib/glibc-hwcaps/x86-64-v3
mkdir -p dist/lib/glibc-hwcaps/x86-64-v4
mkdir -p dist/include

# Function to build and copy
build_target() {
    local target_name=$1
    local dest_dir=$2
    local rust_flags=$3

    echo "Building for $target_name..."
    cargo clean -p expanse-capi
    RUSTFLAGS="$rust_flags" cargo build --release --manifest-path crates/expanse-capi/Cargo.toml

    # Copy the generated shared library
    if [ -f target/release/libexpanse.so ]; then
        cp target/release/libexpanse.so "$dest_dir/libexpanse.so.1.0.0"
    elif [ -f target/release/libexpanse.dylib ]; then
        cp target/release/libexpanse.dylib "$dest_dir/libexpanse.so.1.0.0"
    else
        echo "Error: libexpanse.so/.dylib not found in target/release/"
        exit 1
    fi

    # Create symlinks
    cd "$dest_dir"
    ln -sf libexpanse.so.1.0.0 libexpanse.so.1
    ln -sf libexpanse.so.1 libexpanse.so
    ln -sf libexpanse.so.1 libJudy.so.1
    cd - >/dev/null
}

# x86-64-v1 (baseline)
build_target "x86-64-v1" "dist/lib" "-C target-cpu=x86-64"

# Also copy static library from baseline build
if [ -f target/release/libexpanse.a ]; then
    cp target/release/libexpanse.a dist/lib/
fi

# x86-64-v2 (+popcnt, +sse4.2, +ssse3)
build_target "x86-64-v2" "dist/lib/glibc-hwcaps/x86-64-v2" "-C target-cpu=x86-64-v2"

# x86-64-v3 (+avx2, +bmi2, +popcnt, +lzcnt)
build_target "x86-64-v3" "dist/lib/glibc-hwcaps/x86-64-v3" "-C target-cpu=x86-64-v3"

# x86-64-v4 (+avx512f, +avx512bw, +avx512cd, +avx512dq, +avx512vl)
build_target "x86-64-v4" "dist/lib/glibc-hwcaps/x86-64-v4" "-C target-cpu=x86-64-v4"

echo "Multi-architecture build completed successfully!"
