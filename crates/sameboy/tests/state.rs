//! State blobs and determinism.

mod common;

use common::*;
use gbcli_core::{Budget, Core, CoreConfig, CoreError, Model, Region};
use gbcli_sameboy::SameBoy;

#[test]
fn load_restores_time_memory_and_the_kept_frame() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(4));
    let blob = core.save();
    let now = core.now();
    let frame = core.last_complete_frame().unwrap().tag;
    let wram = core.region(Region::Wram).unwrap().bytes.to_vec();
    run(&mut core, Budget::frames(3));
    assert_ne!(core.now(), now);
    core.load(&blob).unwrap();
    assert_eq!(core.now(), now);
    assert_eq!(core.last_complete_frame().unwrap().tag, frame);
    assert_eq!(core.region(Region::Wram).unwrap().bytes, wram);
    assert_eq!(core.save(), blob);
}

#[test]
fn a_blob_loads_into_a_fresh_instance() {
    let mut a = quick_dmg(BUSY);
    run(&mut a, Budget::frames(4));
    let mut b = quick_dmg(BUSY);
    b.load(&a.save()).unwrap();
    assert_eq!(b.save(), a.save());
    run(&mut a, Budget::frames(2));
    run(&mut b, Budget::frames(2));
    assert_eq!(b.save(), a.save());
}

#[test]
fn a_garbage_blob_is_refused_and_changes_nothing() {
    let mut core = quick_dmg(BUSY);
    run(&mut core, Budget::frames(1));
    let before = core.save();
    let err = core.load(b"definitely not a state").unwrap_err();
    assert!(matches!(err, CoreError::InvalidState { .. }), "{err:?}");
    assert_eq!(core.save(), before);
}

#[test]
fn a_blob_from_another_model_is_refused() {
    let cgb = SameBoy::new(CoreConfig::new(Model::CgbE, BUSY.to_vec())).unwrap();
    let mut core = quick_dmg(BUSY);
    let before = core.save();
    let err = core.load(&cgb.save()).unwrap_err();
    assert!(matches!(err, CoreError::InvalidState { .. }), "{err:?}");
    assert_eq!(core.save(), before);
}

#[test]
fn the_same_seed_gives_identical_power_on_state_and_another_seed_does_not() {
    let a = dmg(BUSY, 7);
    let b = dmg(BUSY, 7);
    let c = dmg(BUSY, 8);
    assert_eq!(a.save(), b.save());
    assert_ne!(
        a.region(Region::Wram).unwrap().bytes,
        c.region(Region::Wram).unwrap().bytes
    );
}

#[test]
fn host_time_does_not_reach_the_state() {
    // SameBoy stamps the RTC epoch with time(NULL) when the RTC mode is set,
    // and that field is saved; two instances a second apart must still match.
    let a = dmg(BUSY, 1);
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    let b = dmg(BUSY, 1);
    assert_eq!(a.save(), b.save());
}
