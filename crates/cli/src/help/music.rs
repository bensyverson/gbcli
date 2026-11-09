//! `gbcli help music`: the song text form, its effects (from gbcli-music's
//! table, so they can't drift) and the worked example.

use std::sync::LazyLock;

const HEAD: &str = "\
Music — hUGETracker songs as text an agent can read and edit.

  gbcli song theme.uge --text theme.song    a .uge (versions 0-6) as text
  gbcli song theme.song --uge theme.uge     the text as a .uge (version 6)
  gbcli song theme.song --piano theme.png   its notes as a piano roll, with
                                            no build; `gbcli song FILE` checks

In a project, `gbcli build` converts every .song or .uge in assets/music to
hUGEDriver data (build/music/NAME.asm, label song_NAME); `gbcli new` makes
assets/music/theme.song (the example below) and src/music.asm, which plays
it. Then `gbcli run --frames 600 --audio-log --piano roll.png` to hear it;
for a short clip, `--spectrogram spec.png --scale 4` to see its spectrum.
The spectrogram runs time left to right, 64 columns a second at its default
scale of 1; frequency bottom to top, linear, 0 to 8160 Hz in 32 Hz rows; and
loudness as color in 64 steps from black (-90 dBFS or quieter) through
blue, purple, red and yellow to white (full scale).

The starter's song plays on all four channels, so a sound effect has none
to itself. Make room by dropping the music calls (MusicInit, MusicVBlank),
or by muting the effect's channel after MusicInit (hUGE_init unmutes all
four): ld b, CH (0-3 for CH1-CH4), ld c, 1 (0 unmutes), call
hUGE_mute_channel. The driver then leaves that channel alone until it is
unmuted.

The data's labels are the song's: song_NAME.P0 for a pattern, and for
routine N song_NAME__hUGE_Routine_N and song_NAME__end_hUGE_Routine_N, not
hUGETracker's __hUGE_Routine_N and __end_hUGE_Routine_N. Routine code that
names hUGETracker's labels fails at link; name the song's instead.

A song with `timer on` ticks the driver from the timer interrupt, about
2,500-5,900 T a tick, and a tick that fires just before VBlank holds the
VBlank handler back until it returns: with `timer on 192` in the starter,
its OAM DMA ran on lines 145-153, past the start of VBlank, in 16 of 600
frames (`run --frames 600 --raster`). Keep VBlank work short, or call
hUGE_dosound once a frame (`timer off`) if that matters.

Each line's first word says what it is; # starts a comment line.

  format gbcli-song 1           the first line
  name|artist|comment \"TEXT\"    quoted; escapes \\\" \\\\ \\n \\r \\t \\xNN
  ticks_per_row N               driver ticks per row, 1-255
  timer off | timer on D        on: the driver ticks 4096/(256-D) times a
                                second, from the timer, not once a frame
  wave N HEX                    wave 0-15: 32 samples, one hex digit each
  instrument BANK N \"NAME\"      BANK is square (channels 1, 2), wave (3) or
                                noise (4); N is 1-15. Its lines:
    length off | length N         N steps of 1/256 s, 1-64 (wave 1-256)
    envelope V up|down P          square, noise: volume 0-15, pace 0-7
                                  (one step every P/64 s; 0 holds)
    sweep T up|down S             square: time 0-7, shift 0-7
    duty 12.5%|25%|50%|75%        square
    level 100%|50%|25%|0%         wave; and waveform N, the wave it plays
    mode 15-bit|7-bit             noise
    subpattern on|off             then rows NN OFFSET .. JUMP EFFECT, one
                                  per tick: OFFSET +00/-05 transposes the
                                  note (... none); JUMP J05 goes next to
                                  row 04 (J counts from 1; ... next row).
                                  hUGEDriver plays rows 00-31 and jumps
                                  J01-J31; `song` warns of any past them
    unused KEY=N                  stored numbers this bank ignores, kept
  routine N [crlf]              code effect 6 calls: | lines, then end
  orders                        then NN P0 P1 P2 P3: the pattern each channel
                                plays at each order, in play order; every
                                channel names one on every order line
  storage P3 P1 ...             the .uge's pattern order, when not by number
  pattern P0 P1 P2 P3           then rows: NN cell | cell | cell | cell;
                                - for none, = for a pattern another block
                                shows. Fewer columns: pattern P4 is
                                pattern P4 = = =, rows then one cell each

A cell is hUGETracker's clipboard spelling: note, instrument, volume, effect.
  C-5 01 ... 2FF   note C-3 to B-8, or ...; instrument 01-15 from the
                   channel's bank, or ..; volume is unused, always ...;
                   effect: code and parameter in hex, or ... for none
Note names are hUGETracker's, an octave above scientific pitch:
C-3 sounds as C2 (65 Hz), and `run --audio-log` names the note it plays C2.
A pattern several orders play needs its rows written once: write them in
one block, and name it = (or leave its column off the end) in the others.
A block that names it again must read the same. A row has one cell per
column its pattern line names. Rows, fields, instruments and waves left out
take hUGETracker's defaults; --text writes the canonical form, with all four
columns of every block, all 64 rows and every field. A value a file stores
outside its range is written !N.

Effects (x and y are the parameter's hex digits):
";

const EXAMPLE_HEAD: &str = "
Example: two orders. P1-P3 play under both leads; the first block shows
them, and the second names only P4.

";

/// The topic's text, built once.
pub fn text() -> &'static str {
    static TEXT: LazyLock<String> = LazyLock::new(|| {
        let mut out = String::from(HEAD);
        for e in gbcli_music::EFFECTS {
            let line = format!("{} {} — {}", e.spelling, e.name, e.meaning);
            out += &wrap(&line, 80, 2, 6);
        }
        out += EXAMPLE_HEAD;
        out += gbcli_music::EXAMPLE;
        out
    });
    &TEXT
}

/// `line` indented by `first`, wrapped at `width`, with continuation lines
/// indented by `rest`.
fn wrap(line: &str, width: usize, first: usize, rest: usize) -> String {
    let mut out = String::new();
    let mut current = " ".repeat(first);
    let mut fresh = true;
    for word in line.split(' ') {
        let len = current.chars().count();
        if !fresh && len + 1 + word.chars().count() > width {
            out += &current;
            out.push('\n');
            current = " ".repeat(rest);
            fresh = true;
        }
        if !fresh {
            current.push(' ');
        }
        current += word;
        fresh = false;
    }
    out += &current;
    out.push('\n');
    out
}
