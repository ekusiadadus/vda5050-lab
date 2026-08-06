use std::{collections::HashSet, fmt};

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};

const DUPLICATE_MARKER: &str = "__VDA5050_DUPLICATE_KEY__:";
const DEPTH_MARKER: &str = "__VDA5050_DEPTH_LIMIT__";

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum StrictJsonError {
    InvalidUtf8,
    Malformed,
    DuplicateKey(String),
    TooDeep,
}

pub(crate) fn parse(bytes: &[u8], max_depth: usize) -> Result<Value, StrictJsonError> {
    if std::str::from_utf8(bytes).is_err() {
        return Err(StrictJsonError::InvalidUtf8);
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = StrictValueSeed {
        depth: 1,
        max_depth,
    }
    .deserialize(&mut deserializer)
    .map_err(|error| classify(&error))?;
    deserializer.end().map_err(|error| classify(&error))?;
    Ok(value)
}

fn classify(error: &serde_json::Error) -> StrictJsonError {
    let message = error.to_string();
    if let Some(rest) = message.split(DUPLICATE_MARKER).nth(1) {
        let key = rest.split(" at line ").next().unwrap_or(rest).to_owned();
        StrictJsonError::DuplicateKey(key)
    } else if message.contains(DEPTH_MARKER) || message.contains("recursion limit exceeded") {
        StrictJsonError::TooDeep
    } else {
        StrictJsonError::Malformed
    }
}

#[derive(Clone, Copy)]
struct StrictValueSeed {
    depth: usize,
    max_depth: usize,
}

impl<'de> DeserializeSeed<'de> for StrictValueSeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if self.depth > self.max_depth {
            return Err(de::Error::custom(DEPTH_MARKER));
        }
        deserializer.deserialize_any(StrictValueVisitor(self))
    }
}

struct StrictValueVisitor(StrictValueSeed);

impl<'de> Visitor<'de> for StrictValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_string(value.to_owned())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        StrictValueSeed {
            depth: self.0.depth + 1,
            max_depth: self.0.max_depth,
        }
        .deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        let child = StrictValueSeed {
            depth: self.0.depth + 1,
            max_depth: self.0.max_depth,
        };
        while let Some(value) = sequence.next_element_seed(child)? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        let mut keys = HashSet::new();
        let child = StrictValueSeed {
            depth: self.0.depth + 1,
            max_depth: self.0.max_depth,
        };
        while let Some(key) = object.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom(format!("{DUPLICATE_MARKER}{key}")));
            }
            let value = object.next_value_seed(child)?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
