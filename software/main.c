/*
    ChibiOS - Copyright (C) 2006..2018 Giovanni Di Sirio

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.
*/

#include "ch.h"
#include "hal.h"

#include "heater.h"
#include "tc_adc.h"
#include "usb_pd.h"
#include "lcd.h"
#include "ui.h"
#include "events.h"

binary_semaphore_t dma_lock;

/*
 * Application entry point.=
 */
int main(void)
{

  /*
   * System initializations.
   * - HAL initialization, this also initializes the configured device drivers
   *   and performs the board-specific initializations.
   * - Kernel initialization, the main() function becomes a thread and the
   *   RTOS is active.
   */
  halInit();
  chSysInit();

  chBSemObjectInit(&switches.bsem, false);
  chBSemObjectInit(&heater.bsem, false);
  chBSemObjectInit(&dma_lock, false);

  chEvtObjectInit(&switch_event_source);
  chEvtObjectInit(&temp_event_source);
  chEvtObjectInit(&power_event_source);
  chEvtObjectInit(&pwm_event_source);

  palClearLine(LINE_PD_RST);
  palClearLine(LINE_PWM);

  /*
   * Creates the switch checker thread.
   */
  chThdCreateStatic(waUiThread, sizeof(waUiThread), NORMALPRIO, uiThread, NULL);

  /*
   * Creates the LCD thread.
   */
  chThdCreateStatic(waLcdThread, sizeof(waLcdThread), NORMALPRIO, lcdThread, NULL);

  /*
   * Creates the USB PD control thread.
   */
  chThdCreateStatic(waUsbPdThread, sizeof(waUsbPdThread), NORMALPRIO, usbPdThread, NULL);

  /*
   * Creates the heater and control loop thread.
   */
  chThdCreateStatic(waHeaterThread, sizeof(waHeaterThread), NORMALPRIO, heaterThread, NULL);

  /*
   * Creates the temperature ADC read thread.
   */
  chThdCreateStatic(waAdcThread, sizeof(waAdcThread), NORMALPRIO, adcThread, NULL);

  while (true)
  {
    chThdSleepMilliseconds(1000);
  }
}
