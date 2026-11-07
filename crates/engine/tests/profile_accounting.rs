//! Profiler accounting at the edges (leaf dTQMbU): a profile begun inside a
//! routine, a measured arrival right after a KEY1 write, and the steps that
//! wake the CPU from HALT or switch its speed. Over `roms/profile.asm`; every
//! expected cost is summed from the opcode table.

mod common;

use common::{cgb_machine, machine, step_to};
use gbcli_core::{Budget, Event, EventKinds, EventSink, Registers, RunRequest};
use gbcli_engine::disasm::{NoSymbols, decode};
use gbcli_engine::profile::{MeasureSpec, Profile, RoutineCost};
use gbcli_engine::{Machine, Symbols};
use gbcli_protocol::Location;

const ROM: &[u8] = include_bytes!("roms/profile.gb");
const SYM: &str = include_str!("roms/profile.sym");

/// An interrupt dispatch, which the opcode table doesn't list: SameBoy's
/// dispatch branch in `GB_cpu_run` (sm83_cpu.c) is a read, an idle cycle and
/// two writes of 4 T each, then 2 + 2 T of pending cycles: 20 T.
const DISPATCH_T: u64 = 20;

fn addr(name: &str) -> u16 {
    Symbols::parse(SYM).unwrap().get(name).unwrap().addr
}

fn table_t(addr: u16) -> u64 {
    let bytes = &ROM[usize::from(addr)..];
    u64::from(decode(bytes, addr, &NoSymbols).unwrap().cycles.taken)
}

fn sum(path: &[u16]) -> u64 {
    path.iter().map(|&a| table_t(a)).sum()
}

fn routine<'a>(profile: &'a Profile, name: &str) -> &'a RoutineCost {
    profile
        .routines
        .iter()
        .find(|r| r.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {name} in {:#?}", profile.routines))
}

fn jump(m: &mut Machine, name: &str) {
    let regs = Registers {
        pc: addr(name),
        ..m.core().registers()
    };
    m.core_mut().set_registers(&regs);
}

/// At MainLoop with the stack set up, then at `name`.
fn at(name: &str) -> Machine {
    let mut m = machine(ROM, Some(SYM));
    step_to(&mut m, addr("MainLoop"));
    jump(&mut m, name);
    m
}

#[test]
fn a_ret_above_the_root_hands_the_root_to_the_routine_it_returns_into() {
    // Begun at Leaf, called from MainLoop: Leaf's RET returns above the
    // profile's start, into MainLoop+3, which is the root from then on.
    let mut m = machine(ROM, Some(SYM));
    step_to(&mut m, addr("MainLoop"));
    step_to(&mut m, addr("Leaf"));
    let profile = m.profile(Budget::instructions(17)).unwrap();
    assert_eq!(
        m.core().registers().pc,
        addr("MainLoop"),
        "the rest of a pass"
    );

    let leaf_t = sum(&[addr("Leaf"), addr("Leaf") + 2]);
    let leaf = routine(&profile, "Leaf");
    assert_eq!(
        (leaf.calls, leaf.exclusive_t, leaf.inclusive_t),
        (1, 2 * leaf_t, 2 * leaf_t),
        "its own pass as the root, then Outer's call"
    );

    let main = addr("MainLoop");
    let caller = routine(&profile, "MainLoop+$3");
    let own = sum(&[main + 3, main + 6, addr("Discard") + 1, main + 9, main + 12]);
    assert_eq!(
        (caller.calls, caller.exclusive_t, caller.inclusive_t),
        (0, own, profile.total_t)
    );
    let exclusive: u64 = profile.routines.iter().map(|r| r.exclusive_t).sum();
    assert_eq!(exclusive, profile.total_t, "every cycle is charged once");
    assert_eq!(profile.shadow.ret_jumps, 1, "Trampoline only");
    assert_eq!(profile.shadow.unwound, 1, "Discard");
}

#[test]
fn a_reti_above_the_root_hands_the_root_to_the_interrupted_code() {
    // Begun inside the vblank handler: its RETI returns into the HALT loop.
    let mut m = at("HaltLoop");
    step_to(&mut m, addr("VBlank"));
    let profile = m.profile(Budget::frames(3)).unwrap();
    assert_eq!(profile.shadow.ret_jumps, 0);
    let waiting = routine(&profile, "HaltLoop.wait+$1");
    assert_eq!(waiting.inclusive_t, profile.total_t);
    let v = addr("VBlank");
    let first = routine(&profile, "VBlank");
    let body = sum(&[v, v + 3, v + 4]); // ld hl; inc [hl]; reti
    assert_eq!(
        (first.calls, first.exclusive_t, first.inclusive_t),
        (0, body, body),
        "the first root, to its RETI"
    );
    let handler = routine(&profile, "VBlankVector");
    assert!(handler.calls >= 2, "{handler:?}");
    assert_eq!(
        waiting.exclusive_t + first.exclusive_t + handler.exclusive_t,
        profile.total_t,
        "every cycle is charged once"
    );
}

fn spec(from: &str, to: &str) -> MeasureSpec<'static> {
    let at = |name: &str| Location {
        addr: addr(name),
        bank: None,
        symbol: Some(name.into()),
    };
    MeasureSpec {
        from: at(from),
        to: at(to),
        iterations: 3,
        budget: Budget::instructions(1_000),
        interrupt: None,
    }
}

