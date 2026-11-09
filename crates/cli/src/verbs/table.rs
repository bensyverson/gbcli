//! The entries. Order within a class is the order the primer lists them.

use super::flags::{
    BATCH, BREAK, CHANGE, CREATE, EVENTS, OUTPUT, PRUNE, READ, RM, RUN, SERVE, SESSION,
    SHOT_OPTIONS, WATCH,
};
use super::table_battery::{POWER_CYCLE_VERB, SAV_VERB};
use super::table_build::{ASSETS_VERB, BUILD_VERB, DOCTOR_VERB, NEW_VERB};
use super::table_opcode::OPCODE_VERB;
use super::table_pixels::PIXELS_VERB;
use super::table_read::{
    CHECKPOINTS_VERB, EQUIV_VERB, MEASURE_VERB, SHOT_VERB, SYM_VERB, VIEW_VERB,
};
use super::table_song::SONG_VERB;
use super::table_version::VERSION_VERB;
use super::{Arity, Class, Verb, VerbSpec};

const NONE: Arity = Arity {
    min: 0,
    max: Some(0),
    name: "",
};

const fn one(name: &'static str) -> Arity {
    Arity {
        min: 1,
        max: Some(1),
        name,
    }
}

const fn many(name: &'static str) -> Arity {
    Arity {
        min: 1,
        max: None,
        name,
    }
}

/// A verb that reads the session and changes nothing.
const READ_VERB: &[&[super::FlagSpec]] = &[SESSION, OUTPUT];
/// A verb that changes the session, so `--rev` can guard it.
const CHANGE_VERB: &[&[super::FlagSpec]] = &[SESSION, CHANGE, OUTPUT];

