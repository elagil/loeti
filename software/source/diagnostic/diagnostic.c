#include "ch.h"
#include "hal.h"
#include "heater.h"
#include "diagnostic.h"
#include "usb_pd.h"
#include "chprintf.h"
#include "events.h"
#include "sensor.h"

THD_WORKING_AREA(waDiagThread, DIAG_THREAD_STACK_SIZE);

#define LED_LINE_COUNT 3
const ioline_t leds[LED_LINE_COUNT] = {LINE_LED2, LINE_LED1, LINE_LED0};

#define UART_STR_LEN 10

void ledSwitch(uint32_t number, bool state)
{
    if (state == true)
    {
        palSetLine(leds[number]);
    }
    else
    {
        palClearLine(leds[number]);
    }
}

void ledToggle(uint32_t number)
{
    palToggleLine(leds[number]);
}

void ledToggleSlow(uint32_t number, uint32_t scale)
{
    static uint32_t counter = 0;

    if (++counter == scale)
    {
        counter = 0;
        ledToggle(number);
    }
}

void allLedsSwitch(bool state)
{
    for (uint32_t led = 0; led < LED_LINE_COUNT; led++)
    {
        ledSwitch(led, state);
    }
}

void allLedsSwitchExcept(uint32_t number, bool state)
{
    for (uint32_t led = 0; led < LED_LINE_COUNT; led++)
    {
        if (led != number)
        {
            ledSwitch(led, state);
        }
    }
}

typedef enum
{
    disconnected,
    waiting,
    connecting,
    heating,
    temperature_reached
} diagnostic_state_t;

THD_FUNCTION(diagThread, arg)
{
    (void)arg;

    event_listener_t temp_event_listener;
    event_listener_t power_event_listener;

    chRegSetThreadName("uart");
    diagnostic_state_t diagnostic_state = disconnected;

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

        switch (diagnostic_state)
        {
        case disconnected:
            allLedsSwitch(false);
            diagnostic_state = waiting;
            break;

        case waiting:
            allLedsSwitchExcept(heater_level, false);
            ledToggleSlow(heater_level, 4);

            if (connected)
            {
                diagnostic_state = connecting;
            }
            else
            {
                diagnostic_state = waiting;
            }
            break;

        case connecting:
            allLedsSwitch(false);
            diagnostic_state = heating;
            break;

        case heating:
            allLedsSwitchExcept(heater_level, false);
            ledToggle(heater_level);

            if (connected)
            {
                if ((is < set) && ((set - is) > 10))
                {
                    diagnostic_state = heating;
                }
                else
                {
                    diagnostic_state = temperature_reached;
                }
            }
            else
            {
                diagnostic_state = disconnected;
            }
            break;

        case temperature_reached:
            allLedsSwitchExcept(heater_level, false);
            ledSwitch(heater_level, true);

            if (connected)
            {
                if ((is < set) && ((set - is) > 10))
                {
                    diagnostic_state = heating;
                }
                else
                {
                    diagnostic_state = temperature_reached;
                }
            }
            else
            {
                diagnostic_state = disconnected;
            }

            break;

        default:
            break;
        }

        chsnprintf(uart_str, UART_STR_LEN + 2, "%5d%5d\n", (uint16_t)(is * 100), (uint16_t)(current * voltage * 100));
        sdWrite(&SD2, (uint8_t *)uart_str, UART_STR_LEN + 1);
    }
}
