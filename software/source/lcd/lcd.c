#include "lcd.h"
#include "ch.h"
#include "ssd1803_calc.h"
#include "ssd1803_def.h"

ssd1803_reg_t ssd1803_reg;

THD_WORKING_AREA(waLcdThread, LCD_THREAD_STACK_SIZE);

THD_FUNCTION(lcdThread, arg) 
{
    (void) arg;
    chRegSetThreadName("lcd");

    ssd1803_calc_initialize(&ssd1803_reg);

}
