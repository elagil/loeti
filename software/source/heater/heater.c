#include "ch.h"
#include "hal.h"
#include "heater.h"

THD_WORKING_AREA(waHeaterThread, HEATER_THREAD_STACK_SIZE);

uint8_t controlLoop(double temperature)
{
}

THD_FUNCTION(waHeaterThread, arg)
{
    (void)arg;
    chRegSetThreadName("heater");

    while (true)
    {
        chThdSleepMilliseconds(100);
    }
}
