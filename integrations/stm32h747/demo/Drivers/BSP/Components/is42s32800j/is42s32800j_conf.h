/**
  ******************************************************************************
  * @file    is42s32800j_conf.h
  * @brief   Configuration for IS42S32800J SDRAM (adapted for STM32H7).
  ******************************************************************************
  */

/* Define to prevent recursive inclusion -------------------------------------*/
#ifndef IS42S32800J_CONF_H
#define IS42S32800J_CONF_H

#ifdef __cplusplus
 extern "C" {
#endif

/* Includes ------------------------------------------------------------------*/
#include "stm32h7xx_hal.h"

/** @addtogroup BSP
  * @{
  */

/** @addtogroup Components
  * @{
  */

/** @addtogroup IS42S32800J
  * @{
  */

/** @addtogroup IS42S32800J_Exported_Constants
  * @{
  */

/*
 * SDRAM refresh counter. The value below comes from the ST template
 * (100 MHz SDRAM clock). It is sufficient for our simple LCD test.
 */
#define REFRESH_COUNT                    ((uint32_t)0x0603)

#define IS42S32800J_TIMEOUT             ((uint32_t)0xFFFF)

#ifdef __cplusplus
}
#endif

#endif /* IS42S32800J_CONF_H */

/**
  * @}
  */

/**
  * @}
  */

/**
  * @}
  */

/**
  * @}
  */
