#ifndef HEATER_H_
#define HEATER_H_

#include "ch.h"

#define HEATER_THREAD_STACK_SIZE 128

extern THD_WORKING_AREA(waHeaterThread, HEATER_THREAD_STACK_SIZE);

typedef struct
{
    double power_safety_margin;
    double resistance;
    double min_temperature;
    double max_temperature;
    double set_temperature;
    double is_temperature;
    double integratedError;
    double error;
    double power_max;
    double pwm;
    double pwm_max;
    double p;
    double i;
    double d;
    binary_semaphore_t bsem;
} heater_t;

extern heater_t heater;

#ifdef __cplusplus
extern "C"
{
#endif
    THD_FUNCTION(heaterThread, arg);
#ifdef __cplusplus
}
#endif

#endif