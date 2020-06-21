#ifndef USB_PD_H_
#define USB_PD_H_

#include "ch.h"

#define USB_PD_THREAD_STACK_SIZE 256

extern THD_WORKING_AREA(waUsbPdThread, USB_PD_THREAD_STACK_SIZE);

#ifdef __cplusplus
extern "C"
{
#endif
    THD_FUNCTION(usbPdThread, arg);
#ifdef __cplusplus
}
#endif

#endif