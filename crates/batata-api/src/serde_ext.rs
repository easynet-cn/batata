//! Serde adapters shared by the HTTP API models and SDK DTOs.
//!
//! These keep the wire format aligned with upstream Nacos while preserving
//! numeric Rust field types in the published crates.

/// Serialize an `i64` as a JSON string and accept either a JSON string or a
/// JSON number on input.
///
/// Upstream Nacos annotates `ConfigBasicInfo.id` with
/// `@JsonFormat(shape = JsonFormat.Shape.STRING)` so that JavaScript clients
/// do not lose precision on large storage ids. batata keeps `i64` in Rust but
/// emits the same string shape; numeric input stays accepted for older peers.
pub mod string_i64 {
    use serde::{Deserializer, Serializer};

    /// Serialize the value as a decimal string.
    pub fn serialize<S>(value: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    /// Deserialize from a decimal string or a JSON integer.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: Deserializer<'de>,
    {
        super::deserialize_i64_from_string_or_number(deserializer)
    }
}

/// Accept either a JSON number or a numeric JSON string on input, but keep
/// emitting a JSON number.
///
/// Upstream Java DTOs declare these fields as `long`/`Long`. Jackson coerces a
/// numeric string into the wrapper on deserialization, so batata must do the
/// same. Unlike [`string_i64`], the output shape stays numeric because upstream
/// does not annotate these fields with `@JsonFormat(shape = STRING)`.
pub mod flex_i64 {
    use serde::{Deserializer, Serializer};

    /// Serialize the value as a JSON integer.
    pub fn serialize<S>(value: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(*value)
    }

    /// Deserialize from a decimal string or a JSON integer.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: Deserializer<'de>,
    {
        super::deserialize_i64_from_string_or_number(deserializer)
    }
}

fn deserialize_i64_from_string_or_number<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    use std::fmt;

    struct StringOrInteger;

    impl<'de> de::Visitor<'de> for StringOrInteger {
        type Value = i64;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an integer or a string containing an integer")
        }

        fn visit_i64<E>(self, value: i64) -> Result<i64, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_u64<E>(self, value: u64) -> Result<i64, E>
        where
            E: de::Error,
        {
            i64::try_from(value).map_err(de::Error::custom)
        }

        fn visit_str<E>(self, value: &str) -> Result<i64, E>
        where
            E: de::Error,
        {
            value.parse::<i64>().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(StringOrInteger)
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Sample {
        #[serde(with = "super::string_i64")]
        id: i64,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct FlexSample {
        #[serde(with = "super::flex_i64")]
        id: i64,
    }

    #[test]
    fn id_is_serialized_as_string() {
        let json = serde_json::to_string(&Sample {
            id: 862_926_428_394_491_904,
        })
        .unwrap();
        assert_eq!(
            json, r#"{"id":"862926428394491904"}"#,
            "upstream encodes storage ids as JSON strings"
        );
    }

    #[test]
    fn id_accepts_string_and_number_input() {
        let from_string: Sample = serde_json::from_str(r#"{"id":"42"}"#).unwrap();
        let from_number: Sample = serde_json::from_str(r#"{"id":42}"#).unwrap();
        assert_eq!(from_string, from_number);
    }

    #[test]
    fn id_rejects_non_numeric_string() {
        assert!(serde_json::from_str::<Sample>(r#"{"id":"not-a-number"}"#).is_err());
    }

    #[test]
    fn flex_id_stays_numeric_on_output() {
        let json = serde_json::to_string(&FlexSample { id: 42 }).unwrap();
        assert_eq!(json, r#"{"id":42}"#);
    }

    #[test]
    fn flex_id_accepts_string_and_number_input() {
        let from_string: FlexSample = serde_json::from_str(r#"{"id":"42"}"#).unwrap();
        let from_number: FlexSample = serde_json::from_str(r#"{"id":42}"#).unwrap();
        assert_eq!(from_string, from_number);
        assert_eq!(from_string.id, 42);
    }

    #[test]
    fn flex_id_rejects_non_numeric_string() {
        assert!(serde_json::from_str::<FlexSample>(r#"{"id":"1.5"}"#).is_err());
    }
}
