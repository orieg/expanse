cmake_minimum_required(VERSION 3.15)
project(rocksdb_expanse VERSION 0.4.0 LANGUAGES CXX C)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Compiler flags
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
    add_compile_options(-Wall -Wextra -O3)
endif()

# Include directories
set(EXPANSE_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../include")
set(INTEGRATION_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/include")

include_directories(
    ${INTEGRATION_INCLUDE_DIR}
    ${EXPANSE_INCLUDE_DIR}
)

# Find or specify libexpanse
find_library(EXPANSE_LIB
    NAMES expanse libexpanse
    PATHS
        "${CMAKE_CURRENT_SOURCE_DIR}/../../target/release"
        "${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug"
        "/usr/local/lib"
        "/usr/lib"
    NO_DEFAULT_PATH
)

if(NOT EXPANSE_LIB)
    # Fallback to direct library location in target
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug/libexpanse.dylib")
        set(EXPANSE_LIB "${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug/libexpanse.dylib")
    elseif(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug/libexpanse.so")
        set(EXPANSE_LIB "${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug/libexpanse.so")
    elseif(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug/libexpanse.a")
        set(EXPANSE_LIB "${CMAKE_CURRENT_SOURCE_DIR}/../../target/debug/libexpanse.a")
    endif()
endif()

# Core static library
add_library(rocksdb_expanse STATIC
    src/expanse_memtable.cc
)
target_include_directories(rocksdb_expanse PUBLIC
    ${INTEGRATION_INCLUDE_DIR}
    ${EXPANSE_INCLUDE_DIR}
    ${EXPANSE_ROOT_INCLUDE_DIR}
)

if(EXPANSE_LIB)
    target_link_libraries(rocksdb_expanse PUBLIC ${EXPANSE_LIB} pthread)
endif()

# Unit tests executable
add_executable(test_expanse_memtable
    tests/test_expanse_memtable.cc
)
target_link_libraries(test_expanse_memtable PRIVATE rocksdb_expanse ${EXPANSE_LIB} pthread)

# Park-point tests: deterministic interleavings. The park points live in
# src/expanse_memtable.cc and exist only under EXPANSE_MEMTABLE_PARK_POINTS, so
# this binary compiles the source itself with that definition instead of
# linking rocksdb_expanse, which is built without it.
add_executable(test_memtable_park_points
    src/expanse_memtable.cc
    tests/test_memtable_park_points.cc
)
target_compile_definitions(test_memtable_park_points PRIVATE EXPANSE_MEMTABLE_PARK_POINTS)
target_link_libraries(test_memtable_park_points PRIVATE ${EXPANSE_LIB} pthread)

# Benchmark executable
add_executable(bench_memtable
    benches/bench_memtable.cc
)
target_link_libraries(bench_memtable PRIVATE rocksdb_expanse ${EXPANSE_LIB} pthread)

# CTest registration
enable_testing()
add_test(NAME test_expanse_memtable COMMAND test_expanse_memtable)
add_test(NAME test_memtable_park_points COMMAND test_memtable_park_points)
