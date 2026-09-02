/* Expanse "two lanes" LCD demo — STM32H747I-DISCO, Cortex-M7 (#605).
 *
 * Step 2: the two lanes are live. Same tracker workload on both halves of
 * the screen — 100,000 records with a TTL, ingest at a steady rate, a sweep
 * of expired records once a second, twelve tracked devices whose positions
 * a 1 kHz timer interrupt per lane reads through that lane's table and
 * draws. Left: Expanse (sync32 by-id index read by the interrupt with a
 * single-attempt reader; by-time index swept with remove_range). Right: an
 * open-addressing hash table whose reader interrupt is masked around every
 * write and around the full-scan sweep.
 *
 * One core, two interrupts, the main loop shared: while one lane's sweep
 * runs, the other lane's ingest pauses (its "records" counter stalls) but
 * its interrupt keeps drawing — unless it is the masked one.
 *
 * Board configuration (clock, MPU, UART, DMA2D, DSI/LTDC) is the prior
 * working set for this board (docs/NT35510_LCD_SETUP.md). Demo, not
 * benchmark: measured claims live in docs/benchmarks/stm32h747. */
#include "main.h"
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include "stm32h747i_discovery.h"
#include "stm32h747i_discovery_lcd.h"
#include "stm32h747i_discovery_sdram.h"
#include "gfx.h"
#include "lanes.h"

#define FRAME_BUFFER_ADDR (SDRAM_DEVICE_ADDR)              /* 0xD0000000, non-cacheable */
#define HEAP_START        (SDRAM_DEVICE_ADDR + 0x1000000U) /* 0xD1000000, 16 MB, cacheable write-back */
#define HEAP_END          (SDRAM_DEVICE_ADDR + 0x2000000U)

#define TARGET_RECORDS 100000u
#define INGEST_PER_S   (TARGET_RECORDS / (TTL_MS / 1000u))  /* steady state: in = out */
#define SWEEP_PERIOD_MS 1000u

DMA2D_HandleTypeDef hdma2d;
UART_HandleTypeDef huart1;
extern LTDC_HandleTypeDef hlcd_ltdc;
typedef enum { LCD_CTRL_NT35510, LCD_CTRL_OTM8009A } LCD_Driver_t;
extern LCD_Driver_t Lcd_Driver_Type;

void SystemClock_Config(void);
void PeriphCommonClock_Config(void);
static void MPU_Config(void);
static void MX_GPIO_Init(void);
static void MX_USART1_UART_Init(void);
static void MX_DMA2D_Init(void);
static void timers_init(void);

/* ---- heap in SDRAM (both lanes, symmetric), host hooks for libexpanse ------ */
static uint8_t *brk = (uint8_t *)HEAP_START;
void *_sbrk(ptrdiff_t incr) {
    if (brk + incr > (uint8_t *)HEAP_END) { errno = ENOMEM; return (void *)-1; }
    void *p = brk; brk += incr; return p;
}
void *expanse_host_malloc(size_t n) { return acct_malloc(n); }
void expanse_host_free(void *p) { acct_free(p); }

/* ---- screen ------------------------------------------------------------------ */
static void draw_lane_frame(const lane_t *l, const char *sub) {
    int x0 = l->x0;
    fill_rect(x0, 48, 392, 424, C_PANEL);
    text(x0 + 12, 56, l->name, l->accent, 2);
    text(x0 + 12, 90, sub, C_MUTED, 1);
    fill_rect(x0 + 12, 110, 368, 200, C_BG); rect(x0 + 12, 110, 368, 200, C_GRID);
    for (int i = 1; i < 6; i++) fill_rect(x0 + 12 + i * 368 / 6, 110, 1, 200, C_GRID);
    for (int i = 1; i < 4; i++) fill_rect(x0 + 12, 110 + i * 50, 368, 1, C_GRID);
    text(x0 + 18, 114, "tracked devices, drawn by this lane's 1 kHz interrupt", C_MUTED, 1);
    fill_rect(x0 + 12, 320, 368, 56, C_BG); rect(x0 + 12, 320, 368, 56, C_GRID);
    text(x0 + 18, 324, "interrupt heartbeat (last 3 s)", C_MUTED, 1);
    const char *labels[4] = { "records", "last sweep", "interrupt latency, max", "missed interrupts" };
    for (int r = 0; r < 4; r++) text(x0 + 12, 388 + r * 20, labels[r], C_MUTED, 1);
}

