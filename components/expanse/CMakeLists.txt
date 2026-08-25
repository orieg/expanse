# CMakeLists.txt for ESP-IDF component `expanse`
#
# Registers Expanse C API headers and ESP-IDF SRAM capability integration.

idf_component_register(
    SRCS
        "src/expanse_esp_idf.c"
    INCLUDE_DIRS
        "include"
    PRIV_REQUIRES
        esp_system
        heap
)
