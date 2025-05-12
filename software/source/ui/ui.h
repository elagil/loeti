#ifndef UI_H_
#define UI_H_

#include "ch.h"

#define UI_THREAD_STACK_SIZE 64

extern THD_WORKING_AREA(waUiThread, UI_THREAD_STACK_SIZE);

typedef union
{
    uint8_t raw;
    struct
    {
        bool sw0 : 1;
    } id;
} switch_state_t;

typedef struct switches_t
{
    switch_state_t current;
    switch_state_t previous;
    binary_semaphore_t bsem;
} switches_t;

extern switches_t switches;

#ifdef __cplusplus
extern "C"
{
#endif
    THD_FUNCTION(uiThread, arg);
#ifdef __cplusplus
}
#endif

#endif