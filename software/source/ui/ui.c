#include "ch.h"
#include "hal.h"
#include "ui.h"
#include "heater.h"

#define DEBOUNCE 5
#define TEMPERATURE_SET_INTERVAL 25

event_source_t switch_event_source;
switches_t switches;

THD_WORKING_AREA(waUiThread, UI_THREAD_STACK_SIZE);

THD_FUNCTION(uiThread, arg)
{
    (void)arg;
    chRegSetThreadName("ui");
    uint8_t debounce = 0;

    while (true)
    {
        switches.current.id.sw0 = palReadLine(LINE_SW);

        if (switches.current.raw < switches.previous.raw)
        {
            if (++debounce == DEBOUNCE)
            {
                chBSemWait(&heater.bsem);

                if (!switches.current.id.sw0)
                {
                    palToggleLine(LINE_LED0);

                    if (heater.sleep)
                    {
                        heater.sleep = false;
                    }
                    else
                    {
                        heater.temperature_control.set -= TEMPERATURE_SET_INTERVAL;
                    }
                }

                if (heater.temperature_control.set > heater.temperatures.max)
                {
                    heater.temperature_control.set = heater.temperatures.max;
                }

                if (heater.temperature_control.set < heater.temperatures.min)
                {
                    heater.temperature_control.set = heater.temperatures.min;
                }

                chBSemSignal(&heater.bsem);
            }
        }
        else
        {
            switches.previous.raw = switches.current.raw;
            debounce = 0;
        }

        chThdSleepMilliseconds(10);
    }
}
