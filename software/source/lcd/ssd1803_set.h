#ifndef SSD1803_REG_H_
#define SSD1803_REG_H_

#include "ssd1803_def.h"

void ssd1803_decode_instruction(uint16_t code, ssd1803_instruction_t * instruction);
void ssd1803_clear_display(ssd1803_instruction_t * instruction);
void ssd1803_return_home(ssd1803_instruction_t * instruction);
void ssd1803_power_down_mode_set(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_entry_mode_set_0(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_entry_mode_set_1(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_display_on_off_control(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_extended_function_set(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_cursor_or_display_shift(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_double_height(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_internal_osc(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_shift_scroll_enable(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_function_set_0(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_function_set_1(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_set_cgram_address(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_set_segram_address(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_power_set(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_follower_control(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_contrast_set(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);
void ssd1803_set_ddram_address(ssd1803_instruction_t * instruction, ssd1803_reg_t * ssd1803_reg);

#endif