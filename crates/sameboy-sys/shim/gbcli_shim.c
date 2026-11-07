// Tiny accessors over SameBoy internals, compiled with GB_INTERNAL so the
// struct layout stays in C and is never mirrored in Rust. Every function here
// is side-effect free unless its name says it sets something.
//
// "Peek" functions mirror SameBoy's read paths (memory.c) minus every side
// effect: no PPU sync, no open-bus update, no OAM bug, no callbacks. Where a
// value can't be known without one, they return -1.

#include "Core/gb.h"

bool gbcli_ime(GB_gameboy_t *gb) { return gb->ime; }
void gbcli_set_ime(GB_gameboy_t *gb, bool ime) { gb->ime = ime; }
bool gbcli_halted(GB_gameboy_t *gb) { return gb->halted; }
void gbcli_set_halted(GB_gameboy_t *gb, bool halted) { gb->halted = halted; }
bool gbcli_stopped(GB_gameboy_t *gb) { return gb->stopped; }
bool gbcli_double_speed(GB_gameboy_t *gb) { return gb->cgb_double_speed; }
bool gbcli_is_mmm01(GB_gameboy_t *gb) { return gb->cartridge_type->mbc_type == GB_MMM01; }

// The per-pixel layer tags the display patch writes while capture is on;
// NULL detaches the buffer. SameBoy keeps the pointer, so the caller owns it.
void gbcli_set_layer_buffer(GB_gameboy_t *gb, uint8_t *buffer) { gb->layer_buffer = buffer; }

// Whether the vblank ending now belongs to the blank frame SameBoy substitutes
// after the LCD is turned on: display.c scrubs the screen to the LCD-off colour
// at that vblank, then clears the state. Read from inside the vblank callback,
// so it still describes the frame just published.
bool gbcli_frame_scrubbed(GB_gameboy_t *gb) { return gb->frame_skip_state == GB_FRAMESKIP_LCD_TURNED_ON; }

// GB_set_rtc_mode stamps last_rtc_second with time(NULL), and that field is in
// the save state; in accurate mode only its increments matter, so pin it.
void gbcli_clear_rtc_epoch(GB_gameboy_t *gb) { gb->last_rtc_second = 0; }

// Whether the PPU has batched cycles it hasn't run yet (mode 3 batching), in
// which case PPU-synced registers in the IO array may lag.
bool gbcli_ppu_batched(GB_gameboy_t *gb) { return gb->display_cycles > 0; }

static uint16_t rom_bank(GB_gameboy_t *gb, uint16_t bank)
{
    if (!gb->rom_size) return 0;
    return ((uint32_t)bank * 0x4000 & (gb->rom_size - 1)) / 0x4000;
}

// The cart RAM bank a write at A000-BFFF lands in: the selected bank masked
// as write_mbc_ram masks it, then by the RAM size, as rom_bank masks by the
// ROM size. So a hit's bank is the bank of the byte it reached, the bank the
// engine aliases an explicit bank to (mem::effective_bank).
static uint16_t cart_ram_bank(GB_gameboy_t *gb)
{
    uint32_t bank = gb->mbc_ram_bank;
    if (gb->cartridge_type->mbc_type == GB_MBC3 && !gb->is_mbc30) bank &= 3;
    if (!gb->mbc_ram_size) return bank;
    return (bank * 0x2000 & (gb->mbc_ram_size - 1)) / 0x2000;
}

// The bank mapped at a canonical address (echo already folded to WRAM).
uint16_t gbcli_bank_for(GB_gameboy_t *gb, uint16_t addr)
{
    if (addr >= 0xE000 && addr < 0xFE00) addr -= 0x2000;
    switch (addr >> 12) {
        case 0x0: case 0x1: case 0x2: case 0x3: return rom_bank(gb, gb->mbc_rom0_bank);
        case 0x4: case 0x5: case 0x6: case 0x7: return rom_bank(gb, gb->mbc_rom_bank);
        case 0x8: case 0x9: return gb->cgb_vram_bank;
        case 0xA: case 0xB: return cart_ram_bank(gb);
        case 0xD: return gb->cgb_ram_bank;
        default: return 0;
    }
}

