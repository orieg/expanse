/* Expanse "two lanes" LCD demo — STM32H747I-DISCO, Cortex-M7 (#605).
 *
 * Step 1: bring the NT35510 800x480 panel up in native landscape from the
 * vendored BSP and draw the static layout of the demo (title bar, two lane
 * panels, field, heartbeat strip, stat rows) as a test card. Later steps
 * add the two data-structure lanes, the per-lane timer interrupts and the
 * live instruments. Clock, MPU, UART, DMA2D and DSI/LTDC setup are the
 * prior working configuration for this board (docs/NT35510_LCD_SETUP.md),
 * kept verbatim except for the landscape orientation.
 *
 * This is a demo, not a benchmark: measured claims live in
 * docs/benchmarks/stm32h747. */
#include "main.h"
#include <stdio.h>
#include <string.h>
#include "stm32h747i_discovery.h"
#include "stm32h747i_discovery_lcd.h"
#include "stm32h747i_discovery_sdram.h"
#include "font16.h"

#define LCD_W 800U
#define LCD_H 480U
#define BYTES_PER_PIXEL 4U
#define FRAME_BUFFER_ADDR (SDRAM_DEVICE_ADDR) /* one ARGB8888 buffer, 1.5 MB */

#define RGB(r, g, b) (0xFF000000U | ((uint32_t)(r) << 16) | ((uint32_t)(g) << 8) | (uint32_t)(b))
#define C_BG     RGB(0x0b, 0x12, 0x20)
#define C_PANEL  RGB(0x11, 0x1a, 0x2e)
#define C_GRID   RGB(0x1c, 0x27, 0x40)
#define C_TEXT   RGB(0xe5, 0xe7, 0xeb)
#define C_MUTED  RGB(0x94, 0xa3, 0xb8)
#define C_BLUE   RGB(0x3b, 0x82, 0xf6)
#define C_ORANGE RGB(0xf5, 0x9e, 0x0b)
#define C_GREEN  RGB(0x22, 0xc5, 0x5e)
#define C_RED    RGB(0xef, 0x44, 0x44)

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

/* ---- framebuffer drawing (single buffer, direct writes) ------------------ */
static volatile uint32_t *fb = (volatile uint32_t *)FRAME_BUFFER_ADDR;

static inline void px(int x, int y, uint32_t c) {
    if ((unsigned)x < LCD_W && (unsigned)y < LCD_H) fb[(unsigned)y * LCD_W + (unsigned)x] = c;
}
static void fill_rect(int x, int y, int w, int h, uint32_t c) {
    if (x < 0) { w += x; x = 0; }
    if (y < 0) { h += y; y = 0; }
    if (x + w > (int)LCD_W) w = (int)LCD_W - x;
    if (y + h > (int)LCD_H) h = (int)LCD_H - y;
    for (int j = 0; j < h; j++) {
        volatile uint32_t *row = fb + (unsigned)(y + j) * LCD_W + (unsigned)x;
        for (int i = 0; i < w; i++) row[i] = c;
    }
}
static void rect(int x, int y, int w, int h, uint32_t c) {
    fill_rect(x, y, w, 1, c); fill_rect(x, y + h - 1, w, 1, c);
    fill_rect(x, y, 1, h, c); fill_rect(x + w - 1, y, 1, h, c);
}
static void fill_circle(int cx, int cy, int r, uint32_t c) {
    for (int dy = -r; dy <= r; dy++)
        for (int dx = -r; dx <= r; dx++)
            if (dx * dx + dy * dy <= r * r) px(cx + dx, cy + dy, c);
}
/* text: 10x16 cell font, integer scale; returns the x after the string */
static int text(int x, int y, const char *s, uint32_t c, int scale) {
    for (; *s; s++) {
        unsigned code = (unsigned char)*s;
        if (code < 32 || code > 126) code = '?';
        const uint16_t *g = font16[code - 32];
        for (unsigned row = 0; row < FONT_H; row++) {
            uint16_t m = g[row];
            if (!m) continue;
            for (unsigned col = 0; col < FONT_W; col++)
                if (m & (1u << (15 - col)))
                    fill_rect(x + (int)col * scale, y + (int)row * scale, scale, scale, c);
        }
        x += (int)FONT_W * scale;
    }
    return x;
}
static int text_right(int xr, int y, const char *s, uint32_t c, int scale) {
    int w = (int)strlen(s) * (int)FONT_W * scale;
    return text(xr - w, y, s, c, scale);
}

/* ---- the layout of the demo screen (mockup frames 2/3) ------------------- */
typedef struct { int x0; const char *title; const char *sub; uint32_t accent; } lane_t;

