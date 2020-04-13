#include "usb_pd.h"
#include "ch.h"
#include "hal.h"
#include "USB_PD_core.h"

event_source_t alert_source;
event_listener_t alert_listener;

USB_PD_I2C_PORT STUSB45DeviceConf;
extern uint8_t Cut;
uint8_t USB_PD_Interupt_Flag;
uint8_t USB_PD_Interupt_PostponedFlag;
uint8_t push_button_Action_Flag;
uint8_t Timer_Action_Flag;
uint8_t flag_once = 1;
/* PDO Variables */
extern USB_PD_StatusTypeDef PD_status;
extern USB_PD_SNK_PDO_TypeDef PDO_SNK[3];
extern STUSB_GEN1S_RDO_REG_STATUS_RegTypeDef Nego_RDO;
extern uint8_t PDO_SNK_NUMB;
extern uint8_t PDO_FROM_SRC_Valid;
extern uint32_t ConnectionStamp;
extern uint8_t TypeC_Only_status;
extern uint8_t PDO_FROM_SRC_Num_Sel;
extern USB_PD_SRC_PDOTypeDef PDO_FROM_SRC[7];

THD_WORKING_AREA(waUsbPdThread, USB_PD_THREAD_STACK_SIZE);

// 400 kHz fast mode for I2C
static const I2CConfig i2ccfg = {
    STM32_TIMINGR_PRESC(0) |
        STM32_TIMINGR_SCLDEL(3) |
        STM32_TIMINGR_SDADEL(1) |
        STM32_TIMINGR_SCLH(3) |
        STM32_TIMINGR_SCLL(9),
    0,
    0};

void toggleAlarmManagement(void *arg)
{
    (void)arg;
    chSysLockFromISR();
    /* Invocation of some I-Class system APIs, never preemptable.*/
    chEvtBroadcastI(&alert_source);
    chSysUnlockFromISR();
}

THD_FUNCTION(usbPdThread, arg)
{
    (void)arg;

    chRegSetThreadName("usb pd");

    STUSB45DeviceConf.I2cDeviceID_7bit = 0x28;

    chEvtObjectInit(&alert_source);
    chEvtRegister(&alert_source, &alert_listener, 0);

    palEnableLineEvent(LINE_PD_ALERT_INT, PAL_EVENT_MODE_FALLING_EDGE);
    palSetLineCallback(LINE_PD_ALERT_INT, toggleAlarmManagement, NULL);

    i2cStart(&I2CD1, &i2ccfg);

    usb_pd_init();

    Read_SNK_PDO();
    Send_Soft_reset_Message();

    chEvtWaitAny(EVENT_MASK(0));

    uint32_t k = 0;
    while (k < 1000)
    {
        ALARM_MANAGEMENT(NULL);
        if (PDO_FROM_SRC_Valid)
        {
            break;
        }
        k++;
    }

    Find_Matching_SRC_PDO(10, 8000, 20000);
    Send_Soft_reset_Message();

    while (true)
    {
        chThdSleepMilliseconds(500);
    }
}