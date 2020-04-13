#include "ch.h"
#include "hal.h"

#include "spiHelper.h"

void spiExchangeHelper(SPIDriver *spi, const SPIConfig *conf, uint32_t length, uint8_t *txbuf, uint8_t *rxbuf)
{
    /* Bus acquisition and SPI reprogramming.*/
    spiAcquireBus(spi);
    spiStart(spi, conf);

    /* Slave selection and data transmission.*/
    spiSelect(spi);

    if (rxbuf != NULL && txbuf != NULL) // exchange data
        spiExchange(spi, length, txbuf, rxbuf);

    else if (txbuf != NULL && rxbuf == NULL) // only send
        spiSend(spi, length, txbuf);

    else if (rxbuf != NULL && txbuf == NULL) // only receive
        spiReceive(spi, length, txbuf);

    spiUnselect(spi);

    /* Releasing the bus.*/
    spiReleaseBus(spi);
}