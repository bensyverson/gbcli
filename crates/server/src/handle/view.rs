//! The `view` verb (design §9, §12): tiles, a tilemap or OAM from the head,
//! as data from `inspect::video`, and as the view crate's PNG when asked.
//! Neither reads more than the core's memory, so the head stays where it was.

use std::path::Path;

use gbcli_bundle::SessionStore;
use gbcli_core::{Core, Region};
use gbcli_engine::Machine;
use gbcli_engine::inspect::video::{
    self, CgbPalette, MapArea, OamEntry, ObjectHeight, TILES_PER_BANK, TileAddressing, oam, tile,
    tilemap,
};
use gbcli_protocol::{
    AddressingView, CgbColorView, CgbPaletteView, DmgPaletteView, ImageMade, MapAreaView, MapSel,
    OamView, ResultBody, SessionFile, TileView, TimeBase, VideoData, ViewArgs, ViewKind, ViewMade,
};
use gbcli_view::{DEFAULT_SCALE, Image, MapChoice, TilePalette, ViewError, map_layout};

use super::shot::{check_image, view_failure, write_atomically};
use super::{Failure, Handler, Need};

const VRAM_BANK: usize = 0x2000;

impl Handler {
    pub(super) fn view(
        &mut self,
        store: &SessionStore,
        meta: &SessionFile,
        args: &ViewArgs,
        warnings: &mut Vec<String>,
    ) -> Result<ResultBody, Failure> {
        if args.map.is_some() && args.what != ViewKind::Map {
            return Err(Failure::new(gbcli_protocol::ErrorBody::new(
                gbcli_protocol::ErrorCode::BadRequest,
                "only the map view takes a map choice",
                "drop --map, or view map",
            )));
        }
        if args.png.is_some() && args.what == ViewKind::Palettes {
            return Err(Failure::new(gbcli_protocol::ErrorBody::new(
                gbcli_protocol::ErrorCode::BadRequest,
                "the palettes view is text only",
                "drop --png; `gbcli shot` shows the colors on screen",
            )));
        }
        if let Some(png) = &args.png {
            check_image(&png.path, png.scale)?;
        }
        let (machine, loaded) = self.machine_for(store, meta, Need::Head, None)?;
        warnings.extend(loaded);
        let made = view_of(&machine, args);
        self.keep_warm(store, meta, machine);
        Ok(ResultBody {
            view: Some(made?),
            ..ResultBody::default()
        })
    }
}

fn view_of(machine: &Machine, args: &ViewArgs) -> Result<ViewMade, Failure> {
    let core = machine.core();
    let failed = |e: ViewError| view_failure(&e, &|n| n as i64, &TimeBase::PowerOn);
    let choice = match args.map.unwrap_or_default() {
        MapSel::Background => MapChoice::Background,
        MapSel::Window => MapChoice::Window,
        MapSel::Map9800 => MapChoice::Area(MapArea::Map9800),
        MapSel::Map9C00 => MapChoice::Area(MapArea::Map9C00),
    };
    let data = match args.what {
        ViewKind::Tiles => tiles(core)?,
        ViewKind::Map => map(core, choice).map_err(failed)?,
        ViewKind::Oam => objects(core).map_err(failed)?,
        ViewKind::Palettes => palettes(core)?,
    };
    let png = match &args.png {
        None => None,
        Some(out) => {
            let image = match args.what {
                ViewKind::Tiles => gbcli_view::tileset(core, TilePalette::Background(0)),
                ViewKind::Map => gbcli_view::tilemap(core, choice),
                ViewKind::Oam => gbcli_view::oam_sheet(core),
                ViewKind::Palettes => unreachable!("refused before the machine loads"),
            };
            Some(write_png(image.map_err(failed)?, &out.path, out.scale)?)
        }
    };
    Ok(ViewMade { data, png })
}

fn write_png(image: Image, path: &str, scale: Option<u32>) -> Result<ImageMade, Failure> {
    let failed = |e: ViewError| view_failure(&e, &|n| n as i64, &TimeBase::PowerOn);
    let scale = scale.unwrap_or(DEFAULT_SCALE);
    let image = image.scaled(scale).map_err(failed)?;
    let bytes = image.png().map_err(failed)?;
    write_atomically(Path::new(path), &bytes)?;
    Ok(ImageMade {
        path: path.to_string(),
        width: image.width,
        height: image.height,
        scale,
    })
}

