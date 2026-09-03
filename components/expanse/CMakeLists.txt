# CMakeLists.txt for ESP-IDF component `expanse`
#
# Builds the bare-metal libexpanse staticlib with cargo and links it into the
# component, alongside the ESP-IDF heap-caps glue that backs its allocator.
#
# Target choice: the Espressif RISC-V parts are built against the mainline
# bare-metal targets (riscv32imc / riscv32imac -unknown-none-elf), NOT
# riscv32imc-esp-espidf. libexpanse is `no_std` and makes no libc calls — it
# reaches memory only through expanse_host_malloc/-_free, defined in
# src/expanse_esp_idf.c — so the bare-metal ABI (ilp32, soft float) matches
# what the ESP-IDF RISC-V toolchain emits, and the whole thing builds on
# stable rustc with no esp-rs fork.
#
# The Xtensa parts (ESP32, ESP32-S2, ESP32-S3) have no mainline rustc target.
# They are supported through the esp-rs rustc fork installed by espup, which
# adds the `esp` rustup toolchain and the xtensa-*-none-elf targets. Those
# targets are tier 3 and ship no precompiled core/alloc, so the Xtensa arm
# additionally builds the sysroot from source (-Z build-std). Everything below
# the target selection -- the archive, the host-allocator glue, the link order
# -- is identical across both ISAs. If the `esp` toolchain is absent the
# configure fails loudly with the install command rather than registering a
# component with no engine behind it.

idf_component_register(
    SRCS
        "src/expanse_esp_idf.c"
        "src/expanse_memtable.c"
        "src/expanse_ble_tracker.c"
    INCLUDE_DIRS
        "include"
        "${CMAKE_CURRENT_LIST_DIR}/../../include"
    PRIV_REQUIRES
        esp_system
        heap
)

set(EXPANSE_REPO_ROOT "${CMAKE_CURRENT_LIST_DIR}/../..")

# Selecting the Rust target also selects how it is built. The RISC-V targets
# are tier 2: stable rustc ships their core/alloc, so a plain `cargo build`
# on the default toolchain is enough. The Xtensa targets exist only in the
# esp-rs fork and are tier 3, so they need the `esp` toolchain and a
# from-source sysroot. EXPANSE_CARGO_TOOLCHAIN and EXPANSE_CARGO_EXTRA_ARGS
# carry that difference into the one cargo invocation below; both are unset
# (an empty list, not an empty string) on the RISC-V arms so they expand to
# nothing under VERBATIM and the RISC-V command line is byte-identical to
# what it was before Xtensa support existed.
set(EXPANSE_CARGO_TOOLCHAIN)
set(EXPANSE_CARGO_EXTRA_ARGS)

if(IDF_TARGET STREQUAL "esp32c2" OR IDF_TARGET STREQUAL "esp32c3")
    set(EXPANSE_RUST_TARGET "riscv32imc-unknown-none-elf")
elseif(IDF_TARGET STREQUAL "esp32c6" OR IDF_TARGET STREQUAL "esp32h2")
    set(EXPANSE_RUST_TARGET "riscv32imac-unknown-none-elf")
elseif(IDF_TARGET STREQUAL "esp32p4")
    # The P4 is RV32IMAFC (docs/HARDWARE.md §4.3, TRM v0.7 §2.1) and ESP-IDF
    # compiles it hard-float: components/soc/project_include.cmake sets
    # -march=rv32imafc (rv32imafcb on newer silicon revisions — a superset
    # that still runs imafc code) and -mabi=ilp32f when the CPU has an FPU.
    # A soft-float riscv32imac archive carries EF_RISCV_FLOAT_ABI_SOFT and
    # the RISC-V linker refuses to mix float ABIs, so the target must be the
    # ilp32f one (#581).
    set(EXPANSE_RUST_TARGET "riscv32imafc-unknown-none-elf")
