#include "ch.h"
#include "hal.h"
#include "usb_pd.h"
#include "heater.h"
#include "events.h"
#include "sensor.h"

#define HEATER_PWM PWMD3
#define HEATER_PWM_CHANNEL 2

#define CURRENT_FIELD 0

uint32_t heater_level;
event_source_t pwm_done_event_source;
event_source_t cur_alert_event_source;

const double heater_levels[HEATER_LEVEL_COUNT] = {310, 340};

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
    .current_control = {.set = 0, .p = HEATER_CURRENT_P, .i = HEATER_CURRENT_I, .error = 0, .integratedError = 0},
    .temperature_control = {.set = 0, .p = HEATER_TEMPERATURE_P, .i = HEATER_TEMPERATURE_I, .d = HEATER_TEMPERATURE_D, .error = 0, .error_last = 0, .integratedError = 0},
    .temperatures = {.min = 150, .max = 375}};

THD_WORKING_AREA(waHeaterThread, HEATER_THREAD_STACK_SIZE);

/**
 * @brief PWM configuration for switching the power transistor
 * 
 */
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
 * @brief Control loop for heater temperature (outer loop)
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

        if ((heater.current_control.set < heater.power.current_target) && (heater.current_control.set >= 0))
        {
            // anti windup and integration of error
            heater.temperature_control.integratedError += heater.temperature_control.error * MS2S(LOOP_TIME_TEMPERATURE_MS);
        }

        // Control equation
        double diff_error = heater.temperature_control.error - heater.temperature_control.error_last;

        heater.current_control.set = heater.temperature_control.d * diff_error + heater.temperature_control.p * heater.temperature_control.error + heater.temperature_control.i * heater.temperature_control.integratedError;

        heater.temperature_control.error_last = heater.temperature_control.error;
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
 * @brief Control loop for heater current (inner loop)
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
        double current_set;

        // Clamp to available power supply current
        if (heater.current_control.set > heater.power.current_target)
        {
            current_set = heater.power.current_target;
        }
        else if (heater.current_control.set < 0)
        {
            current_set = 0;
        }
        else
        {
            current_set = heater.current_control.set;
        }

        // Calculation of actual error
        heater.current_control.error = current_set - heater.current_control.is + heater.power.current_offset;

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

#define ADC_GRP1_NUM_CHANNELS 1
#define ADC_GRP1_BUF_DEPTH 1
static adcsample_t adcsamples[ADC_GRP1_NUM_CHANNELS * ADC_GRP1_BUF_DEPTH];

/**
 * @brief ADC conversion group.
 * @details Mode: Linear buffer, 1 sample of 1 channel, SW triggered.
 * 
 */
static const ADCConversionGroup currentMeasurement = {
    FALSE,
    ADC_GRP1_NUM_CHANNELS,
    NULL,
    NULL,
    ADC_CFGR1_RES_12BIT, /* CFGR1 */
    ADC_TR(0, 0),        /* TR */
    ADC_SMPR_SMP_28P5,   /* SMPR */
    ADC_CHSELR_CHSEL1    /* CHSELR */
};

/**
 * @brief Interrupt for handling overcurrent conditions
 * @detail Immediately stops PWM generation and issues an alert event.
 * 
 * @param arg unused
 */
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

/**
 * @brief Heater thread, controls PWM and current/temperature loops
 * 
 */
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

    // Transparent current limiting mode:
    // Output immediately returns active after fault condition is cleared. ISR disables PWM immediately.
    palClearLine(LINE_CURR_RESET);

    adcStart(&ADCD1, NULL);
    pwmStart(&HEATER_PWM, &pwmcfg);

    // Wait for USB-PD negotiation to succeed
    chEvtWaitAny(POWER_EVENT);

    while (true)
    {
        // Wait for completion of temperature measurement
        chEvtWaitAny(TEMP_EVENT);

        chBSemWait(&heater.bsem);
        // Read selected heater level
        heater.temperature_control.set = heater_levels[heater_level];
        chBSemSignal(&heater.bsem);

        // Calculate new current set value, based on temperature error
        temperatureControlLoop();

        // Current control loop, executed LOOP_TIME_RATIO times.
        for (uint32_t current_loop_counter = 0; current_loop_counter < LOOP_TIME_RATIO; current_loop_counter++)
        {
            currentControlLoop();

            chBSemWait(&heater.bsem);
            uint16_t ratio = heater.power.pwm;
            chBSemSignal(&heater.bsem);

            // Select PWM ratio, according to current control loop output
            pwmEnableChannel(&HEATER_PWM, HEATER_PWM_CHANNEL, PWM_PERCENTAGE_TO_WIDTH(&HEATER_PWM, ratio));

            chThdSleepMilliseconds(LOOP_TIME_CURRENT_MS);

            // Measure heater current at the end of current loop period (wait for current low-pass filter to settle).
            adcConvert(&ADCD1, &currentMeasurement, adcsamples, ADC_GRP1_BUF_DEPTH);

            chBSemWait(&heater.bsem);
            heater.current_control.is = CURRENT_SENSE_RATIO * ADC_TO_VOLT(adcsamples[CURRENT_FIELD]);

            if (!heater.connected)
            {
                heater.power.current_offset = heater.current_control.is;
            }

            chBSemSignal(&heater.bsem);
        }

        // Deactivate PWM before temperature measurement. Required for correct measurement.
        if (pwmIsChannelEnabledI(&HEATER_PWM, HEATER_PWM_CHANNEL))
        {
            pwmDisableChannel(&HEATER_PWM, HEATER_PWM_CHANNEL);
        }

        // Signal the end of the heating routine.
        chEvtBroadcast(&pwm_done_event_source);
    }
}
