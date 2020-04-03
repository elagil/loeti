#ifndef SSD1803_CTRL_H_
#define SSD1803_CTRL_H_

#include "ssd1803_def.h"

extern ssd1803_state_t ssd1803_state;

void ssd1803_initialize(void);
void ssd1803_contrast(uint8_t contrast);
void ssd1803_move_to_line(uint8_t line);
void ssd1803_writeData(uint8_t c);
void ssd1803_writeString(uint8_t *s, uint32_t length);

#endif