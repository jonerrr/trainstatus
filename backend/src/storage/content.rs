use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Immutable bytes, not a provider identity or a process-local slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentId([u8; 32]);

impl ContentId {
    pub fn hash(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        blake3::Hash::from(self.0).to_hex().to_string()
    }

    pub fn parse_hex(value: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(
            value.len() == 64,
            "content identity must contain 64 hexadecimal digits"
        );
        let mut bytes = [0; 32];
        for (output, pair) in bytes.iter_mut().zip(value.as_bytes().as_chunks::<2>().0) {
            let digit = |byte: u8| -> anyhow::Result<u8> {
                match byte {
                    b'0'..=b'9' => Ok(byte - b'0'),
                    b'a'..=b'f' => Ok(byte - b'a' + 10),
                    _ => anyhow::bail!("content identity must use lowercase hexadecimal"),
                }
            };
            *output = (digit(pair[0])? << 4) | digit(pair[1])?;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for ContentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(blake3::Hash::from(self.0).to_hex().as_str())
    }
}

impl Serialize for ContentId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for ContentId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct DigestVisitor;
        impl de::Visitor<'_> for DigestVisitor {
            type Value = ContentId;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a 32-byte content identity")
            }

            fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<ContentId, E> {
                bytes
                    .try_into()
                    .map(ContentId)
                    .map_err(|_| E::invalid_length(bytes.len(), &self))
            }
        }
        deserializer.deserialize_bytes(DigestVisitor)
    }
}
