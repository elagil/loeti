#ifndef USB_PD_CORE_H
#define USB_PD_CORE_H

/* Includes ------------------------------------------------------------------*/

#include "USB_PD_defines.h"
#include "ch.h"

#define USBPD_REV30_SUPPORT

#define LE16(addr) (((uint16_t)(*((uint8_t *)(addr)))) + (((uint16_t)(*(((uint8_t *)(addr)) + 1))) << 8))

#define LE32(addr) ((((uint32_t)(*(((uint8_t *)(addr)) + 0))) +         \
                     (((uint32_t)(*(((uint8_t *)(addr)) + 1))) << 8) +  \
                     (((uint32_t)(*(((uint8_t *)(addr)) + 2))) << 16) + \
                     (((uint32_t)(*(((uint8_t *)(addr)) + 3))) << 24)))

typedef struct
{
  uint8_t I2cDeviceID_7bit;
  uint8_t Dev_Cut;
} USB_PD_I2C_PORT;

typedef struct
{
  uint8_t HW_Reset;
  STUSB_GEN1S_CC_DETECTION_STATUS_RegTypeDef Port_Status; /*!< Specifies the Port status register */
  uint8_t TypeC;
  STUSB_GEN1S_CC_STATUS_RegTypeDef CC_status;
  STUSB_GEN1S_MONITORING_STATUS_RegTypeDef Monitoring_status; /*!< Specifies the  */
  STUSB_GEN1S_HW_FAULT_STATUS_RegTypeDef HWFault_status;
  STUSB_GEN1S_PRT_STATUS_RegTypeDef PRT_status; /*!< Specifies t */
  STUSB_GEN1S_PHY_STATUS_RegTypeDef Phy_status; /*!<  */

} USB_PD_StatusTypeDef;

/** @defgroup USBPD_MsgHeaderStructure_definition USB PD Message header Structure definition
* @brief USB PD Message header Structure definition
* @{
*/
typedef union {
  uint16_t d16;
  struct
  {
#if defined(USBPD_REV30_SUPPORT)
    uint16_t MessageType : /*!< Message Header's message Type                      */
                           5;
#else                       /* USBPD_REV30_SUPPORT */
    uint16_t MessageType : /*!< Message Header's message Type                      */
                           4;
    uint16_t Reserved4 : /*!< Reserved                                           */
                         1;
#endif                      /* USBPD_REV30_SUPPORT */
    uint16_t PortDataRole : /*!< Message Header's Port Data Role                    */
                            1;
    uint16_t SpecificationRevision : /*!< Message Header's Spec Revision                     */
                                     2;
    uint16_t PortPowerRole_CablePlug : /*!< Message Header's Port Power Role/Cable Plug field  */
                                       1;
    uint16_t MessageID : /*!< Message Header's message ID                        */
                         3;
    uint16_t NumberOfDataObjects : /*!< Message Header's Number of data object             */
                                   3;
    uint16_t Extended : /*!< Reserved                                           */
                        1;
  } b;
} USBPD_MsgHeader_TypeDef;

typedef union {
  uint32_t d32;
  struct
  {
    uint32_t Max_Operating_Current : 10;
    uint32_t Voltage : 10;
    uint8_t PeakCurrent : 2;
    uint8_t Reserved : 2;
    uint8_t Unchuncked_Extended : 1;
    uint8_t Dual_RoleData : 1;
    uint8_t Communication : 1;
    uint8_t UnconstraintPower : 1;
    uint8_t SuspendSupported : 1;
    uint8_t DualRolePower : 1;
    uint8_t FixedSupply : 2;
  } fix;
  struct
  {
    uint32_t Operating_Current : 10;
    uint32_t Min_Voltage : 10;
    uint32_t Max_Voltage : 10;
    uint8_t VariableSupply : 2;
  } var;
  struct
  {
    uint32_t Operating_Power : 10;
    uint32_t Min_Voltage : 10;
    uint32_t Max_Voltage : 10;
    uint8_t Battery : 2;
  } bat;
  struct
  {
    /*
        uint8_t Max_Current :7;
        uint8_t  Reserved0 :1;
        uint8_t Min_Voltage:8;
        uint8_t  Reserved1 :1;
        uint8_t Max_Voltage:9;
        uint8_t  Reserved2 :2;
        uint8_t ProgDev : 2 ;
        uint8_t Battery:2; 
        */
    uint8_t Max_Current : 7;
    uint8_t Reserved0 : 1;
    uint16_t Min_Voltage : 8; /* to prevent packing issue ?? */
    uint8_t Reserved1 : 1;
    uint16_t Max_Voltage : 8;
    uint8_t Reserved2 : 3;
    uint8_t ProgDev : 2;
    uint8_t Battery : 2;

  } apdo;
} USB_PD_SRC_PDOTypeDef;

typedef struct
{
  uint8_t PHY;
  uint8_t PRL;
  uint8_t BIST;
  uint8_t PE;
  uint8_t TypeC;
} USB_PD_Debug_FSM_TypeDef;

void typec_connection_status(void);
void HW_Reset_state(void);
void SW_reset_by_Reg(void);
void Send_Soft_reset_Message(void);
void usb_pd_init(void);
void ALARM_MANAGEMENT(void *arg);
void Read_SNK_PDO(void);
void Read_RDO(void);
void Update_PDO(uint8_t PDO_Number, int Voltage, int Current);
void Update_Valid_PDO_Number(uint8_t Number_PDO);
int Find_Matching_SRC_PDO(int Min_Power, int Min_V, int Max_V);
int Request_SRC_PDO_NUMBER(uint8_t SRC_PDO_position);
void Negotiate_5V(void);

#ifdef __cplusplus
}
#endif

#endif /*usbpd core header */

/**
* @}
*/

/**
* @}
*/