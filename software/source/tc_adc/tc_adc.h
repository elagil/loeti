#ifndef TC_ADC_H_
#define TC_ADC_H_

#include "ch.h"

#define ADC_THREAD_STACK_SIZE 256

extern THD_WORKING_AREA(waAdcThread, ADC_THREAD_STACK_SIZE);

#ifdef __cplusplus
extern "C"
{
#endif
    THD_FUNCTION(adcThread, arg);
#ifdef __cplusplus
}
#endif

#endif