static void draw_lane(const lane_t *l) {
    int x0 = l->x0;
    fill_rect(x0, 48, 392, 424, C_PANEL);
    text(x0 + 12, 56, l->title, l->accent, 2);
    text(x0 + 12, 90, l->sub, C_MUTED, 1);
    /* field */
    fill_rect(x0 + 12, 110, 368, 200, C_BG); rect(x0 + 12, 110, 368, 200, C_GRID);
    for (int i = 1; i < 6; i++) fill_rect(x0 + 12 + i * 368 / 6, 110, 1, 200, C_GRID);
    for (int i = 1; i < 4; i++) fill_rect(x0 + 12, 110 + i * 50, 368, 1, C_GRID);
    text(x0 + 18, 114, "tracked devices, read by the 1 kHz interrupt", C_MUTED, 1);
    for (int d = 0; d < 12; d++) /* placeholder dots on a diagonal until the lanes exist */
        fill_circle(x0 + 40 + d * 28, 150 + (d * 37) % 140, 5, l->accent);
    /* heartbeat strip */
    fill_rect(x0 + 12, 320, 368, 56, C_BG); rect(x0 + 12, 320, 368, 56, C_GRID);
    text(x0 + 18, 324, "interrupt heartbeat (last 3 s)", C_MUTED, 1);
    for (int i = 0; i < 368; i++) { /* placeholder wave */
        int y = 348 + ((i / 8) % 2 ? 10 : -10);
        px(x0 + 12 + i, y, l->accent); px(x0 + 12 + i, y + 1, l->accent);
    }
    /* stat rows */
    const char *labels[4] = { "records", "last sweep", "interrupt latency, max", "dropped interrupts" };
    const char *values[4] = { "0", "--", "--", "0" };
    for (int r = 0; r < 4; r++) {
        text(x0 + 12, 388 + r * 20, labels[r], C_MUTED, 1);
        text_right(x0 + 380, 388 + r * 20, values[r], C_TEXT, 1);
    }
}

static void draw_test_card(void) {
    fill_rect(0, 0, LCD_W, LCD_H, C_BG);
    text(12, 10, "EXPANSE on STM32H747 - same workload, two data structures, one screen", C_TEXT, 1);
    text(12, 28, "100,000 records . TTL sweep every second . one interrupt per lane reads its table and draws", C_MUTED, 1);
    lane_t left = { 6, "EXPANSE", "ordered trie . sync32 reader in its interrupt . remove_range sweep", C_BLUE };
    lane_t right = { 402, "HASH TABLE", "open addressing . reader interrupt masked around writes . full scan", C_ORANGE };
    draw_lane(&left);
    draw_lane(&right);
    fill_rect(0, 466, LCD_W, 14, C_BG);
    text(12, 466, "step 1 test card: layout only, no data structures yet  (#605)", C_MUTED, 1);
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
    printf("\r\nEXPANSE stm32h747 lcd demo (step 1: landscape test card)\r\n");

    if (BSP_SDRAM_Init(0) != BSP_ERROR_NONE) { printf("[FAIL] SDRAM init\r\n"); Error_Handler(); }
    printf("[OK] SDRAM\r\n");
    if (BSP_LCD_InitEx(0, LCD_ORIENTATION_LANDSCAPE, LCD_PIXEL_FORMAT_RGB888, LCD_W, LCD_H) != BSP_ERROR_NONE) {
        printf("[FAIL] LCD init (landscape)\r\n"); Error_Handler();
    }
    uint32_t xs = 0, ys = 0;
    BSP_LCD_GetXSize(0, &xs); BSP_LCD_GetYSize(0, &ys);
    printf("[OK] LCD %lux%lu, controller %s\r\n", (unsigned long)xs, (unsigned long)ys,
           Lcd_Driver_Type == LCD_CTRL_NT35510 ? "NT35510" : "OTM8009A");
    if (xs != LCD_W || ys != LCD_H) { printf("[FAIL] geometry is not 800x480\r\n"); Error_Handler(); }

    draw_test_card();
    SCB_CleanDCache(); /* the framebuffer region is non-cacheable (MPU), belt and braces */
    BSP_LCD_SetLayerAddress(0, 0, FRAME_BUFFER_ADDR);
    if (BSP_LCD_DisplayOn(0) != BSP_ERROR_NONE) { printf("[FAIL] display on\r\n"); Error_Handler(); }
    BSP_LCD_SetBrightness(0, 100);
    printf("[OK] test card on screen\r\n");

    uint32_t n = 0;
    for (;;) { /* blink a corner marker so a recording shows the firmware is alive */
        fill_rect(LCD_W - 12, 4, 8, 8, (n++ & 1) ? C_GREEN : C_BG);
        HAL_Delay(500);
    }
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
