//! A request's own wall-guard allowance (design §3, update of 2026-11-09):
//! `wall_guard_s`, whole seconds, bounded. The guard itself (the clock and
//! the thread that watches it) is the adapter's; the protocol only carries
//! the number.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// How long one request may take in host time before the guard ends it,
/// in whole seconds from 1 to [`WallGuardSeconds::MAX`]. The upper bound
/// keeps the guard's promise that every request ends: an hour holds about
/// 250,000 frames even at the 70 frames a second a starved host gave
/// (2026-11-09, load average 134), far past any one experiment; longer work
/// splits with a checkpoint and `--at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct WallGuardSeconds(u64);

impl WallGuardSeconds {
    pub const MIN: u64 = 1;
    pub const MAX: u64 = 3600;

    /// The allowance, or why `seconds` isn't one.
    pub fn new(seconds: u64) -> Result<WallGuardSeconds, String> {
        if (Self::MIN..=Self::MAX).contains(&seconds) {
            Ok(WallGuardSeconds(seconds))
        } else {
            Err(range_error(&seconds.to_string()))
        }
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

fn range_error(text: &str) -> String {
    format!(
        "the wall guard is whole seconds from {} to {}, not {text:?}",
        WallGuardSeconds::MIN,
        WallGuardSeconds::MAX
    )
}

impl TryFrom<u64> for WallGuardSeconds {
    type Error = String;

    fn try_from(seconds: u64) -> Result<WallGuardSeconds, String> {
        WallGuardSeconds::new(seconds)
    }
}

impl From<WallGuardSeconds> for u64 {
    fn from(s: WallGuardSeconds) -> u64 {
        s.0
    }
}

impl FromStr for WallGuardSeconds {
    type Err = String;

    fn from_str(text: &str) -> Result<WallGuardSeconds, String> {
        let seconds: u64 = text.parse().map_err(|_| range_error(text))?;
        WallGuardSeconds::new(seconds)
    }
}

impl fmt::Display for WallGuardSeconds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_seconds_from_one_to_an_hour_are_allowances() {
        assert_eq!(WallGuardSeconds::new(1).map(WallGuardSeconds::get), Ok(1));
        assert_eq!(
            WallGuardSeconds::new(WallGuardSeconds::MAX).map(WallGuardSeconds::get),
            Ok(3600)
        );
        assert_eq!("300".parse::<WallGuardSeconds>().map(|s| s.get()), Ok(300));
    }

    #[test]
    fn zero_past_an_hour_and_non_numbers_are_refused_naming_the_range() {
        for text in ["0", "3601", "-5", "1.5", "60s", ""] {
            let error = text.parse::<WallGuardSeconds>().expect_err(text);
            assert!(error.contains("1 to 3600"), "{text}: {error}");
        }
    }

    #[test]
    fn the_wire_is_a_plain_number_and_refuses_one_out_of_range() {
        let s = WallGuardSeconds::new(120).unwrap();
        assert_eq!(serde_json::to_string(&s).unwrap(), "120");
        assert_eq!(serde_json::from_str::<WallGuardSeconds>("120").unwrap(), s);
        assert!(serde_json::from_str::<WallGuardSeconds>("0").is_err());
        assert!(serde_json::from_str::<WallGuardSeconds>("86400").is_err());
        assert!(serde_json::from_str::<WallGuardSeconds>(r#""120""#).is_err());
    }
}
