#include "ch.h"
#include "hal.h"
#include "heater.h"

THD_WORKING_AREA(waHeaterWa, HEATER_THREAD_STACK_SIZE);

uint8_t controlLoop(double temperature)
{
    (void)temperature;
    return 0;
}

THD_FUNCTION(heaterThread, arg)
{
    (void)arg;
    chRegSetThreadName("heater");

    while (true)
    {
        chThdSleepMilliseconds(100);
    }
}
