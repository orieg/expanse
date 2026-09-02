# ESP-IDF application wrapping the `expanse` component's on-device benchmark
# runner (components/expanse/test/bench_esp32_memtable.c) and its Unity test
# cases, for the hardware harvest tracked in #579.
#
#   . $IDF_PATH/export.sh
#   idf.py -C integrations/esp32 set-target esp32c3     # or esp32c6 / esp32h2 / esp32p4
#   idf.py -C integrations/esp32 build flash monitor
#
# The component's CMakeLists builds libexpanse.a with cargo for the bare-metal
# RISC-V target matching IDF_TARGET; Xtensa parts fail at configure by design.
cmake_minimum_required(VERSION 3.16)

set(EXTRA_COMPONENT_DIRS "${CMAKE_CURRENT_LIST_DIR}/../../components")
include($ENV{IDF_PATH}/tools/cmake/project.cmake)
project(expanse_esp32_bench)
