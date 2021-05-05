#ifndef SENSOR_H_
#define SENSOR_H_

#include "ch.h"

#define SENSOR_THREAD_STACK_SIZE 256

extern THD_WORKING_AREA(waSensorThread, SENSOR_THREAD_STACK_SIZE);

extern event_source_t temp_event;

union sensor_data_t
{
    int16_t value;
    uint8_t array[2];
};

extern union sensor_data_t sensor_data;

#ifdef __cplusplus
extern "C"
{
#endif
    THD_FUNCTION(sensorThread, arg);
#ifdef __cplusplus
}
#endif

#endif