//! The structured OAM forms (design §7): `oam[N].field` in expressions and
//! `object_attributes`, which reads raw OAM and decodes the attribute byte
//! for the PPU's color mode.

use crate::common;

use common::*;
use gbcli_core::{
    ApuState, Core, CoreError, CoreInfo, Feature, FrameRef, IoReg, Keys, LayerMap, PpuMode,
    PpuStatus, Region, RegionView, Registers, RunRequest, StepUnit, Stop, Support, Ticks,
    WrittenView,
};
use gbcli_engine::Machine;
use gbcli_engine::cond::{Condition, EvalCtx, EvalError};
use gbcli_engine::inspect::video::object_attributes;
use gbcli_protocol::TimeMark;
use gbcli_sameboy::SameBoy;

const POWER_ON: TimeMark = TimeMark { ticks: 0, frame: 0 };

fn eval(m: &Machine, text: &str) -> Result<i64, EvalError> {
    let ctx = EvalCtx {
        core: m.core(),
        symbols: m.symbols(),
        access: None,
        base: POWER_ON,
        allow_uninit: true,
    };
    Condition::compile(text, m.symbols())
        .unwrap_or_else(|e| panic!("{text}: {e:?}"))
        .eval(&ctx, &mut Vec::new())
}

/// OAM byte i holds i.
fn oam_counting(m: &mut Machine) {
    let bytes: Vec<u8> = (0..160u8).collect();
    m.core_mut().poke(Region::Oam, 0, &bytes).unwrap();
}

#[test]
fn oam_fields_evaluate_to_the_right_byte_in_any_expression() {
    let mut m = machine(BUSY, None);
    oam_counting(&mut m);
    let cases: &[(&str, i64)] = &[
        ("oam[ 0 ].x", 1),
        ("oam[$27].x", 157),
        ("oam[0x27].tile", 158),
        ("oam[39].attr", 159),
        ("oam[0].x+1", 2),
        ("oam[0].x * 2 + oam[1].y", 6),
        ("-oam[0].x", -1),
        ("!oam[0].y", 1),
        ("oam[0].x==1&&oam[1].x==5", 1),
        ("oam[2].tile<<1", 20),
    ];
    for (text, want) in cases {
        assert_eq!(eval(&m, text).unwrap(), *want, "{text}");
    }
    // A field inside brackets is dereferenced as an address.
    assert_eq!(eval(&m, "[oam[1].attr]").unwrap(), eval(&m, "[7]").unwrap());
    for bad in ["oam[-1].x", "oam[40].x", "oam[0].x.y", "oam[1+1].x"] {
        assert!(Condition::compile(bad, None).is_err(), "{bad}");
    }
}

#[test]
fn a_symbol_named_oam_still_reads_as_its_address() {
    let mut m = machine(BUSY, Some("00:c100 oam\n"));
    oam_counting(&mut m);
    assert_eq!(eval(&m, "oam[0].x").unwrap(), 1);
    assert_eq!(eval(&m, "oam+1").unwrap(), 0xC101);
}

/// Review of e65e509: a field is the byte in OAM, not what the CPU reads
/// there; in modes 2 and 3 the CPU reads $FF. `gbcli help expressions` and
/// design §7 say so.
#[test]
fn an_oam_field_reads_raw_oam_while_the_cpu_is_locked_out() {
    let mut m = machine(BUSY, None);
    oam_counting(&mut m);
    for _ in 0..200_000 {
        let ppu = m.core().ppu();
        if ppu.lcd_on && matches!(ppu.mode, Some(PpuMode::Drawing | PpuMode::OamScan)) {
            assert_eq!(m.core_mut().bus_read(0xFE01).unwrap(), 0xFF, "the CPU");
            assert_eq!(eval(&m, "oam[0].x").unwrap(), 1, "the field");
            return;
        }
        steps(&mut m, 1);
    }
    panic!("never saw mode 2 or 3 with the LCD on");
}

/// SameBoy, without a color mode: the trait's default refuses.
struct NoColorMode(SameBoy);

impl Core for NoColorMode {
    fn info(&self) -> CoreInfo {
        self.0.info()
    }
    fn supports(&self, feature: Feature) -> Support {
        self.0.supports(feature)
    }
    fn run(&mut self, req: &mut RunRequest<'_>) -> Result<Stop, CoreError> {
        self.0.run(req)
    }
    fn step(&mut self, unit: StepUnit) -> Result<Stop, CoreError> {
        self.0.step(unit)
    }
    fn now(&self) -> Ticks {
        self.0.now()
    }
    fn registers(&self) -> Registers {
        self.0.registers()
    }
    fn in_stop(&self) -> bool {
        self.0.in_stop()
    }
    fn region(&self, region: Region) -> Result<RegionView<'_>, CoreError> {
        self.0.region(region)
    }
    fn rom0_bank(&self) -> u16 {
        self.0.rom0_bank()
    }
    fn io_register(&self, reg: IoReg) -> Result<u8, CoreError> {
        self.0.io_register(reg)
    }
    fn ppu(&self) -> PpuStatus {
        self.0.ppu()
    }
    fn last_complete_frame(&self) -> Option<FrameRef<'_>> {
        self.0.last_complete_frame()
    }
    fn pixel_layers(&self) -> Result<LayerMap<'_>, CoreError> {
        self.0.pixel_layers()
    }
    fn set_pixel_layers(&mut self, on: bool) -> Result<(), CoreError> {
        self.0.set_pixel_layers(on)
    }
    fn apu(&self) -> Result<ApuState, CoreError> {
        self.0.apu()
    }
    fn written(&self, region: Region) -> Result<WrittenView<'_>, CoreError> {
        self.0.written(region)
    }
    fn bus_write(&mut self, addr: u16, value: u8) -> Result<(), CoreError> {
        self.0.bus_write(addr, value)
    }
    fn bus_read(&mut self, addr: u16) -> Result<u8, CoreError> {
        self.0.bus_read(addr)
    }
    fn poke(&mut self, region: Region, offset: u32, bytes: &[u8]) -> Result<(), CoreError> {
        self.0.poke(region, offset, bytes)
    }
    fn patch_rom(&mut self, offset: u32, bytes: &[u8]) -> Result<(), CoreError> {
        self.0.patch_rom(offset, bytes)
    }
    fn set_registers(&mut self, regs: &Registers) {
        self.0.set_registers(regs)
    }
    fn set_keys(&mut self, keys: Keys) {
        self.0.set_keys(keys)
    }
    fn save(&mut self) -> Vec<u8> {
        self.0.save()
    }
    fn load(&mut self, blob: &[u8]) -> Result<(), CoreError> {
        self.0.load(blob)
    }
}

/// Review of e65e509: a color-mode error read as DMG mode, so another
/// backend's entry would have shown bit 4 as its palette, wrongly in CGB
/// mode.
#[test]
fn a_core_that_cant_say_its_color_mode_gets_no_guessed_palette() {
    let core = NoColorMode(quick_dmg(BUSY));
    let refused = object_attributes(&core, 0).unwrap_err();
    assert!(
        matches!(
            refused,
            CoreError::Unsupported {
                feature: Feature::DisplayColors,
                ..
            }
        ),
        "{refused:?}"
    );
}
