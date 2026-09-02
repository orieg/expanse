/* Expanse "two lanes" LCD demo — STM32H747I-DISCO, Cortex-M7 (#605).
 *
 * Step 3 (after the review recorded in #605): each lane's interrupt owns the
 * motion it draws, the evidence of a masked interrupt stays on screen as a
 * red ribbon segment, the hash lane's masking mode is cycled with the user
 * button (whole-scan / per-write / unmasked), tracked devices are
 * re-registered under the writer so an unmasked read can genuinely miss,
 * and the instruments measure what they claim: a free-running 20 MHz
 * reference for entry latency and missed ticks, DWT for ISR body cycles,
 * sweep time gross and net of interrupt time, and the mask window itself.
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

#define FRAME_BUFFER_ADDR (SDRAM_DEVICE_ADDR)              /* 0xD0000000, normal non-cacheable */
#define HEAP_START        (SDRAM_DEVICE_ADDR + 0x1000000U) /* 0xD1000000, 16 MB, cacheable write-back */
#define HEAP_END          (SDRAM_DEVICE_ADDR + 0x2000000U)

#define TARGET_RECORDS 100000u
#define INGEST_PER_S   (TARGET_RECORDS / (TTL_MS / 1000u))


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
static const char *mode_name(uint32_t m) {
    return m == HASH_MASK_WHOLE ? "reader masked: whole scan + writes" :
           m == HASH_MASK_PER_WRITE ? "reader masked: each write only" : "reader NOT masked";
}
static void draw_lane_frame(lane_t *l) {
    int x0 = l->x0;
    fill_rect(x0, 44, 392, 428, C_PANEL);
    text(x0 + 12, 50, l->name, l->accent, 3);
    fill_rect(x0 + 12, 86, 368, 16, C_PANEL);
    text(x0 + 12, 86, l->is_expanse ? "reader never masked: BUSY instead" : mode_name(l->mode), C_MUTED, 1);
    fill_rect(x0 + 12, FIELD_Y, FIELD_W, FIELD_H, C_BG); rect(x0 + 12, FIELD_Y, FIELD_W, FIELD_H, C_GRID);
    for (int i = 1; i < N_TRACKED; i++) fill_rect(x0 + 12, FIELD_Y + i * ROW_H, FIELD_W, 1, C_GRID);
    fill_rect(x0 + 12, 312, 368, 40, C_BG); rect(x0 + 12, 312, 368, 40, C_GRID);
    text(x0 + 18, 314, "interrupt heartbeat, last 3 s", C_MUTED, 1);
    text(x0 + 12, 360, "missed interrupt ticks", C_MUTED, 1);
    text(x0 + 12, 430, "worst wait", C_MUTED, 1);
    for (int t = 0; t < N_TRACKED; t++) { l->last_x[t] = -1; l->row_dirty[t] = 0; }
}

/* the button steps through this program: the sweep-rate amplifier first
 * (both lanes sweep more often; the hash lane's whole-scan cost is paid each
 * time, Expanse's range removal only pays for what expired), then the two
 * other masking disciplines at the highest rate */
typedef struct { uint32_t sweep_hz, mode; } step_t;
static const step_t program[] = { {1, HASH_MASK_WHOLE}, {5, HASH_MASK_WHOLE}, {10, HASH_MASK_WHOLE},
                                  {10, HASH_MASK_PER_WRITE}, {10, HASH_UNMASKED} };
#define N_STEPS (sizeof program / sizeof program[0])

static void draw_title(uint32_t sweep_hz) {
    char b[48];
    fill_rect(0, 0, LCD_W, 44, C_BG);
    snprintf(b, sizeof b, "One core, two interrupts, %lu sweep%s/sec", (unsigned long)sweep_hz, sweep_hz == 1 ? "" : "s");
    text(12, 8, b, C_TEXT, 2);
    text(12, 30, "each lane's 1 kHz interrupt moves its own dots; a red mark = the interrupt was held off", C_MUTED, 1);
}
static void draw_frame(uint32_t sweep_hz) {
    fill_rect(0, 0, LCD_W, LCD_H, C_BG);
    draw_title(sweep_hz);
    draw_lane_frame(&lane_a);
    draw_lane_frame(&lane_b);
    text(12, 466, "demo, not benchmark. 100k records. blue button: more sweeps, then other masking", C_MUTED, 1);
}

