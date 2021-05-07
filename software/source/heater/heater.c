#include "ch.h"
#include "hal.h"
#include "usb_pd.h"
#include "heater.h"
#include "events.h"
#include "sensor.h"

#define HEATER_PWM PWMD3
#define HEATER_PWM_CHANNEL 2

#define CURRENT_FIELD 0
#define VOLTAGE_FIELD 1

event_source_t pwm_done_event_source;
event_source_t cur_alert_event_source;

// default heater values, suitable for T245 handles and tips
heater_t heater = {
    .power = {
        .current_offset = 0,
        .voltage_negotiated = 0,
        .current_negotiated = 0,
        .power_negotiated = 0,
        .voltage_meas = 0,
        .current_meas = 0,
        .pwm = 0,
        .pwm_max = PWM_MAX_PERCENTAGE},
    .current_control = {.set = 0.1, .p = HEATER_CURRENT_P, .i = HEATER_CURRENT_I, .error = 0, .integratedError = 0},
    .temperature_control = {.set = 300, .p = HEATER_TEMPERATURE_P, .i = HEATER_TEMPERATURE_I, .d = HEATER_TEMPERATURE_D, .error = 0, .error_last = 0, .integratedError = 0},
    .temperatures = {.min = 150, .max = 380, .local = 25}};

THD_WORKING_AREA(waHeaterThread, HEATER_THREAD_STACK_SIZE);

static PWMConfig pwmcfg = {
    24000000, /* 24 MHz PWM clock frequency.                */
    500,      /* Initial PWM period 20.83 uS. -> 48 kHz PWM */
    NULL,     /* Period callback.                           */
    {
        {PWM_OUTPUT_DISABLED, NULL},    /* CH1 mode and callback.         */
        {PWM_OUTPUT_DISABLED, NULL},    /* CH2 mode and callback.         */
        {PWM_OUTPUT_ACTIVE_HIGH, NULL}, /* CH3 mode and callback.         */
        {PWM_OUTPUT_DISABLED, NULL}     /* CH4 mode and callback.         */
    },
    0, /* Control Register 2.            */
    0  /* DMA/Interrupt Enable Register. */
};

/**
 * @brief Control loop for heater element temperature
 */
void temperatureControlLoop(void)
{
    chBSemWait(&heater.bsem);

    if (
        heater.connected &&
        !heater.sleep &&
        (heater.temperature_control.set <= heater.temperatures.max) &&
        (heater.temperature_control.is <= heater.temperatures.max))
    {
        // Calculation of actual error
        heater.temperature_control.error = heater.temperature_control.set - heater.temperature_control.is;

        if ((heater.current_control.set < heater.power.current_negotiated) && (heater.current_control.set >= 0))
        {
            // anti windup and integration of error
            heater.temperature_control.integratedError += heater.temperature_control.error * MS2S(LOOP_TIME_TEMPERATURE_MS);
        }

        // Control equation
        double diff_error = heater.temperature_control.error - heater.temperature_control.error_last;

        heater.current_control.set = heater.temperature_control.d * diff_error + heater.temperature_control.p * heater.temperature_control.error + heater.temperature_control.i * heater.temperature_control.integratedError;

        heater.temperature_control.error_last = heater.temperature_control.error;

        // Clamp to available power supply current
        if (heater.current_control.set > heater.power.current_negotiated)
        {
            heater.current_control.set = heater.power.current_negotiated;
        }
        else if (heater.current_control.set < 0)
        {
            heater.current_control.set = 0;
        }
    }
    else
    {
        // Reset control after disconnected tool or other error
        heater.temperature_control.error = 0;
        heater.temperature_control.error_last = 0;
        heater.temperature_control.integratedError = 0;
        heater.current_control.set = 0;
    }

    chBSemSignal(&heater.bsem);
}

/**
 * @brief Control loop for heater element current
 */
