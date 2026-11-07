include "hUGE.inc"

SECTION "song_example Song Data", ROMX

song_example::
db 6
dw song_example.order_cnt
dw song_example.order1, song_example.order2, song_example.order3, song_example.order4
dw song_example.duty_instruments, song_example.wave_instruments, song_example.noise_instruments
dw song_example.routines
dw song_example.waves

song_example.order_cnt: db 4
song_example.order1: dw song_example.P0,song_example.P4
song_example.order2: dw song_example.P1,song_example.P1
song_example.order3: dw song_example.P2,song_example.P2
song_example.order4: dw song_example.P3,song_example.P3

song_example.P0:
 dn C_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn E_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn G_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn E_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn F_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn A_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_6,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn A_5,1,$C06
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn G_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn D_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn B_4,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn D_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn G_5,1,$437
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000

song_example.P1:
 dn E_4,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn G_4,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn G_4,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn A_4,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn F_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn B_4,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn D_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn G_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn D_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn G_4,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn B_4,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn D_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000

song_example.P2:
 dn C_4,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn F_3,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn G_3,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_4,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000

song_example.P3:
 dn C_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn F_6,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn F_6,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn F_6,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn F_6,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn F_6,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn F_6,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn F_6,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,2,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn F_6,1,$E02
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000

song_example.P4:
 dn E_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn D_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn E_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn D_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn B_4,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn C_5,1,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$E00
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$000
 dn ___,0,$B00

song_example.duty_instruments:
song_example.itSquareinst1:
db 8
db 128
db 195
dw 0
db 128

song_example.itSquareinst2:
db 8
db 64
db 145
dw 0
db 128



song_example.wave_instruments:
song_example.itWaveinst1:
db 0
db 64
db 4
dw 0
db 128



song_example.noise_instruments:
song_example.itNoiseinst1:
db 129
dw 0
db 252
ds 2

song_example.itNoiseinst2:
db 242
dw 0
db 0
ds 2



song_example__hUGE_Routine_0:

song_example__end_hUGE_Routine_0:
ret

song_example__hUGE_Routine_1:

song_example__end_hUGE_Routine_1:
ret

song_example__hUGE_Routine_2:

song_example__end_hUGE_Routine_2:
ret

song_example__hUGE_Routine_3:

song_example__end_hUGE_Routine_3:
ret

song_example__hUGE_Routine_4:

song_example__end_hUGE_Routine_4:
ret

song_example__hUGE_Routine_5:

song_example__end_hUGE_Routine_5:
ret

song_example__hUGE_Routine_6:

song_example__end_hUGE_Routine_6:
ret

song_example__hUGE_Routine_7:

song_example__end_hUGE_Routine_7:
ret

song_example__hUGE_Routine_8:

song_example__end_hUGE_Routine_8:
ret

song_example__hUGE_Routine_9:

song_example__end_hUGE_Routine_9:
ret

song_example__hUGE_Routine_10:

song_example__end_hUGE_Routine_10:
ret

song_example__hUGE_Routine_11:

song_example__end_hUGE_Routine_11:
ret

song_example__hUGE_Routine_12:

song_example__end_hUGE_Routine_12:
ret

song_example__hUGE_Routine_13:

song_example__end_hUGE_Routine_13:
ret

song_example__hUGE_Routine_14:

song_example__end_hUGE_Routine_14:
ret

song_example__hUGE_Routine_15:

song_example__end_hUGE_Routine_15:
ret

song_example.routines:
dw song_example__hUGE_Routine_0, song_example__hUGE_Routine_1, song_example__hUGE_Routine_2, song_example__hUGE_Routine_3, song_example__hUGE_Routine_4, song_example__hUGE_Routine_5, song_example__hUGE_Routine_6, song_example__hUGE_Routine_7
dw song_example__hUGE_Routine_8, song_example__hUGE_Routine_9, song_example__hUGE_Routine_10, song_example__hUGE_Routine_11, song_example__hUGE_Routine_12, song_example__hUGE_Routine_13, song_example__hUGE_Routine_14, song_example__hUGE_Routine_15

song_example.waves:
song_example.wave0: db 0,0,255,255,255,255,255,255,255,255,255,255,255,255,255,255
song_example.wave1: db 0,0,0,0,255,255,255,255,255,255,255,255,255,255,255,255
song_example.wave2: db 0,0,0,0,0,0,0,0,255,255,255,255,255,255,255,255
song_example.wave3: db 0,0,0,0,0,0,0,0,0,0,0,0,255,255,255,255
song_example.wave4: db 0,1,18,35,52,69,86,103,120,137,154,171,188,205,222,239

; gbcli: how the song wants hUGE_dosound called (not in hUGETracker's export)
DEF song_example_TIMER EQU 0
DEF song_example_TMA EQU 0
EXPORT song_example_TIMER, song_example_TMA
