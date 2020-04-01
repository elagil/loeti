#include "ch.h"
#include "ssd1803_set.h"
#include "ssd1803_def.h"

// Choose number of lines from 1-4
#define LINES 3

// Choose view from TOP or BOTTOM
#define VIEW BOTTOM

void ssd1803_calc_initialize(ssd1803_reg_t *ssd1803_reg)
{

    ssd1803_reg->ssd1803_function_set_0_reg->dl =
        ssd1803_reg->ssd1803_function_set_1_reg->dl = true; // 8 bit wide transfers

    if (LINES == 1 || LINES == 3)
    {
        ssd1803_reg->ssd1803_function_set_0_reg->n =
            ssd1803_reg->ssd1803_function_set_1_reg->n = false; // 1 or 3-line display option
    }
    else
    {
        ssd1803_reg->ssd1803_function_set_0_reg->n =
            ssd1803_reg->ssd1803_function_set_1_reg->n = false; // 2 or 4-line display option
    }

    if (LINES == 1 || LINES == 2)
    {
        ssd1803_reg->ssd1803_extended_function_set_reg->nw = false; // 1 or 2 line option
    }
    else
    {
        ssd1803_reg->ssd1803_extended_function_set_reg->nw = true; // 3 or 4 line option
    }

    ssd1803_reg->ssd1803_extended_function_set_reg->bw = false; // no black/white inversion
    ssd1803_reg->ssd1803_extended_function_set_reg->fw = false; // 5 dot font width
}