/**
  ******************************************************************************
  * @file    ft6x06_conf.h
  * @brief   Configuration for FT6X06 touch controller (from ST template).
  ******************************************************************************
  */

/* Define to prevent recursive inclusion -------------------------------------*/
#ifndef FT6X06_CONF_H
#define FT6X06_CONF_H

#ifdef __cplusplus
extern "C" {
#endif

/* Includes ------------------------------------------------------------------*/
/* Macros --------------------------------------------------------------------*/
/* Exported types ------------------------------------------------------------*/
/* Exported constants --------------------------------------------------------*/

/* Disable auto calibration for now; we only need basic TS support. */
#define FT6X06_AUTO_CALIBRATION_ENABLED      0U

/* LCD resolution for STM32H747I-DISCO panel. */
#define FT6X06_MAX_X_LENGTH                  800U
#define FT6X06_MAX_Y_LENGTH                  480U

#ifdef __cplusplus
}
#endif

#endif /* FT6X06_CONF_H */

/************************ (C) COPYRIGHT STMicroelectronics *****END OF FILE****/
