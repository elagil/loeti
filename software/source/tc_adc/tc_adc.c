#include "tc_adc.h"

#include "ch.h"
#include "hal.h"
#include "heater.h"
#include "usb_pd.h"
#include "spiHelper.h"
#include "events.h"

event_source_t temp_event_source;

#define TC_CONNECT_DEBOUNCE_MS 1000
#define TC_ADC_LEN 2
#define TC_DISCONNECT 32767

// Extracts the upper or lower byte from the register (16 bit length)
#define CONF_REG_LOWER_BYTE(reg) (reg & 0xff)
#define CONF_REG_HIGHER_BYTE(reg) ((reg >> 8) & 0xff)
#define REG_TO_TEMP(x) ((x & 0xff) << 8) | ((x >> 8) & 0xff)

#define SS_POS 15
// Single shot conversion start (or not)
#define SS_NOP 0
#define SS_START 1

#define MUX_POS 12
// Input selection Px (positive) and Nx (negative) range from 0..3 or can be G(nd)
#define MUX_P0_N1 0
#define MUX_P0_N3 1
#define MUX_P1_N3 2
#define MUX_P2_N3 3
#define MUX_P0_NG 4
#define MUX_P1_NG 5
#define MUX_P2_NG 6
#define MUX_P3_NG 7

#define PGA_POS 9
// Sets full scale max. input of gain amplifier (peak-to-peak)
#define PGA_6144mV 0
#define PGA_4096mV 1
#define PGA_2048mV 2
#define PGA_1024mV 3
#define PGA_512mV 4
#define PGA_256mV 5
#define PGA_256mV_ALT1 6
#define PGA_256mV_ALT2 7

#define MODE_POS 8
// Acquisition mode (single shot or continuous)
#define MODE_CONT 0
#define MODE_SS 1

#define DR_POS 5
// Data rate of the ADC in samples per second
#define DR_8_SPS 0
#define DR_16_SPS 1
#define DR_32_SPS 2
#define DR_64_SPS 3
#define DR_128_SPS 4
#define DR_250_SPS 5
#define DR_475_SPS 6
#define DR_860_SPS 7

#define TS_MODE_POS 4
// Sensor mode: ADC or internal temperature
#define TS_MODE_ADC 0
#define TS_MODE_INTERNAL 1

#define PULL_UP_POS 3
// Pull up resistor on DOUT/DRDY pin
#define PULL_UP_DISABLE 0
#define PULL_UP_ENABLE 1

#define NOP_POS 1
// No operation due to config write
#define NOP_INVALID 0
#define NOP_VALID 1
#define NOP_INVALID_ALT1 2
#define NOP_INVALID_ALT2 3

// Read external TC
#define TC_ADC_SETTINGS (NOP_VALID << NOP_POS |          \
                         PULL_UP_ENABLE << PULL_UP_POS | \
                         TS_MODE_ADC << TS_MODE_POS |    \
                         DR_860_SPS << DR_POS |          \
                         MODE_SS << MODE_POS |           \
                         PGA_256mV << PGA_POS |          \
                         MUX_P2_NG << MUX_POS |          \
                         SS_START << SS_POS)

// Read local temperature
#define LOCAL_ADC_SETTINGS (NOP_VALID << NOP_POS |            \
                            PULL_UP_ENABLE << PULL_UP_POS |   \
                            TS_MODE_INTERNAL << TS_MODE_POS | \
                            DR_860_SPS << DR_POS |            \
                            MODE_SS << MODE_POS |             \
                            SS_START << SS_POS)

// Do not change ADC settings, by setting invalid flag
#define UNCHANGED_ADC_SETTINGS (NOP_INVALID << NOP_POS)

#define TC_SLOPE 0.2706
#define TC_OFFSET 5
#define TC_READ_DEAD_TIME_US 500 // wait for anti alias low pass in thermocouple amplifier
#define TC_READ_DELAY_US 1200

