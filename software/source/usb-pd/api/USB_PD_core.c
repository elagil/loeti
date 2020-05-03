#include "USB_PD_core.h"
#include "ch.h"
#include "hal.h"
#include "dma_lock.h"

#define DEVICE_ID 0x25

uint8_t Cut;
USB_PD_StatusTypeDef PD_status;
USB_PD_SNK_PDO_TypeDef PDO_SNK[3];
USB_PD_SRC_PDOTypeDef PDO_FROM_SRC[7];
uint8_t PDO_FROM_SRC_Num = 0;
uint8_t PDO_FROM_SRC_Valid = 0;
STUSB_GEN1S_RDO_REG_STATUS_RegTypeDef Nego_RDO;
uint32_t ConnectionStamp = 0;
uint8_t TypeC_Only_status = 0;
uint8_t PDO_SNK_NUMB;

extern uint8_t USB_PD_Status_change_flag;

extern USB_PD_I2C_PORT STUSB45DeviceConf;

unsigned char DataRW[40];
uint8_t txbuf[50];

// 400 kHz fast mode for I2C
static const I2CConfig i2ccfg = {
    STM32_TIMINGR_PRESC(0) |
        STM32_TIMINGR_SCLDEL(3) |
        STM32_TIMINGR_SDADEL(1) |
        STM32_TIMINGR_SCLH(3) |
        STM32_TIMINGR_SCLL(9),
    0,
    0};

msg_t I2C_Write_USB_PD(i2caddr_t address, uint16_t reg, uint8_t *buf, uint16_t length)
{
  chBSemWait(&dma_lock);

  txbuf[0] = reg & 0xff;

  for (uint32_t i = 0; i < length; i++)
  {
    txbuf[i + 1] = buf[i];
  }

  i2cAcquireBus(&I2CD1);
  i2cStart(&I2CD1, &i2ccfg);

  msg_t status = i2cMasterTransmit(&I2CD1, address, txbuf, length + 1, NULL, 0);

  i2cStop(&I2CD1);
  i2cReleaseBus(&I2CD1);

  chBSemSignal(&dma_lock);
  return status;
}

msg_t I2C_Read_USB_PD(i2caddr_t address, uint16_t reg, uint8_t *rxbuf, uint16_t length)
{
  chBSemWait(&dma_lock);
  txbuf[0] = reg & 0xff;

  i2cAcquireBus(&I2CD1);
  i2cStart(&I2CD1, &i2ccfg);

  msg_t status = i2cMasterTransmit(&I2CD1, address, txbuf, 1, rxbuf, length);

  i2cStop(&I2CD1);
  i2cReleaseBus(&I2CD1);

  chBSemSignal(&dma_lock);
  return status;
}

/****
 * query the internal device ID register of the STUSB4500 and verify it matches
 * the expected manufacturer-specified ID. this is used to determine if the
 * device has powered on and can respond to I2C read requests.
 ****/
void USB_PD_ready(void)
{
  uint8_t dev_id;
  do /* wait for NVM to be reloaded */
  {
    chThdSleepMilliseconds(1);
    I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, REG_DEVICE_ID, &dev_id, 1);
  } while (dev_id != DEVICE_ID);
}

/**
* @brief  asserts and de-asserts the STUSB4500 Hardware reset pin.
* @param  I2C Port used (I2C1 or I2C2).
* @param  none
* @retval none
*/

/************************   HW_Reset_state(void)  ***************************
This function asserts and de-asserts the STUSB4500 Hardware reset pin.  
After reset, STUSB4500 behave according to Non Volatile Memory defaults settings. 
************************************************************************************/
void HW_Reset_state(void)
{
  palSetLine(LINE_PD_RST);
  chThdSleepMilliseconds(15); /*time to be dedected by the source */
  palClearLine(LINE_PD_RST);
  chThdSleepMilliseconds(15); /* this to left time to Device to load NVM*/
  usb_pd_init();
}

/************************   SW_reset_by_Reg (void)  *************************
This function resets STUSB45 type-C and USB PD state machines. It also clears any
ALERT. By initialisating Type-C pull-down termination, it forces electrical USB type-C
disconnection (both on SOURCE and SINK sides). 
************************************************************************************/

void SW_reset_by_Reg(void)
{
  USB_PD_ready();

  msg_t Status;
  //uint8_t Buffer[12];
  DataRW[0] = 1;
  Status = I2C_Write_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, STUSB_GEN1S_RESET_CTRL_REG, &DataRW[0], 1);

  if (Status == MSG_OK)
  {
    Status = I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, ALERT_STATUS_1, &DataRW[0], 12); // clear ALERT Status
    chThdSleepMilliseconds(27);                                                                   // on source , the debounce time is more than 15ms error recovery < at 25ms
    DataRW[0] = 0;
    Status = I2C_Write_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, STUSB_GEN1S_RESET_CTRL_REG, &DataRW[0], 1);
  }
}