#[test]
fn measure_sees_an_arrival_right_after_a_key1_write() {
    let k = addr("Key1Poke");
    let result = at("Key1Poke")
        .measure(&spec("Key1Poke", "Key1Poke.after"))
        .unwrap();
    let t = sum(&[k, k + 1]); // xor a; ldh [KEY1], a
    assert_eq!((result.t().min, result.t().max), (t, t));
}

#[test]
fn measure_sees_a_pass_that_ends_right_after_a_key1_write() {
    let k = addr("Key1Poke");
    let after = addr("Key1Poke.after");
    let result = at("Key1Poke.after")
        .measure(&spec("Key1Poke.after", "Key1Poke.after"))
        .unwrap();
    let t = sum(&[after, k, k + 1]); // jr; xor a; ldh [KEY1], a
    assert_eq!((result.t().min, result.t().max), (t, t));
}

#[test]
fn a_halting_frame_is_active_only_for_its_instructions_and_the_dispatch() {
    // Each frame after the first: the dispatch that wakes the CPU, the
    // handler, then `nop; jr; halt` back into HALT. The HALT cycles SameBoy
    // runs in the waking step (`GB_cpu_run`'s halted branch) are idle.
    let mut m = at("HaltLoop");
    let frames = m.frame_budget(3).unwrap();
    let (v, w) = (addr("VBlank"), addr("HaltLoop.wait"));
    let work = DISPATCH_T
        + table_t(addr("VBlankVector"))
        + sum(&[v, v + 3, v + 4])
        + sum(&[w, w + 1, w + 2]);
    for f in &frames[1..] {
        assert_eq!(f.active_t, work, "{f:?}");
    }
}

/// Every execution event of a run.
#[derive(Default)]
struct Log(Vec<Event>);

impl EventSink for Log {
    fn kinds(&self) -> EventKinds {
        EventKinds {
            exec: true,
            ..EventKinds::default()
        }
    }

    fn event(&mut self, event: Event) {
        self.0.push(event);
    }
}

#[test]
fn a_step_that_switches_speed_reports_the_speed_it_began_at() {
    let mut m = cgb_machine(ROM, Some(SYM));
    step_to(&mut m, addr("Main") + 1); // past `di`
    jump(&mut m, "SpeedSwitch");
    for _ in 0..1_000 {
        let before = m.core().registers().double_speed;
        let mut log = Log::default();
        let mut req = RunRequest::new(Budget::instructions(1));
        req.sink = Some(&mut log);
        m.core_mut().run(&mut req).unwrap();
        for event in &log.0 {
            if let Event::Exec { double_speed, .. } = event {
                assert_eq!(*double_speed, before, "{event:?}");
            }
        }
        if m.core().registers().double_speed != before {
            return;
        }
    }
    panic!("the speed never switched");
}
