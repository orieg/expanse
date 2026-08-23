vcpkg_from_github(
    OUT_SOURCE_PATH SOURCE_PATH
    REPO orieg/expanse
    REF v0.3.0
    SHA512 0  # To be filled by vcpkg
    HEAD_REF main
)

# Since it's a Rust project, we can use vcpkg's rust support or just download the pre-built binaries.
# Here we'll build it if rust is available, or we could fetch the release artifact.
# Given it's a standard Rust build, we could use cargo:

vcpkg_find_acquire_program(CARGO)

message(STATUS "Building expanse in ${SOURCE_PATH}")
vcpkg_execute_build_process(
    COMMAND "${CARGO}" build --workspace --release
    WORKING_DIRECTORY "${SOURCE_PATH}"
    LOGNAME "build-${TARGET_TRIPLET}"
)

# Install headers
file(INSTALL "${SOURCE_PATH}/crates/expanse-capi/include/expanse.h" DESTINATION "${CURRENT_PACKAGES_DIR}/include")
file(INSTALL "${SOURCE_PATH}/crates/expanse-capi/include/Judy.h" DESTINATION "${CURRENT_PACKAGES_DIR}/include")

# Install libs
if(VCPKG_TARGET_IS_WINDOWS)
    file(INSTALL "${SOURCE_PATH}/target/release/expanse.dll" DESTINATION "${CURRENT_PACKAGES_DIR}/bin")
    file(INSTALL "${SOURCE_PATH}/target/release/expanse.lib" DESTINATION "${CURRENT_PACKAGES_DIR}/lib")
else()
    file(INSTALL "${SOURCE_PATH}/target/release/libexpanse.a" DESTINATION "${CURRENT_PACKAGES_DIR}/lib")
    file(INSTALL "${SOURCE_PATH}/target/release/libexpanse.so" DESTINATION "${CURRENT_PACKAGES_DIR}/lib")
endif()

# Handle copyright
file(INSTALL "${SOURCE_PATH}/LICENSE-MIT" DESTINATION "${CURRENT_PACKAGES_DIR}/share/${PORT}" RENAME copyright)
