#ifndef HEATER_H_
#define HEATER_H_

#include "ch.h"

#define HEATER_THREAD_STACK_SIZE 128

extern THD_WORKING_AREA(waHeaterThread, HEATER_THREAD_STACK_SIZE);

/**
 * Maximum ratio that can be set for the heater PWM
 */
#define PWM_MAX_PERCENTAGE 10000

typedef struct
{
    double power_safety_margin; //<<< A number < 1, indicating the maximum amount of power to draw from the supply
    double resistance;          //<<< Heater element resistance
    double voltage;             //<<< Negotiated voltage
    double current;             //<<< Negotiated current
    double min_temperature;     //<<< Minimum heater temperature
    double max_temperature;     //<<< Maximum heater temperature
    double set_temperature;     //<<< Desired heater temperature
    double is_temperature;      //<<< Actual heater temperature
    double integratedError;     //<<< Error from I-component of control loop
    double error;               //<<< Error from P-component of control loop
    double power_max;           //<<< Maximum power that the supply can deliver
    double pwm;                 //<<< Current PWM ratio
    double pwm_max;             //<<< Maximum PWM ratio for not exceeding maximum supply power
    double p;                   //<<< Contol loop P variable
    double i;                   //<<< Contol loop I variable
    double d;                   //<<< Contol loop D variable (currently unused)
    binary_semaphore_t bsem;    //<<< Locking semaphore
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