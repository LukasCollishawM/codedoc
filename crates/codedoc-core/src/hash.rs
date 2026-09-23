use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ParseError;

pub const DIGEST_LENGTH: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest([u8; DIGEST_LENGTH]);

impl Digest {
    pub fn of_domain(domain: &str, payload: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain.as_bytes());
        hasher.update(&[0u8]);
        hasher.update(payload);
        Digest(*hasher.finalize().as_bytes())
    }

    pub fn bytes(&self) -> &[u8; DIGEST_LENGTH] {
        &self.0
    }

    pub fn from_bytes(bytes: [u8; DIGEST_LENGTH]) -> Self {
        Digest(bytes)
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self}")
    }
}

impl FromStr for Digest {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != DIGEST_LENGTH * 2 {
            return Err(ParseError::DigestLength { found: text.len() });
        }
        let mut bytes = [0u8; DIGEST_LENGTH];
        for (index, slot) in bytes.iter_mut().enumerate() {
            let pair = &text[index * 2..index * 2 + 2];
            *slot = u8::from_str_radix(pair, 16)
                .map_err(|_| ParseError::DigestEncoding { found: pair.to_owned() })?;
        }
        Ok(Digest(bytes))
    }
}

impl Serialize for Digest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

macro_rules! digest_newtype {
    ($name:ident, $domain:literal) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Digest);

        impl $name {
            pub const DOMAIN: &'static str = $domain;

            pub fn of(payload: &[u8]) -> Self {
                $name(Digest::of_domain($domain, payload))
            }

            pub fn digest(&self) -> &Digest {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}({})", stringify!($name), self.0)
            }
        }

        impl FromStr for $name {
            type Err = ParseError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                Ok($name(text.parse()?))
            }
        }
    };
}

digest_newtype!(RecordId, "codedoc.record.v1");
digest_newtype!(AnchorId, "codedoc.anchor.v1");
digest_newtype!(FileId, "codedoc.file.v1");
digest_newtype!(ContentFingerprint, "codedoc.fingerprint.content.v1");
digest_newtype!(StructuralFingerprint, "codedoc.fingerprint.structural.v1");
digest_newtype!(ContextFingerprint, "codedoc.fingerprint.context.v1");
digest_newtype!(LedgerHead, "codedoc.ledger.head.v1");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_round_trips_through_hex() {
        let digest = Digest::of_domain("test", b"payload");
        let rendered = digest.to_string();
        assert_eq!(rendered.len(), 64);
        assert_eq!(rendered.parse::<Digest>().unwrap(), digest);
    }

    #[test]
    fn domains_separate_identical_payloads() {
        let record = RecordId::of(b"same");
        let file = FileId::of(b"same");
        assert_ne!(record.digest(), file.digest());
    }

    #[test]
    fn rejects_short_hex() {
        assert!("abcd".parse::<Digest>().is_err());
    }

    #[test]
    fn rejects_non_hex() {
        let bad = "z".repeat(64);
        assert!(bad.parse::<Digest>().is_err());
    }
}