pub const VERBS: &[VerbSpec] = &[
    VerbSpec {
        verb: Verb::Run,
        name: "run",
        class: Class::Act,
        summary: "advance the machine to a stop, then read and save",
        usage: "[ROM] [--frames N | --until EXPR | --to ADDR] [--keys TOKENS] [--read EXPR]… \
                [--shot PATH] [--audio-log] [--wav PATH] [--measure FROM..TO] [--profile] \
                [--oam-log] [flags]",
        arity: Arity {
            min: 0,
            max: Some(1),
            name: "ROM",
        },
        flags: RUN,
        groups: &[SHOT_OPTIONS, SESSION, CHANGE, OUTPUT],
        about: "The experiment verb. In this order, whatever the order of the flags: start \
                from --at, the head, or the boot handoff; apply --patch, --poke and --write \
                in the order given; schedule --keys; run until --until holds, --to is reached, a breakpoint or \
                watch fires, or the budget ends (--profile, --frame-budget, --raster and --oam-log collect \
                during the run); --measure on a copy of the stop; evaluate --read and --expect; save --shot; save the audio \
                (--audio-log, --audio-summary, --wav, --piano, --spectrogram); save \
                --checkpoint (not when an --expect is false). The stop \
                becomes the session's new head, whatever --expect says. A ROM argument is the same as --rom.",
        example: (
            "gbcli run game.gb --session dx --until 'u16[wFrame] >= 60' --read '[wSpriteX]'",
            "creates session dx from game.gb, runs to the frame where wFrame reaches 60, \
             and prints the stop and the byte in wSpriteX",
        ),
    },
    VerbSpec {
        verb: Verb::Step,
        name: "step",
        class: Class::Act,
        summary: "advance by one instruction, over, out, frame or scanline",
        usage: "[instruction|over|out|frame|scanline] [flags]",
        arity: Arity {
            min: 0,
            max: Some(1),
            name: "UNIT",
        },
        flags: &[],
        groups: CHANGE_VERB,
        about: "Advance the head by one unit: an instruction (the default), over a call, out \
                of the current routine, to the next frame, or to the next scanline. To run to \
                an address, use `gbcli run --to ADDR`.",
        example: (
            "gbcli step over --session dx",
            "runs the call at PC to its return and prints the stop",
        ),
    },
    VerbSpec {
        verb: Verb::Checkpoint,
        name: "checkpoint",
        class: Class::Act,
        summary: "save the head under a name",
        usage: "NAME [flags]",
        arity: one("NAME"),
        flags: &[],
        groups: CHANGE_VERB,
        about: "Save the session's head as a named checkpoint, which `restore` and `run --at` \
                read and which `sessions prune` never removes. The head does not change.",
        example: (
            "gbcli checkpoint title --session dx",
            "saves the head as `title` and prints its path, frame and rev",
        ),
    },
    VerbSpec {
        verb: Verb::Restore,
        name: "restore",
        class: Class::Act,
        summary: "make a checkpoint or history entry the head",
        usage: "NAME|~N [flags]",
        arity: one("NAME"),
        flags: &[],
        groups: CHANGE_VERB,
        about: "Replace the head with a named checkpoint, or with ~N, the head N changes back. \
                Breakpoints and watches belong to the session and are kept.",
        example: (
            "gbcli restore title --session dx",
            "makes the `title` checkpoint the head",
        ),
    },
    VerbSpec {
        verb: Verb::Undo,
        name: "undo",
        class: Class::Act,
        summary: "step the head back to the previous state",
        usage: "[flags]",
        arity: NONE,
        flags: &[],
        groups: CHANGE_VERB,
        about: "Restore the state before the last head change. Repeat to walk further back, \
                up to eight changes.",
        example: (
            "gbcli undo --session dx",
            "puts back the head from before the last run, step, write or restore",
        ),
    },
    POWER_CYCLE_VERB,
    VerbSpec {
        verb: Verb::Write,
        name: "write",
        class: Class::Act,
        summary: "write bytes as the CPU would (IO side effects happen)",
        usage: "TARGET=BYTES… [flags]",
        arity: many("TARGET=BYTES"),
        flags: &[],
        groups: CHANGE_VERB,
        about: "A bus write: DIV resets, DMA starts, banks switch, the LCD turns on or off. \
                BYTES are one or more comma-separated bytes: $91, 0x91, 145 or %10010001. \
                See `gbcli help writes` for the three kinds of write.",
        example: (
            "gbcli write rLCDC=0x91 --session dx",
            "turns the LCD on, as `ld [rLCDC], a` with a = $91 would",
        ),
    },
    VerbSpec {
        verb: Verb::Poke,
        name: "poke",
        class: Class::Act,
        summary: "write backing memory, with no side effects",
        usage: "TARGET=BYTES… [flags]",
        arity: many("TARGET=BYTES"),
        flags: &[],
        groups: CHANGE_VERB,
        about: "Write RAM, VRAM, OAM or any region's backing bytes directly; nothing else \
                happens. ROM is refused: use `gbcli patch`. Poked bytes count as written.",
        example: (
            "gbcli poke wSpeed=2 --session dx",
            "sets wSpeed to 2 in the head",
        ),
    },
    VerbSpec {
        verb: Verb::Patch,
        name: "patch",
        class: Class::Act,
        summary: "patch ROM bytes (recorded in every later checkpoint)",
        usage: "TARGET=BYTES… [flags]",
        arity: many("TARGET=BYTES"),
        flags: &[],
        groups: CHANGE_VERB,
        about: "Change the ROM's backing store at a bank-qualified address or rom:OFFSET. \
                Patches are recorded in checkpoints, so a restore brings back exactly its \
                patches. The header ($0100-$014F) is refused: rebuild instead.",
        example: (
            "gbcli patch 01:4123=0,0 --session dx",
            "replaces two bytes in bank 1 with nops",
        ),
    },
    VerbSpec {
        verb: Verb::Show,
        name: "show",
        class: Class::Read,
        summary: "registers, the PPU and the last stop",
        usage: "[flags]",
        arity: NONE,
        flags: &[],
        groups: READ_VERB,
        about: "Print the head's CPU registers and flags, the PPU's line and mode, and the \
                stop record of the last run.",
        example: ("gbcli show --session dx", "prints where the machine is"),
    },
    VerbSpec {
        verb: Verb::Read,
        name: "read",
        class: Class::Read,
        summary: "evaluate addresses and expressions at the head",
        usage: "EXPR… [--since BASE] [--allow-uninit] [flags]",
        arity: many("EXPR"),
        flags: READ,
        groups: READ_VERB,
        about: "Each argument is an expression (`gbcli help expressions`): [wFrame], \
                u16[$C000], rLY, frame, pc. A bare symbol is its address: wX prints \
                where wX lives, [wX] (or u8[wX]) the byte it holds, u16[wX] two bytes. A \
                byte the program never wrote is an error unless --allow-uninit.",
        example: (
            "gbcli read 'u16[wFrame]' rLY --session dx",
            "prints the frame counter and the current scanline",
        ),
    },
    MEASURE_VERB,
    EQUIV_VERB,
    SHOT_VERB,
    VIEW_VERB,
    SYM_VERB,
    CHECKPOINTS_VERB,
    SAV_VERB,
    VerbSpec {
        verb: Verb::Events,
        name: "events",
        class: Class::Read,
        summary: "the session's event log: who changed what",
        usage: "[--limit N] [flags]",
        arity: NONE,
        flags: EVENTS,
        groups: READ_VERB,
        about: "One row per request the session answered: when, who (--as), the op and its \
                arguments, the outcome and the revisions before and after.",
        example: (
            "gbcli events --session dx --limit 5",
            "prints the last five requests",
        ),
    },
    VerbSpec {
        verb: Verb::Gaps,
        name: "gaps",
        class: Class::Project,
        summary: "what the core could not do, counted",
        usage: "[flags]",
        arity: NONE,
        flags: &[],
        groups: &[OUTPUT],
        about: "The gap ledger: one row per feature the SameBoy backend answered \
                `unsupported` for, with how often, across every session in this project.",
        example: ("gbcli gaps", "prints each missing feature and its count"),
    },
    VerbSpec {
        verb: Verb::Sessions,
        name: "sessions",
        class: Class::Project,
        summary: "list this project's sessions",
        usage: "[flags]",
        arity: NONE,
        flags: &[],
        groups: &[OUTPUT],
        about: "One row per session in .gbcli/: name, ROM, revision, head frame, size, \
                named checkpoints and last use. `sessions prune` and `sessions rm` remove them.",
        example: ("gbcli sessions", "lists every session"),
    },
    VerbSpec {
        verb: Verb::Prune,
        name: "sessions prune",
        class: Class::Project,
        summary: "remove sessions unused for a while",
        usage: "--unused-for DURATION [flags]",
        arity: NONE,
        flags: PRUNE,
        groups: &[OUTPUT],
        about: "Remove every session idle for at least DURATION. Sessions that hold named \
                checkpoints are kept, and ones another process is using are skipped; both \
                are listed.",
        example: (
            "gbcli sessions prune --unused-for 7d",
            "removes sessions nobody has used for a week",
        ),
    },
    VerbSpec {
        verb: Verb::Rm,
        name: "sessions rm",
        class: Class::Project,
        summary: "remove one session",
        usage: "NAME [--with-checkpoints] [flags]",
        arity: one("NAME"),
        flags: RM,
        groups: &[OUTPUT],
        about: "Remove a session and its history. A session with named checkpoints is \
                refused unless --with-checkpoints.",
        example: ("gbcli sessions rm dx", "removes the session dx"),
    },
    VerbSpec {
        verb: Verb::Break,
        name: "break",
        class: Class::Configure,
        summary: "add a breakpoint",
        usage: "ADDR [--if EXPR] [flags]",
        arity: one("ADDR"),
        flags: BREAK,
        groups: CHANGE_VERB,
        about: "Stop every later run before the instruction at ADDR executes (when --if \
                holds). Prints the breakpoint's id, which `delete` takes. Set by a symbol, \
                it follows that symbol when the ROM is rebuilt, with a warning when it \
                moves; while a rebuild lacks the symbol it is disarmed (kept, warned about, \
                stopping nothing) until a build has it again. Set by an address, it stays.",
        example: (
            "gbcli break Main.loop --session dx --if 'a == 3'",
            "stops runs at Main.loop whenever a is 3",
        ),
    },
    VerbSpec {
        verb: Verb::Watch,
        name: "watch",
        class: Class::Configure,
        summary: "add a watchpoint on reads, writes or both",
        usage: "read|write|access ADDR [--if EXPR] [--once] [flags]",
        arity: Arity {
            min: 2,
            max: Some(2),
            name: "read|write|access ADDR",
        },
        flags: WATCH,
        groups: CHANGE_VERB,
        about: "Stop every later run at an access to ADDR. In --if, `old` and `new` are the \
                byte before and after a write. --once deletes it after it fires. Set by a \
                symbol, it follows that symbol when the ROM is rebuilt, and is disarmed \
                while a rebuild lacks the symbol.",
        example: (
            "gbcli watch write rDMA --session dx --once",
            "stops the next run when OAM DMA starts",
        ),
    },
    VerbSpec {
        verb: Verb::Delete,
        name: "delete",
        class: Class::Configure,
        summary: "remove a breakpoint or watchpoint by id",
        usage: "ID [flags]",
        arity: one("ID"),
        flags: &[],
        groups: CHANGE_VERB,
        about: "Remove the breakpoint or watchpoint with this id, as `break` or `watch` \
                printed it.",
        example: ("gbcli delete 1 --session dx", "removes breakpoint 1"),
    },
    NEW_VERB,
    BUILD_VERB,
    ASSETS_VERB,
    SONG_VERB,
    PIXELS_VERB,
    VerbSpec {
        verb: Verb::Serve,
        name: "serve",
        class: Class::Server,
        summary: "JSON Lines over stdio, machine kept warm",
        usage: "[--session NAME] [--rom FILE] [flags]",
        arity: NONE,
        flags: SERVE,
        groups: &[],
        about: "One request per stdin line, one response per stdout line, each flushed. \
                stdout carries protocol only. --session sets the default session (created \
                from --rom if new). See `gbcli schema` for the request shape.",
        example: (
            "gbcli serve --session dx --rom game.gb",
            "answers requests such as {\"id\":\"1\",\"op\":\"show\"} until stdin closes",
        ),
    },
    VerbSpec {
        verb: Verb::Batch,
        name: "batch",
        class: Class::Server,
        summary: "apply a file of request lines, one status row each",
        usage: "-F FILE [flags]",
        arity: NONE,
        flags: BATCH,
        groups: &[CREATE, OUTPUT],
        about: "Each line is a protocol request (`gbcli schema`). Every line is applied in \
                order; a failed line doesn't stop the ones after it. Prints one row per line \
                and exits with the first failure's code; --json prints each response instead.",
        example: (
            "gbcli batch -F ops.jsonl --session dx",
            "applies ops.jsonl to dx and prints one status row per line",
        ),
    },
    VerbSpec {
        verb: Verb::Help,
        name: "help",
        class: Class::Help,
        summary: "a topic, or a verb's help",
        usage: "[TOPIC|VERB]",
        arity: Arity {
            min: 0,
            max: Some(1),
            name: "TOPIC",
        },
        flags: &[],
        groups: &[],
        about: "Topics: addresses, expressions, time, checkpointing, writes, budgets, \
                exit-codes, hardware, traps, optimize, rgbds, music, images, pixels, flash. \
                `gbcli help VERB` is `gbcli VERB --help`, except where a topic shares the \
                verb's name to teach its file format: `gbcli help pixels` is the format, \
                `gbcli pixels --help` the verb.",
        example: ("gbcli help addresses", "explains the address forms"),
    },
    VerbSpec {
        verb: Verb::Recipes,
        name: "recipes",
        class: Class::Help,
        summary: "goal → command",
        usage: "",
        arity: NONE,
        flags: &[],
        groups: &[],
        about: "Common goals and the command for each.",
        example: ("gbcli recipes", "prints the recipes"),
    },
    VerbSpec {
        verb: Verb::Schema,
        name: "schema",
        class: Class::Help,
        summary: "the request grammar for serve and batch",
        usage: "",
        arity: NONE,
        flags: &[],
        groups: &[],
        about: "Every op and its arguments, with an example request line for each.",
        example: ("gbcli schema", "prints the request grammar"),
    },
    DOCTOR_VERB,
    OPCODE_VERB,
    VERSION_VERB,
];