static void draw_frame(void) {
    fill_rect(0, 0, LCD_W, LCD_H, C_BG);
    text(12, 10, "EXPANSE on STM32H747 - same workload, two data structures, one core", C_TEXT, 1);
    text(12, 28, "100,000 records . TTL 60 s . sweep every second . one 1 kHz interrupt per lane reads its table and draws", C_MUTED, 1);
    draw_lane_frame(&lane_a, "ordered trie . sync32 reader in its interrupt . remove_range sweep");
    draw_lane_frame(&lane_b, "open addressing . its interrupt masked around writes . full-scan sweep");
    text(12, 466, "demo, not benchmark: the mechanism is measured in docs/benchmarks/stm32h747; this scales the table so it shows at human speed", C_MUTED, 1);
}

static void fmt_u(char *b, uint32_t v) { snprintf(b, 16, "%lu", (unsigned long)v); }
static void fmt_us(char *b, uint32_t us) {
    if (us >= 10000u) snprintf(b, 16, "%lu ms", (unsigned long)(us / 1000u));
    else if (us >= 1000u) snprintf(b, 16, "%lu.%lu ms", (unsigned long)(us / 1000u), (unsigned long)((us % 1000u) / 100u));
    else snprintf(b, 16, "%lu us", (unsigned long)us);
}

static void draw_stats(lane_t *l, uint32_t now_ms, uint32_t start_ms) {
    char b[16]; int x0 = l->x0, xr = x0 + 380;
    fill_rect(x0 + 200, 388, 180, 80, C_PANEL);
    fmt_u(b, l->count); text_right(xr, 388, b, C_TEXT, 1);
    fmt_us(b, l->sweep_us); text_right(xr, 408, b, l->sweep_us > 5000u ? C_RED : C_GREEN, 1);
    uint32_t lat_us_x10 = l->lat_max_ticks / 2u; /* ticks are 50 ns: 2 ticks = 0.1 us */
    if (lat_us_x10 >= 100000u) snprintf(b, 16, "%lu ms", (unsigned long)(lat_us_x10 / 10000u));
    else if (lat_us_x10 >= 10000u) snprintf(b, 16, "%lu.%lu ms", (unsigned long)(lat_us_x10 / 10000u), (unsigned long)((lat_us_x10 / 1000u) % 10u));
    else snprintf(b, 16, "%lu.%lu us", (unsigned long)(lat_us_x10 / 10u), (unsigned long)(lat_us_x10 % 10u));
    text_right(xr, 428, b, lat_us_x10 > 1000u ? C_RED : C_GREEN, 1);
    uint32_t elapsed = now_ms - start_ms, served = l->isr_served + 1u; /* +1: the tick in flight at start */
    uint32_t missed = elapsed > served ? elapsed - served : 0;
    fmt_u(b, missed); text_right(xr, 448, b, missed ? C_RED : C_GREEN, 1);
}

/* heartbeat: 368 columns over the last 3 s, 8 ms per column; red where the
 * lane's interrupt did not run in that window */
static void draw_heartbeat(lane_t *l, uint32_t now_ms) {
    int x0 = l->x0 + 12;
    for (int col = 0; col < 368; col++) {
        uint32_t t_end = now_ms - (uint32_t)(367 - col) * 8u;
        bool any = false;
        for (uint32_t k = 0; k < 8; k++) if (l->served_ms[(t_end - k) & 4095u]) { any = true; break; }
        uint32_t c = any ? l->accent : C_RED;
        fill_rect(x0 + col, 340, 1, 30, C_BG);
        fill_rect(x0 + col, any ? 352 : 340, 1, any ? 6 : 30, c);
    }
    /* forget what is older than the window */
    for (uint32_t k = 3000; k < 4096; k++) l->served_ms[(now_ms - k) & 4095u] = 0;
}