// Whether gbcli_cart_ram_offset mirrors this mapper's cart RAM path, so every
// change to cart RAM is a CPU write it can place. Not MBC2, MBC7, HuC1/3,
// TPP1 or the camera: their RAM paths have modes and side channels we don't
// mirror.
bool gbcli_cart_ram_mirrored(GB_gameboy_t *gb)
{
    switch (gb->cartridge_type->mbc_type) {
        case GB_NO_MBC: case GB_MBC1: case GB_MBC3: case GB_MBC5: case GB_MMM01: return true;
        default: return false;
    }
}

// The offset into cart RAM that a CPU write at addr (A000-BFFF) reaches, as
// write_mbc_ram computes it, or -1 when it reaches no RAM (disabled, RTC
// mapped) or the mapper isn't one gbcli_cart_ram_mirrored covers.
int32_t gbcli_cart_ram_offset(GB_gameboy_t *gb, uint16_t addr)
{
    if (!gbcli_cart_ram_mirrored(gb)) return -1;
    if (!gb->mbc_ram_enable || !gb->mbc_ram || !gb->mbc_ram_size) return -1;
    if (gb->cartridge_type->has_rtc && gb->mbc3.rtc_mapped) return -1;
    uint8_t bank = gb->mbc_ram_bank;
    if (gb->cartridge_type->mbc_type == GB_MBC3 && !gb->is_mbc30) {
        if (gb->cartridge_type->has_rtc && bank > 3) return -1;
        bank &= 3;
    }
    return ((addr & 0x1FFF) + bank * 0x2000) & (gb->mbc_ram_size - 1);
}

// Whether the APU has run every cycle that has passed. SameBoy batches APU
// cycles (apu.apu_cycles) and catches up on any APU register access.
bool gbcli_apu_synced(GB_gameboy_t *gb) { return gb->apu.apu_cycles == 0; }

// Mirrors GB_apu_read (apu.c) minus its GB_apu_run: -1 where that run could
// change the result (NR52's channel bits, wave RAM while channel 3 plays).
static int apu_peek(GB_gameboy_t *gb, uint8_t reg)
{
    if (reg == GB_IO_NR52) {
        if (!gbcli_apu_synced(gb)) return -1;
        uint8_t value = 0;
        for (unsigned i = 0; i < GB_N_CHANNELS; i++) {
            value >>= 1;
            if (gb->apu.is_active[i]) value |= 0x8;
        }
        if (gb->apu.global_enable) value |= 0x80;
        return value | 0x70;
    }
    static const uint8_t read_mask[GB_IO_WAV_END - GB_IO_NR10 + 1] = {
        0x80, 0x3F, 0x00, 0xFF, 0xBF, // NR1X
        0xFF, 0x3F, 0x00, 0xFF, 0xBF, // NR2X
        0x7F, 0xFF, 0x9F, 0xFF, 0xBF, // NR3X
        0xFF, 0xFF, 0x00, 0x00, 0xBF, // NR4X
        0x00, 0x00, 0x70, 0xFF, 0xFF, // NR5X
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // Unused
        0, // Wave RAM
    };
    if (reg >= GB_IO_WAV_START && reg <= GB_IO_WAV_END && gb->apu.is_active[GB_WAVE]) {
        if (!gbcli_apu_synced(gb)) return -1;
        if (!GB_is_cgb(gb) && !gb->apu.wave_channel.wave_form_just_read) return 0xFF;
        if (gb->model > GB_MODEL_CGB_E) return 0xFF;
        reg = GB_IO_WAV_START + gb->apu.wave_channel.current_sample_index / 2;
    }
    return gb->io_registers[reg] | read_mask[reg - GB_IO_NR10];
}

