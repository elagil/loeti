#ifndef LCD_H_
#define LCD_H_

#include "ch.h"

#define LCD_THREAD_STACK_SIZE 512

extern THD_WORKING_AREA(waLcdThread, LCD_THREAD_STACK_SIZE);

#ifdef __cplusplus
extern "C"
{
#endif
    THD_FUNCTION(lcdThread, arg);
#ifdef __cplusplus
}
#endif

#endif