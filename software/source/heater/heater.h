#ifndef HEATER_H_
#define HEATER_H_

#include "ch.h"

#define HEATER_LEVEL_COUNT 2
#define DEFAULT_HEATER_LEVEL 0

extern const double heater_levels[HEATER_LEVEL_COUNT];
extern uint32_t heater_level;

#define LOOP_TIME_RATIO 10
#define LOOP_TIME_TEMPERATURE_MS 100
#define LOOP_TIME_CURRENT_MS (LOOP_TIME_TEMPERATURE_MS / LOOP_TIME_RATIO)

#if LOOP_TIME_CURRENT_MS < 5
#error "Current loop too fast. 5 ms of settling time are required for low-pass filtering."
#endif

#define MS2S(x) ((double)x / 1000.0)

#ifdef WMRP
#define HEATER_RESISTANCE 2.1
#endif

#ifdef C210
#define HEATER_RESISTANCE 2.5
#endif

#ifdef C245
#define HEATER_RESISTANCE 3
#endif

#define HEATER_CURRENT_LIMIT 0.9
#define HEATER_CURRENT_P 0
#define HEATER_CURRENT_I_SCALE 0.5
#define HEATER_CURRENT_I (HEATER_CURRENT_I_SCALE * HEATER_RESISTANCE / (2 * MS2S(LOOP_TIME_CURRENT_MS)))

#ifdef C210
#define HEATER_TEMPERATURE_P 0.025
#define HEATER_TEMPERATURE_I (0.005 / (MS2S(LOOP_TIME_TEMPERATURE_MS)))
#define HEATER_TEMPERATURE_D (0 * (MS2S(LOOP_TIME_TEMPERATURE_MS)))
#endif

#ifdef C245
#define HEATER_TEMPERATURE_P 0.2
#define HEATER_TEMPERATURE_I (0.005 / (MS2S(LOOP_TIME_TEMPERATURE_MS)))
#define HEATER_TEMPERATURE_D (0.2 * (MS2S(LOOP_TIME_TEMPERATURE_MS)))
#endif

#ifdef WMRP
#define HEATER_TEMPERATURE_P 0.05
#define HEATER_TEMPERATURE_I (0.00025 / (MS2S(LOOP_TIME_TEMPERATURE_MS)))
#define HEATER_TEMPERATURE_D (0 * (MS2S(LOOP_TIME_TEMPERATURE_MS)))
#endif

// Ratios as defined by resistors and inherent gains of the parts
#define VOLTAGE_SENSE_RATIO 11
#define CURRENT_SENSE_RATIO 5

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
    double current_target;     //<<< The target current, slightly below negotiated current
    double power_negotiated;   //<<< Maximum power that the supply can deliver
    double voltage_meas;       //<<< Measured voltage
    double current_meas;       //<<< Measured current
    double pwm;                //<<< Current PWM ratio
    double pwm_max;            //<<< Maximum PWM ratio that is settable
} power_t;

typedef struct
{
    double min; //<<< Minimum heater temperature
    double max; //<<< Maximum heater temperature
} temperatures_t;

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
    power_t power;  //<<< Heater power structure
    temperatures_t temperatures;
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