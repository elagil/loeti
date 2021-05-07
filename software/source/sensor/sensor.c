#include "ch.h"
#include "hal.h"
#include "heater.h"
#include "usb_pd.h"
#include "spiHelper.h"
#include "events.h"
#include "sensor.h"

event_source_t temp_event_source;

#define TEMP_FIELD 0
#define exchangeSpiAdc(txbuf, rxbuf) spiExchangeHelper(&SPID1, &tc_adc_spicfg, TC_ADC_LEN, txbuf, rxbuf)

THD_WORKING_AREA(waSensorThread, SENSOR_THREAD_STACK_SIZE);

#define ADC_GRP1_NUM_CHANNELS 1
#define ADC_GRP1_BUF_DEPTH 1
static adcsample_t adcsample[ADC_GRP1_NUM_CHANNELS * ADC_GRP1_BUF_DEPTH];

/*
 * ADC conversion group.
 * Mode:        Linear buffer, 1 sample of 1 channels, SW triggered.
 * Channel:     2
 */
static const ADCConversionGroup temperatureMeasurement = {
    FALSE,
    ADC_GRP1_NUM_CHANNELS,
    NULL,
    NULL,
    ADC_CFGR1_RES_12BIT, /* CFGR1 */
    ADC_TR(0, 0),        /* TR */
    ADC_SMPR_SMP_28P5,   /* SMPR */
    ADC_CHSELR_CHSEL2    /* CHSELR */
};

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

        // Wait for heating to stop
        chEvtWaitAny(PWM_EVENT);

        chThdSleepMilliseconds(10);

        // Measure iron temperature sensor
        adcConvert(&ADCD1, &temperatureMeasurement, adcsample, ADC_GRP1_BUF_DEPTH);

        chBSemWait(&heater.bsem);
        uint16_t raw = adcsample[TEMP_FIELD];
        heater.temperature_control.is = (raw - 2410) * 0.33152;

        if (raw >= (ADC_FS_READING - 100))
        {
            heater.connected = false;
        }
        else
        {
            heater.connected = true;
        }

        chBSemSignal(&heater.bsem);
    }
}
