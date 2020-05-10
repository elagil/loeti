#include "ch.h"
#include "hal.h"
#include "heater.h"
#include "usb_pd.h"
#include "USB_PD_core.h"
#include "events.h"

#define USB_PD_TIMEOUT 100

event_source_t power_event_source;
event_source_t alert_event_source;
event_listener_t alert_event_listener;

USB_PD_I2C_PORT STUSB45DeviceConf;
extern uint8_t Cut;
/* PDO Variables */
extern USB_PD_StatusTypeDef PD_status;
extern USB_PD_SNK_PDO_TypeDef PDO_SNK[3];
extern STUSB_GEN1S_RDO_REG_STATUS_RegTypeDef Nego_RDO;
extern uint8_t PDO_SNK_NUMB;
extern uint8_t PDO_FROM_SRC_Valid;
extern uint32_t ConnectionStamp;
extern uint8_t TypeC_Only_status;
extern USB_PD_SRC_PDOTypeDef PDO_FROM_SRC[7];

THD_WORKING_AREA(waUsbPdThread, USB_PD_THREAD_STACK_SIZE);

static void toggleAlarmManagement(void *arg)
{
    (void)arg;
    chSysLockFromISR();
    /* Invocation of some I-Class system APIs, never preemptable.*/
    chEvtBroadcastI(&alert_event_source);
    chSysUnlockFromISR();
}

/**
 * @brief Exchange information with power source
 * @detail Soft reset the link, in order to force the source to send link information
 * Also requests source power profiles. 
 * 
 * After the alert pin is toggled, as a result of soft reset, the alarm management can handle messages
 */
static void exchangeSrc(void)
{
    while (true)
    {
        Send_Soft_reset_Message();

        if (chEvtWaitAnyTimeout(PD_ALERT_EVENT, TIME_MS2I(USB_PD_TIMEOUT)))
        {
            uint32_t k = 0;
            while (++k < 500)
            {
                ALARM_MANAGEMENT(NULL);
                if (PDO_FROM_SRC_Valid)
                {
                    return;
                }
            }
        }

        chThdSleepMilliseconds(USB_PD_TIMEOUT);
    }
}

THD_FUNCTION(usbPdThread, arg)
{
    (void)arg;

    chRegSetThreadName("usb pd");

    STUSB45DeviceConf.I2cDeviceID_7bit = 0x28;

    chEvtObjectInit(&alert_event_source);
    chEvtRegisterMask(&alert_event_source, &alert_event_listener, PD_ALERT_EVENT);

    palEnableLineEvent(LINE_PD_ALERT_INT, PAL_EVENT_MODE_FALLING_EDGE);
    palSetLineCallback(LINE_PD_ALERT_INT, toggleAlarmManagement, NULL);

    usb_pd_init();

    Read_SNK_PDO();
    Read_RDO();

    // Get power profiles from source
    exchangeSrc();

    // Select source profile with highest power output
    volatile uint8_t pdo = FindHighestSrcPower();

    // Wait for source to accept selected profile
    exchangeSrc();

    // Calculate provided power from source voltage and current
    volatile uint32_t current = getPdoCurrent(pdo);
    volatile uint32_t voltage = getPdoVoltage(pdo);

    chBSemWait(&heater.bsem);

    // Convert mV to V and mA to A
    heater.power.voltage = voltage / 1000;
    heater.power.current = current / 1000;
    heater.power.max = heater.power.current * heater.power.voltage;

    // Calculate maximum possible PWM ratio, for not exceeding source current
    volatile uint32_t max_current = (uint32_t)((double)voltage / heater.power.resistance);
    volatile double pwm_max = heater.power.power_safety_margin * PWM_MAX_PERCENTAGE * current / max_current;

    // calculate I component of PID loop, based on available power. This prevents overshoot
    // Higher supply power leads to higher I component, because less time is spent integrating
    heater.control.i = heater.control.i_per_W * heater.power.max;

    // Clamp PWM ratio to maximum possible values
    if (pwm_max > PWM_MAX_PERCENTAGE)
    {
        heater.power.pwm_max = PWM_MAX_PERCENTAGE;
    }
    else
    {
        heater.power.pwm_max = pwm_max;
    }

    chBSemSignal(&heater.bsem);

    chEvtBroadcast(&power_event_source);

    while (true)
    {
        chThdSleepMilliseconds(1000);
    }
}