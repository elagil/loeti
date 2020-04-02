#include "lcd.h"

#include "ch.h"
#include "hal.h"

#include "ssd1803_reg.h"
#include "ssd1803_calc.h"
#include "ssd1803_set.h"
#include "ssd1803_def.h"

ssd1803_reg_t ssd1803_reg;
ssd1803_state_t ssd1803_state;

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

THD_WORKING_AREA(waLcdThread, LCD_THREAD_STACK_SIZE);

void writeLcdRegister(uint8_t *buffer)
{
    /* Bush acquisition and SPI reprogramming.*/
    spiAcquireBus(&SPID1);
    spiStart(&SPID1, &lcd_spicfg);

    /* Slave selection and data transmission.*/
    spiSelect(&SPID1);
    spiStartSend(&SPID1, SSD1803_SPI_TX_LEN, buffer);
    spiUnselect(&SPID1);

    /* Releasing the bus.*/
    spiReleaseBus(&SPID1);
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

    // build spi buffer from all its components
    uint8_t buffer[SSD1803_SPI_TX_LEN];

    buffer[0] = SSD1803_SPI_START_BYTE_LSB_ORDER |
                (instruction->rs << SSD1803_SPI_START_BYTE_RS_POS) |
                (instruction->rw << SSD1803_SPI_START_BYTE_RW_POS);

    buffer[1] = instruction->payload & 0xf;        // lower 4 bit
    buffer[2] = (instruction->payload >> 4) & 0xf; // upper 4 bit

    writeLcdRegister(buffer);
}

THD_FUNCTION(lcdThread, arg)
{
    (void)arg;
    ssd1803_state.row = 0;
    ssd1803_state.col = 0;
    ssd1803_state.is = false;
    ssd1803_state.re = false;

    chRegSetThreadName("lcd");

    ssd1803_calc_initialize(&ssd1803_reg);
}
