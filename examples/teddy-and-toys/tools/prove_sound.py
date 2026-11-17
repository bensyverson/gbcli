"""Proves the sound: each effect plays on channel 1, louder than the music.

    python3 tools/prove_sound.py [--gbcli PATH] [-v]

Session `sound`, from a round's start (START pressed, 30 frames in):
  music    600 frames of the tune alone: it writes no channel 1 register,
           and its starting volumes (NRx2's top nibble; the wave's level)
  effects  each effect, set up by pokes and keys, with --audio-log: it
           plays on channel 1, at the volume it starts at
  jingle   the score effect (a toy lands on the Finished belt) is a run of
           rising notes on channel 1, with gbcli's piano roll of it
  louder   the score effect over the music, saved with --wav --channels
           each: channel 1's loudest moment against the three music
           channels together (windowed RMS)
The numbers come from gbcli's decoded log and WAVs; nothing is assumed.
Files land in build/prove/ (sound-*.wav).
"""

import sys
import wave
from pathlib import Path

import gbcli as g
import tuning

SESSION = "sound"
T = tuning.load()
OUT = g.PROJECT / "build" / "prove"
RED_FRAMES = T["CLOCK_RED_SECONDS"] * 60
SLOT, ELLE_CENTER_X = 8, 68         # as tools/rules_check.py
PARTS_AT_SLOT = [f"wPartsX=0,{(SLOT * 16 + 8 - ELLE_CENTER_X) % 256}"]
FIN_AT_SLOT = [f"wFinishedX=0,{(4 * 16 + 8 - ELLE_CENTER_X) % 256}", "wFinishedSlots+4=0"]

NR12, NR22, NR32, NR42 = 0xFF12, 0xFF17, 0xFF1C, 0xFF21
NR13, NR14 = 0xFF13, 0xFF14
JINGLE_MIN_NOTES = 5               # Ben's ruling: a short rising run of notes, not a chord
CHANNEL_OF = {}
for ch, base in (("square1", 0xFF10), ("square2", 0xFF15), ("wave", 0xFF1A), ("noise", 0xFF1F)):
    for i in range(5):
        CHANNEL_OF[base + i] = ch
WAVE_LEVEL = {0: "0%", 1: "100%", 2: "50%", 3: "25%"}
TURN = T["TURN_FRAMES"]

EFFECTS = [  # (name, pokes, keys, frames)
    ("grab", PARTS_AT_SLOT + ["wPartsSlots+8=1"], "A@2 -A@3", 20),
    ("assemble", ["wHands=1,2", "wHeldCount=2"], "B@2 A@2 -B@3 -A@3", 20),
    ("score", FIN_AT_SLOT + ["wToy=1", "wFacing=1"], "B@2 -B@3", 40),
    ("buzz", [], "B@2 -B@3", 20),
    ("turn", [], "UP@2 -UP@3", 20),
    ("warn, then a tick a second", [f"wRoundLeft={(RED_FRAMES + 5) & 255},{(RED_FRAMES + 5) >> 8}"], "", 130),
    ("time's up", ["wRoundLeft=5,0"], "", 60),
    ("pause", [], "START@2 -START@3", 20),
    ("shuffle (Elle steps)", [], "RIGHT@2 -RIGHT@20", 20),
    ("the pause menu's cursor", [], "START@2 -START@3 DOWN@20 -DOWN@21", 30),
]


def run(cli, frames, pokes=(), keys="", extra=()):
    args = ["run", "--session", SESSION, "--at", "play", "--frames", str(frames), "--audio-log"]
    for p in pokes:
        args += ["--poke", p]
    if keys:
        args += ["--keys", keys]
    return cli.run(*args, *extra)["result"]


def volumes(log):
    """{channel: the loudest starting volume written} from an audio log."""
    out = {}
    for w in log:
        if w["addr"] in (NR12, NR22, NR42):
            ch = CHANNEL_OF[w["addr"]]
            out[ch] = max(out.get(ch, 0), w["value"] >> 4)
        elif w["addr"] == NR32:
            level = (w["value"] >> 5) & 3
            if level:
                out["wave"] = WAVE_LEVEL[level]
    return out


