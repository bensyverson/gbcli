// Battery saves without host time. SameBoy's GB_load_battery_from_buffer
// advances the clock by the host time since the footer's stamp (or resets it
// when the stamp is implausible), and GB_rtc_set_time is only reached through
// host time; these load the file's clock as written and advance it on request.

#include <string.h>

#include "Core/gb.h"

// What a battery save holds on this cart: 0 nothing (no battery, or a battery
// with nothing behind it), 1 cart RAM, 2 cart RAM and an MBC3 clock footer,
// 3 a clock whose footer gbcli doesn't read (HuC3, TPP1).
int gbcli_battery_kind(GB_gameboy_t *gb)
{
    if (GB_save_battery_size(gb) == 0) return 0;
    switch (gb->cartridge_type->mbc_type) {
        case GB_HUC3:
        case GB_TPP1:
            return 3;
        default:
            return gb->cartridge_type->has_rtc ? 2 : 1;
    }
}

// Whether the cart has a clock that advance_rtc can move.
bool gbcli_has_clock(GB_gameboy_t *gb)
{
    return gb->cartridge_type->has_rtc || gb->cartridge_type->mbc_type == GB_HUC3;
}

// The cart RAM's size in bytes.
uint32_t gbcli_cart_ram_size(GB_gameboy_t *gb) { return gb->mbc_ram_size; }

// Loads cart RAM, and the MBC3 clock when `real` and `latched` (five registers
// each: seconds, minutes, hours, days low, days high) are given. Without them
// on a clocked cart, the day counter's carry bit is set, as SameBoy's loader
// does to tell the game its clock needs setting. `ram_len` must be the cart
// RAM's size; the caller checks.
void gbcli_load_battery(GB_gameboy_t *gb, const uint8_t *ram, size_t ram_len,
                        const uint8_t *real, const uint8_t *latched)
{
    memcpy(gb->mbc_ram, ram, ram_len < gb->mbc_ram_size ? ram_len : gb->mbc_ram_size);
    if (real && latched) {
        memcpy(gb->rtc_real.data, real, sizeof(gb->rtc_real.data));
        memcpy(gb->rtc_latched.data, latched, sizeof(gb->rtc_latched.data));
    }
    else if (gb->cartridge_type->has_rtc) {
        gb->rtc_real.high |= 0x80;
    }
}

// Moves the clock on by whole seconds. In accurate mode the clock counts only
// increments of last_rtc_second, so its value is an epoch, not host time; a
// halted clock is left alone by GB_rtc_set_time itself.
void gbcli_advance_rtc(GB_gameboy_t *gb, uint64_t seconds)
{
    GB_rtc_set_time(gb, gb->last_rtc_second + seconds);
}
