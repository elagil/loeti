#include "ch.h"
#include "hal.h"
#include "heater.h"
#include "usb_pd.h"
#include "USB_PD_core.h"
#include "events.h"

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

void toggleAlarmManagement(void *arg)
{
    (void)arg;
    chSysLockFromISR();
    /* Invocation of some I-Class system APIs, never preemptable.*/
    chEvtBroadcastI(&alert_event_source);
    chSysUnlockFromISR();
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

    while (true)
    {
        Send_Soft_reset_Message();

        chEvtWaitAny(PD_ALERT_EVENT);

        uint32_t k = 0;
        while (k < 500)
        {
            ALARM_MANAGEMENT(NULL);
            if (PDO_FROM_SRC_Valid)
            {
                break;
            }
            k++;
        }

        if (PDO_FROM_SRC_Valid)
        {
            break;
        }

        chThdSleepMilliseconds(100);
    }

    chThdSleepMilliseconds(1000);

    volatile uint8_t pdo = FindHighestSrcPower();

    chThdSleepMilliseconds(1000);

    Send_Soft_reset_Message();

    Read_SNK_PDO();
    Read_RDO();

    volatile uint32_t current = getPdoCurrent(pdo);
    volatile uint32_t voltage = getPdoVoltage(pdo);
    heater.power_max = (current / 1000) * (voltage / 1000);

    chBSemWait(&heater.bsem);
    volatile uint32_t max_current = (uint32_t)((double)voltage / heater.resistance);
    volatile double pwm_max = heater.power_safety_margin * 10000 * current / max_current;

    if (pwm_max > 10000)
    {
        heater.pwm_max = 10000;
    }
    else
    {
        heater.pwm_max = pwm_max;
    }

    chBSemSignal(&heater.bsem);

    chEvtBroadcast(&power_event_source);

    while (true)
    {
        chThdSleepMilliseconds(1000);
    }
}