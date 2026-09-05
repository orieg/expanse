use std::path::{Path, PathBuf};
use std::process::Command;

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

    let rowex = std::env::var_os("CARGO_FEATURE_ROWEX").is_some();

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

    if rowex {
        // The concurrent arm (#692, METHODOLOGY.md §11). ROWEX needs TBB for
        // its per-thread reclamation state and nothing else; libtbb is built
        // from HOT's own pinned nested submodule into OUT_DIR so the competitor
        // runs the TBB its authors built against and no host is modified.
        let tbb_dir = build_tbb(&hot);
        build
            .file(manifest.join("cpp/hot_rowex_shim.cpp"))
            .include(hot.join("libs/hot/rowex/include"))
            .include(hot.join("third-party/tbb/include"));
        println!("cargo:rustc-link-search=native={}", tbb_dir.display());
        println!("cargo:rustc-link-lib=dylib=tbb");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", tbb_dir.display());
        println!("cargo:rerun-if-changed=cpp/hot_rowex_shim.cpp");
    }
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

/// Builds `libtbb.so.2` (release only; `tbbmalloc` is neither needed nor
/// buildable under C++17) from HOT's nested `third-party/tbb` submodule and
/// returns the directory holding it.
///
/// Fails loud if the nested submodule is absent or the build does not produce
/// the library: an arm that silently ran without its competitor is the failure
/// mode AGENTS.md §8.1 forbids.
fn build_tbb(hot: &Path) -> PathBuf {
    let tbb_root = hot.join("third-party/tbb");
    if !tbb_root.join("Makefile").exists() {
        panic!(
            "TBB sources are missing at {}\n\
             The `rowex` feature needs HOT's nested TBB submodule. From the repo root run:\n\
             \n    git -C third_party/hot submodule update --init --depth 1 third-party/tbb\n\n\
             No system TBB is consulted, by design (METHODOLOGY.md §11.3).",
            tbb_root.display()
        );
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("tbb");
    let prefix = "expanse";
    let lib_dir = out.join(format!("{prefix}_release"));
    if !lib_dir.join("libtbb.so.2").exists() {
        let status = Command::new("make")
            .current_dir(&tbb_root)
            .args([
                "tbb",
                "compiler=gcc",
                "stdver=c++17",
                &format!("tbb_build_dir={}", out.display()),
                &format!("tbb_build_prefix={prefix}"),
            ])
            .status()
            .expect("failed to spawn `make` for TBB");
        assert!(
            status.success() && lib_dir.join("libtbb.so.2").exists(),
            "TBB build failed (status {status}); libtbb.so.2 not found in {}",
            lib_dir.display()
        );
    }
    lib_dir
}
