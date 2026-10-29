/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Portable Schema And Content Identity Values

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fingerprint(pub [u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(pub [u8; 32]);

#[cfg(test)]
mod tests {
    use crate::{Message, Portable};

    #[test]
    fn float_identity_vectors() {
        assert_eq!(f64::fingerprint().to_string(), "c2be9d59dd899a0610a0733ea228e01124f88493c4769be406a3298311a6c907");
        for (bits, expected) in [
            (0, "35f3767c67d333522f9e1bbc6af672f58ab5b5ce79a682eb73b07509495ef0bf"),
            (0x8000000000000000, "119a7f9f7f4abd037483acd7fc28dff27e8790b537c553e6d95c274dfc4c6cc3"),
            (0x7ff8000000000001, "f79e77241c991154c361b16a433784f1bacbe34570cbf8afda3f0543a3112802"),
            (0x7ff8000000000002, "73786941275e993a7bc30aa530a9b6a7c765139f8e47a576c8e71b5d99989a83"),
        ] {
            let value = f64::from_bits(bits);
            assert_eq!(value.digest().to_string(), expected);
            assert_eq!(f64::decode(&value.encode()).unwrap().to_bits(), bits);
        }
        let fingerprint = f64::fingerprint();
        assert_eq!(postcard::to_allocvec(&fingerprint).unwrap(), fingerprint.0);
        assert_eq!(serde_json::from_str::<super::Fingerprint>(&serde_json::to_string(&fingerprint).unwrap()).unwrap(), fingerprint);
    }
}