fn tiles(core: &dyn Core) -> Result<VideoData, Failure> {
    let vram = core
        .region(Region::Vram)
        .map_err(|e| Failure::from_core(&e))?;
    let banks = (vram.bytes.len() / VRAM_BANK) as u8;
    let mut shown = Vec::new();
    let mut blank = 0;
    for bank in 0..banks {
        for index in 0..TILES_PER_BANK {
            let t = tile(core, bank, index).map_err(|e| Failure::from_core(&e))?;
            if t.pixels.iter().flatten().all(|&p| p == 0) {
                blank += 1;
                continue;
            }
            let rows = t
                .pixels
                .iter()
                .map(|row| row.iter().map(|p| char::from(b'0' + p)).collect())
                .collect();
            shown.push(TileView { bank, index, rows });
        }
    }
    Ok(VideoData::Tiles {
        tiles: shown,
        blank,
    })
}

fn hex_rows(bytes: impl Iterator<Item = u8>) -> Vec<String> {
    let bytes: Vec<String> = bytes.map(|b| format!("{b:02x}")).collect();
    bytes.chunks(32).map(|row| row.join(" ")).collect()
}

fn map(core: &dyn Core, choice: MapChoice) -> Result<VideoData, ViewError> {
    let layout = map_layout(core, choice)?;
    let map = tilemap(core, layout.area, layout.addressing)?;
    let ids = hex_rows(map.entries.iter().map(|e| e.tile_id));
    // Bank 1 exists only on a CGB; its attribute bytes mean nothing on a DMG.
    let attributes = map
        .entries
        .iter()
        .map(|e| {
            e.attributes.map(|a| {
                a.palette
                    | a.bank << 3
                    | u8::from(a.x_flip) << 5
                    | u8::from(a.y_flip) << 6
                    | u8::from(a.priority) << 7
            })
        })
        .collect::<Option<Vec<u8>>>()
        .map(|bytes| hex_rows(bytes.into_iter()));
    Ok(VideoData::Map {
        area: match layout.area {
            MapArea::Map9800 => MapAreaView::Map9800,
            MapArea::Map9C00 => MapAreaView::Map9C00,
        },
        addressing: match layout.addressing {
            TileAddressing::Unsigned8000 => AddressingView::Unsigned8000,
            TileAddressing::Signed8800 => AddressingView::Signed8800,
        },
        background: layout.background,
        ids,
        attributes,
    })
}

fn palettes(core: &dyn Core) -> Result<VideoData, Failure> {
    let p = video::palettes(core).map_err(|e| Failure::from_core(&e))?;
    let set = |palettes: &[CgbPalette; 8]| {
        palettes
            .iter()
            .enumerate()
            .map(|(i, p)| CgbPaletteView {
                index: i as u8,
                colors: p.colors.iter().map(|&c| CgbColorView::of(c)).collect(),
            })
            .collect()
    };
    let (bg, obj) = match &p.cgb {
        Some((bg, obj)) => (set(bg), set(obj)),
        None => (Vec::new(), Vec::new()),
    };
    Ok(VideoData::Palettes {
        bgp: DmgPaletteView::of(p.bgp),
        obp0: DmgPaletteView::of(p.obp0),
        obp1: DmgPaletteView::of(p.obp1),
        bg,
        obj,
    })
}

fn objects(core: &dyn Core) -> Result<VideoData, ViewError> {
    let height = gbcli_view::object_height(core)?;
    let objects = oam(core, height)?.iter().map(oam_view).collect();
    Ok(VideoData::Oam {
        height: match height {
            ObjectHeight::Eight => 8,
            ObjectHeight::Sixteen => 16,
        },
        objects,
    })
}

/// One OAM entry on the wire, as `view oam` and `run --oam-log` give it.
pub(super) fn oam_view(o: &OamEntry) -> OamView {
    OamView {
        index: o.index,
        y: o.y,
        x: o.x,
        tile: o.tile,
        flags: o.flags,
        screen_x: o.screen_x,
        screen_y: o.screen_y,
        on_screen: o.on_screen,
        dropped_by_line_limit: o.dropped_by_line_limit,
    }
}
