//! Key input on a run (design §11 step 3): a timeline of presses and
//! releases at frame offsets from the run's start. On the wire and on the CLI
//! an event is one token, `A@0` (press A at frame 0) or `-A@2` (release it at
//! frame 2); a script is tokens separated by whitespace, `#` starting a
//! comment to the end of the line (the `--keys-file` form).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The grammar, as an error shows it.
pub const KEY_GRAMMAR: &str = "[-]BUTTON@FRAME, where BUTTON is one of A B SELECT START RIGHT LEFT UP DOWN, `-` releases, and FRAME counts frames from the run's start (e.g. `A@0 -A@2 START@10`)";

/// How a key error's message begins, so a request error can point at the grammar.
pub(crate) const KEY_ERROR: &str = "bad key event";

/// A joypad button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

impl Button {
    pub const ALL: [Button; 8] = [
        Button::A,
        Button::B,
        Button::Select,
        Button::Start,
        Button::Right,
        Button::Left,
        Button::Up,
        Button::Down,
    ];

    /// The bit `gbcli_core::Keys` holds this button in.
    pub fn bit(self) -> u8 {
        match self {
            Button::Right => 1 << 0,
            Button::Left => 1 << 1,
            Button::Up => 1 << 2,
            Button::Down => 1 << 3,
            Button::A => 1 << 4,
            Button::B => 1 << 5,
            Button::Select => 1 << 6,
            Button::Start => 1 << 7,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Button::Right => "RIGHT",
            Button::Left => "LEFT",
            Button::Up => "UP",
            Button::Down => "DOWN",
            Button::A => "A",
            Button::B => "B",
            Button::Select => "SELECT",
            Button::Start => "START",
        }
    }
}

/// Press or release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    Press,
    Release,
}

/// One event: `button` is pressed or released when the run has completed
/// `at` frames (at the vblank that begins frame `at`; 0 is before the first
/// instruction).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct KeyEvent {
    pub at: u64,
    pub button: Button,
    pub action: KeyAction,
}

/// A token that isn't a key event: the token, why, and the grammar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyError {
    pub token: String,
    pub reason: String,
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{KEY_ERROR} `{}`: {}; the grammar is {KEY_GRAMMAR}",
            self.token, self.reason
        )
    }
}

impl std::error::Error for KeyError {}

impl fmt::Display for KeyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = match self.action {
            KeyAction::Press => "",
            KeyAction::Release => "-",
        };
        write!(f, "{sign}{}@{}", self.button.name(), self.at)
    }
}

impl FromStr for KeyEvent {
    type Err = KeyError;

    fn from_str(token: &str) -> Result<KeyEvent, KeyError> {
        let bad = |reason: String| KeyError {
            token: token.to_string(),
            reason,
        };
        let (action, rest) = match token.strip_prefix('-') {
            Some(rest) => (KeyAction::Release, rest),
            None => (KeyAction::Press, token),
        };
        let Some((name, frame)) = rest.split_once('@') else {
            return Err(bad("it has no `@FRAME`".into()));
        };
        let button = Button::ALL
            .into_iter()
            .find(|b| b.name().eq_ignore_ascii_case(name))
            .ok_or_else(|| bad(format!("`{name}` isn't a button")))?;
        let at = frame
            .parse::<u64>()
            .map_err(|_| bad(format!("`{frame}` isn't a frame number")))?;
        Ok(KeyEvent { at, button, action })
    }
}

impl TryFrom<String> for KeyEvent {
    type Error = KeyError;

    fn try_from(token: String) -> Result<KeyEvent, KeyError> {
        token.parse()
    }
}

impl From<KeyEvent> for String {
    fn from(event: KeyEvent) -> String {
        event.to_string()
    }
}

/// Parses a key script: tokens separated by whitespace, with `#` comments.
/// The first bad token is the error.
pub fn parse_keys(script: &str) -> Result<Vec<KeyEvent>, KeyError> {
    script
        .lines()
        .map(|line| line.split_once('#').map_or(line, |(code, _)| code))
        .flat_map(str::split_whitespace)
        .map(str::parse)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(at: u64, button: Button, action: KeyAction) -> KeyEvent {
        KeyEvent { at, button, action }
    }

    #[test]
    fn the_design_example_parses_in_order() {
        let events = parse_keys("A@0 -A@2 START@10").unwrap();
        assert_eq!(
            events,
            vec![
                event(0, Button::A, KeyAction::Press),
                event(2, Button::A, KeyAction::Release),
                event(10, Button::Start, KeyAction::Press),
            ]
        );
    }

    #[test]
    fn a_script_file_takes_lines_and_comments() {
        let script = "# walk right\nRIGHT@0   -RIGHT@30\n\n  a@31 # jump\n-a@33\n";
        let events = parse_keys(script).unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(events[2], event(31, Button::A, KeyAction::Press));
    }

    #[test]
    fn a_malformed_token_is_named_with_the_grammar() {
        for (script, token, why) in [
            ("A@0 X@3", "X@3", "`X` isn't a button"),
            ("A@0 START", "START", "no `@FRAME`"),
            ("-B@two", "-B@two", "`two` isn't a frame number"),
            ("A@-1", "A@-1", "`-1` isn't a frame number"),
        ] {
            let err = parse_keys(script).unwrap_err();
            assert_eq!(err.token, token, "{script}");
            let text = err.to_string();
            assert!(text.contains(&format!("`{token}`")), "{text}");
            assert!(text.contains(why), "{text}");
            assert!(text.contains(KEY_GRAMMAR), "{text}");
        }
    }

    #[test]
    fn an_event_is_its_token_on_the_wire() {
        let events = parse_keys("A@0 -start@10").unwrap();
        let json = serde_json::to_string(&events).unwrap();
        assert_eq!(json, r#"["A@0","-START@10"]"#);
        let back: Vec<KeyEvent> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, events);
        let err = serde_json::from_str::<Vec<KeyEvent>>(r#"["Q@1"]"#).unwrap_err();
        assert!(err.to_string().contains("`Q@1`"), "{err}");
    }

    #[test]
    fn a_malformed_token_in_a_request_is_a_usage_error_naming_it_and_the_grammar() {
        let line = r#"{"id":"1","op":"run","args":{"frames":3,"keys":["A@0","X@3"]}}"#;
        let err = crate::parse_request(line).unwrap_err();
        assert!(err.message.contains("`X@3`"), "{}", err.message);
        assert!(err.remedy.contains(KEY_GRAMMAR), "{}", err.remedy);
    }

    #[test]
    fn each_button_has_the_bit_the_core_holds_it_in() {
        let all = Button::ALL.iter().fold(0u8, |bits, b| bits | b.bit());
        assert_eq!(all, 0xFF);
        assert_eq!(Button::A.bit(), 0x10);
        assert_eq!(Button::Start.bit(), 0x80);
        assert_eq!(Button::Right.bit(), 0x01);
    }
}
