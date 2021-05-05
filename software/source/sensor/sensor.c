#include "sensor.h"

#include "ch.h"
#include "hal.h"
#include "heater.h"
#include "usb_pd.h"
#include "spiHelper.h"
#include "events.h"

event_source_t temp_event_source;

#define exchangeSpiAdc(txbuf, rxbuf) spiExchangeHelper(&SPID1, &tc_adc_spicfg, TC_ADC_LEN, txbuf, rxbuf)

THD_WORKING_AREA(waSensorThread, SENSOR_THREAD_STACK_SIZE);

THD_FUNCTION(sensorThread, arg)
{
    (void)arg;
    event_listener_t power_event_listener;
    event_listener_t pwm_event_listener;

    chRegSetThreadName("sensor");

    chEvtRegisterMask(&power_event_source, &power_event_listener, POWER_EVENT);
    chEvtRegisterMask(&pwm_done_event_source, &pwm_event_listener, PWM_EVENT);

    chEvtWaitAny(POWER_EVENT);

    while (true)
    {
        chEvtBroadcast(&temp_event_source);

        chThdSleepMilliseconds(LOOP_TIME_TEMPERATURE_MS / 2);

        // Wait for heating to stop
        chEvtWaitAny(PWM_EVENT);
    }
}
