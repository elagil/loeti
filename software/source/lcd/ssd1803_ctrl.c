#include "ch.h"
#include "hal.h"
#include "spi.h"

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

#define writeLcdRegister(buffer) exchangeSpi(&SPID1, &lcd_spicfg, SSD1803_SPI_TX_LEN, buffer, NULL)

/*
 * SPI configuration (1/32 f_pclk, CPHA=1, CPOL=1, 8 bit, LSB first).
 */
static const SPIConfig lcd_spicfg = {
    false,                                                         // circular buffer mode
    NULL,                                                          // end callback
    GPIOA,                                                         // chip select port
    GPIOA_SPI1_NSS2,                                               // chip select line
    SPI_CR1_CPHA | SPI_CR1_CPOL | SPI_CR1_BR_2 | SPI_CR1_LSBFIRST, // CR1 settings
    SPI_CR2_DS_2 | SPI_CR2_DS_1 | SPI_CR2_DS_0                     // CR2 settings
};

ssd1803_reg_t ssd1803_reg;
ssd1803_state_t ssd1803_state;

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

    // build spi buffer from all its components
    uint8_t buffer[SSD1803_SPI_TX_LEN];

    buffer[0] = SSD1803_SPI_START_BYTE_LSB_ORDER |
                (instruction->rs << SSD1803_SPI_START_BYTE_RS_POS) |
                (instruction->rw << SSD1803_SPI_START_BYTE_RW_POS);

    buffer[1] = instruction->payload & 0xf;        // lower 4 bit
    buffer[2] = (instruction->payload >> 4) & 0xf; // upper 4 bit

    writeLcdRegister(buffer);
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
    ssd1803_reg.ssd1803_function_set_0_reg->dl =
        ssd1803_reg.ssd1803_function_set_1_reg->dl = true; // 8 bit wide transfers

    // set number of lines in the display
    if (LINES == 1 || LINES == 3)
    {
        ssd1803_reg.ssd1803_function_set_0_reg->n =
            ssd1803_reg.ssd1803_function_set_1_reg->n = false; // 1 or 3-line display option
    }
    else
    {
        ssd1803_reg.ssd1803_function_set_0_reg->n =
            ssd1803_reg.ssd1803_function_set_1_reg->n = false; // 2 or 4-line display option
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
    ssd1803_reg.ssd1803_internal_osc_reg->f1 = false;
    ssd1803_reg.ssd1803_internal_osc_reg->f2 = false;

    // set lcd driving voltage end enable internal divider:  1+Rb/Ra = 5.3
    ssd1803_reg.ssd1803_follower_control_reg->rab0 = false;
    ssd1803_reg.ssd1803_follower_control_reg->rab1 = true;
    ssd1803_reg.ssd1803_follower_control_reg->don = true;
    ssd1803_reg.ssd1803_follower_control_reg->rab0 = true;

    // enable dcdc converter and regulator circuit
    ssd1803_reg.ssd1803_power_icon_contrast_set_reg->bon = true;
    ssd1803_reg.ssd1803_power_icon_contrast_set_reg->ion = false;

    ssd1803_reg.ssd1803_double_height_reg->ud1 = false; // middle line is double height
    ssd1803_reg.ssd1803_double_height_reg->ud2 = true;

    // select rom A
    // ssd1803_reg.ssd1803_rom_selection_set_reg->rom1 = 0;
    // ssd1803_reg.ssd1803_rom_selection_set_reg->rom2 = 0;

    ssd1803_reg.ssd1803_display_on_off_control_reg->d = true;  // switch on display
    ssd1803_reg.ssd1803_display_on_off_control_reg->c = false; // switch off cursor
    ssd1803_reg.ssd1803_display_on_off_control_reg->b = false; // switch off blinking

    // set contrast
    ssd1803_contrast(CONTRAST);

    ssd1803_instruction_t instruction;

    ssd1803_function_set_0(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    ssd1803_function_set_1(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    ssd1803_extended_function_set(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    ssd1803_entry_mode_set_1(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    ssd1803_internal_osc(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    ssd1803_follower_control(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    ssd1803_power_set(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    ssd1803_double_height(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    ssd1803_double_height(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    ssd1803_display_on_off_control(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);

    ssd1803_return_home(&instruction);
    writeInstruction(&instruction);
}

void ssd1803_move_to_line(uint8_t line)
{
    if (VIEW == BOTTOM)
    {
        ssd1803_reg.ssd1803_set_ddram_address_reg->ac = SSD1803_DDRAM_ADR_BOT + line * SSD1803_DDRAM_ADR_OFFSET;
    }
    else if (VIEW == TOP)
    {
        ssd1803_reg.ssd1803_set_ddram_address_reg->ac = SSD1803_DDRAM_ADR_TOP + line * SSD1803_DDRAM_ADR_OFFSET;
    }

    ssd1803_instruction_t instruction;
    ssd1803_set_ddram_address(&instruction, &ssd1803_reg);
    writeInstruction(&instruction);
}

void ssd1803_writeData(uint8_t c)
{
    ssd1803_instruction_t instruction;
    instruction.rs = true;
    instruction.rw = false;
    instruction.payload = c;

    writeInstruction(&instruction);
}

void ssd1803_writeString(uint8_t *s, uint32_t length)
{
    for (uint32_t pos = 0; pos < length; pos++)
    {
        ssd1803_writeData(*(s + pos));
    }
}