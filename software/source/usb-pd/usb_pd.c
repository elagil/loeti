#include "usb_pd.h"
#include "ch.h"

THD_WORKING_AREA(waUsbPdThread, USB_PD_THREAD_STACK_SIZE);

THD_FUNCTION(usbPdThread, arg) 
{
    (void) arg;
    chRegSetThreadName("usb pd");

}
