#include "ch.h"
#include "hal.h"
#include "usb_pd.h"
#include "heater.h"
#include "events.h"

#define MS2S(x) ((double)x / 1000.0)

#define HEATER_RESISTANCE 3.0
#define HEATER_CURRENT_I_SCALE 0.5
#define HEATER_CURRENT_I (HEATER_CURRENT_I_SCALE * HEATER_RESISTANCE / (2 * MS2S(LOOP_TIME_CURRENT_MS)))

#define VOLTAGE_SENSE_RATIO 11
#define CURRENT_SENSE_RATIO 2.5
#define ADC_REF_VOLTAGE 3.3
#define ADC_FS_READING 4096
#define ADC_TO_VOLT(x) ((double)x / (double)ADC_FS_READING * (double)ADC_REF_VOLTAGE)

event_source_t pwm_event_source;

// default heater values, suitable for T245 handles and tips
heater_t heater = {
    .power = {
        .current_safety_margin = 1,
        .voltage_negotiated = 0,
        .current_negotiated = 0,
        .power_negotiated = 0,
        .voltage_meas = 0,
        .current_meas = 0,
        .pwm = 0,
        .pwm_max = 0},
    .current_control = {.set = 1, .p = 0, .i = HEATER_CURRENT_I, .d = 0, .error = 0, .integratedError = 0},
    .temperature_control = {.set = 300, .p = 0.1, .i = 0.25, .d = 0, .error = 0, .integratedError = 0},
    .temperatures = {.min = 150, .max = 380, .local = 25}};

#define POWER_EVENT EVENT_MASK(0)

THD_WORKING_AREA(waHeaterThread, HEATER_THREAD_STACK_SIZE);