void currentControlLoop(void)
{
    chBSemWait(&heater.bsem);

    if (
        heater.connected &&
        !heater.sleep &&
        (heater.temperature_control.set <= heater.temperatures.max) &&
        (heater.temperature_control.is <= heater.temperatures.max))
    {
        // Calculation of actual error
        heater.current_control.error = heater.current_control.set - heater.current_control.is + heater.power.current_offset;

        if ((heater.power.pwm < heater.power.pwm_max) && (heater.power.pwm >= 0))
        {
            // anti windup and integration of error
            heater.current_control.integratedError += heater.current_control.error * MS2S(LOOP_TIME_CURRENT_MS);
        }
        // Control equation, convert voltage to PWM ratio
        heater.power.pwm = heater.power.pwm_max * (heater.current_control.p * heater.current_control.error + heater.current_control.i * heater.current_control.integratedError) / heater.power.voltage_negotiated;

        // Clamping of PWM ratio
        if (heater.power.pwm > heater.power.pwm_max)
        {
            heater.power.pwm = heater.power.pwm_max;
        }
        else if (heater.power.pwm <= 0)
        {
            heater.power.pwm = 0;
        }
    }
    else
    {
        // Reset control after disconnected tool or other error
        heater.current_control.error = 0;
        heater.current_control.integratedError = 0;
        heater.power.pwm = 0;
    }

    chBSemSignal(&heater.bsem);
}

#define ADC_GRP1_NUM_CHANNELS 2
#define ADC_GRP1_BUF_DEPTH 1
static adcsample_t adcsamples[ADC_GRP1_NUM_CHANNELS * ADC_GRP1_BUF_DEPTH];

/*
 * ADC conversion group.
 * Mode:        Linear buffer, 1 samples of 2 channels, SW triggered.
 * Channels:    1, 3
 */
static const ADCConversionGroup adcgrpcfg = {
    FALSE,
    ADC_GRP1_NUM_CHANNELS,
    NULL,
    NULL,
    ADC_CFGR1_RES_12BIT,                  /* CFGR1 */
    ADC_TR(0, 0),                         /* TR */
    ADC_SMPR_SMP_1P5,                     /* SMPR */
    ADC_CHSELR_CHSEL1 | ADC_CHSELR_CHSEL3 /* CHSELR */
};

static void curAlert(void *arg)
{
    (void)arg;
    chSysLockFromISR();

    /* Invocation of some I-Class system APIs, never preemptable.*/
    if (pwmIsChannelEnabledI(&HEATER_PWM, HEATER_PWM_CHANNEL))
    {
        pwmDisableChannelI(&HEATER_PWM, HEATER_PWM_CHANNEL);
    }
    chEvtBroadcastI(&cur_alert_event_source);

    chSysUnlockFromISR();
}

THD_FUNCTION(heaterThread, arg)
{
    (void)arg;
    chRegSetThreadName("heater");

    event_listener_t power_event_listener;
    event_listener_t temp_event_listener;

    palEnableLineEvent(LINE_CURRENT_ALERT, PAL_EVENT_MODE_FALLING_EDGE);
    palSetLineCallback(LINE_CURRENT_ALERT, curAlert, NULL);

    chEvtRegisterMask(&power_event_source, &power_event_listener, POWER_EVENT);
    chEvtRegisterMask(&temp_event_source, &temp_event_listener, TEMP_EVENT);

    adcStart(&ADCD1, NULL);

    chEvtWaitAny(POWER_EVENT);
    pwmStart(&HEATER_PWM, &pwmcfg);

    while (true)
    {
        chEvtWaitAny(TEMP_EVENT);

        // temperatureControlLoop();

        for (uint32_t current_loop_counter = 0; current_loop_counter < LOOP_TIME_RATIO; current_loop_counter++)
        {
            currentControlLoop();
            chBSemWait(&heater.bsem);
            uint16_t ratio = heater.power.pwm;
            chBSemSignal(&heater.bsem);

            // pwmEnableChannel(&HEATER_PWM, HEATER_PWM_CHANNEL, PWM_PERCENTAGE_TO_WIDTH(&HEATER_PWM, ratio));

            chThdSleepMilliseconds(LOOP_TIME_CURRENT_MS);

            // Measure heater current
            adcConvert(&ADCD1, &adcgrpcfg, adcsamples, ADC_GRP1_BUF_DEPTH);

            chBSemWait(&heater.bsem);
            heater.power.voltage_meas = VOLTAGE_SENSE_RATIO * ADC_TO_VOLT(adcsamples[VOLTAGE_FIELD]);
            heater.current_control.is = CURRENT_SENSE_RATIO * ADC_TO_VOLT(adcsamples[CURRENT_FIELD]);

            if (!heater.connected)
            {
                heater.power.current_offset = heater.current_control.is;
            }
            chBSemSignal(&heater.bsem);
        }

        if (pwmIsChannelEnabledI(&HEATER_PWM, HEATER_PWM_CHANNEL))
        {
            pwmDisableChannel(&HEATER_PWM, HEATER_PWM_CHANNEL);
        }

        chEvtBroadcast(&pwm_done_event_source);
    }
}
