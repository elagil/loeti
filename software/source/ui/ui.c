#include "ch.h"
#include "hal.h"
#include "ui.h"
#include "heater.h"

#define DEBOUNCE 5

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
        switches.current.id.sw0 = palReadLine(LINE_SW0);
        switches.current.id.sw1 = palReadLine(LINE_SW1);

        if (switches.current.raw != switches.previous.raw)
        {
            if (++debounce == DEBOUNCE)
            {
                chBSemWait(&heater.bsem);

                if (!switches.current.id.sw1)
                {
                    heater.temperatures.set += 10;
                }
                else if (!switches.current.id.sw0)
                {
                    heater.temperatures.set -= 10;
                }

                if (heater.temperatures.set > heater.temperatures.max)
                {
                    heater.temperatures.set = heater.temperatures.max;
                }

                if (heater.temperatures.set < heater.temperatures.min)
                {
                    heater.temperatures.set = heater.temperatures.min;
                }

                chBSemSignal(&heater.bsem);

                // chEvtBroadcast(&switch_event_source);
                switches.previous.raw = switches.current.raw;
            }
        }
        else
        {
            debounce = 0;
        }

        chThdSleepMilliseconds(10);
    }
}