int main(void) {
    MPU_Config();
    SCB_EnableICache();
    SCB_EnableDCache();
    HAL_Init();
    SystemClock_Config();
    PeriphCommonClock_Config();
    MX_GPIO_Init();
    MX_USART1_UART_Init();
    MX_DMA2D_Init();
    CoreDebug->DEMCR |= CoreDebug_DEMCR_TRCENA_Msk; DWT->LAR = 0xC5ACCE55; DWT->CYCCNT = 0; DWT->CTRL |= 1;
    /* SDRAM first: the heap (and therefore printf's stdio buffer) lives there */
    if (BSP_SDRAM_Init(0) != BSP_ERROR_NONE) { for (;;) { __NOP(); } }
    printf("\r\nEXPANSE stm32h747 lcd demo (step 2: two lanes)\r\n");
    if (BSP_LCD_InitEx(0, LCD_ORIENTATION_LANDSCAPE, LCD_PIXEL_FORMAT_RGB888, LCD_W, LCD_H) != BSP_ERROR_NONE) {
        printf("[FAIL] LCD init\r\n"); Error_Handler();
    }
    gfx_init(FRAME_BUFFER_ADDR);
    fill_rect(0, 0, LCD_W, LCD_H, C_BG);
    text(12, 10, "EXPANSE on STM32H747 - loading 100,000 records into both lanes...", C_TEXT, 1);
    BSP_LCD_SetLayerAddress(0, 0, FRAME_BUFFER_ADDR);
    BSP_LCD_DisplayOn(0);
    BSP_LCD_SetBrightness(0, 100);

    uint32_t t0 = HAL_GetTick();
    if (!lanes_init(TARGET_RECORDS)) { printf("[FAIL] lanes init (heap %lu KB used)\r\n", (unsigned long)((brk - (uint8_t *)HEAP_START) / 1024u)); Error_Handler(); }
    printf("[OK] lanes: %lu records each, prefill %lu ms, heap %lu KB, requested %lu KB\r\n",
           (unsigned long)lane_a.count, (unsigned long)(HAL_GetTick() - t0),
           (unsigned long)((brk - (uint8_t *)HEAP_START) / 1024u), (unsigned long)(acct_live() / 1024u));

    draw_frame();
    lane_a.last_isr_ms = lane_b.last_isr_ms = HAL_GetTick(); /* before the first tick can fire */
    timers_init();
    HAL_Delay(2);
    lane_a.lat_max_ticks = lane_b.lat_max_ticks = 0;          /* the first tick's phase is not a latency */
    uint32_t start_ms = HAL_GetTick(), last_stats = start_ms, last_sweep = start_ms, last_log = start_ms;
    lane_a.last_sweep_ms = lane_b.last_sweep_ms = start_ms;
    for (;;) {
        uint32_t now = HAL_GetTick();
        lane_move_tracked(&lane_a, now);
        lane_move_tracked(&lane_b, now);
        lane_ingest(&lane_a, now, INGEST_PER_S);
        lane_ingest(&lane_b, now, INGEST_PER_S);
        if (now - last_sweep >= SWEEP_PERIOD_MS) {
            last_sweep = now;
            lane_sweep(&lane_a, now);
            lane_sweep(&lane_b, now);
            lane_field_border(&lane_b, C_GRID);   /* the mask is over; the beads keep the gap */
        }
        if (now - last_stats >= 100u) {
            last_stats = now;
            draw_stats(&lane_a, now, start_ms); draw_stats(&lane_b, now, start_ms);
            draw_heartbeat(&lane_a, now); draw_heartbeat(&lane_b, now);
        }
        if (now - last_log >= 1000u) {
            last_log = now;
            printf("RESULT t=%lu a_records=%lu a_sweep_us=%lu a_lat_max_ns=%lu a_served=%lu a_busy=%lu a_bad=%lu a_refused=%lu "
                   "b_records=%lu b_sweep_us=%lu b_lat_max_ns=%lu b_served=%lu b_bad=%lu\r\n",
                   (unsigned long)(now - start_ms), (unsigned long)lane_a.count, (unsigned long)lane_a.sweep_us,
                   (unsigned long)(lane_a.lat_max_ticks * 50u), (unsigned long)lane_a.isr_served, (unsigned long)lane_a.isr_busy,
                   (unsigned long)lane_a.isr_bad, (unsigned long)lane_a.refused,
                   (unsigned long)lane_b.count, (unsigned long)lane_b.sweep_us, (unsigned long)(lane_b.lat_max_ticks * 50u),
                   (unsigned long)lane_b.isr_served, (unsigned long)lane_b.isr_bad);
        }
    }
}