elseif(IDF_TARGET STREQUAL "esp32" OR IDF_TARGET STREQUAL "esp32s2"
       OR IDF_TARGET STREQUAL "esp32s3")
    # Xtensa. One fork target per part -- they differ in ISA options (the S3
    # adds the PIE vector unit, the S2 drops the FPU), so the target triple
    # is not interchangeable between them the way the RISC-V ones are within
    # an ISA string.
    set(EXPANSE_RUST_TARGET "xtensa-${IDF_TARGET}-none-elf")
    set(EXPANSE_CARGO_TOOLCHAIN "+esp")
    # Tier 3: no precompiled sysroot is published, so core and alloc are
    # compiled from source alongside the crate. This needs the rust-src
    # component, which espup installs with the toolchain.
    set(EXPANSE_CARGO_EXTRA_ARGS "-Z" "build-std=core,alloc")

    # Fail at configure with the fix, not 200 lines into a cargo error.
    execute_process(
        COMMAND rustup run esp rustc --version
        RESULT_VARIABLE EXPANSE_ESP_TOOLCHAIN_RC
        OUTPUT_QUIET ERROR_QUIET)
    if(NOT EXPANSE_ESP_TOOLCHAIN_RC EQUAL 0)
        message(FATAL_ERROR
            "expanse: IDF_TARGET '${IDF_TARGET}' is an Xtensa part and needs the "
            "esp-rs rustc fork, but the `esp` rustup toolchain was not found. "
            "Install it with:\n"
            "    cargo install espup --locked\n"
            "    espup install --targets ${IDF_TARGET}\n"
            "See integrations/esp32/README.md.")
    endif()
else()
    message(FATAL_ERROR
        "expanse: IDF_TARGET '${IDF_TARGET}' is not supported. Supported: "
        "esp32c2, esp32c3, esp32c6, esp32h2, esp32p4 (mainline rustc, RISC-V) "
        "and esp32, esp32s2, esp32s3 (esp-rs rustc fork, Xtensa). "
        "See docs/COMPAT.md.")
endif()

# Debug builds of the trie are far too slow for a microcontroller and the
# staticlib is much larger, so always take the release profile.
set(EXPANSE_STATICLIB
    "${EXPANSE_REPO_ROOT}/target/${EXPANSE_RUST_TARGET}/release/libexpanse.a")

# Every Rust source and manifest the archive is built from. Without these,
# CMake sees the archive file already exists, treats it as up to date, and
# never re-invokes cargo -- so a build after an engine change relinks the
# *previous* libexpanse.a and reports success. That is silent, and on the
# benchmark application it means measuring an engine that is not the one
# checked out (AGENTS.md 8.1: fail loud, never a green step that did nothing).
# Cargo still does the real staleness decision; this list only guarantees it
# is asked. CONFIGURE_DEPENDS re-globs when a source is added or removed.
file(GLOB_RECURSE EXPANSE_RUST_SOURCES CONFIGURE_DEPENDS
    "${EXPANSE_REPO_ROOT}/crates/expanse/src/*.rs"
    "${EXPANSE_REPO_ROOT}/crates/expanse-capi/src/*.rs")
list(APPEND EXPANSE_RUST_SOURCES
    "${EXPANSE_REPO_ROOT}/Cargo.toml"
    "${EXPANSE_REPO_ROOT}/Cargo.lock"
    "${EXPANSE_REPO_ROOT}/crates/expanse/Cargo.toml"
    "${EXPANSE_REPO_ROOT}/crates/expanse-capi/Cargo.toml")

add_custom_command(
    OUTPUT "${EXPANSE_STATICLIB}"
    COMMAND cargo ${EXPANSE_CARGO_TOOLCHAIN} build --release
            -p expanse-capi
            --no-default-features
            --features embedded-panic-handler
            --target "${EXPANSE_RUST_TARGET}"
            ${EXPANSE_CARGO_EXTRA_ARGS}
    DEPENDS ${EXPANSE_RUST_SOURCES}
    WORKING_DIRECTORY "${EXPANSE_REPO_ROOT}"
    COMMENT "Building libexpanse.a for ${EXPANSE_RUST_TARGET}"
    VERBATIM
)

add_custom_target(expanse_staticlib DEPENDS "${EXPANSE_STATICLIB}")

# The archive imports expanse_host_malloc/-_free from this component's own
# objects, and ESP-IDF does not wrap the final link in --start-group, so the
# order on the link line has to come from CMake's dependency graph: the
# engine is an imported library that links back to the component. The two
# form a static-library cycle, which CMake resolves by repeating them, and
# the linker then sees the host pair after the archive that needs it. A bare
# path in INTERFACE_LINK_LIBRARIES (the previous form) landed after every
# component and left both symbols undefined (#579, first on-device build).
add_prebuilt_library(expanse_engine "${EXPANSE_STATICLIB}")
add_dependencies(expanse_engine expanse_staticlib)
target_link_libraries(expanse_engine INTERFACE ${COMPONENT_LIB})
target_link_libraries(${COMPONENT_LIB} PUBLIC expanse_engine)
