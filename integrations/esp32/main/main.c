/* app_main for the on-device Expanse harvest (#579): print the target facts
 * the harvest needs for provenance, run the Unity cases from the component
 * (they must pass before any number is worth harvesting), then the
 * benchmark runner, which emits one JSON object per line for
 * scripts/esp32_bench_harvest.py. Everything goes to the default console. */
#include <stdio.h>
#include <string.h>
#include "esp_chip_info.h"
#include "esp_clk_tree.h"
#include "esp_idf_version.h"
#include "esp_heap_caps.h"
#include "esp_system.h"
#include "soc/clk_tree_defs.h"
#include "unity.h"
#include "expanse.h"

void app_main_benchmarks(void);

static const char *model_name(esp_chip_model_t m) {
    switch (m) {
        case CHIP_ESP32: return "esp32";
        case CHIP_ESP32S2: return "esp32s2";
        case CHIP_ESP32S3: return "esp32s3";
        case CHIP_ESP32C3: return "esp32c3";
        case CHIP_ESP32C2: return "esp32c2";
        case CHIP_ESP32C6: return "esp32c6";
        case CHIP_ESP32H2: return "esp32h2";
        case CHIP_ESP32P4: return "esp32p4";
        default: return "unknown";
    }
}

void app_main(void) {
    esp_chip_info_t ci;
    esp_chip_info(&ci);
    uint32_t cpu_hz = 0;
    esp_clk_tree_src_get_freq_hz(SOC_MOD_CLK_CPU, ESP_CLK_TREE_SRC_FREQ_PRECISION_APPROX, &cpu_hz);
    printf("\nEXPANSE esp32 harvest\n");
    printf("{\"target\": \"%s\", \"revision\": %u, \"cores\": %u, \"cpu_hz\": %lu, \"idf\": \"%s\", \"expanse\": \"%s\", "
           "\"free_internal\": %u, \"largest_internal\": %u}\n",
           model_name(ci.model), (unsigned)ci.revision, (unsigned)ci.cores, (unsigned long)cpu_hz, esp_get_idf_version(),
           expanse_version(), (unsigned)heap_caps_get_free_size(MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT),
           (unsigned)heap_caps_get_largest_free_block(MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT));

    UNITY_BEGIN();
    unity_run_tests_by_tag("[expanse]", false);
    int failures = UNITY_END();
    if (failures) {
        printf("[FAIL] %d unity case(s) failed; not benchmarking a library that does not pass its tests\n", failures);
        return;
    }
    app_main_benchmarks();
    printf("EXPANSE esp32 harvest done\n");
}