/* ---- the two lane timers: TIM6 -> lane A, TIM7 -> lane B, 1 kHz each ------- */
static void timers_init(void) {
    __HAL_RCC_TIM6_CLK_ENABLE(); __HAL_RCC_TIM7_CLK_ENABLE();
    /* APB1 timers run at 2 x PCLK1 = 200 MHz; TIM6/7 are 16-bit, so prescale
     * to a 20 MHz tick (50 ns) and reload every 20,000 ticks = 1 kHz */
    TIM6->PSC = 9; TIM6->ARR = 20000u - 1u; TIM6->EGR = TIM_EGR_UG; TIM6->SR = 0; TIM6->DIER = TIM_DIER_UIE;
    TIM7->PSC = 9; TIM7->ARR = 20000u - 1u; TIM7->EGR = TIM_EGR_UG; TIM7->SR = 0; TIM7->DIER = TIM_DIER_UIE;
    HAL_NVIC_SetPriority(TIM6_DAC_IRQn, 5, 0);
    HAL_NVIC_SetPriority(TIM7_IRQn, 6, 0);
    NVIC_EnableIRQ(TIM6_DAC_IRQn); NVIC_EnableIRQ(TIM7_IRQn);
    TIM6->CR1 = TIM_CR1_CEN; TIM7->CR1 = TIM_CR1_CEN;
}
void TIM6_DAC_IRQHandler(void) {
    uint32_t entry = TIM6->CNT; TIM6->SR = 0;
    lane_isr(&lane_a, entry, HAL_GetTick());
}
void TIM7_IRQHandler(void) {
    uint32_t entry = TIM7->CNT; TIM7->SR = 0;
    lane_isr(&lane_b, entry, HAL_GetTick());
}

/* ---- board configuration: prior working values, kept verbatim ------------ */
void SystemClock_Config(void)
{
  RCC_OscInitTypeDef RCC_OscInitStruct = {0};
  RCC_ClkInitTypeDef RCC_ClkInitStruct = {0};
  HAL_PWREx_ConfigSupply(PWR_DIRECT_SMPS_SUPPLY);
  __HAL_PWR_VOLTAGESCALING_CONFIG(PWR_REGULATOR_VOLTAGE_SCALE1);
  while (!__HAL_PWR_GET_FLAG(PWR_FLAG_VOSRDY)) {}
  RCC_OscInitStruct.OscillatorType = RCC_OSCILLATORTYPE_HSI | RCC_OSCILLATORTYPE_HSE;
  RCC_OscInitStruct.HSEState = RCC_HSE_ON;
  RCC_OscInitStruct.HSIState = RCC_HSI_DIV1;
  RCC_OscInitStruct.HSICalibrationValue = RCC_HSICALIBRATION_DEFAULT;
  RCC_OscInitStruct.PLL.PLLState = RCC_PLL_ON;
  RCC_OscInitStruct.PLL.PLLSource = RCC_PLLSOURCE_HSE;
  RCC_OscInitStruct.PLL.PLLM = 5;
  RCC_OscInitStruct.PLL.PLLN = 160;
  RCC_OscInitStruct.PLL.PLLP = 2;
  RCC_OscInitStruct.PLL.PLLQ = 5;
  RCC_OscInitStruct.PLL.PLLR = 2;
  RCC_OscInitStruct.PLL.PLLRGE = RCC_PLL1VCIRANGE_2;
  RCC_OscInitStruct.PLL.PLLVCOSEL = RCC_PLL1VCOWIDE;
  RCC_OscInitStruct.PLL.PLLFRACN = 0;
  if (HAL_RCC_OscConfig(&RCC_OscInitStruct) != HAL_OK) Error_Handler();
  RCC_ClkInitStruct.ClockType = RCC_CLOCKTYPE_HCLK | RCC_CLOCKTYPE_SYSCLK | RCC_CLOCKTYPE_PCLK1 | RCC_CLOCKTYPE_PCLK2 | RCC_CLOCKTYPE_D3PCLK1 | RCC_CLOCKTYPE_D1PCLK1;
  RCC_ClkInitStruct.SYSCLKSource = RCC_SYSCLKSOURCE_PLLCLK;
  RCC_ClkInitStruct.SYSCLKDivider = RCC_SYSCLK_DIV1;
  RCC_ClkInitStruct.AHBCLKDivider = RCC_HCLK_DIV2;
  RCC_ClkInitStruct.APB3CLKDivider = RCC_APB3_DIV2;
  RCC_ClkInitStruct.APB1CLKDivider = RCC_APB1_DIV2;
  RCC_ClkInitStruct.APB2CLKDivider = RCC_APB2_DIV2;
  RCC_ClkInitStruct.APB4CLKDivider = RCC_APB4_DIV2;
  if (HAL_RCC_ClockConfig(&RCC_ClkInitStruct, FLASH_LATENCY_4) != HAL_OK) Error_Handler();
  HAL_RCC_MCOConfig(RCC_MCO1, RCC_MCO1SOURCE_HSI, RCC_MCODIV_1);
}

