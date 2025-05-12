#ifndef DIAG_H_
#define DIAG_H_

#include "ch.h"

#define DIAG_THREAD_STACK_SIZE 256

extern THD_WORKING_AREA(waDiagThread, DIAG_THREAD_STACK_SIZE);

#ifdef __cplusplus
extern "C"
{
#endif
    THD_FUNCTION(diagThread, arg);
#ifdef __cplusplus
}
#endif

#endif