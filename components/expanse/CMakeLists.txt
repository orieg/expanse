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
# The Xtensa parts (ESP32, ESP32-S2, ESP32-S3) have no mainline rustc target
# and are NOT supported; the configure below fails loudly rather than
# registering a component with no engine behind it.

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
else()
    message(FATAL_ERROR
        "expanse: IDF_TARGET '${IDF_TARGET}' is not supported. The Xtensa parts "
        "(esp32, esp32s2, esp32s3) have no mainline rustc target, so no "
        "libexpanse can be built for them. Supported: esp32c2, esp32c3, "
        "esp32c6, esp32h2, esp32p4. See docs/COMPAT.md.")
endif()

# Debug builds of the trie are far too slow for a microcontroller and the
# staticlib is much larger, so always take the release profile.
set(EXPANSE_STATICLIB
    "${EXPANSE_REPO_ROOT}/target/${EXPANSE_RUST_TARGET}/release/libexpanse.a")

add_custom_command(
    OUTPUT "${EXPANSE_STATICLIB}"
    COMMAND cargo build --release
            -p expanse-capi
            --no-default-features
            --features embedded-panic-handler
            --target "${EXPANSE_RUST_TARGET}"
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
