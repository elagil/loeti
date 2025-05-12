#ifndef SENSOR_H_
#define SENSOR_H_

#include "ch.h"

#define SENSOR_THREAD_STACK_SIZE 256

extern THD_WORKING_AREA(waSensorThread, SENSOR_THREAD_STACK_SIZE);

extern event_source_t temp_event;

#define ADC_REF_VOLTAGE 3.3
#define ADC_FS_READING 4096
#define ADC_FS_MARGIN 100
#define ADC_TO_VOLT(x) ((double)x / (double)ADC_FS_READING * (double)ADC_REF_VOLTAGE)

typedef union
{
    int16_t value;
    uint8_t array[2];
} sensor_data_t;

extern sensor_data_t local_temp_sensor_data;
extern sensor_data_t iron_temp_sensor_data;

#ifdef __cplusplus
extern "C"
{
#endif
    THD_FUNCTION(sensorThread, arg);
#ifdef __cplusplus
}
#endif

#endif