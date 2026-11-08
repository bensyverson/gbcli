//! `gbcli schema`: the request grammar `serve` and `batch` read (design §5).
//! A test parses every example line with the protocol's own parser, so the
//! text can't drift from the types.

pub const SCHEMA: &str = r#"Requests — one JSON object per line, for `gbcli serve` and `gbcli batch -F`.

  {"id": TEXT, "session"?: NAME, "as"?: NAME, "rev"?: "sN",
   "allow_rom_change"?: true, "op": OP, "args"?: {…}}

id is yours and is echoed back. session and as default to serve's --session
and --as. A mutation that passes rev is refused (rev_mismatch) if the session
moved on. Unknown and repeated fields are refused, naming the nearest field.
Fields marked ? may be left out; an op with no required field may omit args.

OP      ARGS
run     at? from?: "boot"|"power-on"  since?: "boot"|"power-on"|CHECKPOINT
        until?: EXPR  to?: ADDR  frames? instructions? ticks?: N
        writes?: [{kind: "bus"|"poke"|"patch", target: ADDR, bytes: HEX}]
        read?: [EXPR]  checkpoint?: NAME  allow_uninit?: bool
        keys?: ["A@0", "-A@2", …]  shot?: SHOT
        measure?: {from: ADDR, to: ADDR}  profile?: {top?: N}
        frame_budget?: bool  raster?: bool
measure from: ADDR  to: ADDR  at?  iterations?: N  max_t?: N
        frames? instructions? ticks?: N
equiv   frames: N  b?: {rom?: PATH, sym?: PATH, writes?: [WRITE]}  at?  from?
        keys?  ram?: [{at: ADDR, len: N}]  audio?: "writes"|"timed"|"off"
shot    shot: SHOT  since?
        SHOT = {path: PATH, scale?: 1-8, crop?: {x, y, width, height},
                live?: bool, any_frame?: bool}
view    what: "tiles"|"map"|"oam"  map?: "background"|"window"|"9800"|"9c00"
        png?: {path: PATH, scale?: N}
sym     pattern: GLOB
checkpoints  —
step    unit?: "instruction"|"over"|"out"|"frame"|"scanline"
show    —
read    reads: [EXPR]  since?  allow_uninit?
        (each read is {expr, value}; "oam[N]" is {expr, oam: {index, y, x,
        tile, attr, priority, yflip, xflip, palette?, cgb_bank?, cgb_palette?}}:
        palette outside CGB mode, cgb_bank and cgb_palette in it)