static PWMConfig pwmcfg = {
    24000000, /* 24 MHz PWM clock frequency.                */
    1200,     /* Initial PWM period 50 uS. -> 20 kHz PWM    */
    NULL,     /* Period callback.                           */
    {
        {PWM_OUTPUT_ACTIVE_HIGH, NULL}, /* CH1 mode and callback.         */
        {PWM_OUTPUT_DISABLED, NULL},    /* CH2 mode and callback.         */
        {PWM_OUTPUT_DISABLED, NULL},    /* CH3 mode and callback.         */
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

    if (heater.connected && !heater.sleep)
    { // Heater connected, so heating can occur.
        if ((heater.temperature_control.set <= heater.temperatures.max) && (heater.temperature_control.is <= heater.temperatures.max))
        { // Safety feature, for not letting heater temperature exceed maximum limit

            // Calculation of temperature error
            heater.temperature_control.error = heater.temperature_control.set - heater.temperature_control.is;

            if ((heater.current_control.set < heater.power.current_negotiated) && (heater.current_control.set > 0))
            {
                // anti windup and integration of error
                heater.temperature_control.integratedError += heater.temperature_control.error * MS2S(LOOP_TIME_TEMPERATURE_MS);
            }

            // Control equation
            heater.current_control.set = heater.temperature_control.p * heater.temperature_control.error + heater.temperature_control.i * MS2S(LOOP_TIME_TEMPERATURE_MS) * heater.temperature_control.integratedError;

            // Clamping of power supply current
            if (heater.current_control.set > heater.power.current_negotiated * heater.power.current_safety_margin)
            {
                heater.current_control.set = heater.power.current_negotiated * heater.power.current_safety_margin;
            }
            else if (heater.current_control.set <= 0)
            {
                heater.current_control.set = 0;
            }
        }
        else
        { // Shutdown heating if heater exceeds limits
            heater.current_control.set = 0;
        }
    }
    else
    {
        // Reset control after disconnected tool or other error
        heater.temperature_control.error = 0;
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

    if (heater.connected)
    { // Heater connected, so heating can occur.
        if ((heater.temperature_control.set <= heater.temperatures.max) && (heater.temperature_control.is <= heater.temperatures.max))
        { // Safety feature, for not letting heater temperature exceed maximum limit

            // Calculation of current error
            heater.current_control.error = heater.current_control.set - heater.current_control.is;

            if ((heater.power.pwm < PWM_MAX_PERCENTAGE) && (heater.power.pwm >= 0))
            {
                // anti windup and integration of error
                heater.current_control.integratedError += heater.current_control.error * MS2S(LOOP_TIME_CURRENT_MS);
            }

            // Control equation, convert voltage to PWM ratio
            heater.power.pwm = (double)PWM_MAX_PERCENTAGE * (heater.current_control.p * heater.current_control.error + heater.current_control.i * heater.current_control.integratedError) / heater.power.voltage_negotiated;

            // Clamping of PWM ratio
            if (heater.power.pwm > PWM_MAX_PERCENTAGE)
            {
                heater.power.pwm = PWM_MAX_PERCENTAGE;
            }
            else if (heater.power.pwm < 1)
            {
                heater.power.pwm = 0;
            }
        }
        else
        { // Shutdown heating if heater exceeds limits
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
#define ADC_GRP1_BUF_DEPTH 2
static adcsample_t fields[ADC_GRP1_NUM_CHANNELS * ADC_GRP1_BUF_DEPTH];

/*
 * ADC conversion group.
 * Mode:        Linear buffer, 1 samples of 2 channels, SW triggered.
 * Channels:    IN10.
 */
static const ADCConversionGroup adcgrpcfg1 = {
    FALSE,
    ADC_GRP1_NUM_CHANNELS,
    NULL,
    NULL,
    ADC_CFGR1_RES_12BIT,                  /* CFGR1 */
    ADC_TR(0, 0),                         /* TR */
    ADC_SMPR_SMP_1P5,                     /* SMPR */
    ADC_CHSELR_CHSEL2 | ADC_CHSELR_CHSEL7 /* CHSELR */
};

THD_FUNCTION(heaterThread, arg)
{
    (void)arg;
    chRegSetThreadName("heater");

    event_listener_t power_event_listener;
    event_listener_t temp_event_listener;

    chEvtRegisterMask(&power_event_source, &power_event_listener, POWER_EVENT);
    chEvtRegisterMask(&temp_event_source, &temp_event_listener, TEMP_EVENT);

    adcStart(&ADCD1, NULL);

    chEvtWaitAny(POWER_EVENT);
    pwmStart(&PWMD1, &pwmcfg);

    while (true)
    {
        chEvtWaitAny(TEMP_EVENT);

        temperatureControlLoop();

        for (uint32_t current_loop_counter = 0; current_loop_counter < LOOP_TIME_RATIO; current_loop_counter++)
        {
            currentControlLoop();
            chBSemWait(&heater.bsem);
            uint16_t ratio = heater.power.pwm;
            chBSemSignal(&heater.bsem);

            systime_t pwm_start_time = chVTGetSystemTimeX();
            pwmEnableChannel(&PWMD1, 0, PWM_PERCENTAGE_TO_WIDTH(&PWMD1, ratio));

            chThdSleepUntil(pwm_start_time + TIME_MS2I(LOOP_TIME_CURRENT_MS / 2));

            // Measure heater current
            adcConvert(&ADCD1, &adcgrpcfg1, fields, ADC_GRP1_BUF_DEPTH);

            chBSemWait(&heater.bsem);
            heater.power.voltage_meas = VOLTAGE_SENSE_RATIO * ADC_TO_VOLT(fields[1]);
            heater.current_control.is = CURRENT_SENSE_RATIO * ADC_TO_VOLT(fields[0]);
            chBSemSignal(&heater.bsem);

            chThdSleepUntil(pwm_start_time + TIME_MS2I(LOOP_TIME_CURRENT_MS));
        }

        pwmDisableChannel(&PWMD1, 0);

        chEvtBroadcast(&pwm_event_source);
    }
}
