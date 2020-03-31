#include "lcd.h"
#include "ch.h"

THD_WORKING_AREA(waLcdThread, LCD_THREAD_STACK_SIZE);

THD_FUNCTION(lcdThread, arg) 
{
    (void) arg;
    chRegSetThreadName("lcd");

}
