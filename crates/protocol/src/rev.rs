//! Session revisions: `s6`, the sixth head of a session (design §5).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A session's revision counter. Every head change bumps it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rev(pub u64);

impl fmt::Display for Rev {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "s{}", self.0)
    }
}

impl FromStr for Rev {
    type Err = String;

    fn from_str(text: &str) -> Result<Rev, String> {
        let digits = text
            .strip_prefix('s')
            .ok_or_else(|| format!("a revision looks like s6, not {text:?}"))?;
        digits
            .parse()
            .map(Rev)
            .map_err(|_| format!("a revision looks like s6, not {text:?}"))
    }
}

impl Serialize for Rev {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Rev {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Rev, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisions_read_and_write_as_s_and_a_number() {
        assert_eq!(Rev(6).to_string(), "s6");
        assert_eq!("s0".parse::<Rev>(), Ok(Rev(0)));
        assert_eq!(serde_json::to_string(&Rev(12)).unwrap(), r#""s12""#);
        assert_eq!(serde_json::from_str::<Rev>(r#""s12""#).unwrap(), Rev(12));
    }

    #[test]
    fn malformed_revisions_are_refused() {
        assert!("s".parse::<Rev>().is_err());
        assert!("6".parse::<Rev>().is_err());
        assert!("s-1".parse::<Rev>().is_err());
    }
}