break   at: ADDR  condition?: EXPR
watch   access: "read"|"write"|"access"  at: ADDR  condition?: EXPR  once?: bool
delete  id: N
checkpoint  name: NAME
restore name: NAME|"~N"
undo    —
write   writes: [{kind, target, bytes}]   (bytes: lowercase hex, "0a02")
events  limit?: N
sessions, gaps   —
prune   unused_for: "30m"|"12h"|"7d"
rm      name: NAME  with_checkpoints?: bool
build   dir?: PATH  src?: [PATH]  include?: [PATH]  out?: PATH
        target?: "dmg"|"gbc"  ("gbc": dual mode, rgbfix -c; default "dmg")
        restore?: CHECKPOINT  (into session, whose ROM must be the build's)
doctor  —
new     dir: PATH
opcode  query: TEXT  ("$20", "$CB37", "jr nz, .loop", "ld a, [hl+]")
        (build, doctor, new and opcode need no session; paths are relative
        to the server's working directory)
power_cycle  sav?: PATH  rtc_advance?: "45s"|"30m"|"12h"|"7d"
        (a new head at power-on, keeping the cart's battery RAM and clock;
        rtc_advance at most "1024d" on MBC3, "3584d" on TPP1, "65536d" on HuC3;
        a flash session (serve --flash) also imports the chip image beside
        it, game.sav → game.flash.gb; either file may be missing)
sav     path: PATH   (writes the head's battery save, and on a flash session
        the chip image beside it, game.sav → game.flash.gb; changes nothing)

Response, one line per request:
  {"id", "ok": true, "rev"?: "sN", "result": {…}, "warnings": [TEXT]}
  {"id", "ok": false, "rev"?, "error": {"code", "exit", "message", "remedy",
   "stop"?, "flash_notes"?, "result"?}, "warnings"}
result holds only the fields the op fills: stop, reads, shot, measure, profile,
frame_budget, raster, equiv, view, symbols,
checkpoints, keys_unapplied, writes, checkpoint, restored, breakpoint, watch,
deleted, registers, ppu, sessions, events, gaps, pruned, kept, busy, removed,
build, doctor, created, song, song_problem, opcodes, power_cycle, sav,
flash_notes (run, step: each flash note, also a warning; a failed run's or
step's are on its error).
`gbcli VERB --json` prints the whole response line, as serve writes it.

GBC images, in result.build.images (gbcli help images). A --gbc build fills
them for indexed art; color art's GBC conversion is not built yet.
  images[]  tile_map?: [ENTRY|null]  (each 8x8 tile, each 8x16 unit of a
            sheet, rows top to bottom; null: a transparent unit, no tile)
            gbc?: {art: "color"|"indexed", palette?: NAME,
            palettes?: [PALETTE]}
            kind: "bg"|"sprites"|"tiles" (tiles: a tileset, no map)
  gbc?: {obj_palettes: [PALETTE]}  (the OBJ palettes every sheet shares,
            slot order: assigned palettes first, then packed ones)
  problems[].limit also: gbc_palettes, gbc_tile_colors, dmg_override_size
ENTRY = {index: N, flip?: "x"|"y"|"xy", palette?: 0-7}: the tile in the
image's own data (a sheet unit's top tile, even), how a sheet unit is
flipped, and its GBC palette slot (on a GBC image, every entry has one).
Named palettes (assets/palettes.txt reads into this; refused like the file:
a name that can't be a label, a repeated name, "dmg" as a name or assigned,
an unknown palette, an IMAGE not spelled bg/, sprites/ or tiles/ NAME.png):
  {dmg?: PALETTE, palettes: [{name: NAME, colors: PALETTE}],
   assignments?: {IMAGE: NAME}}   (IMAGE: its path under assets/)
PALETTE = [RGB555 ×4], index 0 first. RGB555 = 0-32767, the word BCPD and
OCPD take: red in bits 0-4, green 5-9, blue 10-14.
  gbc   {"art":"indexed","palette":"forest"}
  entry {"index":2,"flip":"x","palette":1}
  named {"dmg":[32767,22197,10570,0],"palettes":[{"name":"forest","colors":[32767,9009,6534,2176]}],"assignments":{"bg/town.png":"forest"}}

Examples:
  {"id":"1","session":"dx","op":"run","args":{"frames":120}}
  {"id":"2","op":"run","args":{"until":"u16[wFrame] >= 60","read":["[wSpeed]"]}}
  {"id":"3","op":"run","args":{"at":"before","writes":[{"kind":"poke","target":"wSpeed","bytes":"02"}],"frames":60}}
  {"id":"4","op":"step","args":{"unit":"over"}}
  {"id":"5","op":"show"}
  {"id":"6","op":"read","args":{"reads":["u16[wFrame]","pc"]}}
  {"id":"7","op":"break","args":{"at":"Main.loop","condition":"a == 3"}}
  {"id":"8","op":"watch","args":{"access":"write","at":"rDMA","once":true}}
  {"id":"9","op":"delete","args":{"id":1}}
  {"id":"10","op":"checkpoint","args":{"name":"before"}}
  {"id":"11","op":"restore","args":{"name":"~1"}}
  {"id":"12","rev":"s4","op":"undo"}
  {"id":"13","op":"write","args":{"writes":[{"kind":"bus","target":"rLCDC","bytes":"91"}]}}
  {"id":"14","op":"events","args":{"limit":5}}
  {"id":"15","op":"sessions"}
  {"id":"16","op":"gaps"}
  {"id":"17","op":"prune","args":{"unused_for":"7d"}}
  {"id":"18","op":"rm","args":{"name":"dx","with_checkpoints":true}}
  {"id":"19","op":"run","args":{"frames":120,"keys":["START@10","-START@12"],"shot":{"path":"a.png"}}}
  {"id":"20","op":"shot","args":{"shot":{"path":"now.png","scale":2,"any_frame":true}}}
  {"id":"21","op":"view","args":{"what":"map","map":"window","png":{"path":"m.png"}}}
  {"id":"22","op":"sym","args":{"pattern":"w*"}}
  {"id":"23","op":"checkpoints"}
  {"id":"24","op":"measure","args":{"from":"Main.loop","to":"Main.loop","iterations":60,"max_t":40000}}
  {"id":"25","op":"run","args":{"frames":60,"profile":{"top":10},"frame_budget":true}}
  {"id":"26","op":"equiv","args":{"b":{"rom":"fast.gb"},"frames":600,"keys":["RIGHT@10"],"ram":[{"at":"wX","len":1}]}}
  {"id":"27","session":"game","op":"build","args":{"dir":"game","restore":"start"}}
  {"id":"28","op":"doctor"}
  {"id":"29","op":"new","args":{"dir":"game"}}
  {"id":"30","op":"opcode","args":{"query":"jr nz, .loop"}}
  {"id":"31","op":"power_cycle","args":{"sav":"game.sav","rtc_advance":"8h"}}
  {"id":"32","op":"sav","args":{"path":"game.sav"}}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use gbcli_protocol::parse_request;

    #[test]
    fn every_example_parses_and_every_op_has_one() {
        let examples: Vec<&str> = SCHEMA
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with(r#"{"id":""#))
            .collect();
        let mut ops: Vec<&str> = Vec::new();
        for line in &examples {
            let request = parse_request(line).unwrap_or_else(|e| panic!("{line}: {e:?}"));
            ops.push(request.op.name());
        }
        ops.sort();
        ops.dedup();
        assert_eq!(ops.len(), 28, "one example per op: {ops:?}");
    }

    /// The GBC image types (leaf ZA5Z7c): their examples parse into the
    /// protocol's own types, and every wire name they add is listed.
    #[test]
    fn the_gbc_image_types_are_listed_and_their_examples_parse() {
        use gbcli_protocol::{
            Flip, GbcImage, ImageArt, ImageKind, ImageLimit, NamedPalettes, Rgb555, TileEntry,
        };
        let example = |tag: &str| {
            SCHEMA
                .lines()
                .find_map(|l| l.trim().strip_prefix(tag))
                .unwrap_or_else(|| panic!("no {tag:?} example"))
                .trim()
        };
        let gbc: GbcImage = serde_json::from_str(example("gbc   ")).unwrap();
        assert_eq!(
            gbc.art,
            ImageArt::Indexed {
                palette: "forest".into()
            }
        );
        let entry: TileEntry = serde_json::from_str(example("entry ")).unwrap();
        assert_eq!((entry.index, entry.flip), (2, Flip::X));
        let named: NamedPalettes = serde_json::from_str(example("named ")).unwrap();
        assert_eq!(named.assignments["bg/town.png"], "forest");
        assert!(named.get("forest").is_some());
        // The dmg entry is the grays #ffffff #aaaaaa #555555 #000000.
        let grays = [0xFF, 0xAA, 0x55, 0x00].map(|v| Rgb555::from_rgb888(v, v, v));
        assert_eq!(named.dmg.map(|d| d.0), Some(grays));
        for limit in [
            ImageLimit::GbcPalettes,
            ImageLimit::GbcTileColors,
            ImageLimit::DmgOverrideSize,
        ] {
            assert!(SCHEMA.contains(limit.name()), "{}", limit.name());
        }
        for word in [
            "tile_map",
            "\"xy\"",
            "obj_palettes",
            "\"indexed\"",
            "assignments",
            ImageKind::Tiles.dir(),
        ] {
            assert!(SCHEMA.contains(word), "{word}");
        }
    }
}