void PeriphCommonClock_Config(void)
{
  RCC_PeriphCLKInitTypeDef PeriphClkInitStruct = {0};
  PeriphClkInitStruct.PeriphClockSelection = RCC_PERIPHCLK_ADC;
  PeriphClkInitStruct.PLL2.PLL2M = 2;
  PeriphClkInitStruct.PLL2.PLL2N = 12;
  PeriphClkInitStruct.PLL2.PLL2P = 2;
  PeriphClkInitStruct.PLL2.PLL2Q = 2;
  PeriphClkInitStruct.PLL2.PLL2R = 2;
  PeriphClkInitStruct.PLL2.PLL2RGE = RCC_PLL2VCIRANGE_3;
  PeriphClkInitStruct.PLL2.PLL2VCOSEL = RCC_PLL2VCOMEDIUM;
  PeriphClkInitStruct.PLL2.PLL2FRACN = 0;
  PeriphClkInitStruct.AdcClockSelection = RCC_ADCCLKSOURCE_PLL2;
  if (HAL_RCCEx_PeriphCLKConfig(&PeriphClkInitStruct) != HAL_OK) Error_Handler();
}

static void MX_DMA2D_Init(void)
{
  hdma2d.Instance = DMA2D;
  hdma2d.Init.Mode = DMA2D_M2M;
  hdma2d.Init.ColorMode = DMA2D_OUTPUT_ARGB8888;
  hdma2d.Init.OutputOffset = 0;
  hdma2d.LayerCfg[1].InputOffset = 0;
  hdma2d.LayerCfg[1].InputColorMode = DMA2D_INPUT_ARGB8888;
  hdma2d.LayerCfg[1].AlphaMode = DMA2D_NO_MODIF_ALPHA;
  hdma2d.LayerCfg[1].InputAlpha = 0;
  hdma2d.LayerCfg[1].AlphaInverted = DMA2D_REGULAR_ALPHA;
  hdma2d.LayerCfg[1].RedBlueSwap = DMA2D_RB_REGULAR;
  hdma2d.LayerCfg[1].ChromaSubSampling = DMA2D_NO_CSS;
  if (HAL_DMA2D_Init(&hdma2d) != HAL_OK) Error_Handler();
  if (HAL_DMA2D_ConfigLayer(&hdma2d, 1) != HAL_OK) Error_Handler();
}

static void MX_USART1_UART_Init(void)
{
  huart1.Instance = USART1;
  huart1.Init.BaudRate = 115200;
  huart1.Init.WordLength = UART_WORDLENGTH_8B;
  huart1.Init.StopBits = UART_STOPBITS_1;
  huart1.Init.Parity = UART_PARITY_NONE;
  huart1.Init.Mode = UART_MODE_TX_RX;
  huart1.Init.HwFlowCtl = UART_HWCONTROL_NONE;
  huart1.Init.OverSampling = UART_OVERSAMPLING_16;
  if (HAL_UART_Init(&huart1) != HAL_OK) Error_Handler();
}

