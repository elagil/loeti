#include "ch.h"
#include "hal.h"
#include "usb_pd.h"
#include "heater.h"
#include "events.h"

#define LOOP_TIME 200

// default heater values, suitable for T245 handles and tips
heater_t heater = {
    .power_safety_margin = 1,
    .resistance = 2.6,
    .min_temperature = 150,
    .max_temperature = 370,
    .set_temperature = 300,
    .p = 80,
    .i = 0.025,
    .d = 0,
    .error = 0,
    .integratedError = 0,
    .power_max = 0,
    .pwm = 0,
    .pwm_max = 0,
    .voltage = 0,
    .current = 0};

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

    if ((heater.set_temperature <= heater.max_temperature) && (heater.is_temperature <= heater.max_temperature))
    {
        // Safety feature, for not letting heater temperature exceed maximum limit

        // Calculation of temperature error
        heater.error = heater.set_temperature - heater.is_temperature;

        if ((heater.pwm < heater.pwm_max) && (heater.pwm > 0))
        {
            // anti windup and integration of error
            heater.integratedError += heater.error;
        }

        // Control equation
        heater.pwm = heater.p * heater.error + heater.i * heater.integratedError * LOOP_TIME;

        // Clamping of PWM ratio
        if (heater.pwm > heater.pwm_max)
        {
            heater.pwm = heater.pwm_max;
        }
        else if (heater.pwm < 1)
        {
            heater.pwm = 0;
        }
        chBSemSignal(&heater.bsem);
        return (uint16_t)heater.pwm;
    }
    else
    {
        // Reset control after disconnected tool or other error
        heater.error = 0;
        heater.integratedError = 0;
        heater.pwm = 0;
        chBSemSignal(&heater.bsem);
        return 0;
    }
}

THD_FUNCTION(heaterThread, arg)
{
    (void)arg;
    chRegSetThreadName("heater");

    event_listener_t power_event_listener;
    chEvtRegisterMask(&power_event_source, &power_event_listener, POWER_EVENT);

    chEvtWaitAny(POWER_EVENT);
    pwmStart(&PWMD1, &pwmcfg);

    while (true)
    {
        thread_t *tp = chMsgWait();
        msg_t msg = chMsgGet(tp);
        (void)msg;

        pwmEnableChannel(&PWMD1, 0, PWM_PERCENTAGE_TO_WIDTH(&PWMD1, controlLoop()));
        chThdSleepMilliseconds(LOOP_TIME);
        pwmDisableChannel(&PWMD1, 0);

        chMsgRelease(tp, MSG_OK);
    }
}
