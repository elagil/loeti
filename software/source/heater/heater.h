#ifndef HEATER_H_
#define HEATER_H_

#include "ch.h"

#define LOOP_TIME 200
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
    double max;                 //<<< Maximum power that the supply can deliver
    double pwm;                 //<<< Current PWM ratio
    double pwm_max;             //<<< Maximum PWM ratio for not exceeding maximum supply power
} heater_power_t;

typedef struct
{
    double min;            //<<< Minimum heater temperature
    double max;            //<<< Maximum heater temperature
    double set;            //<<< Desired heater temperature
    double is_temperature; //<<< Actual heater temperature
    double local;          //<<< Local temperature of station
} heater_temperatures_t;

typedef struct
{
    double integratedError; //<<< Error from I-component of control loop
    double error;           //<<< Error from P-component of control loop
    double p;               //<<< Contol loop P variable
    double i;               //<<< Contol loop I variable
    double d;               //<<< Contol loop D variable (currently unused)
    double i_per_W;         //<<< Contol loop I variable, per Watt
} heater_control_t;
typedef struct
{
    bool connected; //<<< True, if heater is connected to the station
    heater_power_t power;
    heater_temperatures_t temperatures;
    heater_control_t control;
    binary_semaphore_t bsem; //<<< Locking semaphore
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