/************************   Send_Soft_reset_Message (void)  ***************************/
/**
* @brief Send Power delivery reset message).
* @retval none
*/

void Send_Soft_reset_Message(void)
{
  USB_PD_ready();

  msg_t Status;
  //unsigned char DataRW[2];
  // Set Tx Header to Soft Reset
  DataRW[0] = Soft_Reset_Message_type;
  Status = I2C_Write_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, TX_HEADER, &DataRW[0], 1);

  // send command message
  if (Status == MSG_OK)
  {
    DataRW[0] = Send_Message;
    Status = I2C_Write_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, STUSB_GEN1S_CMD_CTRL, &DataRW[0], 1);
  }
  PDO_FROM_SRC_Valid = 0;
}

/***************************   usb_pd_init(void)  ***************************
this function clears all interrupts and unmasks the useful interrupts
*****************************************************************************/

void usb_pd_init(void)
{
  STUSB_GEN1S_ALERT_STATUS_MASK_RegTypeDef Alert_Mask;
  uint32_t Status = MSG_OK;
  //static unsigned char DataRW[13];
  DataRW[0] = 0;

  USB_PD_ready();

  Alert_Mask.d8 = 0xFF;
  Alert_Mask.b.CC_DETECTION_STATUS_AL_MASK = 0;
  Alert_Mask.b.PD_TYPEC_STATUS_AL_MASK = 0;
  Alert_Mask.b.PRT_STATUS_AL_MASK = 0;

  DataRW[0] = Alert_Mask.d8;

  Status = I2C_Write_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, ALERT_STATUS_MASK, &DataRW[0], 1); // unmask port status alarm
  /* clear ALERT Status */

  if (Status == MSG_OK)
  {
    I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, ALERT_STATUS_1, &DataRW[0], 12);
  }

  PD_status.Port_Status.d8 = DataRW[3];
  PD_status.CC_status.d8 = DataRW[6];
  PD_status.HWFault_status.d8 = DataRW[8];
  PD_status.Monitoring_status.d8 = DataRW[5];
  TypeC_Only_status = 0;

  return;
}

/**********************   ALARM_MANAGEMENT(void)  ***************************
device interrupt Handler
************************************************************************************/

void typec_connection_status(void)
{
  USB_PD_ready();
  I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, CC_STATUS, &PD_status.CC_status.d8, 1);
}

void ALARM_MANAGEMENT(void *arg)
{
  (void)arg;
  STUSB_GEN1S_ALERT_STATUS_RegTypeDef Alert_Status;
  STUSB_GEN1S_ALERT_STATUS_MASK_RegTypeDef Alert_Mask;
  //static unsigned char DataRW[40];

  USB_PD_ready();

  I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, CC_STATUS, &DataRW[0], 1);
  PD_status.CC_status.d8 = DataRW[0];

  if (palReadLine(LINE_PD_ALERT_INT) == 0)
  {
    I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, ALERT_STATUS_1, &DataRW[0], 2);
    Alert_Mask.d8 = DataRW[1];
    Alert_Status.d8 = DataRW[0] & ~Alert_Mask.d8;
    if (Alert_Status.d8 != 0)
    {
      PD_status.HW_Reset = (DataRW[0] >> 7);

      if (Alert_Status.b.CC_DETECTION_STATUS_AL != 0)
      {
        I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, PORT_STATUS_TRANS, &DataRW[0], 2);
        PD_status.Port_Status.d8 = DataRW[1];
        if (PD_status.Port_Status.b.CC_ATTACH_STATE != 0)
        {
          ConnectionStamp = chVTGetSystemTime();
          I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, CC_STATUS, &PD_status.CC_status.d8, 1);
        }
        else /* Detached detected */
        {

          ConnectionStamp = 0;
        }
      }
      if (Alert_Status.b.MONITORING_STATUS_AL != 0)
      {
        I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, TYPEC_MONITORING_STATUS_0, &DataRW[0], 2);
        PD_status.Monitoring_status.d8 = DataRW[1];
      }
      I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, CC_STATUS, &DataRW[0], 1);
      PD_status.CC_status.d8 = DataRW[0];

      if (Alert_Status.b.HW_FAULT_STATUS_AL != 0)
      {
        I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, CC_HW_FAULT_STATUS_0, &DataRW[0], 2);
        PD_status.HWFault_status.d8 = DataRW[1];
      }

      if (Alert_Status.b.PRT_STATUS_AL != 0)
      {
        USBPD_MsgHeader_TypeDef Header;
        I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, PRT_STATUS, &PD_status.PRT_status.d8, 1);

        if (PD_status.PRT_status.b.MSG_RECEIVED == 1)
        {
          I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, RX_HEADER, &DataRW[0], 2);
          Header.d16 = LE16(&DataRW[0]);

          if (Header.b.NumberOfDataObjects > 0)
          {
            switch (Header.b.MessageType)
            {
            case 0x01:
            {
              static uint32_t i, j;
              I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, RX_DATA_OBJ, &DataRW[0], Header.b.NumberOfDataObjects * 4);
              j = 0;

              PDO_FROM_SRC_Num = Header.b.NumberOfDataObjects;
              for (i = 0; i < Header.b.NumberOfDataObjects; i++)
              {
                PDO_FROM_SRC[i].d32 = (uint32_t)(DataRW[j] + (DataRW[j + 1] << 8) + (DataRW[j + 2] << 16) + (DataRW[j + 3] << 24));
                j += 4;
              }
              PDO_FROM_SRC_Valid = 1;
            }
            break;
            default:
              break;
            }
          }
          else
          {
            if (Header.b.MessageType == 0x06) /*if request accepted */
            {                                 // TODO: react
              __NOP();
            }
          }
        }
      }
    }
  }
}

