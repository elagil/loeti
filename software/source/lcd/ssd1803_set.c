#include "ch.h"
#include "ssd1803_reg.h"
#include "ssd1803_def.h"

void ssd1803_decode_instruction(uint16_t code, ssd1803_instruction_t *instruction)
{
    // Set up special register markers
    if (code & SSD1803_SET_RS)
        instruction->rs = true;
    else
        instruction->rs = false;

    // Set re, if required
    if (code & SSD1803_SET_RE0)
    {
        instruction->set_re = true;
        instruction->re = false;
    }
    else if (code & SSD1803_SET_RE1)
    {
        instruction->set_re = true;
        instruction->re = true;
    }
    else
    {
        instruction->set_re = false;
    }

    // Set is, if required
    if (code & SSD1803_SET_IS0)
    {
        instruction->set_is = true;
        instruction->is = false;
    }
    else if (code & SSD1803_SET_IS1)
    {
        instruction->set_is = true;
        instruction->is = true;
    }
    else
    {
        instruction->set_is = false;
    }

    // Store actual instruction payload in the corresponding field
    instruction->payload = code & 0xff;
}

void ssd1803_clear_display(ssd1803_instruction_t *instruction)
{
    ssd1803_decode_instruction(SSD1803_CLEAR_DISPLAY, instruction);
}

void ssd1803_return_home(ssd1803_instruction_t *instruction)
{
    ssd1803_decode_instruction(SSD1803_RETURN_HOME, instruction);
}

