//! What creating a session reports (leaf NQKwil): the bundle it went into,
//! so a caller whose `.gbcli/` was found by walking up sees which one, and a
//! note when a ROM that supports color starts as a DMG only because no model
//! was named. Decided here so every client says the same.

use gbcli_core::Model;
use gbcli_protocol::{CgbFlag, SessionCreated, SessionNote};

use crate::store::SessionStore;

/// A session opened by name ([`crate::Bundle::start_session`]), and what
/// creating it made when this call created it.
#[derive(Debug)]
pub struct Opened {
    pub store: SessionStore,
    pub created: Option<SessionCreated>,
}

/// The report for a session just created as `model` over a ROM whose header
/// says `cgb`, `named` being the model its creator asked for, if any.
pub(crate) fn report(
    store: &SessionStore,
    named: Option<Model>,
    model: Model,
    cgb: Option<CgbFlag>,
) -> SessionCreated {
    SessionCreated {
        name: store.name().to_string(),
        bundle: store.bundle().root().display().to_string(),
        model,
        cgb,
        note: color_note(named, model, cgb),
    }
}

/// [`SessionNote::ColorRomOnDmg`] when no model was named, the session is a
/// DMG, and the ROM runs in color on a GBC (`$0143` dual or CGB-only). A
/// model named outright, even a DMG one, is what its creator meant.
pub fn color_note(named: Option<Model>, model: Model, cgb: Option<CgbFlag>) -> Option<SessionNote> {
    let color_rom = matches!(cgb, Some(CgbFlag::Dual | CgbFlag::Only));
    (named.is_none() && model.has_dmg_shades() && color_rom).then_some(SessionNote::ColorRomOnDmg)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    use super::*;
    use crate::bundle::Bundle;
    use crate::memory_store::MemoryStore;
    use crate::project_store::{Key, ProjectStore};
    use crate::session::RomSource;
    use crate::test_fixture::{Fixture, ROM};

    /// `demo.gb` with its header's CGB flag set to `flag`.
    fn rom_flagged(f: &Fixture, flag: u8) {
        let mut rom = ROM.to_vec();
        rom[0x143] = flag;
        fs::write(f.root.join("demo.gb"), rom).unwrap();
    }

    fn source(f: &Fixture, model: Option<Model>) -> RomSource {
        RomSource {
            model,
            ..f.source()
        }
    }

    #[test]
    fn creating_a_session_reports_its_bundle_and_reopening_reports_nothing() {
        let f = Fixture::new("created-bundle");
        let opened = f
            .bundle
            .start_session("demo", Some(&source(&f, None)))
            .unwrap();
        let created = opened.created.expect("created");
        assert_eq!(created.name, "demo");
        assert_eq!(Path::new(&created.bundle), f.root.join(".gbcli"));
        assert_eq!(created.model, Model::DmgB);
        assert_eq!(created.cgb, Some(CgbFlag::Dmg));
        assert_eq!(created.note, None, "a DMG ROM needs no note");
        let again = f
            .bundle
            .start_session("demo", Some(&source(&f, None)))
            .unwrap();
        assert_eq!(again.created, None);
        let named = f.bundle.start_session("demo", None).unwrap();
        assert_eq!(named.created, None);
    }

    #[test]
    fn a_color_rom_without_a_model_is_noted_and_with_one_is_not() {
        for (flag, cgb) in [(0x80, CgbFlag::Dual), (0xC0, CgbFlag::Only)] {
            let f = Fixture::new(&format!("created-note-{flag:x}"));
            rom_flagged(&f, flag);
            let bare = f.bundle.start_session("bare", Some(&source(&f, None)));
            let bare = bare.unwrap().created.unwrap();
            assert_eq!((bare.model, bare.cgb), (Model::DmgB, Some(cgb)));
            assert_eq!(bare.note, Some(SessionNote::ColorRomOnDmg));
            for model in [Model::CgbE, Model::DmgB] {
                let name = format!("named-{}", gbcli_protocol::model_name(model));
                let named = f
                    .bundle
                    .start_session(&name, Some(&source(&f, Some(model))));
                assert_eq!(named.unwrap().created.unwrap().note, None, "{model:?}");
            }
        }
    }

    #[test]
    fn the_note_needs_no_model_named_a_dmg_and_a_color_rom() {
        use CgbFlag::*;
        let note = Some(SessionNote::ColorRomOnDmg);
        assert_eq!(color_note(None, Model::DmgB, Some(Dual)), note);
        assert_eq!(color_note(None, Model::DmgB, Some(Only)), note);
        assert_eq!(color_note(None, Model::DmgB, Some(Dmg)), None);
        assert_eq!(color_note(None, Model::DmgB, None), None);
        assert_eq!(color_note(None, Model::CgbE, Some(Dual)), None);
        assert_eq!(color_note(Some(Model::DmgB), Model::DmgB, Some(Dual)), None);
    }

    /// A memory-backed store has no host folder; it reports the path it
    /// names everything by, its project path joined with the bundle.
    #[test]
    fn a_memory_store_reports_its_virtual_bundle_path() {
        let clock = Arc::new(crate::file_store::SystemClock);
        let store = Arc::new(MemoryStore::new("/web/game", clock.clone()).unwrap());
        store.put(&Key::new("demo.gb"), ROM).unwrap();
        store.make_dir(&Key::new(".gbcli")).unwrap();
        let bundle = Bundle::new(store, Key::new(".gbcli"), clock);
        let source = RomSource {
            rom: "demo.gb".into(),
            sym: None,
            model: None,
            seed: None,
            flash: None,
        };
        let opened = bundle.start_session("web", Some(&source)).unwrap();
        assert_eq!(opened.created.unwrap().bundle, "/web/game/.gbcli");
    }
}
