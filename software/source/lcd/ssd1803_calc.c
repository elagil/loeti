#include "ch.h"
#include "ssd1803_set.h"
#include "ssd1803_def.h"

// Choose number of lines from 1-4
#define LINES 3

// Contrast, goes up to 63
#define CONTRAST 42

// Choose view from TOP or BOTTOM
#define VIEW BOTTOM

void ssd1803_calc_contrast(ssd1803_reg_t *ssd1803_reg, uint8_t contrast)
{
    // set contrast, upper two bit ...
    ssd1803_reg->ssd1803_power_icon_contrast_set_reg->c4 = 1 & (contrast >> 4);
    ssd1803_reg->ssd1803_power_icon_contrast_set_reg->c5 = 1 & (contrast >> 5);
    // set contrast, lower 4 bit
    ssd1803_reg->ssd1803_contrast_set_reg->c = contrast & 0xF;
}

void ssd1803_calc_initialize(ssd1803_reg_t *ssd1803_reg)
{

    ssd1803_reg->ssd1803_function_set_0_reg->dl =
        ssd1803_reg->ssd1803_function_set_1_reg->dl = true; // 8 bit wide transfers

    // set number of lines in the display
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

    // set rotation of the lcd
    if (VIEW == BOTTOM)
    {
        ssd1803_reg->ssd1803_entry_mode_set_reg_1->bdc = true;
        ssd1803_reg->ssd1803_entry_mode_set_reg_1->bds = false;
    }
    else
    {
        ssd1803_reg->ssd1803_entry_mode_set_reg_1->bdc = false;
        ssd1803_reg->ssd1803_entry_mode_set_reg_1->bds = true;
    }

    // set bias of voltage divider
    ssd1803_reg->ssd1803_internal_osc_reg->bs0 = true; // bias of 1/6

    // set oscillator frequency
    ssd1803_reg->ssd1803_internal_osc_reg->f0 = true; // oscillator set to 540 kHz
    ssd1803_reg->ssd1803_internal_osc_reg->f1 = false;
    ssd1803_reg->ssd1803_internal_osc_reg->f2 = false;

    // set lcd driving voltage end enable internal divider:  1+Rb/Ra = 5.3
    ssd1803_reg->ssd1803_follower_control_reg->rab0 = false;
    ssd1803_reg->ssd1803_follower_control_reg->rab1 = true;
    ssd1803_reg->ssd1803_follower_control_reg->don = true;
    ssd1803_reg->ssd1803_follower_control_reg->rab0 = true;

    // enable dcdc converter and regulator circuit
    ssd1803_reg->ssd1803_power_icon_contrast_set_reg->bon = true;
    ssd1803_reg->ssd1803_power_icon_contrast_set_reg->ion = false;

    // set contrast
    ssd1803_calc_contrast(ssd1803_reg, CONTRAST);

    ssd1803_reg->ssd1803_extended_function_set_reg->bw = false; // no black/white inversion
    ssd1803_reg->ssd1803_extended_function_set_reg->fw = false; // 5 dot font width

    ssd1803_reg->ssd1803_double_height_reg->ud1 = false; // middle line is double height
    ssd1803_reg->ssd1803_double_height_reg->ud2 = true;

    // select rom A
    ssd1803_reg->ssd1803_rom_selection_set_reg->rom1 = 0;
    ssd1803_reg->ssd1803_rom_selection_set_reg->rom2 = 0;

    ssd1803_reg->ssd1803_display_on_off_control_reg->d = true;  // switch on display
    ssd1803_reg->ssd1803_display_on_off_control_reg->c = false; // switch off cursor
    ssd1803_reg->ssd1803_display_on_off_control_reg->b = false; // switch off blinking
}