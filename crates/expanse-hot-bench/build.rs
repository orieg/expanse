use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let hot = manifest.join("../../third_party/hot");

    // Fail loud, never silently degrade to "no HOT arm" (AGENTS.md §8.1).
    let probe =
        hot.join("libs/hot/single-threaded/include/hot/singlethreaded/HOTSingleThreaded.hpp");
    if !probe.exists() {
        panic!(
            "HOT sources are missing at {}\n\
             The submodule is not initialised. Run:\n\
             \n    git submodule update --init --depth 1 third_party/hot\n\n\
             This crate has no fallback path and must not build without HOT.",
            hot.display()
        );
    }

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .file(manifest.join("cpp/hot_shim.cpp"))
        .include(hot.join("libs/hot/single-threaded/include"))
        .include(hot.join("libs/hot/commons/include"))
        .include(hot.join("libs/idx/content-helpers/include"))
        .opt_level(3)
        .define("NDEBUG", None)
        // HOT requires AVX2 and BMI2 (Haswell and newer); its authors specify
        // this flag. §3.3 of the suite methodology binds the Expanse side to the
        // same ISA target, set in the runner as `-C target-cpu=haswell`, so no
        // published cell compares an AVX2 C++ arm against a baseline Rust arm.
        .flag("-march=haswell")
        // Load-bearing, not cosmetic: the compiler knows the allocator family as
        // builtins and may assume they do not touch globals, which lets it cache
        // the census counters across a call. That defect was observed at the
        // Step 0 gate, where `free` ran while the byte total did not move.
        .flag("-fno-builtin-malloc")
        .flag("-fno-builtin-calloc")
        .flag("-fno-builtin-realloc")
        .flag("-fno-builtin-free")
        .warnings(false);
    build.compile("hot_shim");

    // Interpose the C allocator family for the whole binary. Rust's allocator
    // bottoms out in these same symbols, so one instrument measures the HOT arm
    // and the Expanse arm under one definition (§8.3).
    for sym in [
        "malloc",
        "calloc",
        "realloc",
        "free",
        "posix_memalign",
        "aligned_alloc",
    ] {
        println!("cargo:rustc-link-arg=-Wl,--wrap={sym}");
    }

    println!("cargo:rustc-link-lib=stdc++");
    println!("cargo:rerun-if-changed=cpp/hot_shim.cpp");
}
