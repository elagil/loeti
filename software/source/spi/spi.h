#ifndef SPI_H_
#define SPI_H_

#include "ch.h"
#include "hal.h"

void exchangeSpi(SPIDriver *spi, const SPIConfig *conf, uint32_t length, uint8_t *txbuf, uint8_t *rxbuf);

#endif