// Mirrors read_high_memory's IO cases (memory.c), in its order.
int gbcli_io_peek(GB_gameboy_t *gb, uint8_t reg)
{
    switch (reg) {
        // The PPU syncs before these reads; without a sync they may lag.
        case GB_IO_IF: case GB_IO_LCDC: case GB_IO_STAT: case GB_IO_SCY:
        case GB_IO_SCX: case GB_IO_LY: case GB_IO_LYC: case GB_IO_BGP:
        case GB_IO_OBP0: case GB_IO_OBP1: case GB_IO_WY: case GB_IO_WX:
        case GB_IO_HDMA5: case GB_IO_BGPI: case GB_IO_BGPD: case GB_IO_OBPI:
        case GB_IO_OBPD: case GB_IO_OPRI:
            if (gbcli_ppu_batched(gb)) return -1;
            switch (reg) {
                case GB_IO_IF: return gb->io_registers[reg] | 0xE0;
                case GB_IO_STAT: return gb->io_registers[reg] | 0x80;
                case GB_IO_OPRI:
                    return GB_is_cgb(gb) ? gb->io_registers[reg] | 0xFE : 0xFF;
                case GB_IO_HDMA5:
                    if (!gb->cgb_mode) return 0xFF;
                    return ((gb->hdma_on || gb->hdma_on_hblank) ? 0 : 0x80) |
                           ((gb->hdma_steps_left - 1) & 0x7F);
                case GB_IO_BGPI: case GB_IO_OBPI:
                    return GB_is_cgb(gb) ? gb->io_registers[reg] | 0x40 : 0xFF;
                case GB_IO_BGPD: case GB_IO_OBPD: {
                    if (!gb->cgb_mode && gb->boot_rom_finished) return 0xFF;
                    if (gb->cgb_palettes_blocked) return 0xFF;
                    uint8_t index = gb->io_registers[reg - 1] & 0x3F;
                    return (reg == GB_IO_BGPD ? gb->background_palettes_data
                                              : gb->object_palettes_data)[index];
                }
                default: return gb->io_registers[reg];
            }
        case GB_IO_TAC: return gb->io_registers[reg] | 0xF8;
        case GB_IO_PCM12: case GB_IO_PCM34: {
            if (!GB_is_cgb(gb)) return 0xFF;
            if (!gbcli_apu_synced(gb)) return -1;
            bool high = reg == GB_IO_PCM34;
            GB_channel_t lo = high ? GB_WAVE : GB_SQUARE_1, hi = high ? GB_NOISE : GB_SQUARE_2;
            uint8_t value = (gb->apu.is_active[hi] ? gb->apu.samples[hi] << 4 : 0) |
                            (gb->apu.is_active[lo] ? gb->apu.samples[lo] : 0);
            return value & (gb->model <= GB_MODEL_CGB_C ? gb->apu.pcm_mask[high] : 0xFF);
        }
        case GB_IO_TMA: case GB_IO_SC: case GB_IO_SB: case GB_IO_DMA:
            return gb->io_registers[reg];
        case GB_IO_JOYP:
            if (gb->joyp_switching_delay) {
                return (gb->io_registers[reg] & ~0x30) | (gb->joyp_switch_value & 0x30);
            }
            return gb->io_registers[reg];
        case GB_IO_TIMA:
            return gb->tima_reload_state == GB_TIMA_RELOADING ? 0 : gb->io_registers[reg];
        case GB_IO_DIV: return gb->div_counter >> 8;
        case GB_IO_SVBK: return gb->cgb_mode ? gb->io_registers[reg] : 0xFF;
        case GB_IO_VBK: return GB_is_cgb(gb) ? (gb->cgb_vram_bank | ~0x1) & 0xFF : 0xFF;
        case GB_IO_KEY1:
            if (!gb->cgb_mode) return 0xFF;
            return (gb->io_registers[reg] & 0x7F) | (gb->cgb_double_speed ? 0xFE : 0x7E);
        case GB_IO_BANK: return 0xFE | gb->boot_rom_finished;
        case GB_IO_RP: {
            if (!gb->cgb_mode) return 0xFF;
            uint8_t value = (gb->io_registers[reg] & 0xC1) | 0x2E;
            if (gb->model != GB_MODEL_CGB_E) value |= 0x10;
            if ((gb->io_registers[reg] & 0xC0) == 0xC0 && gb->effective_ir_input &&
                gb->model <= GB_MODEL_CGB_E) {
                value &= ~2;
            }
            return value;
        }
        case GB_IO_PSWX: case GB_IO_PSWY:
            return GB_is_cgb(gb) ? gb->io_registers[reg] : 0xFF;
        case GB_IO_PSW: return gb->cgb_mode ? gb->io_registers[reg] : 0xFF;
        case GB_IO_PGB: return GB_is_cgb(gb) ? gb->io_registers[reg] | 0x8F : 0xFF;
        default:
            if (reg >= GB_IO_NR10 && reg <= GB_IO_WAV_END) return apu_peek(gb, reg);
            return 0xFF;
    }
}

