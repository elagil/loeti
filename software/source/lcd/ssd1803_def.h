#ifndef SSD1803_DEF_H_
#define SSD1803_DEF_H_

#include "ch.h"

#define TOP 0
#define BOTTOM 1

/**
 * ssd1803 controller state, keeps track of register contents
 */
typedef struct
{   
    uint32_t row;           ///< cursor row
    uint32_t col;           ///< cursor column
    bool re;                ///< Extended register enable bit
    bool is;                ///< Special register enable bit
} ssd1803_state_t;

/**
 * ssd1803 controller instruction wrapper structure
 */
typedef struct
{
    uint8_t payload;        ///< payload byte 
    bool rs;                ///< RS enable bit
    bool re;                ///< Extended register enable bit
    bool is;                ///< Special register enable bit
} ssd1803_instruction_t;

typedef struct
{
    bool pd;                ///< power down bit (high -> power down lcd)
} ssd1803_power_down_mode_set_reg_t;

typedef struct
{
    bool s;                 ///< cursor shift setting
    bool id;                ///< increment/decrement of ddram address
} ssd1803_entry_mode_set_reg_0_t;

typedef struct
{
    bool bdc;               ///< data shift direction of common
    bool bds;               ///< data shift direction of segment
} ssd1803_entry_mode_set_reg_1_t;

typedef struct
{
    bool b;               ///< display control
    bool c;               ///< cursor control
    bool d;               ///< cursor blink control
} ssd1803_display_on_off_control_reg_t;

typedef struct
{
    bool nw;               ///< 4 line mode enable bit
    bool bw;               ///< black white inversion control
    bool fw;               ///< font width control
} ssd1803_extended_function_set_reg_t;

typedef struct
{
    bool sc;
    bool rl;
} ssd1803_cursor_or_display_shift_reg_t;

typedef struct
{
    bool dh;                ///< display shift enable selection
    bool bs1;               ///< bias divider
    bool ud1;               ///< double height features   
    bool ud2;               ///< double height features   
} ssd1803_double_height_reg_t;

typedef struct
{
    bool f0;
    bool f1;
    bool f2;
    bool bs0;
} ssd1803_internal_osc_reg_t;

typedef struct
{
    bool s1;
    bool s2;
    bool s3;
    bool s4;
} ssd1803_shift_scroll_enable_reg_t;

typedef struct
{
    bool is;
    bool re;
    bool dh;
    bool n;
    bool dl;
} ssd1803_function_set_0_reg_t;

typedef struct
{
    bool rev;
    bool re;
    bool be;
    bool n;
    bool dl;
} ssd1803_function_set_1_reg_t;

typedef struct
{
    unsigned int ac : 6;
} ssd1803_set_cgram_address_reg_t;

typedef struct
{
    unsigned int ac : 4;
} ssd1803_set_segram_address_reg_t;

typedef struct
{
    bool c4;
    bool c5;
    bool bon;
    bool ion;
} ssd1803_power_icon_contrast_set_reg_t;

typedef struct
{
    bool rab0;
    bool rab1;
    bool rab2;
    bool don;
} ssd1803_follower_control_reg_t;

typedef struct
{
    unsigned int c : 4;
} ssd1803_contrast_set_reg_t;

typedef struct
{
    unsigned int ac : 7;
} ssd1803_set_ddram_address_reg_t;

typedef struct
{
    unsigned int sq : 6;
} ssd1803_set_scroll_quantitiy_reg_t;

typedef struct
{
    ssd1803_power_down_mode_set_reg_t * ssd1803_power_down_mode_set_reg;
    ssd1803_entry_mode_set_reg_0_t * ssd1803_entry_mode_set_reg_0;
    ssd1803_entry_mode_set_reg_1_t * ssd1803_entry_mode_set_reg_1;
    ssd1803_display_on_off_control_reg_t * ssd1803_display_on_off_control_reg;
    ssd1803_extended_function_set_reg_t * ssd1803_extended_function_set_reg;
    ssd1803_cursor_or_display_shift_reg_t * ssd1803_cursor_or_display_shift_reg;
    ssd1803_double_height_reg_t * ssd1803_double_height_reg;
    ssd1803_internal_osc_reg_t * ssd1803_internal_osc_reg;
    ssd1803_shift_scroll_enable_reg_t * ssd1803_shift_scroll_enable_reg;
    ssd1803_function_set_0_reg_t * ssd1803_function_set_0_reg;
    ssd1803_function_set_1_reg_t * ssd1803_function_set_1_reg;
    ssd1803_set_cgram_address_reg_t * ssd1803_set_cgram_address_reg;
    ssd1803_set_segram_address_reg_t * ssd1803_set_segram_address_reg;
    ssd1803_power_icon_contrast_set_reg_t * ssd1803_power_icon_contrast_set_reg;
    ssd1803_follower_control_reg_t * ssd1803_follower_control_reg;
    ssd1803_contrast_set_reg_t * ssd1803_contrast_set_reg;
    ssd1803_set_ddram_address_reg_t * ssd1803_set_ddram_address_reg;
} ssd1803_reg_t;

#endif