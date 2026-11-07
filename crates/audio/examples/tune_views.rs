//! Preview: every audio view of the tune test ROM, written to a directory.
//!
//!     cargo run -p gbcli-audio --example tune_views -- <out-dir> [scale]
//!
//! Writes the decoded log (`log.txt`), the mixed and per-channel WAVs, the
//! piano roll and the spectrograms (PNG, nearest-neighbor scaled, default 4).

use std::path::PathBuf;

use gbcli_audio::{
    ApuSnapshot, PianoRoll, channel_name, decode, note_spans, piano_roll, record_takes,
    spectrogram, wav,
};
use gbcli_core::{ApuChannel, BootRom, Budget, Core, CoreConfig, Model, RunRequest, StepUnit};
use gbcli_sameboy::SameBoy;

const TUNE: &[u8] = include_bytes!("../tests/roms/tune.gb");
const QUICK_BOOT: &[u8] = include_bytes!("../../sameboy/tests/roms/quick_boot.bin");
const FRAMES: u64 = 32;

fn tune_core() -> Result<SameBoy, gbcli_core::CoreError> {
    let mut config = CoreConfig::new(Model::DmgB, TUNE.to_vec());
    config.boot_rom = BootRom::Custom(QUICK_BOOT.to_vec());
    let mut core = SameBoy::new(config)?;
    for _ in 0..4 {
        core.step(StepUnit::Instruction)?;
    }
    Ok(core)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("usage: tune_views <out-dir> [scale]")?);
    let scale: u32 = args.next().map_or(Ok(4), |s| s.parse())?;
    std::fs::create_dir_all(&out)?;

    let mut core = tune_core()?;
    let start = core.save();
    let snapshot = ApuSnapshot::from_core(&core)?;
    core.set_apu_write_log(true)?;
    let from = core.now();
    core.run(&mut RunRequest::new(Budget::frames(FRAMES)))?;
    let log = decode(&snapshot, &core.take_apu_writes()?);
    let text: String = log
        .iter()
        .map(|w| format!("{:>10} {w}\n", w.write.at.0))
        .collect();
    std::fs::write(out.join("log.txt"), text)?;
    let roll = piano_roll(
        &note_spans(&log, core.now()),
        from,
        core.now(),
        &PianoRoll::default(),
    )?;
    std::fs::write(out.join("piano.png"), roll.scaled(scale)?.png()?)?;

    let takes = record_takes(
        || {
            let mut core = tune_core()?;
            core.load(&start)?;
            Ok(core)
        },
        |core: &mut SameBoy| {
            core.run(&mut RunRequest::new(Budget::frames(FRAMES)))
                .map(|_| ())
        },
    )?;
    std::fs::write(out.join("mixed.wav"), wav(&takes.mixed.samples))?;
    let image = spectrogram(&takes.mixed, "mixed")?;
    std::fs::write(
        out.join("spectrogram-mixed.png"),
        image.scaled(scale)?.png()?,
    )?;
    for channel in ApuChannel::ALL {
        let name = channel_name(channel).replace(' ', "");
        let audio = takes.channel(channel);
        std::fs::write(out.join(format!("{name}.wav")), wav(&audio.samples))?;
        let image = spectrogram(audio, channel_name(channel))?;
        std::fs::write(
            out.join(format!("spectrogram-{name}.png")),
            image.scaled(scale)?.png()?,
        )?;
    }
    println!("wrote {} (residual {})", out.display(), takes.residual());
    Ok(())
}
