#include "lcd.h"

#include "ch.h"
#include "hal.h"
#include "heater.h"
#include "usb_pd.h"
#include "chprintf.h"
#include "events.h"

#include "tc_adc.h"

#include "ssd1803_reg.h"
#include "ssd1803_ctrl.h"
#include "ssd1803_set.h"
#include "ssd1803_def.h"

THD_WORKING_AREA(waLcdThread, LCD_THREAD_STACK_SIZE);

THD_FUNCTION(lcdThread, arg)
{
    (void)arg;

    event_listener_t temp_event_listener;
    event_listener_t power_event_listener;

    ssd1803_state.row = 0;
    ssd1803_state.col = 0;
    ssd1803_state.is = false;
    ssd1803_state.re = true;

    chRegSetThreadName("lcd");

    chEvtRegisterMask(&temp_event_source, &temp_event_listener, TEMP_EVENT);
    chEvtRegisterMask(&power_event_source, &power_event_listener, POWER_EVENT);

    chEvtWaitAny(POWER_EVENT);

    palSetLine(LINE_LCD_NRST);
    chThdSleepMilliseconds(1);
    palClearLine(LINE_LCD_NRST);
    chThdSleepMilliseconds(1);
    palSetLine(LINE_LCD_NRST);

    ssd1803_initialize();

    char str[10];

    while (true)
    {
        chEvtWaitAny(TEMP_EVENT);

        chBSemWait(&heater.bsem);
        double is = heater.temperatures.is_temperature;
        double set = heater.temperatures.set;
        double max = heater.temperatures.max;
        double voltage = heater.power.voltage;
        double current = heater.power.current;
        double powerRatio = 100 * heater.power.pwm / heater.power.pwm_max;
        chBSemSignal(&heater.bsem);

        ssd1803_move_to_line(0);
        chsnprintf(str, 11, "%2dV   %1.1fA", (uint16_t)voltage, current);
        ssd1803_writeByteArray((uint8_t *)str, 10);

        ssd1803_move_to_line(1);
        if (is > max)
        {
            chsnprintf(str, 11, "    ---   ");
            ssd1803_writeByteArray((uint8_t *)str, 10);
        }
        else
        {
            chsnprintf(str, 11, "    %3d   ", (uint16_t)is);
            ssd1803_writeByteArray((uint8_t *)str, 10);
        }

        ssd1803_move_to_line(2);
        if (powerRatio <= 25)
        {
            chsnprintf(str, 11, "\x10%3d     "
                                " ",
                       (uint16_t)set);
        }
        else if (powerRatio <= 50)
        {
            chsnprintf(str, 11, "\x10%3d     "
                                "\x93",
                       (uint16_t)set);
        }
        else if (powerRatio <= 75)
        {
            chsnprintf(str, 11, "\x10%3d    "
                                "\x93\x93",
                       (uint16_t)set);
        }
        else
        {
            chsnprintf(str, 11, "\x10%3d   "
                                "\x93\x93\x93",
                       (uint16_t)set);
        }

        ssd1803_writeByteArray((uint8_t *)str, 10);
    }
}
