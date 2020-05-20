#ifndef HEATER_H_
#define HEATER_H_

#include "ch.h"

#define TEMPERATURE_SET_INTERVAL 10
#define LOOP_TIME_RATIO 50
#define LOOP_TIME_TEMPERATURE_MS 100
#define LOOP_TIME_CURRENT_MS (LOOP_TIME_TEMPERATURE_MS / LOOP_TIME_RATIO)

#define HEATER_THREAD_STACK_SIZE 128

extern THD_WORKING_AREA(waHeaterThread, HEATER_THREAD_STACK_SIZE);

/**
 * Maximum ratio that can be set for the heater PWM
 */
#define PWM_MAX_PERCENTAGE 10000
typedef struct
{
    double voltage_negotiated; //<<< Negotiated voltage
    double current_negotiated; //<<< Negotiated current
    double power_negotiated;   //<<< Maximum power that the supply can deliver
    double voltage_meas;       //<<< Measured voltage
    double current_meas;       //<<< Measured current
    double pwm;                //<<< Current PWM ratio
    double pwm_max;            //<<< Maximum PWM ratio that is settable
} heater_power_t;

typedef struct
{
    double min;   //<<< Minimum heater temperature
    double max;   //<<< Maximum heater temperature
    double local; //<<< Local temperature of station
} heater_temperatures_t;

typedef struct
{
    double is;
    double set;
    double integratedError; //<<< Error from I-component of control loop
    double error;           //<<< Error from P-component of control loop
    double p;               //<<< Contol loop P variable
    double i;               //<<< Contol loop I variable
} pi_t;
typedef struct
{
    bool sleep;     //<<< True, if heater is in sleep mode
    bool connected; //<<< True, if heater is connected to the station
    heater_power_t power;
    heater_temperatures_t temperatures;
    pi_t temperature_control;
    pi_t current_control;
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