static void fmt_ticks(char *b, size_t n, uint32_t ticks) {   /* 50 ns ticks -> us/ms */
    uint32_t us10 = ticks / 2u;
    if (us10 >= 100000u) snprintf(b, n, "%lu ms", (unsigned long)(us10 / 10000u));
    else if (us10 >= 10000u) snprintf(b, n, "%lu.%lu ms", (unsigned long)(us10 / 10000u), (unsigned long)((us10 / 1000u) % 10u));
    else snprintf(b, n, "%lu.%lu us", (unsigned long)(us10 / 10u), (unsigned long)(us10 % 10u));
}

static void draw_stats(lane_t *l) {
    char b[64]; int x0 = l->x0, xr = x0 + 380;
    fill_rect(x0 + 12, 372, 368, 52, C_PANEL);
    snprintf(b, sizeof b, "%lu", (unsigned long)l->missed_ticks);
    text_right(xr, 372, b, l->missed_ticks ? C_RED : C_GREEN, 4);
    fill_rect(x0 + 140, 426, 240, 40, C_PANEL);
    fmt_ticks(b, sizeof b, l->lat_max_ticks);
    text_right(xr, 426, b, l->lat_max_ticks > 2 * TICKS_PER_MS ? C_RED : C_GREEN, 2);
    snprintf(b, sizeof b, "sweep %lu ms  busy %lu  miss %lu  bad %lu", (unsigned long)(l->sweep_net_us / 1000u),
             (unsigned long)l->isr_busy, (unsigned long)l->isr_nf, (unsigned long)l->isr_bad);
    fill_rect(x0 + 12, 450, 368, 16, C_PANEL);
    text(x0 + 12, 450, b, C_MUTED, 1);
}

/* heartbeat: a ring of 368 columns, 8 ms each, cursor sweeping left to right;
 * accent where the lane's interrupt ran in that window, red bar where not */