#define exchangeSpiAdc(txbuf, rxbuf) spiExchangeHelper(&SPID1, &tc_adc_spicfg, TC_ADC_LEN, txbuf, rxbuf)

/*
 * SPI configuration, 5 MHz max.
 * (1/32 f_pclk, CPHA=1, CPOL=1, 8 bit, LSB first).
 */
static const SPIConfig tc_adc_spicfg = {
    false,                                     // circular buffer mode
    NULL,                                      // end callback
    GPIOA,                                     // chip select port
    GPIOA_SPI1_NSS1,                           // chip select line
    SPI_CR1_CPHA | SPI_CR1_BR_1,               // CR1 settings
    SPI_CR2_DS_2 | SPI_CR2_DS_1 | SPI_CR2_DS_0 // CR2 settings
};

THD_WORKING_AREA(waAdcThread, ADC_THREAD_STACK_SIZE);

void calcBuffer(uint8_t *txbuf, uint16_t config)
{
    *(txbuf) = CONF_REG_HIGHER_BYTE(config);
    *(txbuf + 1) = CONF_REG_LOWER_BYTE(config);
}

uint32_t temp_log_idx;
volatile uint16_t temp_log[384];

THD_FUNCTION(adcThread, arg)
{
    (void)arg;
    event_listener_t power_event_listener;
    event_listener_t pwm_event_listener;

    chRegSetThreadName("tc_adc");

    chEvtRegisterMask(&power_event_source, &power_event_listener, POWER_EVENT);
    chEvtRegisterMask(&pwm_event_source, &pwm_event_listener, PWM_EVENT);

    uint8_t conf_acquire_local[TC_ADC_LEN];
    uint8_t conf_acquire_tc[TC_ADC_LEN];
    uint8_t conf_read[TC_ADC_LEN];

    calcBuffer(conf_acquire_local, LOCAL_ADC_SETTINGS);
    calcBuffer(conf_acquire_tc, TC_ADC_SETTINGS);
    calcBuffer(conf_read, UNCHANGED_ADC_SETTINGS);

    chEvtWaitAny(POWER_EVENT);

    uint16_t raw;
    int16_t converted;

    // initial conversion
    exchangeSpiAdc(conf_acquire_tc, (uint8_t *)&raw);

    chThdSleepMicroseconds(TC_READ_DELAY_US);

    uint32_t debounce = 0;
    while (true)
    {
        // read conversion
        exchangeSpiAdc(conf_read, (uint8_t *)&raw);
        converted = REG_TO_TEMP(raw);

        chBSemWait(&heater.bsem);
        if (converted == TC_DISCONNECT)
        {
            debounce = 0;
            heater.connected = false;
        }
        else
        {
            if (++debounce >= TC_CONNECT_DEBOUNCE_MS / LOOP_TIME_TEMPERATURE_MS)
            {
                heater.connected = true;
            }
        }
        // calculate actual heater temperature, including cold junction compensation
        heater.temperature_control.is = converted * TC_SLOPE + TC_OFFSET + heater.temperatures.local;

        chBSemSignal(&heater.bsem);

        chEvtBroadcast(&temp_event_source);

        chThdSleepMilliseconds(LOOP_TIME_TEMPERATURE_MS / 2);

        // Measure local temperature while heater is working
        exchangeSpiAdc(conf_acquire_local, (uint8_t *)&raw);
        chThdSleepMicroseconds(TC_READ_DELAY_US);

        exchangeSpiAdc(conf_read, (uint8_t *)&raw);
        converted = REG_TO_TEMP(raw);

        chBSemWait(&heater.bsem);
        heater.temperatures.local = (converted >> 2) * 0.03125;
        chBSemSignal(&heater.bsem);

        // Wait for PWM to stop
        chEvtWaitAny(PWM_EVENT);

        chThdSleepMicroseconds(TC_READ_DEAD_TIME_US);

        // start new conversion after heater switched off
        exchangeSpiAdc(conf_acquire_tc, (uint8_t *)&raw);

        chThdSleepMicroseconds(TC_READ_DELAY_US);
    }
}
