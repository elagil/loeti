#ifndef HEATER_H_
#define HEATER_H_

#include "ch.h"

#define TEMPERATURE_SET_INTERVAL 10
#define LOOP_TIME_RATIO 20
#define LOOP_TIME_TEMPERATURE_MS 100
#define LOOP_TIME_CURRENT_MS (LOOP_TIME_TEMPERATURE_MS / LOOP_TIME_RATIO)

#if LOOP_TIME_CURRENT_MS < 5
#error "Current loop too fast. 5 ms of settling time are required for low-pass filtering."
#endif

#define MS2S(x) ((double)x / 1000.0)

#ifdef C210
#define HEATER_RESISTANCE 2.5
#endif

#ifdef C245
#define HEATER_RESISTANCE 3
#endif

#define HEATER_CURRENT_P 0
#define HEATER_CURRENT_I_SCALE 0.5
#define HEATER_CURRENT_I (HEATER_CURRENT_I_SCALE * HEATER_RESISTANCE / (2 * MS2S(LOOP_TIME_CURRENT_MS)))

#ifdef C210
#define HEATER_TEMPERATURE_P 0.007
#define HEATER_TEMPERATURE_I (0.0005 / (MS2S(LOOP_TIME_TEMPERATURE_MS)))
#endif

#ifdef C245
#define HEATER_TEMPERATURE_P 0.2
#define HEATER_TEMPERATURE_I (0.01 / (MS2S(LOOP_TIME_TEMPERATURE_MS)))
#define HEATER_TEMPERATURE_D (0 * (MS2S(LOOP_TIME_TEMPERATURE_MS)))
#endif

#define VOLTAGE_SENSE_RATIO 11
#define CURRENT_SENSE_RATIO 5
#define ADC_REF_VOLTAGE 3.3
#define ADC_FS_READING 4096
#define ADC_TO_VOLT(x) ((double)x / (double)ADC_FS_READING * (double)ADC_REF_VOLTAGE)

#define HEATER_THREAD_STACK_SIZE 128

extern THD_WORKING_AREA(waHeaterThread, HEATER_THREAD_STACK_SIZE);

/**
 * Maximum ratio that can be set for the heater PWM
 */
#define PWM_MAX_PERCENTAGE 10000
typedef struct
{
    double current_offset;     //<<< Offset current without load
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
    double error_last;      //<<< Last error value
    double p;               //<<< Contol loop P variable
    double i;               //<<< Contol loop I variable
    double d;               //<<< Contol loop D variable
} pid_t;
typedef struct
{
    bool sleep;     //<<< True, if heater is in sleep mode
    bool connected; //<<< True, if heater is connected to the station
    heater_power_t power;
    heater_temperatures_t temperatures;
    pid_t temperature_control;
    pid_t current_control;
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