//! Bytes on the wire as lowercase hex: one spelling for patches, writes and
//! hashes.

pub(crate) fn encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn decode(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return Err(format!("not an even run of hex digits: {text:?}"));
    }
    (0..text.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&text[i..i + 2], 16)
                .map_err(|_| format!("not hex: {:?}", &text[i..i + 2]))
        })
        .collect()
}

/// `#[serde(with = "hex::bytes")]` for a `Vec<u8>`.
pub(crate) mod bytes {
    use super::{decode, encode};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        decode(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_round_trip_as_lowercase_hex() {
        assert_eq!(encode(&[0x00, 0xC9, 0xFF]), "00c9ff");
        assert_eq!(decode("00c9ff"), Ok(vec![0x00, 0xC9, 0xFF]));
    }

    #[test]
    fn malformed_hex_is_refused() {
        assert!(decode("abc").is_err(), "odd length");
        assert!(decode("zz").is_err(), "not hex");
    }
}
