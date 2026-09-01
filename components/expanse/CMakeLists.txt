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
    INCLUDE_DIRS
        "include"
        "${CMAKE_CURRENT_LIST_DIR}/../../crates/expanse-capi/include"
    PRIV_REQUIRES
        esp_system
        heap
)

set(EXPANSE_REPO_ROOT "${CMAKE_CURRENT_LIST_DIR}/../..")

if(IDF_TARGET STREQUAL "esp32c2" OR IDF_TARGET STREQUAL "esp32c3")
    set(EXPANSE_RUST_TARGET "riscv32imc-unknown-none-elf")
elseif(IDF_TARGET STREQUAL "esp32c6" OR IDF_TARGET STREQUAL "esp32h2" OR IDF_TARGET STREQUAL "esp32p4")
    set(EXPANSE_RUST_TARGET "riscv32imac-unknown-none-elf")
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
add_dependencies(${COMPONENT_LIB} expanse_staticlib)

# INTERFACE, not PUBLIC: the archive resolves against the component's own
# expanse_host_malloc/-_free, so the two must stay in the same link group.
target_link_libraries(${COMPONENT_LIB} INTERFACE "${EXPANSE_STATICLIB}")
