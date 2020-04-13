#include "lcd.h"

#include "ch.h"
#include "hal.h"

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
    ssd1803_state.re = true;

    chRegSetThreadName("lcd");

    palSetLine(LINE_LCD_NRST);
    chThdSleepMilliseconds(1);
    palClearLine(LINE_LCD_NRST);
    chThdSleepMilliseconds(1);
    palSetLine(LINE_LCD_NRST);

    ssd1803_initialize();

    ssd1803_move_to_line(0);

    uint8_t juhu[] = "Moin Gurli";
    ssd1803_writeByteArray(juhu, 10);

    ssd1803_move_to_line(1);

    uint8_t juhu2[] = "289";
    ssd1803_writeByteArray(juhu2, 3);

    ssd1803_move_to_line(2);

    uint8_t juhu3[] = "\xd6\xd6\xd6\xd7";
    ssd1803_writeByteArray(juhu3, 4);

    uint8_t cnt;

    while (true)
    {
        cnt++;
        chThdSleepMilliseconds(100);
    }
}
