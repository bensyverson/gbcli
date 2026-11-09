use super::*;
use gbcli_protocol::Flip;

/// The symbols the generated files define: exported labels in the
/// assembly, constants in the include.
fn defined(asm: &str, inc: &str) -> Vec<String> {
    let labels = asm.lines().filter_map(|l| l.strip_suffix("::"));
    let constants = inc
        .lines()
        .filter_map(|l| l.strip_prefix("DEF ")?.split_once(" EQU"))
        .map(|(name, _)| name)
        .filter(|&name| name != "GBCLI_IMAGES_INC");
    let mut all: Vec<String> = labels.chain(constants).map(str::to_string).collect();
    all.sort();
    all
}

/// With a GBC version, every symbol `symbols` reserves is defined.
#[test]
fn symbols_lists_what_each_kind_defines_but_frame_labels() {
    let palettes = [gbcli_protocol::GbcPalette(
        [gbcli_protocol::Rgb555::WHITE; 4],
    )];
    let plan = SheetPlan {
        object: gbcli_protocol::ObjectSize::Tall,
        spec: String::new(),
        merges: Vec::new(),
        obp: 0,
        width: 1,
        height: 2,
        frame: (8, 16),
        kept: vec![(0, 0)],
        drawn: vec![vec![0; 128]],
        uses: vec![2],
        frames: vec![Vec::new(), Vec::new()],
        units: vec![Some((0, Flip::None))],
        warnings: Vec::new(),
    };
    let entries = [
        Entry::Bg {
            source: "town.png",
            label: "Town",
            constant: "TOWN",
            data: "town",
            width: 1,
            height: 1,
            tiles: 1,
            gbc: Some(Gbc {
                tiles: true,
                palettes: &palettes,
                slots: Vec::new(),
            }),
        },
        Entry::Sheet {
            source: "hero.png",
            label: "Hero",
            constant: "HERO",
            data: "hero",
            plan: &plan,
            gbc: Some(Gbc {
                tiles: true,
                palettes: &[],
                slots: vec![0],
            }),
        },
        Entry::Tiles {
            source: "font.png",
            label: "Font",
            constant: "FONT",
            data: "font",
            tiles: 1,
            gbc: Some(Gbc {
                tiles: true,
                palettes: &palettes,
                slots: vec![0],
            }),
        },
    ];
    let shared = Shared {
        gbc: None,
        named: &[],
        objects: None,
    };
    let mut listed: Vec<String> = symbols(ImageKind::Bg, "Town", "TOWN")
        .into_iter()
        .chain(symbols(ImageKind::Sprites, "Hero", "HERO"))
        .chain(symbols(ImageKind::Tiles, "Font", "FONT"))
        .chain(["HeroFrame0".into(), "HeroFrame1".into()])
        .collect();
    listed.sort();
    assert_eq!(
        defined(&asm(&entries, &shared), &inc(&entries, &shared)),
        listed
    );
}

/// An indexed sheet defines a copy of each named palette, reordered by its
/// OBP, under the labels `swap_symbols` reserves; nothing it defines is
/// unreserved.
#[test]
fn an_indexed_sheet_defines_its_swap_symbols() {
    use gbcli_protocol::{GbcPalette, NamedPalette, Rgb555};
    let colors = [
        Rgb555::WHITE,
        Rgb555::new(1, 2, 3).unwrap(),
        Rgb555::new(4, 5, 6).unwrap(),
        Rgb555::BLACK,
    ];
    let named = [NamedPalette {
        name: "ice_2".into(),
        colors: GbcPalette(colors),
    }];
    let plan = SheetPlan {
        object: gbcli_protocol::ObjectSize::Tall,
        spec: String::new(),
        merges: Vec::new(),
        obp: 0b11_10_00_00,
        width: 1,
        height: 2,
        frame: (8, 16),
        kept: vec![(0, 0)],
        drawn: vec![vec![0; 128]],
        uses: vec![1],
        frames: vec![Vec::new()],
        units: vec![Some((0, Flip::None))],
        warnings: Vec::new(),
    };
    let entries = [Entry::Sheet {
        source: "hero.png",
        label: "Hero",
        constant: "HERO",
        data: "hero",
        plan: &plan,
        gbc: Some(Gbc {
            tiles: false,
            palettes: &[],
            slots: vec![0],
        }),
    }];
    let shared = Shared {
        gbc: None,
        named: &named,
        objects: None,
    };
    let asm = asm(&entries, &shared);
    let swaps = swap_symbols("Hero", "ice_2");
    let reserved: Vec<String> = symbols(ImageKind::Sprites, "Hero", "HERO")
        .into_iter()
        .chain(palette_symbols("ice_2"))
        .chain(swaps.iter().cloned())
        .chain(["HeroFrame0".into()])
        .collect();
    for symbol in defined(&asm, &inc(&entries, &shared)) {
        assert!(reserved.contains(&symbol), "{symbol} is unreserved");
    }
    assert_eq!(swaps, ["HeroPaletteIce2", "HeroPaletteIce2End"]);
    // Index 1 is shade 0, 2 shade 2, 3 shade 3.
    let want = gbc::dw(&GbcPalette([colors[0], colors[0], colors[2], colors[3]]));
    let block = format!("HeroPaletteIce2::\n{want}HeroPaletteIce2End::\n");
    assert!(asm.contains(&block), "{asm}");
}
