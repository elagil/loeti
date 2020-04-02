#include "lcd.h"
#include "ch.h"
#include "ssd1803_calc.h"
#include "ssd1803_def.h"

ssd1803_reg_t ssd1803_reg;
ssd1803_state_t ssd1803_state;

THD_WORKING_AREA(waLcdThread, LCD_THREAD_STACK_SIZE);

void writeRegister(uint16_t buffer)
{
}

void writeInstruction(ssd1803_instruction_t *instruction)
{
    ssd1803_instruction_t intermediate_instruction;

    // IS has to be adjusted first ...
    // Only set is, if it was changed, or the instruction requires it
    if ((ssd1803_state.is != instruction->is) && instruction->set_is)
    {
        // update state structure
        ssd1803_state.is = instruction->is;

        ssd1803_reg.ssd1803_function_set_0_reg->is = instruction->is;
        ssd1803_function_set_0(&intermediate_instruction, &ssd1803_reg);
        writeInstruction(&intermediate_instruction);
    }

    // ... afterwards, RE.
    // Only set re, if it was changed, or the instruction requires it
    if ((ssd1803_state.re != instruction->re) && instruction->set_re)
    {
        bool previous_re = ssd1803_state.re;

        // update state structure
        ssd1803_state.re = instruction->re;

        // re has to be adjusted in function_set_0 or _1, depending on its previous state
        if (previous_re == false)
        {
            ssd1803_reg.ssd1803_function_set_0_reg->re = instruction->re;
            ssd1803_function_set_0(&intermediate_instruction, &ssd1803_reg);
            writeInstruction(&intermediate_instruction);
        }
        else
        {
            ssd1803_reg.ssd1803_function_set_1_reg->re = instruction->re;
            ssd1803_function_set_1(&intermediate_instruction, &ssd1803_reg);
            writeInstruction(&intermediate_instruction);
        }
    }

    uint16_t buffer = (instruction->rs << 9) | (instruction->rw << 8) | instruction->payload;
    writeRegister(buffer);
}

THD_FUNCTION(lcdThread, arg)
{
    (void)arg;
    ssd1803_state.row = 0;
    ssd1803_state.col = 0;
    ssd1803_state.is = false;
    ssd1803_state.re = false;
    ssd1803_state.rw = false;

    chRegSetThreadName("lcd");

    ssd1803_calc_initialize(&ssd1803_reg);
}
