#include "ch.h"
#include "hal.h"
#include "heater.h"
#include "usb_pd.h"
#include "USB_PD_core.h"
#include "events.h"

#define USB_PD_TIMEOUT 50
#define USB_PD_TIMEOUT_TICKS TIME_MS2I(USB_PD_TIMEOUT)

event_source_t power_event_source;
event_source_t pd_alert_event_source;
event_listener_t pd_alert_event_listener;

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
    chEvtBroadcastI(&pd_alert_event_source);
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

        if (chEvtWaitAnyTimeout(PD_ALERT_EVENT, USB_PD_TIMEOUT_TICKS))
        {

            systime_t start_time = chVTGetSystemTime();

            while (true)
            {
                // catch alarm
                ALARM_MANAGEMENT(NULL);

                if (PDO_FROM_SRC_Valid)
                {
                    return;
                }
                else if (chVTTimeElapsedSinceX(start_time) >= USB_PD_TIMEOUT_TICKS)
                {
                    break;
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

    chEvtObjectInit(&pd_alert_event_source);
    chEvtRegisterMask(&pd_alert_event_source, &pd_alert_event_listener, PD_ALERT_EVENT);

    palEnableLineEvent(LINE_PD_ALERT_INT, PAL_EVENT_MODE_FALLING_EDGE);
    palSetLineCallback(LINE_PD_ALERT_INT, toggleAlarmManagement, NULL);

    usb_pd_init();

    Read_SNK_PDO();
    Read_RDO();

    // Get power profiles from source
    exchangeSrc();

    // Select source profile with highest power output
    uint8_t pdo = FindHighestSrcPower();

    // Wait for source to accept selected profile
    exchangeSrc();

    // Calculate provided power from source voltage and current
    uint32_t current = getPdoCurrent(pdo);
    uint32_t voltage = getPdoVoltage(pdo);

    chBSemWait(&heater.bsem);

    // Convert mV to V and mA to A
    heater.power.voltage_negotiated = voltage / 1000;
    heater.power.current_negotiated = current / 1000;
    heater.power.power_negotiated = heater.power.current_negotiated * heater.power.voltage_negotiated;

    chBSemSignal(&heater.bsem);

    chEvtBroadcast(&power_event_source);

    while (true)
    {
        chThdSleepMilliseconds(1000);
    }
}