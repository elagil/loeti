#include "lcd.h"

#include "ch.h"

#include "ssd1803_reg.h"
#include "ssd1803_ctrl.h"
#include "ssd1803_set.h"
#include "ssd1803_def.h"

THD_WORKING_AREA(waLcdThread, LCD_THREAD_STACK_SIZE);

THD_FUNCTION(lcdThread, arg)
{
    (void)arg;
    ssd1803_state.row = 0;
    ssd1803_state.col = 0;
    ssd1803_state.is = false;
    ssd1803_state.re = false;

    chRegSetThreadName("lcd");

    ssd1803_initialize();

    uint8_t juhu[] = "hi sophi.";
    ssd1803_writeString(juhu, 9);
}