void ssd1803_power_down_mode_set(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_POWER_DOWN_MODE |
                    ssd1803_reg->ssd1803_power_down_mode_set_reg->pd << SSD1803_POWER_DOWN;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_entry_mode_set_0(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_ENTRY_MODE_SET_0 |
                    ssd1803_reg->ssd1803_power_down_mode_set_reg->pd << SSD1803_POWER_DOWN;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_entry_mode_set_1(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_ENTRY_MODE_SET_1 |
                    ssd1803_reg->ssd1803_entry_mode_set_reg_1->bdc << SSD1803_ENTRY_MODE_SET_1_BDC |
                    ssd1803_reg->ssd1803_entry_mode_set_reg_1->bds << SSD1803_ENTRY_MODE_SET_1_BDS;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_display_on_off_control(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_DISPLAY_ON_OFF_CONTROL |
                    ssd1803_reg->ssd1803_display_on_off_control_reg->b << SSD1803_DISPLAY_ON_OFF_CONTROL_B |
                    ssd1803_reg->ssd1803_display_on_off_control_reg->c << SSD1803_DISPLAY_ON_OFF_CONTROL_C |
                    ssd1803_reg->ssd1803_display_on_off_control_reg->d << SSD1803_DISPLAY_ON_OFF_CONTROL_D;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_extended_function_set(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_EXTENDED_FUNCTION_SET |
                    ssd1803_reg->ssd1803_extended_function_set_reg->bw << SSD1803_EXTENDED_FUNCTION_SET_BW |
                    ssd1803_reg->ssd1803_extended_function_set_reg->fw << SSD1803_EXTENDED_FUNCTION_SET_FW |
                    ssd1803_reg->ssd1803_extended_function_set_reg->nw << SSD1803_EXTENDED_FUNCTION_SET_NW;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_cursor_or_display_shift(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_CURSOR_OR_DISPLAY_SHIFT |
                    ssd1803_reg->ssd1803_cursor_or_display_shift_reg->rl << SSD1803_CURSOR_OR_DISPLAY_SHIFT_RL |
                    ssd1803_reg->ssd1803_cursor_or_display_shift_reg->sc << SSD1803_CURSOR_OR_DISPLAY_SHIFT_SC;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_double_height(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_DOUBLE_HEIGHT |
                    ssd1803_reg->ssd1803_double_height_reg->bs1 << SSD1803_DOUBLE_HEIGHT_BS1 |
                    ssd1803_reg->ssd1803_double_height_reg->dh << SSD1803_DOUBLE_HEIGHT_DH |
                    ssd1803_reg->ssd1803_double_height_reg->ud1 << SSD1803_DOUBLE_HEIGHT_UD1 |
                    ssd1803_reg->ssd1803_double_height_reg->ud2 << SSD1803_DOUBLE_HEIGHT_UD2;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_internal_osc(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_INTERNAL_OSC |
                    ssd1803_reg->ssd1803_internal_osc_reg->bs0 << SSD1803_INTERNAL_OSC_BS0 |
                    ssd1803_reg->ssd1803_internal_osc_reg->f0 << SSD1803_INTERNAL_OSC_F0 |
                    ssd1803_reg->ssd1803_internal_osc_reg->f1 << SSD1803_INTERNAL_OSC_F1 |
                    ssd1803_reg->ssd1803_internal_osc_reg->f2 << SSD1803_INTERNAL_OSC_F2;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_shift_scroll_enable(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_SHIFT_SCROLL_ENABLE |
                    ssd1803_reg->ssd1803_shift_scroll_enable_reg->s1 << SSD1803_SHIFT_SCROLL_ENABLE_S1 |
                    ssd1803_reg->ssd1803_shift_scroll_enable_reg->s2 << SSD1803_SHIFT_SCROLL_ENABLE_S2 |
                    ssd1803_reg->ssd1803_shift_scroll_enable_reg->s3 << SSD1803_SHIFT_SCROLL_ENABLE_S3 |
                    ssd1803_reg->ssd1803_shift_scroll_enable_reg->s4 << SSD1803_SHIFT_SCROLL_ENABLE_S4;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_function_set_0(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_FUNCTION_SET_0 |
                    ssd1803_reg->ssd1803_function_set_0_reg->dh << SSD1803_FUNCTION_SET_0_DH |
                    ssd1803_reg->ssd1803_function_set_0_reg->dl << SSD1803_FUNCTION_SET_0_DL |
                    ssd1803_reg->ssd1803_function_set_0_reg->is << SSD1803_FUNCTION_SET_0_IS |
                    ssd1803_reg->ssd1803_function_set_0_reg->n << SSD1803_FUNCTION_SET_0_N |
                    ssd1803_reg->ssd1803_function_set_0_reg->re << SSD1803_FUNCTION_SET_0_RE;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_function_set_1(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_FUNCTION_SET_1 |
                    ssd1803_reg->ssd1803_function_set_1_reg->be << SSD1803_FUNCTION_SET_1_BE |
                    ssd1803_reg->ssd1803_function_set_1_reg->dl << SSD1803_FUNCTION_SET_1_DL |
                    ssd1803_reg->ssd1803_function_set_1_reg->rev << SSD1803_FUNCTION_SET_1_REV |
                    ssd1803_reg->ssd1803_function_set_1_reg->n << SSD1803_FUNCTION_SET_1_N |
                    ssd1803_reg->ssd1803_function_set_1_reg->re << SSD1803_FUNCTION_SET_1_RE;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_set_cgram_address(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_SET_CGRAM_ADDRESS |
                    ssd1803_reg->ssd1803_set_cgram_address_reg->ac;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_set_segram_address(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_SET_SEGRAM_ADDRESS |
                    ssd1803_reg->ssd1803_set_segram_address_reg->ac;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_power_set(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_POWER_SET |
                    ssd1803_reg->ssd1803_power_icon_contrast_set_reg->bon << SSD1803_POWER_SET_BON |
                    ssd1803_reg->ssd1803_power_icon_contrast_set_reg->c4 << SSD1803_POWER_SET_C4 |
                    ssd1803_reg->ssd1803_power_icon_contrast_set_reg->c5 << SSD1803_POWER_SET_C5 |
                    ssd1803_reg->ssd1803_power_icon_contrast_set_reg->ion << SSD1803_POWER_SET_ION;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_follower_control(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_FOLLOWER_CONTROL |
                    ssd1803_reg->ssd1803_follower_control_reg->don << SSD1803_FOLLOWER_CONTROL_DON |
                    ssd1803_reg->ssd1803_follower_control_reg->rab0 << SSD1803_FOLLOWER_CONTROL_RAB0 |
                    ssd1803_reg->ssd1803_follower_control_reg->rab1 << SSD1803_FOLLOWER_CONTROL_RAB1 |
                    ssd1803_reg->ssd1803_follower_control_reg->rab2 << SSD1803_FOLLOWER_CONTROL_RAB2;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_contrast_set(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_CONTRAST_SET |
                    ssd1803_reg->ssd1803_contrast_set_reg->c;

    ssd1803_decode_instruction(code, instruction);
}

void ssd1803_set_ddram_address(ssd1803_instruction_t *instruction, ssd1803_reg_t *ssd1803_reg)
{
    uint16_t code = SSD1803_SET_DDRAM_ADDRESS |
                    ssd1803_reg->ssd1803_set_ddram_address_reg->ac;

    ssd1803_decode_instruction(code, instruction);
}
