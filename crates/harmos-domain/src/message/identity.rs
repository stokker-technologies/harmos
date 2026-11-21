/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Portable Schema and Content Identity Values

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fmt;
// Third Party Imports
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Local Imports

/* === Constants === */

/* === Macros === */

macro_rules! identity {
    ($ty:ident) => {
        impl fmt::Display for $ty {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in self.0 {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }

        impl Serialize for $ty {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                if serializer.is_human_readable() { serializer.collect_str(self) } else { self.0.serialize(serializer) }
            }
        }

        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                if !deserializer.is_human_readable() {
                    return <[u8; 32]>::deserialize(deserializer).map(Self);
                }
                let text = String::deserialize(deserializer)?;
                if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
                    return Err(serde::de::Error::custom("identity must be 64 lowercase hexadecimal digits"));
                }
                let mut bytes = [0; 32];
                for (index, byte) in bytes.iter_mut().enumerate() {
                    *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).map_err(serde::de::Error::custom)?;
                }
                Ok(Self(bytes))
            }
        }
    };
}

identity!(Fingerprint);
identity!(Digest);

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fingerprint(pub [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(pub [u8; 32]);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use crate::{Message, Portable};

    #[test]
    fn float_identity_round_trips() {
        for bits in [0, 0x8000000000000000, 0x7ff8000000000001, 0x7ff8000000000002] {
            let value = f64::from_bits(bits);
            assert_eq!(f64::decode(&value.encode()).unwrap().to_bits(), bits);
        }
        let fingerprint = f64::fingerprint();
        assert_eq!(postcard::to_allocvec(&fingerprint).unwrap(), fingerprint.0);
        assert_eq!(serde_json::from_str::<super::Fingerprint>(&serde_json::to_string(&fingerprint).unwrap()).unwrap(), fingerprint);
    }
}
