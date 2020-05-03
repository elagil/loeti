#include "ch.h"
#include "hal.h"
#include "spiHelper.h"

#include "ssd1803_ctrl.h"
#include "ssd1803_set.h"
#include "ssd1803_def.h"
#include "ssd1803_reg.h"

// Choose number of lines from 1-4
#define LINES 3

// Contrast, goes up to 63
#define CONTRAST 42

// Choose view from TOP or BOTTOM
#define VIEW BOTTOM

/*
 * SPI configuration (1/64 f_pclk, CPHA=1, CPOL=1, 8 bit, LSB first).
 */
static const SPIConfig lcd_spicfg = {
    false,                                                                        // circular buffer mode
    NULL,                                                                         // end callback
    GPIOA,                                                                        // chip select port
    GPIOA_SPI1_NSS2,                                                              // chip select line
    SPI_CR1_CPHA | SPI_CR1_CPOL | SPI_CR1_BR_2 | SPI_CR1_BR_0 | SPI_CR1_LSBFIRST, // CR1 settings
    SPI_CR2_DS_2 | SPI_CR2_DS_1 | SPI_CR2_DS_0                                    // CR2 settings
};

bool ssd1803_busy(void);

void writeLcdRegLong(uint8_t *buf, uint32_t bytes)
{
    spiExchangeHelper(&SPID1, &lcd_spicfg, bytes, buf, NULL);
}

#define readLcdRegLong(buf, bytes) spiExchangeHelper(&SPID1, &lcd_spicfg, bytes, buf, buf + 2)
#define readLcdReg(buf) readLcdRegLong(buf, 2)
#define writeLcdReg(buf) writeLcdRegLong(buf, 3)

ssd1803_reg_t ssd1803_reg;
ssd1803_power_down_mode_set_reg_t ssd1803_power_down_mode_set_reg;
ssd1803_entry_mode_set_reg_0_t ssd1803_entry_mode_set_reg_0;
ssd1803_entry_mode_set_reg_1_t ssd1803_entry_mode_set_reg_1;
ssd1803_display_on_off_control_reg_t ssd1803_display_on_off_control_reg;
ssd1803_extended_function_set_reg_t ssd1803_extended_function_set_reg;
ssd1803_cursor_or_display_shift_reg_t ssd1803_cursor_or_display_shift_reg;
ssd1803_double_height_reg_t ssd1803_double_height_reg;
ssd1803_internal_osc_reg_t ssd1803_internal_osc_reg;
ssd1803_shift_scroll_enable_reg_t ssd1803_shift_scroll_enable_reg;
ssd1803_function_set_0_reg_t ssd1803_function_set_0_reg;
ssd1803_function_set_1_reg_t ssd1803_function_set_1_reg;
ssd1803_set_cgram_address_reg_t ssd1803_set_cgram_address_reg;
ssd1803_set_segram_address_reg_t ssd1803_set_segram_address_reg;
ssd1803_power_icon_contrast_set_reg_t ssd1803_power_icon_contrast_set_reg;
ssd1803_follower_control_reg_t ssd1803_follower_control_reg;
ssd1803_contrast_set_reg_t ssd1803_contrast_set_reg;
ssd1803_set_ddram_address_reg_t ssd1803_set_ddram_address_reg;
ssd1803_rom_selection_set_reg_t ssd1803_rom_selection_set_reg;

ssd1803_state_t ssd1803_state;

ssd1803_instruction_t instruction;
ssd1803_instruction_t intermediate_instruction;

uint8_t buf_instruction[64];
uint8_t buf_intermediate_instruction[4];

void setRe(bool val)
{
    if (ssd1803_state.re != val)
    {
        if (ssd1803_state.re)
        {
            ssd1803_reg.ssd1803_function_set_1_reg->re = val;
            ssd1803_function_set_1(&intermediate_instruction, &ssd1803_reg);
            writeLcdReg(intermediate_instruction.payload);
        }
        else
        {
            ssd1803_reg.ssd1803_function_set_1_reg->re = val;
            ssd1803_function_set_1(&intermediate_instruction, &ssd1803_reg);
            writeLcdReg(intermediate_instruction.payload);
        }

        ssd1803_state.re = val;
    }
}

void setIs(bool val)
{
    if (ssd1803_state.is != val)
    {
        setRe(false);

        ssd1803_reg.ssd1803_function_set_0_reg->is = val;
        ssd1803_function_set_0(&intermediate_instruction, &ssd1803_reg);
        writeLcdReg(intermediate_instruction.payload);

        ssd1803_state.is = val;
    }
}

