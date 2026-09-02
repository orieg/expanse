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
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "unity.h"
#include "expanse.h"

void app_main_benchmarks(void);

/* Every case in components/expanse/test/test_expanse.c carries this tag. The
 * count is asserted below rather than assumed: the cases register through
 * `__attribute__((constructor))` helpers that nothing else references, so if
 * the main component is ever linked without WHOLE_ARCHIVE the linker drops
 * the whole archive member and Unity reports "0 Tests ... OK" -- a green gate
 * that ran nothing. That is how the first ESP32 harvest passed its test gate
 * while the engine went entirely unexercised (#579). */
#define EXPANSE_EXPECTED_TEST_CASES 13

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
    unsigned ran = (unsigned)Unity.NumberOfTests;
    int failures = UNITY_END();
    if (failures) {
        printf("[FAIL] %d unity case(s) failed; not benchmarking a library that does not pass its tests\n", failures);
        return;
    }
    if (ran != EXPANSE_EXPECTED_TEST_CASES) {
        printf("[FAIL] expected %d unity case(s) tagged [expanse], ran %u; the test gate did not "
               "exercise the engine, so no number from this boot is trustworthy\n",
               EXPANSE_EXPECTED_TEST_CASES, ran);
        return;
    }
    app_main_benchmarks();

    /* Smallest free stack the main task reached across the whole suite. The
     * descent through the trie is the deepest call chain here, and on Xtensa
     * the windowed-ABI register spills make each frame markedly larger than
     * the RISC-V equivalent -- the ESP-IDF default of 3584 bytes overflows
     * this task into the adjacent heap (#579). Published so the requirement
     * is a measured number rather than a guess, and so a regression shows up
     * as a shrinking margin instead of a corrupted heap. */
    printf("{\"stack_min_free_bytes\": %u, \"stack_total_bytes\": %u}\n",
           (unsigned)uxTaskGetStackHighWaterMark(NULL),
           (unsigned)CONFIG_ESP_MAIN_TASK_STACK_SIZE);
    printf("EXPANSE esp32 harvest done\n");
}