def loudness(path):
    """The loudest 1/30 s of a WAV's left channel, as RMS around its mean.

    Not the peak: every channel's DAC switching on pops at full scale,
    whatever its volume, so peaks all read alike."""
    with wave.open(str(path)) as f:
        rate = f.getframerate()
        left = memoryview(f.readframes(f.getnframes())).cast("h")[0::2]
    win, best = rate // 30, 0.0
    # From 0.2 s in: a channel file starts with a step (its DAC was already
    # on when the run began), which reads as loud as anything.
    for i in range(rate // 5, max(1, len(left) - win), win // 2):
        seg = left[i:i + win]
        mean = sum(seg) / len(seg)
        best = max(best, (sum((x - mean) ** 2 for x in seg) / len(seg)) ** 0.5)
    return round(best)


def main():
    args = g.arg_parser(__doc__.splitlines()[0]).parse_args()
    cli = g.Gbcli(args.gbcli, args.verbose)
    OUT.mkdir(parents=True, exist_ok=True)
    cli.text("sessions", "rm", SESSION)
    cli.run("run", g.ROM, "--session", SESSION, "--from", "boot", "--frames", "30")
    cli.run("run", "--session", SESSION, "--frames", "30", "--keys", "START@1 -START@2", "--checkpoint", "play")
    ok = True

    log = run(cli, 600)["audio"]["log"]
    music = volumes(log)
    ch1 = sum(1 for w in log if CHANNEL_OF.get(w["addr"]) == "square1")
    good = ch1 == 0 and all(isinstance(v, str) or v <= 7 for v in music.values())
    ok &= good
    print(f"{'PASS' if good else 'FAIL'} music: 600 frames, {len(log)} writes, channel 1 writes {ch1}; "
          f"starting volumes {music}")

    for name, pokes, keys, frames in EFFECTS:
        log = run(cli, frames, pokes, keys)["audio"]["log"]
        vol = volumes(log)
        good = vol.get("square1", 0) > max(v for v in music.values() if isinstance(v, int))
        ok &= good
        print(f"{'PASS' if good else 'FAIL'} effect {name}: channel 1 starts at volume {vol.get('square1')} "
              f"(music at {music.get('square2')} and below)")
    # The jingle: the score effect's notes on channel 1, from the decoded log
    # (a note starts where NR14 is written with its trigger bit), and gbcli's
    # piano roll of the same run.
    piano = OUT / "jingle-piano.png"
    log = run(cli, 60, EFFECTS[2][1], EFFECTS[2][2], ["--piano", str(piano), "--scale", "4"])["audio"]["log"]
    notes, lo = [], 0
    for w in log:
        if w["addr"] == NR13:
            lo = w["value"]
        elif w["addr"] == NR14 and w["value"] & 0x80:
            notes.append(round(131072 / (2048 - (lo | (w["value"] & 7) << 8))))
    rising = len(notes) >= JINGLE_MIN_NOTES and all(a < b for a, b in zip(notes, notes[1:]))
    ok &= rising
    print(f"{'PASS' if rising else 'FAIL'} jingle: the score effect plays {len(notes)} notes on channel 1, "
          f"{'each higher than the last' if rising else 'not a rising run'} "
          f"(Hz {notes}; want {JINGLE_MIN_NOTES}+ rising; {piano.relative_to(g.PROJECT)})")
    # The score effect 20 frames in, after the replay's opening step.
    wav = OUT / "sound-score.wav"
    run(cli, 60, EFFECTS[2][1], "B@20 -B@21", ["--wav", str(wav), "--channels", "each"])
    rms = {ch: loudness(wav.with_name(f"{wav.stem}-{ch}.wav")) for ch in ("square1", "square2", "wave", "noise")}
    # The three music channels together (uncorrelated, so their RMS add as squares).
    music_rms = round(sum(rms[ch] ** 2 for ch in ("square2", "wave", "noise")) ** 0.5)
    good = rms["square1"] > 1.5 * music_rms
    ok &= good
    print(f"{'PASS' if good else 'FAIL'} louder: the score effect's loudest 1/30 s is RMS {rms['square1']}, "
          f"the whole music's {music_rms}, x{rms['square1'] / music_rms:.1f} (per channel {rms}; "
          f"{wav.relative_to(g.PROJECT)} and its -<channel> files)")
    print(f"{cli.calls} gbcli calls")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