/**********************     Read_SNK_PDO(void)   ***************************
This function reads the PDO registers. 

****************************************************************************/

void Read_SNK_PDO(void)
{
  USB_PD_ready();
  //static unsigned char DataRW[12];
  DataRW[0] = 0;

  static uint32_t i, j;

  if (I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, DPM_PDO_NUMB, &DataRW[0], 1) == MSG_OK)
  {

    PDO_SNK_NUMB = (DataRW[0] & 0x03);
    I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, DPM_SNK_PDO1, &DataRW[0], PDO_SNK_NUMB * 4);
    j = 0;
    for (i = 0; i < PDO_SNK_NUMB; i++)
    {
      PDO_SNK[i].d32 = (uint32_t)(DataRW[j] + (DataRW[j + 1] << 8) + (DataRW[j + 2] << 16) + (DataRW[j + 3] << 24));
      j += 4;
    }
  }

  return;
}

/**********************     Read_RDO(void)   ***************************
This function reads the Requested Data Object (RDO) register. 
************************************************************************/
void Read_RDO(void)
{
  USB_PD_ready();
  I2C_Read_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, RDO_REG_STATUS, (uint8_t *)&Nego_RDO.d32, 4);
}

/******************   Update_PDO(PDO_number, Voltage, Current)   *************
This function must be used to overwrite PDO2 or PDO3 content in RAM.
Arguments are:
- PDO Number : 2 or 3 , 
- Voltage in(mV) truncated by 50mV ,
- Current in(mv) truncated by 10mA
************************************************************************************/
void Update_PDO(uint8_t PDO_Number, uint32_t Voltage, uint32_t Current)
{
  uint8_t reg;

  PDO_SNK[PDO_Number - 1].fix.Voltage = Voltage / 50;
  PDO_SNK[PDO_Number - 1].fix.Operationnal_Current = Current / 10;
  if ((PDO_Number == 2) || (PDO_Number == 3))
  {
    reg = DPM_SNK_PDO1 + 4 * (PDO_Number - 1);
    I2C_Write_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, reg, (uint8_t *)&PDO_SNK[PDO_Number - 1].d32, 4);
  }
}

/************* Update_Valid_PDO_Number(PDO_Number)  ***************************
This function is used to overwrite the number of valid PDO
Arguments are:
- active PDO Number: from 1 to 3 
************************************************************************************/

void Update_Valid_PDO_Number(uint8_t Number_PDO)
{
  if (Number_PDO >= 1 && Number_PDO <= 3)
  {
    PDO_SNK_NUMB = Number_PDO;
    I2C_Write_USB_PD(STUSB45DeviceConf.I2cDeviceID_7bit, DPM_PDO_NUMB, &Number_PDO, 1);
  }
}

/****************************     Negotiate_5V(void)    ***************************
Sample function that reconfigures the PDO number to only one, so by default PDO1. 
This drives the STUSB4500 to negotiates 5V back with the SOURCE.
************************************************************************************/

void Negotiate_5V(void)
{
  Update_Valid_PDO_Number(1);
}

