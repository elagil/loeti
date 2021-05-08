#include "ch.h"
#include "hal.h"
#include "heater.h"
#include "usb_pd.h"
#include "spiHelper.h"
#include "events.h"
#include "sensor.h"
#include "dma_lock.h"

event_source_t temp_event_source;

#define HEATER_DEBOUNCE_LIMIT 10
#define TEMP_FIELD 0
#define exchangeSpiAdc(txbuf, rxbuf) spiExchangeHelper(&SPID1, &tc_adc_spicfg, TC_ADC_LEN, txbuf, rxbuf)

THD_WORKING_AREA(waSensorThread, SENSOR_THREAD_STACK_SIZE);

#define ADC_GRP1_NUM_CHANNELS 1
#define ADC_GRP1_BUF_DEPTH 1
static adcsample_t adcsample[ADC_GRP1_NUM_CHANNELS * ADC_GRP1_BUF_DEPTH];

/*
 * ADC conversion group.
 * Mode:        Linear buffer, 1 sample of 1 channels, SW triggered.
 * Channel:     2
 */
static const ADCConversionGroup temperatureMeasurement = {
    FALSE,
    ADC_GRP1_NUM_CHANNELS,
    NULL,
    NULL,
    ADC_CFGR1_RES_12BIT, /* CFGR1 */
    ADC_TR(0, 0),        /* TR */
    ADC_SMPR_SMP_28P5,   /* SMPR */
    ADC_CHSELR_CHSEL2    /* CHSELR */
};

/*
 * I2C configuration for TMP100 sensor
 * 400 kHz fast mode
 */
static const uint16_t tmp100_address = 0x48;

static const I2CConfig i2ccfg = {
    STM32_TIMINGR_PRESC(0) |
        STM32_TIMINGR_SCLDEL(3) |
        STM32_TIMINGR_SDADEL(1) |
        STM32_TIMINGR_SCLH(3) |
        STM32_TIMINGR_SCLL(9),
    0,
    0};

msg_t I2C_Read_TMP100(i2caddr_t address, uint8_t *rxbuf, uint16_t length)
{
    chBSemWait(&dma_lock);
    uint8_t txbuf[1] = {0};

    i2cAcquireBus(&I2CD1);
    i2cStart(&I2CD1, &i2ccfg);

    msg_t status = i2cMasterTransmit(&I2CD1, address, txbuf, 1, rxbuf, length);

    i2cStop(&I2CD1);
    i2cReleaseBus(&I2CD1);

    chBSemSignal(&dma_lock);
    return status;
}

double measureLocalTemperature(void)
{
    uint8_t rxbuf[2];
    sensor_data_t tmp100_sensor_data;

    I2C_Read_TMP100(tmp100_address, rxbuf, 2);
    tmp100_sensor_data.array[0] = rxbuf[1];
    tmp100_sensor_data.array[1] = rxbuf[0];

    double local_temperature = (double)tmp100_sensor_data.value / 256;

    return local_temperature;
}

THD_FUNCTION(sensorThread, arg)
{
    (void)arg;
    event_listener_t power_event_listener;
    event_listener_t pwm_event_listener;

    chRegSetThreadName("sensor");

    chEvtRegisterMask(&power_event_source, &power_event_listener, POWER_EVENT);
    chEvtRegisterMask(&pwm_done_event_source, &pwm_event_listener, PWM_EVENT);

    chEvtWaitAny(POWER_EVENT);

    uint32_t heater_debounce = 0;

    while (true)
    {
        // Wait for temperature sensor value to settle
        chThdSleepMilliseconds(1);

        // Finally measure iron temperature
        adcConvert(&ADCD1, &temperatureMeasurement, adcsample, ADC_GRP1_BUF_DEPTH);

        chBSemWait(&heater.bsem);
        adcsample_t raw = adcsample[TEMP_FIELD];
        double iron_temperature = adcsample[TEMP_FIELD] * 0.1333;

        // Measure local PCB temperature, for cold junction compensation
        double local_temperature = measureLocalTemperature();

        heater.temperature_control.is = iron_temperature + local_temperature;

        if (raw >= (ADC_FS_READING - 100))
        {
            heater_debounce = 0;
            heater.connected = false;
        }
        else
        {
            if (heater_debounce == HEATER_DEBOUNCE_LIMIT)
            {
                heater.connected = true;
            }
            else
            {
                heater_debounce++;
            }
        }

        chBSemSignal(&heater.bsem);

        // Temperature measurement complete, notify listening threads
        chEvtBroadcast(&temp_event_source);

        // Wait for heating to stop
        chEvtWaitAny(PWM_EVENT);
    }
}
