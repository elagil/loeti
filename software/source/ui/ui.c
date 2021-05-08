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

    heater_level = DEFAULT_HEATER_LEVEL;

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
                    if (++heater_level == HEATER_LEVEL_COUNT)
                    {
                        heater_level = 0;
                    }
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
