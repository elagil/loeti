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

#define LINE_LENGTH 10
#define TEMP_AVGS 5

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
    char uart_str[10];
    uint8_t waiting = 0;

    chBSemWait(&heater.bsem);
    double power_negotiated = heater.power.power_negotiated;
    chBSemSignal(&heater.bsem);

    ssd1803_move_to_line(0);
    chsnprintf(str, LINE_LENGTH + 1, "      %3dW", (uint16_t)power_negotiated);
    ssd1803_writeByteArray((uint8_t *)str, LINE_LENGTH);

    double temps[TEMP_AVGS];
    uint32_t temp_idx = 0;
    double avg = 0;

    while (true)
    {
        chEvtWaitAny(TEMP_EVENT);

        chBSemWait(&heater.bsem);
        bool connected = heater.connected;
        double is = heater.temperature_control.is;
        double set = heater.temperature_control.set;
        double max = heater.temperatures.max;
        double current = heater.current_control.is - heater.power.current_offset;
        double voltage = heater.power.voltage_meas;
        double power = (current * voltage) / heater.power.power_negotiated;
        chBSemSignal(&heater.bsem);

        if (++temp_idx == TEMP_AVGS)
        {
            temp_idx = 0;
        }

        temps[temp_idx] = is;

        avg = 0;
        for (uint32_t i = 0; i < TEMP_AVGS; i++)
        {
            avg += temps[i] / TEMP_AVGS;
        }

        chsnprintf(uart_str, 12, "%5d%5d\n", (uint16_t)(is * 100), (uint16_t)(current * voltage * 100));
        sdWrite(&SD2, (uint8_t *)uart_str, 11);

        ssd1803_move_to_line(1);
        if (connected && (is < max) && (is > 0))
        {
            waiting = 0;
            chsnprintf(str, LINE_LENGTH + 1, "    %3d   ", (uint16_t)(avg + 0.5));
        }
        else
        {
            switch (waiting)
            {
            case 0:
                chsnprintf(str, LINE_LENGTH + 1, "          ");
                break;

            case 1:
                chsnprintf(str, LINE_LENGTH + 1, "    \xdd     ");
                break;

            case 2:
                chsnprintf(str, LINE_LENGTH + 1, "    \xdd\xdd    ");
                break;

            case 3:
                chsnprintf(str, LINE_LENGTH + 1, "    \xdd\xdd\xdd   ");
                break;

            case 4:
                chsnprintf(str, LINE_LENGTH + 1, "     \xdd\xdd   ");
                break;

            case 5:
                chsnprintf(str, LINE_LENGTH + 1, "      \xdd   ");
                break;

            default:
                break;
            }
            if (++waiting >= 6)
            {
                waiting = 0;
            }
        }
        ssd1803_writeByteArray((uint8_t *)str, LINE_LENGTH);

        ssd1803_move_to_line(2);
        if (heater.sleep)
        {
            chsnprintf(str, LINE_LENGTH + 1, "\x10%3d "
                                             "SLEEP",
                       (uint16_t)set);
        }
        else
        {

            if (power <= 0.25)
            {
                chsnprintf(str, LINE_LENGTH + 1, "\x10%3d     "
                                                 " ",
                           (uint16_t)set);
            }
            else if (power <= 0.50)
            {
                chsnprintf(str, LINE_LENGTH + 1, "\x10%3d     "
                                                 "\x93",
                           (uint16_t)set);
            }
            else if (power <= 0.75)
            {
                chsnprintf(str, LINE_LENGTH + 1, "\x10%3d    "
                                                 "\x93\x93",
                           (uint16_t)set);
            }
            else
            {
                chsnprintf(str, LINE_LENGTH + 1, "\x10%3d   "
                                                 "\x93\x93\x93",
                           (uint16_t)set);
            }
        }

        ssd1803_writeByteArray((uint8_t *)str, LINE_LENGTH);
    }
}