void writeInstruction(ssd1803_instruction_t *instruction)
{
    // Only set is, if the instruction requires it
    if (instruction->set_is)
    {
        setIs(instruction->is);
    }

    // Only set re, if the instruction requires it
    if (instruction->set_re)
    {
        setRe(instruction->re);
    }

    writeLcdRegLong(instruction->payload, instruction->payload_length);
}

bool ssd1803_busy(void)
{
    return false;

    ssd1803_busy_addr_cnt(&instruction);

    readLcdRegLong(instruction.payload, instruction.payload_length);

    if (*(instruction.payload + 3) & 0x80)
    {
        return true;
    }
    else
    {
        return false;
    }
}

void ssd1803_contrast(uint8_t contrast)
{
    // set contrast, upper two bit ...
    ssd1803_reg.ssd1803_power_icon_contrast_set_reg->c4 = 1 & (contrast >> 4);
    ssd1803_reg.ssd1803_power_icon_contrast_set_reg->c5 = 1 & (contrast >> 5);
    // set contrast, lower 4 bit
    ssd1803_reg.ssd1803_contrast_set_reg->c = contrast & 0xF;
}

void ssd1803_initialize(void)
{
    chBSemObjectInit(&instruction.bsem, true);
    chBSemObjectInit(&intermediate_instruction.bsem, true);

    instruction.payload = buf_instruction;

    intermediate_instruction.payload = buf_intermediate_instruction;
    intermediate_instruction.payload_length = 1;

    chBSemSignal(&instruction.bsem);
    chBSemSignal(&intermediate_instruction.bsem);

    ssd1803_reg.ssd1803_power_down_mode_set_reg = &ssd1803_power_down_mode_set_reg;
    ssd1803_reg.ssd1803_entry_mode_set_reg_0 = &ssd1803_entry_mode_set_reg_0;
    ssd1803_reg.ssd1803_entry_mode_set_reg_1 = &ssd1803_entry_mode_set_reg_1;
    ssd1803_reg.ssd1803_display_on_off_control_reg = &ssd1803_display_on_off_control_reg;
    ssd1803_reg.ssd1803_extended_function_set_reg = &ssd1803_extended_function_set_reg;
    ssd1803_reg.ssd1803_cursor_or_display_shift_reg = &ssd1803_cursor_or_display_shift_reg;
    ssd1803_reg.ssd1803_double_height_reg = &ssd1803_double_height_reg;
    ssd1803_reg.ssd1803_internal_osc_reg = &ssd1803_internal_osc_reg;
    ssd1803_reg.ssd1803_shift_scroll_enable_reg = &ssd1803_shift_scroll_enable_reg;
    ssd1803_reg.ssd1803_function_set_0_reg = &ssd1803_function_set_0_reg;
    ssd1803_reg.ssd1803_function_set_1_reg = &ssd1803_function_set_1_reg;
    ssd1803_reg.ssd1803_set_cgram_address_reg = &ssd1803_set_cgram_address_reg;
    ssd1803_reg.ssd1803_set_segram_address_reg = &ssd1803_set_segram_address_reg;
    ssd1803_reg.ssd1803_power_icon_contrast_set_reg = &ssd1803_power_icon_contrast_set_reg;
    ssd1803_reg.ssd1803_follower_control_reg = &ssd1803_follower_control_reg;
    ssd1803_reg.ssd1803_contrast_set_reg = &ssd1803_contrast_set_reg;
    ssd1803_reg.ssd1803_set_ddram_address_reg = &ssd1803_set_ddram_address_reg;
    ssd1803_reg.ssd1803_rom_selection_set_reg = &ssd1803_rom_selection_set_reg;

    ssd1803_reg.ssd1803_function_set_0_reg->dl =
        ssd1803_reg.ssd1803_function_set_1_reg->dl = true; // 8 bit wide transfers

    ssd1803_reg.ssd1803_function_set_0_reg->dh = true; // Enable double height fonts

    // set number of lines in the display
    if (LINES == 1 || LINES == 2)
    {
        ssd1803_reg.ssd1803_function_set_0_reg->n =
            ssd1803_reg.ssd1803_function_set_1_reg->n = false; // 1 or 2-line display option
    }
    else
    {
        ssd1803_reg.ssd1803_function_set_0_reg->n =
            ssd1803_reg.ssd1803_function_set_1_reg->n = true; // 3 or 4-line display option
    }

    if (LINES == 1 || LINES == 2)
    {
        ssd1803_reg.ssd1803_extended_function_set_reg->nw = false; // 1 or 2 line option
    }
    else
    {
        ssd1803_reg.ssd1803_extended_function_set_reg->nw = true; // 3 or 4 line option
    }

    ssd1803_reg.ssd1803_extended_function_set_reg->bw = false; // no black/white inversion
    ssd1803_reg.ssd1803_extended_function_set_reg->fw = false; // 5 dot font width

    // set rotation of the lcd
    if (VIEW == BOTTOM)
    {
        ssd1803_reg.ssd1803_entry_mode_set_reg_1->bdc = true;
        ssd1803_reg.ssd1803_entry_mode_set_reg_1->bds = false;
    }
    else
    {
        ssd1803_reg.ssd1803_entry_mode_set_reg_1->bdc = false;
        ssd1803_reg.ssd1803_entry_mode_set_reg_1->bds = true;
    }

    // set bias of voltage divider
    ssd1803_reg.ssd1803_internal_osc_reg->bs0 = true; // bias of 1/6

    // set oscillator frequency
    ssd1803_reg.ssd1803_internal_osc_reg->f0 = true; // oscillator set to 540 kHz
    ssd1803_reg.ssd1803_internal_osc_reg->f1 = true;
    ssd1803_reg.ssd1803_internal_osc_reg->f2 = false;

    // set lcd driving voltage end enable internal divider:  1+Rb/Ra = 5.3
    ssd1803_reg.ssd1803_follower_control_reg->rab0 = false;
    ssd1803_reg.ssd1803_follower_control_reg->rab1 = true;
    ssd1803_reg.ssd1803_follower_control_reg->rab2 = true;
    ssd1803_reg.ssd1803_follower_control_reg->don = true;

    // enable dcdc converter and regulator circuit
    ssd1803_reg.ssd1803_power_icon_contrast_set_reg->bon = true;
    ssd1803_reg.ssd1803_power_icon_contrast_set_reg->ion = false;

    ssd1803_reg.ssd1803_double_height_reg->dh = true;
    ssd1803_reg.ssd1803_double_height_reg->bs1 = true; // bias of 1/6
    ssd1803_reg.ssd1803_double_height_reg->ud1 = true; // middle line is double height
    ssd1803_reg.ssd1803_double_height_reg->ud2 = false;

    // select rom A
    ssd1803_reg.ssd1803_rom_selection_set_reg->rom1 = false;
    ssd1803_reg.ssd1803_rom_selection_set_reg->rom2 = false;

    ssd1803_reg.ssd1803_display_on_off_control_reg->d = true;  // switch on display
    ssd1803_reg.ssd1803_display_on_off_control_reg->c = false; // switch off cursor
    ssd1803_reg.ssd1803_display_on_off_control_reg->b = false; // switch off blinking

    // set contrast
    ssd1803_contrast(CONTRAST);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    ssd1803_clear_display(&instruction);
    writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    ssd1803_function_set_1(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    ssd1803_function_set_0(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    //ssd1803_rom_selection(&instruction);
    //writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    // ssd1803_rom_selection_set(&instruction, &ssd1803_reg);
    //writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    ssd1803_extended_function_set(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    ssd1803_entry_mode_set_1(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    ssd1803_double_height(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    ssd1803_internal_osc(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    ssd1803_follower_control(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    ssd1803_power_set(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    ssd1803_contrast_set(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }
    ssd1803_display_on_off_control(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);
}

void ssd1803_move_home(void)
{
    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }

    ssd1803_return_home(&instruction);
    writeInstruction(&instruction);
}

void ssd1803_move_to_line(uint8_t line)
{
    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }

    if (VIEW == BOTTOM)
    {
        ssd1803_reg.ssd1803_set_ddram_address_reg->ac = SSD1803_DDRAM_ADR_BOT + line * SSD1803_DDRAM_ADR_OFFSET;
    }
    else if (VIEW == TOP)
    {
        ssd1803_reg.ssd1803_set_ddram_address_reg->ac = SSD1803_DDRAM_ADR_TOP + line * SSD1803_DDRAM_ADR_OFFSET;
    }

    ssd1803_set_ddram_address(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);
}

void ssd1803_writeByte(uint8_t c)
{
    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }

    ssd1803DecodeInstruction(SSD1803_SET_RS | c, &instruction);
    writeInstruction(&instruction);
}

void ssd1803_writeByteArray(uint8_t *s, uint32_t length)
{
    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }

    ssd1803Decode(s, length, SSD1803_SET_RS, &instruction);
    writeInstruction(&instruction);
}

void ssd1803_clear(void)
{
    while (ssd1803_busy())
    {
        chThdSleepMilliseconds(10);
    }

    ssd1803_clear_display(&instruction);
    writeInstruction(&instruction);
}