// Throughput: SameBoy DMG_B running a ROM for N emulated frames, turbo, rendering on, audio off.
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "Core/gb.h"
static uint32_t px[256*224];
static uint32_t enc(GB_gameboy_t *gb, uint8_t r, uint8_t g, uint8_t b){ return (r<<16)|(g<<8)|b; }
static void logcb(GB_gameboy_t *gb, const char *s, GB_log_attributes_t a){ }
static char *noin(GB_gameboy_t *gb){ return NULL; }
static double now(void){ struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return t.tv_sec+t.tv_nsec/1e9; }
int main(int argc, char **argv){
  int frames = atoi(argv[2]); int render = atoi(argv[3]);
  GB_random_set_enabled(false);
  GB_gameboy_t *gb = GB_init(GB_alloc(), GB_MODEL_DMG_B);
  GB_set_log_callback(gb, logcb); GB_set_input_callback(gb, noin); GB_set_async_input_callback(gb, noin);
  GB_set_rgb_encode_callback(gb, enc); GB_set_pixels_output(gb, px);
  GB_set_rendering_disabled(gb, !render);
  GB_set_turbo_mode(gb, true, true); GB_set_turbo_cap(gb, 0);
  if (GB_load_rom(gb, argv[1])) { fprintf(stderr,"rom load failed\n"); return 4; }
  GB_write_memory(gb, 0xFF50, 1); GB_get_registers(gb)->pc = 0x100; GB_get_registers(gb)->sp = 0xFFFE;
  double t0 = now(); unsigned long long ns = 0;
  unsigned long long ticks=0; unsigned long long target=(unsigned long long)frames*140448ULL; while (ticks < target) ticks += GB_run(gb); ns = ticks*1000000000ULL/8388608ULL;
  double dt = now()-t0;
  size_t n = GB_get_save_state_size(gb); uint8_t *s = malloc(n); GB_save_state_to_buffer(gb, s);
  t0 = now(); for (int i=0;i<10000;i++) GB_load_state_from_buffer(gb, s, n); double lt = (now()-t0)/10000;
  t0 = now(); for (int i=0;i<10000;i++) GB_save_state_to_buffer(gb, s); double st = (now()-t0)/10000;
  printf("frames=%d emulated=%.2fs wall=%.3fs speed=%.1fx realtime (%.0f fps) state=%zuB load=%.1fus save=%.1fus render=%d\n",
    frames, ns/1e9, dt, (ns/1e9)/dt, frames/dt, n, lt*1e6, st*1e6, render);
  return 0;
}