int gbcli_peek(GB_gameboy_t *gb, uint16_t addr)
{
    switch (addr >> 12) {
        case 0x0: case 0x1: case 0x2: case 0x3:
            if (!gb->boot_rom_finished) {
                if (addr < 0x100) return gb->boot_rom[addr];
                if (addr >= 0x200 && addr < 0x900 && GB_is_cgb(gb)) return gb->boot_rom[addr];
            }
            if (!gb->rom_size) return 0xFF;
            return gb->rom[((addr & 0x3FFF) + gb->mbc_rom0_bank * 0x4000) & (gb->rom_size - 1)];
        case 0x4: case 0x5: case 0x6: case 0x7:
            if (!gb->rom_size) return 0xFF;
            return gb->rom[((addr & 0x3FFF) + gb->mbc_rom_bank * 0x4000) & (gb->rom_size - 1)];
        case 0x8: case 0x9:
            if (gbcli_ppu_batched(gb) || gb->vram_read_blocked || gb->display_state == 22) return -1;
            return gb->vram[(addr & 0x1FFF) + (gb->cgb_vram_bank ? 0x2000 : 0)];
        case 0xA: case 0xB: {
            int32_t offset = gbcli_cart_ram_offset(gb, addr);
            return offset < 0 ? -1 : gb->mbc_ram[offset];
        }
        case 0xC: case 0xE: return gb->ram[addr & 0x0FFF];
        case 0xD: return gb->ram[(addr & 0x0FFF) + gb->cgb_ram_bank * 0x1000];
        default: break;
    }
    if (addr < 0xFE00) return gb->ram[(addr & 0x0FFF) + gb->cgb_ram_bank * 0x1000];
    if (addr < 0xFEA0) {
        if (gbcli_ppu_batched(gb) || gb->oam_read_blocked || GB_is_dma_active(gb)) return -1;
        return gb->oam[addr - 0xFE00];
    }
    if (addr < 0xFF00) return -1;
    if (addr < 0xFF80) return gbcli_io_peek(gb, addr & 0xFF);
    if (addr == 0xFFFF) return gb->interrupt_enable;
    return gb->hram[addr - 0xFF80];
}

// The opcode the next GB_run will fetch and execute, or -1 if it won't fetch
// one (STOP, HALT, interrupt dispatch, the SGB intro) or the byte can't be
// known without a bus read (an OAM DMA bus conflict, a blocked region).
int gbcli_next_opcode(GB_gameboy_t *gb)
{
    if (gb->sgb && gb->sgb->intro_animation < 96) return -1;
    if (gb->stopped || gb->halted) return -1;
    if (gb->ime && (gb->interrupt_enable & gb->io_registers[GB_IO_IF] & 0x1F)) return -1;
    if (GB_is_dma_active(gb) && gb->pc < 0xFF80) return -1;
    return gbcli_peek(gb, gb->pc);
}


// What the run loop must do before the next GB_run, in one call because it
// runs before every instruction.
enum {
    GBCLI_PRE_RUN = 0,         // run the next step
    GBCLI_PRE_ILLEGAL = 1,     // the next step executes an illegal opcode (in *opcode)
    GBCLI_PRE_STOPPED = 2,     // in STOP mode with no joypad line low to wake it
};

static const bool illegal_opcode[256] = {
    [0xD3] = true, [0xDB] = true, [0xDD] = true, [0xE3] = true, [0xE4] = true, [0xEB] = true,
    [0xEC] = true, [0xED] = true, [0xF4] = true, [0xFC] = true, [0xFD] = true,
};

// Hot: called before every instruction. The common case (code in cartridge
// ROM, a legal opcode) costs one ROM read and a table lookup; the full
// conditions are checked only when the byte at PC is illegal.
int gbcli_pre_run(GB_gameboy_t *gb, uint8_t *opcode)
{
    if (unlikely(gb->stopped)) {
        return (gbcli_io_peek(gb, GB_IO_JOYP) & 0x0F) == 0x0F ? GBCLI_PRE_STOPPED : GBCLI_PRE_RUN;
    }
    uint16_t pc = gb->pc;
    int byte;
    if (likely(pc < 0x8000 && gb->boot_rom_finished && gb->rom_size)) {
        uint32_t bank = pc < 0x4000 ? gb->mbc_rom0_bank : gb->mbc_rom_bank;
        byte = gb->rom[((pc & 0x3FFF) + bank * 0x4000) & (gb->rom_size - 1)];
    }
    else {
        byte = gbcli_peek(gb, pc);
    }
    if (likely(byte < 0 || !illegal_opcode[byte])) return GBCLI_PRE_RUN;
    if (gbcli_next_opcode(gb) != byte) return GBCLI_PRE_RUN; // HALT, an interrupt, a DMA conflict
    *opcode = byte;
    return GBCLI_PRE_ILLEGAL;
}
