#include "ch.h"
#include "hal.h"
#include "heater.h"
#include "diagnostic.h"
#include "usb_pd.h"
#include "chprintf.h"
#include "events.h"
#include "sensor.h"

THD_WORKING_AREA(waDiagThread, DIAG_THREAD_STACK_SIZE);

#define UART_STR_LEN 10

THD_FUNCTION(diagThread, arg)
{
    (void)arg;

    event_listener_t temp_event_listener;
    event_listener_t power_event_listener;

    chRegSetThreadName("uart");

    chEvtRegisterMask(&temp_event_source, &temp_event_listener, TEMP_EVENT);
    chEvtRegisterMask(&power_event_source, &power_event_listener, POWER_EVENT);

    chEvtWaitAny(POWER_EVENT);

    char uart_str[UART_STR_LEN];

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

        chsnprintf(uart_str, UART_STR_LEN + 2, "%5d%5d\n", (uint16_t)(is * 100), (uint16_t)(current * voltage * 100));
        sdWrite(&SD2, (uint8_t *)uart_str, UART_STR_LEN + 1);
    }
}
