# Teddy & Toys

A toy-factory game for the Game Boy: grab matching halves off a moving belt, snap them into toys, and send them off before the clock runs out.

Stub: the full README (how to play, what's notable, the Tuning section and the proofs) comes with the polish pass. This is the placeholder build for playtesting: the art, sound and music are stand-ins.

## Build, check, play

From this folder, with `gbcli` and RGBDS on `PATH`:

```sh
python3 tools/make.py            # regenerate, build both ROMs, run the rules check
gbcli run build/factory.gb --session play --frames 600 --shot now.png
```

The ROMs land in `build/factory.gb` (DMG) and `build/gbc/factory.gbc` (GBC, also runs on a DMG).

Controls: Down faces the parts belt (front), Up faces the Finished belt (back). A grabs, B puts back or drops, and B+A together assembles. START begins a round.

## Tuning

Every gameplay number is in `include/tuning.inc`. The toys are `data/toys.txt` with art in `art/toys/`. After any change, `python3 tools/make.py`.