static void draw_heartbeat(lane_t *l, uint32_t now_ms, uint32_t from_ms) {
    int x0 = l->x0 + 12;
    for (uint32_t t = from_ms; t < now_ms; t += 8u) {
        int col = (int)((t / 8u) % 368u);
        bool any = false;
        for (uint32_t k = 0; k < 8; k++) if (l->served_ms[(t + k) & 4095u]) { any = true; break; }
        fill_rect(x0 + col, 328, 1, 22, C_BG);
        fill_rect(x0 + col, any ? 337 : 328, 1, any ? 4 : 22, any ? l->accent : C_RED);
        fill_rect(x0 + (col + 1) % 368, 328, 1, 22, C_TEXT);   /* cursor */
        for (uint32_t k = 0; k < 8; k++) l->served_ms[(t + k + 3000u) & 4095u] = 0; /* forget the window ahead */
    }
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
    if (BSP_SDRAM_Init(0) != BSP_ERROR_NONE) { for (;;) { __NOP(); } }   /* heap (and printf) live there */
    printf("\r\nEXPANSE stm32h747 lcd demo (step 3: interrupt-owned motion, masking modes)\r\n");
    if (BSP_LCD_InitEx(0, LCD_ORIENTATION_LANDSCAPE, LCD_PIXEL_FORMAT_RGB888, LCD_W, LCD_H) != BSP_ERROR_NONE) {
        printf("[FAIL] LCD init\r\n"); Error_Handler();
    }
    BSP_PB_Init(BUTTON_WAKEUP, BUTTON_MODE_GPIO);
    gfx_init(FRAME_BUFFER_ADDR);
    fill_rect(0, 0, LCD_W, LCD_H, C_BG);
    text(12, 10, "loading 100,000 records into both lanes...", C_TEXT, 2);
    BSP_LCD_SetLayerAddress(0, 0, FRAME_BUFFER_ADDR);
    BSP_LCD_DisplayOn(0);
    BSP_LCD_SetBrightness(0, 100);

    uint32_t t0 = HAL_GetTick();
    if (!lanes_init(TARGET_RECORDS)) { printf("[FAIL] lanes init\r\n"); Error_Handler(); }
    printf("[OK] lanes: %lu records each, prefill %lu ms, heap %lu KB\r\n", (unsigned long)lane_a.count,
           (unsigned long)(HAL_GetTick() - t0), (unsigned long)((brk - (uint8_t *)HEAP_START) / 1024u));
    uint32_t step = 0, sweep_period = 1000u / program[0].sweep_hz;
    lane_b.mode = program[0].mode;
    draw_frame(program[0].sweep_hz);
    timers_init();
    printf("INFO nvic tim7(hash)=%lu tim6(expanse)=%lu systick=%lu ltdc=%lu\r\n",
           (unsigned long)NVIC_GetPriority(TIM7_IRQn), (unsigned long)NVIC_GetPriority(TIM6_DAC_IRQn),
           (unsigned long)NVIC_GetPriority(SysTick_IRQn), (unsigned long)NVIC_GetPriority(LTDC_IRQn));
    uint32_t start_ms = HAL_GetTick(), last_stats = start_ms, last_sweep = start_ms, last_log = start_ms + 500u, last_hb = start_ms;
    uint32_t btn_was = 0;
    lane_a.last_sweep_ms = lane_b.last_sweep_ms = lane_a.last_churn_ms = lane_b.last_churn_ms = start_ms;
    for (;;) {
        uint32_t now = HAL_GetTick();
        lane_clear_dirty_rows(&lane_a);
        lane_clear_dirty_rows(&lane_b);
        lane_ingest(&lane_a, now, INGEST_PER_S);
        lane_ingest(&lane_b, now, INGEST_PER_S);
        lane_churn(&lane_a, now);
        lane_churn(&lane_b, now);
        if (now - last_sweep >= sweep_period) {
            last_sweep = now;
            lane_sweep(&lane_a, now);
            lane_sweep(&lane_b, now);
        }
        if (now - last_hb >= 8u) { draw_heartbeat(&lane_a, now, last_hb); draw_heartbeat(&lane_b, now, last_hb); last_hb = now; }
        if (now - last_stats >= 200u) { last_stats = now; draw_stats(&lane_a); draw_stats(&lane_b); }
        uint32_t btn = BSP_PB_GetState(BUTTON_WAKEUP);
        if (btn && !btn_was) {                       /* next program step, reset the counters */
            step = (step + 1u) % N_STEPS;
            sweep_period = 1000u / program[step].sweep_hz;
            lane_b.mode = program[step].mode;
            lane_b.missed_ticks = lane_b.lat_max_ticks = lane_b.isr_nf = lane_b.isr_bad = lane_b.mask_max_cyc = 0;
            lane_a.missed_ticks = lane_a.lat_max_ticks = lane_a.isr_nf = lane_a.isr_bad = lane_a.isr_busy = 0;
            draw_title(program[step].sweep_hz);
            fill_rect(lane_b.x0 + 12, 86, 368, 16, C_PANEL);
            text(lane_b.x0 + 12, 86, mode_name(lane_b.mode), C_MUTED, 1);
            printf("INFO step=%lu sweep_hz=%lu mode=%lu %s\r\n", (unsigned long)step, (unsigned long)program[step].sweep_hz,
                   (unsigned long)lane_b.mode, mode_name(lane_b.mode));
        }
        btn_was = btn;
        if (now - last_log >= 1000u) {               /* half a period away from the sweep */
            last_log = now;
            printf("RESULT t=%lu sweep_hz=%lu mode=%lu "
                   "a_records=%lu a_sweep_us=%lu a_sweep_net_us=%lu a_lat_max_ns=%lu a_missed=%lu a_served=%lu a_ok=%lu a_busy=%lu a_nf=%lu a_bad=%lu a_body_max_cyc=%lu a_drops=%lu "
                   "b_records=%lu b_sweep_us=%lu b_sweep_net_us=%lu b_lat_max_ns=%lu b_missed=%lu b_served=%lu b_ok=%lu b_nf=%lu b_bad=%lu b_body_max_cyc=%lu b_mask_max_us=%lu b_drops=%lu\r\n",
                   (unsigned long)(now - start_ms), (unsigned long)program[step].sweep_hz, (unsigned long)lane_b.mode,
                   (unsigned long)lane_a.count, (unsigned long)lane_a.sweep_us, (unsigned long)lane_a.sweep_net_us, (unsigned long)(lane_a.lat_max_ticks * 50u),
                   (unsigned long)lane_a.missed_ticks, (unsigned long)lane_a.isr_served, (unsigned long)lane_a.isr_ok, (unsigned long)lane_a.isr_busy,
                   (unsigned long)lane_a.isr_nf, (unsigned long)lane_a.isr_bad, (unsigned long)lane_a.body_max_cyc, (unsigned long)lane_a.drops,
                   (unsigned long)lane_b.count, (unsigned long)lane_b.sweep_us, (unsigned long)lane_b.sweep_net_us, (unsigned long)(lane_b.lat_max_ticks * 50u),
                   (unsigned long)lane_b.missed_ticks, (unsigned long)lane_b.isr_served, (unsigned long)lane_b.isr_ok, (unsigned long)lane_b.isr_nf,
                   (unsigned long)lane_b.isr_bad, (unsigned long)lane_b.body_max_cyc, (unsigned long)(lane_b.mask_max_cyc / 400u), (unsigned long)lane_b.drops);
        }
    }
}