/**********************     Find_Matching_SRC_PDO(uint32_t Min_Power,uint32_t Min_V , uint32_t Max_V)   ************************/
/**
* @brief scans the SOURCE PDO (received at connection). If one of the SOURCE PDO
falls within the range of the functions arguments, ie. within a Voltage range and 
Power Range relevant for the applications, then it redefines the SINK_PDO3 with such
PDO parameters and re-negotiates. This allows STUSB4500 to best match to the SOURCE
capabilities.
* @param  I2C Port used (I2C1 or I2C2).
* @param  Min Power  in W 
* @param  Min Voltage in mV
* @param  Max Voltage in mV
* @retval 0 if PDO3 updated 1 if not 
*********************************************************************************************************************************/
uint32_t Find_Matching_SRC_PDO(uint32_t Min_Power, uint32_t Min_V, uint32_t Max_V)
{
  static uint8_t i;
  uint32_t PDO_V;
  uint32_t PDO_I;
  uint32_t PDO_P;
  uint32_t PDO1_updated = 0;

  if (PDO_FROM_SRC_Num > 1)
  {
    for (i = 1; i < PDO_FROM_SRC_Num; i++) // loop started from PDO2
    {
      PDO_V = PDO_FROM_SRC[i].fix.Voltage * 50;
      PDO_I = PDO_FROM_SRC[i].fix.Max_Operating_Current * 10;
      PDO_P = (int)((PDO_V / 1000) * (PDO_I / 1000));
      if ((PDO_P >= Min_Power) && (PDO_V > Min_V) && (PDO_V <= Max_V))
      {
        Update_Valid_PDO_Number(3);
        Update_PDO(3, PDO_V, PDO_I);
        PDO1_updated = 1;
        break;
      }
    }
  }

  if (PDO1_updated)
    return 0;

  return 1;
}

/************ Request_SRC_PDO_NUMBER (uint8_t SRC_PDO_position)   ******************/
/*
* @brief This function copies the SRC_PDO corresponding to the position set in parameter into STUSB4500 PDO2
This allows STUSB4500 to negotiate with the SOURCE on the given PDO index, whatever its Voltage node.
* @param  SRC_PDO_index
* @retval 0 if PDO updated 1 if not 
******************************************************************************************************/
uint32_t Request_SRC_PDO_NUMBER(uint8_t SRC_PDO_position)
{
  uint32_t PDO_V;
  uint32_t PDO_I;
  uint32_t PDO1_updated = 0;

  if (SRC_PDO_position < 1)
  {
    // must be > 1
  }
  else if (SRC_PDO_position == 1)
  {
    Update_Valid_PDO_Number(1);
  }

  else if (SRC_PDO_position <= PDO_FROM_SRC_Num)
  {
    if (PDO_FROM_SRC[SRC_PDO_position - 1].fix.FixedSupply == 0)
    {
      PDO_V = PDO_FROM_SRC[SRC_PDO_position - 1].fix.Voltage * 50;
      PDO_I = PDO_FROM_SRC[SRC_PDO_position - 1].fix.Max_Operating_Current * 10;

      Update_PDO(3, PDO_V, PDO_I);
      PDO1_updated = 1;
      Update_Valid_PDO_Number(3);
    }
    else
    {
      return 1;
    }
  }

  if (PDO1_updated)
    return 0;

  return 1;
}

uint8_t FindHighestSrcPower(void)
{
  static uint8_t i_max_power;
  uint32_t PDO_V;
  uint32_t PDO_I;
  uint32_t PDO_P;

  if (PDO_FROM_SRC_Num > 1)
  {
    for (uint8_t i = 1; i < PDO_FROM_SRC_Num; i++) // loop started from PDO2
    {
      PDO_V = PDO_FROM_SRC[i].fix.Voltage * 50;
      PDO_I = PDO_FROM_SRC[i].fix.Max_Operating_Current * 10;
      uint32_t new_PDO_P = (uint32_t)((PDO_V / 1000) * (PDO_I / 1000));

      if (new_PDO_P > PDO_P)
      {
        PDO_P = new_PDO_P;
        i_max_power = i;
      }
    }
  }

  Request_SRC_PDO_NUMBER(i_max_power + 1);
  return (i_max_power + 1);
}

uint32_t getPdoCurrent(uint8_t pdo)
{
  if (pdo > 0)
  {
    return PDO_FROM_SRC[pdo - 1].fix.Max_Operating_Current * 10;
  }
  else
  {
    return 0;
  }
}

uint32_t getPdoVoltage(uint8_t pdo)
{
  if (pdo > 0)
  {
    return PDO_FROM_SRC[pdo - 1].fix.Voltage * 50;
  }
  else
  {
    return 0;
  }
}