static void MX_GPIO_Init(void)
{
  GPIO_InitTypeDef GPIO_InitStruct = {0};
  __HAL_RCC_GPIOC_CLK_ENABLE();
  __HAL_RCC_GPIOA_CLK_ENABLE();
  __HAL_RCC_GPIOH_CLK_ENABLE();
  __HAL_RCC_GPIOJ_CLK_ENABLE();
  GPIO_InitStruct.Pin = CEC_CK_MCO1_Pin;
  GPIO_InitStruct.Mode = GPIO_MODE_AF_PP;
  GPIO_InitStruct.Pull = GPIO_NOPULL;
  GPIO_InitStruct.Speed = GPIO_SPEED_FREQ_LOW;
  GPIO_InitStruct.Alternate = GPIO_AF0_MCO;
  HAL_GPIO_Init(CEC_CK_MCO1_GPIO_Port, &GPIO_InitStruct);
  GPIO_InitStruct.Pin = GPIO_PIN_2;
  GPIO_InitStruct.Mode = GPIO_MODE_AF_PP;
  GPIO_InitStruct.Pull = GPIO_NOPULL;
  GPIO_InitStruct.Speed = GPIO_SPEED_FREQ_LOW;
  GPIO_InitStruct.Alternate = GPIO_AF13_DSI;
  HAL_GPIO_Init(GPIOJ, &GPIO_InitStruct);
}

/* MPU: region 0 = default no-access map; region 1 = SDRAM 32 MB
 * non-cacheable (framebuffer, scanned by LTDC); region 2 = the top 16 MB of
 * SDRAM cacheable write-back — the heap both lanes share, so the data
 * structures run through the M7 D-cache symmetrically. */
void MPU_Config(void)
{
  MPU_Region_InitTypeDef MPU_InitStruct = {0};
  HAL_MPU_Disable();
  MPU_InitStruct.Enable = MPU_REGION_ENABLE;
  MPU_InitStruct.Number = MPU_REGION_NUMBER0;
  MPU_InitStruct.BaseAddress = 0x0;
  MPU_InitStruct.Size = MPU_REGION_SIZE_4GB;
  MPU_InitStruct.SubRegionDisable = 0x87;
  MPU_InitStruct.AccessPermission = MPU_REGION_NO_ACCESS;
  MPU_InitStruct.DisableExec = MPU_INSTRUCTION_ACCESS_DISABLE;
  MPU_InitStruct.IsShareable = MPU_ACCESS_SHAREABLE;
  MPU_InitStruct.IsCacheable = MPU_ACCESS_NOT_CACHEABLE;
  MPU_InitStruct.IsBufferable = MPU_ACCESS_NOT_BUFFERABLE;
  HAL_MPU_ConfigRegion(&MPU_InitStruct);
  MPU_InitStruct.Number = MPU_REGION_NUMBER1;
  MPU_InitStruct.BaseAddress = 0xD0000000;
  MPU_InitStruct.Size = MPU_REGION_SIZE_32MB;
  MPU_InitStruct.SubRegionDisable = 0x0;
  MPU_InitStruct.TypeExtField = MPU_TEX_LEVEL0;
  MPU_InitStruct.AccessPermission = MPU_REGION_FULL_ACCESS;
  MPU_InitStruct.DisableExec = MPU_INSTRUCTION_ACCESS_ENABLE;
  MPU_InitStruct.IsShareable = MPU_ACCESS_NOT_SHAREABLE;
  MPU_InitStruct.IsCacheable = MPU_ACCESS_NOT_CACHEABLE;
  MPU_InitStruct.IsBufferable = MPU_ACCESS_NOT_BUFFERABLE;
  HAL_MPU_ConfigRegion(&MPU_InitStruct);
  MPU_InitStruct.Number = MPU_REGION_NUMBER2;
  MPU_InitStruct.BaseAddress = HEAP_START;
  MPU_InitStruct.Size = MPU_REGION_SIZE_16MB;
  MPU_InitStruct.TypeExtField = MPU_TEX_LEVEL1;
  MPU_InitStruct.IsCacheable = MPU_ACCESS_CACHEABLE;
  MPU_InitStruct.IsBufferable = MPU_ACCESS_BUFFERABLE;
  HAL_MPU_ConfigRegion(&MPU_InitStruct);
  HAL_MPU_Enable(MPU_PRIVILEGED_DEFAULT);
}

void HAL_LTDC_LineEventCallback(LTDC_HandleTypeDef *hltdc) { (void)hltdc; }

void Error_Handler(void)
{
  printf("[CRITICAL] Error Handler Triggered! Halting.\r\n");
  __disable_irq();
  while (1) {}
}

int __io_putchar(int ch)
{
  HAL_UART_Transmit(&huart1, (uint8_t *)&ch, 1, 0xFFFF);
  return ch;
}

#ifdef USE_FULL_ASSERT
void assert_failed(uint8_t *file, uint32_t line) { (void)file; (void)line; }
#endif
