#include "ch.h"
#include "hal.h"
#include "usb_pd.h"
#include "heater.h"
#include "events.h"

// default heater values, suitable for T245 handles and tips

heater_t heater = {
    .power = {
        .power_safety_margin = 1,
        .resistance = 2.6,
        .voltage = 0,
        .current = 0,
        .max = 0,
        .pwm = 0,
        .pwm_max = 0},
    .control = {.p = 80, .i_per_W = 0.0004, .i = 0, .d = 0, .error = 0, .integratedError = 0},
    .temperatures = {.min = 150, .max = 360, .set = 300, .local = 25}};

#define POWER_EVENT EVENT_MASK(0)

THD_WORKING_AREA(waHeaterThread, HEATER_THREAD_STACK_SIZE);

static PWMConfig pwmcfg = {
    24000000, /* 24 MHz PWM clock frequency.     */
    1200,     /* Initial PWM period 50 uS.       */
    NULL,     /* Period callback.                */
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
 * @brief Control loop for heater element
 */
uint16_t controlLoop(void)
{
    chBSemWait(&heater.bsem);

    uint16_t ratio = 0;

    if (heater.connected && (heater.temperatures.set <= heater.temperatures.max) && (heater.temperatures.is_temperature <= heater.temperatures.max))
    {
        // Safety feature, for not letting heater temperature exceed maximum limit

        // Calculation of temperature error
        heater.control.error = heater.temperatures.set - heater.temperatures.is_temperature;

        if ((heater.power.pwm < heater.power.pwm_max) && (heater.power.pwm > 0))
        {
            // anti windup and integration of error
            heater.control.integratedError += heater.control.error;
        }

        // Control equation
        heater.power.pwm = heater.control.p * heater.control.error + heater.control.i * heater.control.integratedError * LOOP_TIME;

        // Clamping of PWM ratio
        if (heater.power.pwm > heater.power.pwm_max)
        {
            heater.power.pwm = heater.power.pwm_max;
        }
        else if (heater.power.pwm < 1)
        {
            heater.power.pwm = 0;
        }
        ratio = (uint16_t)heater.power.pwm;
    }
    else
    {
        // Reset control after disconnected tool or other error
        heater.control.error = 0;
        heater.control.integratedError = 0;
        heater.power.pwm = 0;
        ratio = 0;
    }

    chBSemSignal(&heater.bsem);
    return ratio;
}

THD_FUNCTION(heaterThread, arg)
{
    (void)arg;
    chRegSetThreadName("heater");

    event_listener_t power_event_listener;
    event_listener_t temp_event_listener;

    chEvtRegisterMask(&power_event_source, &power_event_listener, POWER_EVENT);
    chEvtRegisterMask(&temp_event_source, &temp_event_listener, TEMP_EVENT);

    chEvtWaitAny(POWER_EVENT);
    pwmStart(&PWMD1, &pwmcfg);

    while (true)
    {
        chEvtWaitAny(TEMP_EVENT);

        pwmEnableChannel(&PWMD1, 0, PWM_PERCENTAGE_TO_WIDTH(&PWMD1, controlLoop()));
        chThdSleepMilliseconds(LOOP_TIME);
        pwmDisableChannel(&PWMD1, 0);
    }
}
