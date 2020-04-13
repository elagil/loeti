#ifndef SPI_HELPER_H_
#define SPI_HELPER_H_

#include "ch.h"
#include "hal.h"

void spiExchangeHelper(SPIDriver *spi, const SPIConfig *conf, uint32_t length, uint8_t *txbuf, uint8_t *rxbuf);

#endif