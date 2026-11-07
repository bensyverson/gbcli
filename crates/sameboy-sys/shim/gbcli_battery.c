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
// halted clock is left alone by GB_rtc_set_time itself. False, and nothing
// moved, when the target second doesn't fit in 64 bits.
bool gbcli_advance_rtc(GB_gameboy_t *gb, uint64_t seconds)
{
    uint64_t target;
    if (__builtin_add_overflow(gb->last_rtc_second, seconds, &target)) return false;
    GB_rtc_set_time(gb, target);
    return true;
}

static void put_le16(uint8_t *p, uint16_t v)
{
    p[0] = v;
    p[1] = v >> 8;
}

static uint16_t get_le16(const uint8_t *p) { return p[0] | p[1] << 8; }

static void put_le64(uint8_t *p, uint64_t v)
{
    for (unsigned i = 0; i < 8; i++) p[i] = v >> (8 * i);
}

static uint64_t get_le64(const uint8_t *p)
{
    uint64_t v = 0;
    for (unsigned i = 8; i--;) v = v << 8 | p[i];
    return v;
}

// The bytes a battery state's clock takes after the cart RAM: MBC3's real and
// latched registers and the clock's epoch (18), HuC3's epoch, counters and
// alarm (17), or TPP1's MR4, registers and epoch (13).
static size_t clock_state_size(GB_gameboy_t *gb)
{
    switch (gb->cartridge_type->mbc_type) {
        case GB_HUC3: return 17;
        case GB_TPP1: return 13;
        default: return gb->cartridge_type->has_rtc ? 18 : 0;
    }
}

// Whether `len` is this cart's battery state size: GB_save_battery_size, so a
// power cycle reports the size of the .sav it stands for, with room for the
// clock.
static bool state_size_fits(GB_gameboy_t *gb, size_t len)
{
    int size = GB_save_battery_size(gb);
    return size > 0 && (size_t)size == len && len - gb->mbc_ram_size >= clock_state_size(gb);
}

// The battery's RAM and clock in gbcli's own layout, never a file: cart RAM,
// then the clock (clock_state_size), zero-padded to GB_save_battery_size. Every
// field is emulated state, so a core built from the same ROM takes it over
// exactly with gbcli_load_battery_state; it carries what the .sav import
// refuses. False when `len` isn't the size.
bool gbcli_save_battery_state(GB_gameboy_t *gb, uint8_t *out, size_t len)
{
    if (!state_size_fits(gb, len)) return false;
    memset(out, 0, len);
    if (gb->mbc_ram_size) memcpy(out, gb->mbc_ram, gb->mbc_ram_size);
    uint8_t *c = out + gb->mbc_ram_size;
    switch (gb->cartridge_type->mbc_type) {
        case GB_HUC3:
            put_le64(c, gb->last_rtc_second);
            put_le16(c + 8, gb->huc3.minutes);
            put_le16(c + 10, gb->huc3.days);
            put_le16(c + 12, gb->huc3.alarm_minutes);
            put_le16(c + 14, gb->huc3.alarm_days);
            c[16] = gb->huc3.alarm_enabled;
            break;
        case GB_TPP1:
            c[0] = gb->tpp1_mr4;
            memcpy(c + 1, gb->rtc_real.data, 4);
            put_le64(c + 5, gb->last_rtc_second);
            break;
        default:
            if (!gb->cartridge_type->has_rtc) break;
            memcpy(c, gb->rtc_real.data, 5);
            memcpy(c + 5, gb->rtc_latched.data, 5);
            put_le64(c + 10, gb->last_rtc_second);
            break;
    }
    return true;
}

// Takes over gbcli_save_battery_state's bytes from a core on the same cart.
// False, and nothing changed, when `len` isn't this cart's size.
bool gbcli_load_battery_state(GB_gameboy_t *gb, const uint8_t *in, size_t len)
{
    if (!state_size_fits(gb, len)) return false;
    if (gb->mbc_ram_size) memcpy(gb->mbc_ram, in, gb->mbc_ram_size);
    const uint8_t *c = in + gb->mbc_ram_size;
    switch (gb->cartridge_type->mbc_type) {
        case GB_HUC3:
            gb->last_rtc_second = get_le64(c);
            gb->huc3.minutes = get_le16(c + 8);
            gb->huc3.days = get_le16(c + 10);
            gb->huc3.alarm_minutes = get_le16(c + 12);
            gb->huc3.alarm_days = get_le16(c + 14);
            gb->huc3.alarm_enabled = c[16];
            break;
        case GB_TPP1:
            gb->tpp1_mr4 = c[0];
            memcpy(gb->rtc_real.data, c + 1, 4);
            gb->last_rtc_second = get_le64(c + 5);
            break;
        default:
            if (!gb->cartridge_type->has_rtc) break;
            memcpy(gb->rtc_real.data, c, 5);
            memcpy(gb->rtc_latched.data, c + 5, 5);
            gb->last_rtc_second = get_le64(c + 10);
            break;
    }
    return true;
}