/* ---- timers: TIM2 free-running 20 MHz reference; TIM6 -> lane A, TIM7 -> lane B,
 * 1 kHz each, half a period apart, the hash lane at the HIGHER priority ------- */
static void timers_init(void) {
    __HAL_RCC_TIM2_CLK_ENABLE(); __HAL_RCC_TIM6_CLK_ENABLE(); __HAL_RCC_TIM7_CLK_ENABLE();
    /* APB1 timers run at 2 x PCLK1 = 200 MHz; prescale by 10 -> 20 MHz, 50 ns ticks */
    TIM2->PSC = 9; TIM2->ARR = 0xFFFFFFFFu; TIM2->EGR = TIM_EGR_UG; TIM2->CR1 = TIM_CR1_CEN;
    TIM6->PSC = 9; TIM6->ARR = TICKS_PER_MS - 1u; TIM6->EGR = TIM_EGR_UG; TIM6->SR = 0; TIM6->DIER = TIM_DIER_UIE;
    TIM7->PSC = 9; TIM7->ARR = TICKS_PER_MS - 1u; TIM7->EGR = TIM_EGR_UG; TIM7->SR = 0; TIM7->DIER = TIM_DIER_UIE;
    HAL_NVIC_SetPriority(TIM7_IRQn, 5, 0);
    HAL_NVIC_SetPriority(TIM6_DAC_IRQn, 6, 0);
    NVIC_EnableIRQ(TIM6_DAC_IRQn); NVIC_EnableIRQ(TIM7_IRQn);
    uint32_t ref = TIM2->CNT;
    TIM6->CNT = 0;                 lane_a.expected_cnt = ref + TICKS_PER_MS;
    TIM7->CNT = TICKS_PER_MS / 2;  lane_b.expected_cnt = ref + TICKS_PER_MS / 2;
    TIM6->CR1 = TIM_CR1_CEN; TIM7->CR1 = TIM_CR1_CEN;
}
void TIM6_DAC_IRQHandler(void) {
    uint32_t now = TIM2->CNT; TIM6->SR = 0;
    lane_isr(&lane_a, now, HAL_GetTick());
}
void TIM7_IRQHandler(void) {
    uint32_t now = TIM2->CNT; TIM7->SR = 0;
    lane_isr(&lane_b, now, HAL_GetTick());
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

/* MPU: region 0 = default no-access map; region 1 = SDRAM 32 MB normal
 * non-cacheable (framebuffer, scanned by LTDC; stores go through the write
 * buffer, unlike the strongly-ordered TEX0 setting); region 2 = the top
 * 16 MB of SDRAM cacheable write-back — the heap both lanes share. */
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
  MPU_InitStruct.TypeExtField = MPU_TEX_LEVEL1;
  MPU_InitStruct.AccessPermission = MPU_REGION_FULL_ACCESS;
  MPU_InitStruct.DisableExec = MPU_INSTRUCTION_ACCESS_ENABLE;
  MPU_InitStruct.IsShareable = MPU_ACCESS_NOT_SHAREABLE;
  MPU_InitStruct.IsCacheable = MPU_ACCESS_NOT_CACHEABLE;
  MPU_InitStruct.IsBufferable = MPU_ACCESS_BUFFERABLE;   /* stores merge in the write buffer; no cache to keep coherent with LTDC */
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
