#ifndef HEATER_H_
#define HEATER_H_

#define HEATER_THREAD_STACK_SIZE 256

extern THD_WORKING_AREA(waHeaterThread, HEATER_THREAD_STACK_SIZE);

#ifdef __cplusplus
extern "C"
{
#endif
    THD_FUNCTION(heaterThread, arg);
#ifdef __cplusplus
}
#endif

#endif