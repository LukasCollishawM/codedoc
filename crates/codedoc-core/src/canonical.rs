use std::collections::BTreeMap;
use std::fmt;

use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::error::CanonicalError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Canonical {
    Null,
    Bool(bool),
    Integer(i64),
    Text(String),
    Array(Vec<Canonical>),
    Object(BTreeMap<String, Canonical>),
}

impl Canonical {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_into(&mut out);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CanonicalError> {
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);
        let value = Canonical::deserialize(&mut deserializer)
            .map_err(|source| CanonicalError::Malformed { detail: source.to_string() })?;
        deserializer
            .end()
            .map_err(|source| CanonicalError::Malformed { detail: source.to_string() })?;
        Ok(value)
    }

    pub fn from_serializable<T: Serialize>(value: &T) -> Result<Self, CanonicalError> {
        let json = serde_json::to_vec(value)
            .map_err(|source| CanonicalError::Malformed { detail: source.to_string() })?;
        Canonical::decode(&json)
    }

    pub fn into_deserializable<T: for<'de> Deserialize<'de>>(&self) -> Result<T, CanonicalError> {
        serde_json::from_slice(&self.encode())
            .map_err(|source| CanonicalError::Malformed { detail: source.to_string() })
    }

    pub fn field(&self, name: &str) -> Option<&Canonical> {
        match self {
            Canonical::Object(fields) => fields.get(name),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Canonical::Text(text) => Some(text),
            _ => None,
        }
    }

    fn write_into(&self, out: &mut Vec<u8>) {
        match self {
            Canonical::Null => out.extend_from_slice(b"null"),
            Canonical::Bool(true) => out.extend_from_slice(b"true"),
            Canonical::Bool(false) => out.extend_from_slice(b"false"),
            Canonical::Integer(number) => out.extend_from_slice(number.to_string().as_bytes()),
            Canonical::Text(text) => write_text(text, out),
            Canonical::Array(items) => {
                out.push(b'[');
                for (position, item) in items.iter().enumerate() {
                    if position > 0 {
                        out.push(b',');
                    }
                    item.write_into(out);
                }
                out.push(b']');
            }
            Canonical::Object(fields) => {
                out.push(b'{');
                for (position, (key, value)) in fields.iter().enumerate() {
                    if position > 0 {
                        out.push(b',');
                    }
                    write_text(key, out);
                    out.push(b':');
                    value.write_into(out);
                }
                out.push(b'}');
            }
        }
    }
}

fn write_text(text: &str, out: &mut Vec<u8>) {
    out.push(b'"');
    for character in text.chars() {
        match character {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{08}' => out.extend_from_slice(b"\\b"),
            '\u{0c}' => out.extend_from_slice(b"\\f"),
            '\n' => out.extend_from_slice(b"\\n"),
            '\r' => out.extend_from_slice(b"\\r"),
            '\t' => out.extend_from_slice(b"\\t"),
            control if (control as u32) < 0x20 => {
                out.extend_from_slice(format!("\\u{:04x}", control as u32).as_bytes());
            }
            ordinary => {
                let mut buffer = [0u8; 4];
                out.extend_from_slice(ordinary.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    out.push(b'"');
}

impl fmt::Display for Canonical {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&String::from_utf8_lossy(&self.encode()))
    }
}

struct CanonicalVisitor;

impl<'de> Visitor<'de> for CanonicalVisitor {
    type Value = Canonical;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("canonical JSON without floating point numbers")
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Canonical, E> {
        Ok(Canonical::Null)
    }

    fn visit_none<E: serde::de::Error>(self) -> Result<Canonical, E> {
        Ok(Canonical::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Canonical, D::Error> {
        deserializer.deserialize_any(CanonicalVisitor)
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Canonical, E> {
        Ok(Canonical::Bool(value))
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Canonical, E> {
        Ok(Canonical::Integer(value))
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Canonical, E> {
        i64::try_from(value)
            .map(Canonical::Integer)
            .map_err(|_| E::custom(CanonicalError::IntegerRange { found: value.to_string() }))
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Canonical, E> {
        Err(E::custom(CanonicalError::FloatRejected { found: value.to_string() }))
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Canonical, E> {
        Ok(Canonical::Text(value.to_owned()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Canonical, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = access.next_element()? {
            items.push(item);
        }
        Ok(Canonical::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Canonical, A::Error> {
        let mut fields = BTreeMap::new();
        while let Some((key, value)) = access.next_entry::<String, Canonical>()? {
            if fields.insert(key.clone(), value).is_some() {
                return Err(serde::de::Error::custom(CanonicalError::DuplicateKey { key }));
            }
        }
        Ok(Canonical::Object(fields))
    }
}

impl<'de> Deserialize<'de> for Canonical {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(CanonicalVisitor)
    }
}

impl Serialize for Canonical {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Canonical::Null => serializer.serialize_unit(),
            Canonical::Bool(value) => serializer.serialize_bool(*value),
            Canonical::Integer(value) => serializer.serialize_i64(*value),
            Canonical::Text(value) => serializer.serialize_str(value),
            Canonical::Array(items) => items.serialize(serializer),
            Canonical::Object(fields) => fields.serialize(serializer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_keys_sort_regardless_of_input_order() {
        let forward = Canonical::decode(br#"{"a":1,"b":2,"c":3}"#).unwrap();
        let reverse = Canonical::decode(br#"{"c":3,"b":2,"a":1}"#).unwrap();
        assert_eq!(forward.encode(), reverse.encode());
        assert_eq!(forward.encode(), br#"{"a":1,"b":2,"c":3}"#);
    }

    #[test]
    fn floats_are_rejected() {
        assert!(Canonical::decode(br#"{"value":1.5}"#).is_err());
    }

    #[test]
    fn duplicate_keys_are_rejected() {
        assert!(Canonical::decode(br#"{"a":1,"a":2}"#).is_err());
    }

    #[test]
    fn trailing_content_is_rejected() {
        assert!(Canonical::decode(br#"{"a":1} trailing"#).is_err());
    }

    #[test]
    fn control_characters_escape_to_lowercase_hex() {
        assert_eq!(Canonical::Text("\u{1}".to_owned()).encode(), br#""\u0001""#);
    }

    #[test]
    fn non_ascii_is_emitted_raw() {
        let value = Canonical::Text("\u{e9}\u{1f600}".to_owned());
        assert_eq!(value.encode(), "\"\u{e9}\u{1f600}\"".as_bytes());
    }

    #[test]
    fn encoding_is_idempotent_across_round_trips() {
        let source = br#"{"z":[1,{"b":null,"a":true}],"y":"text"}"#;
        let once = Canonical::decode(source).unwrap().encode();
        let twice = Canonical::decode(&once).unwrap().encode();
        assert_eq!(once, twice);
    }
}
