//! Chrono-compatible helpers backed by [`time`].
//!
//! The project historically serialized naive datetimes as chrono's `Debug`
//! form (`YYYY-MM-DDTHH:MM:SS[.fraction]`) and UTC instants as RFC 3339 with
//! an explicit `+00:00` offset. This module keeps those bytes stable while
//! using `time` as the only datetime library.

use std::fmt;
use time::format_description::well_known::Rfc3339;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcDateTime, UtcOffset};

/// Alias preserving chrono's `Utc` spelling.
pub type Utc = UtcDateTime;
/// Explicit name for the UTC instant type.
pub use time::UtcDateTime as DateTimeUtc;
/// Alias preserving chrono's `NaiveDateTime` spelling.
pub type NaiveDateTime = PrimitiveDateTime;

/// Epoch naive datetime, matching chrono's `NaiveDateTime::default`.
pub fn default_naive() -> PrimitiveDateTime {
    PrimitiveDateTime::new(
        Date::from_calendar_date(1970, Month::January, 1).expect("valid epoch date"),
        Time::MIDNIGHT,
    )
}

/// Epoch UTC instant, matching chrono's `DateTime<Utc>::default`.
pub fn default_utc() -> UtcDateTime {
    utc(default_naive())
}

/// Returns midnight UTC for the current day.
pub fn start_of_day_utc() -> UtcDateTime {
    UtcDateTime::new(now().date(), Time::MIDNIGHT)
}
/// Signed duration used by the compatibility helpers.
pub type Duration = time::SignedDuration;

/// Error returned by the chrono-compatible parsers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

/// Returns the current UTC instant.
#[inline]
pub fn now() -> UtcDateTime {
    UtcDateTime::now()
}

/// Builds a UTC instant from Unix milliseconds, mirroring chrono's
/// `DateTime::from_timestamp_millis`.
#[inline]
pub fn from_millis(millis: i64) -> Option<UtcDateTime> {
    UtcDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000).ok()
}

/// Builds a UTC instant from Unix seconds.
#[inline]
pub fn from_secs(secs: i64) -> Option<UtcDateTime> {
    UtcDateTime::from_unix_timestamp(secs).ok()
}

/// Builds a UTC instant from Unix nanoseconds.
#[inline]
pub fn from_nanos(nanos: i128) -> Option<UtcDateTime> {
    UtcDateTime::from_unix_timestamp_nanos(nanos).ok()
}

/// Builds a UTC instant from Unix seconds and nanoseconds.
#[inline]
pub fn from_timestamp(secs: i64, nsecs: u32) -> Option<UtcDateTime> {
    UtcDateTime::from_unix_timestamp(secs)
        .ok()
        .and_then(|dt| dt.checked_add(Duration::nanoseconds(i64::from(nsecs))))
}

/// Parses an RFC 3339 timestamp into a UTC instant.
#[inline]
pub fn parse_rfc3339(value: &str) -> Result<UtcDateTime, ParseError> {
    DateTimeExt::parse_from_rfc3339(value)
}

/// Parses a naive datetime string into a chrono-compatible value.
#[inline]
pub fn parse_naive_str(value: &str) -> Result<PrimitiveDateTime, ParseError> {
    parse_naive(value).ok_or_else(|| ParseError("invalid datetime".to_string()))
}

/// Returns the Unix millisecond value of a UTC instant.
#[inline]
pub fn millis(dt: UtcDateTime) -> i64 {
    (dt.unix_timestamp_nanos() / 1_000_000) as i64
}

/// Returns the Unix second value of a UTC instant.
#[inline]
pub fn secs(dt: UtcDateTime) -> i64 {
    dt.unix_timestamp()
}

/// Returns the Unix nanosecond value of a UTC instant, when representable.
#[inline]
pub fn nanos_opt(dt: UtcDateTime) -> Option<i64> {
    i64::try_from(dt.unix_timestamp_nanos()).ok()
}

/// Converts a UTC instant into a naive UTC datetime.
#[inline]
pub fn naive(dt: UtcDateTime) -> PrimitiveDateTime {
    PrimitiveDateTime::new(dt.date(), dt.time())
}

/// Interprets a naive datetime as UTC.
#[inline]
pub fn utc(dt: PrimitiveDateTime) -> UtcDateTime {
    UtcDateTime::new(dt.date(), dt.time())
}

fn fraction(ns: u32) -> String {
    if ns == 0 {
        return String::new();
    }
    if ns.is_multiple_of(1_000_000) {
        format!(".{:03}", ns / 1_000_000)
    } else if ns.is_multiple_of(1_000) {
        format!(".{:06}", ns / 1_000)
    } else {
        format!(".{:09}", ns)
    }
}

/// Formats a naive datetime exactly like chrono's `NaiveDateTime` `Debug`
/// implementation, which is what serde used on the wire and on disk.
pub fn format_naive(dt: PrimitiveDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        fraction(dt.nanosecond())
    )
}

/// Formats a naive datetime as second-precision RFC 3339 with `+00:00`.
pub fn format_naive_secs_plus00(dt: PrimitiveDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00:00",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second()
    )
}

/// Parses the naive datetime forms accepted by the historical code.
///
/// Both the chrono serde form (`T` separator) and the `time` default form
/// (space separator) are accepted, with or without a fractional part.
pub fn parse_naive(value: &str) -> Option<PrimitiveDateTime> {
    let (date_time, rest) = {
        let pos = value.find(['T', ' '])?;
        (&value[..pos], &value[pos + 1..])
    };
    let mut date_parts = date_time.split('-');
    let year = date_parts.next()?.parse::<i32>().ok()?;
    let month = Month::try_from(date_parts.next()?.parse::<u8>().ok()?).ok()?;
    let day = date_parts.next()?.parse::<u8>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let (clock, frac) = match rest.split_once('.') {
        Some((clock, frac)) => {
            if frac.is_empty() || frac.len() > 9 || !frac.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let mut ns = frac.parse::<u32>().ok()?;
            for _ in frac.len()..9 {
                ns *= 10;
            }
            (clock, ns)
        }
        None => (rest, 0),
    };
    let mut time_parts = clock.split(':');
    let hour = time_parts.next()?.parse::<u8>().ok()?;
    let minute = time_parts.next()?.parse::<u8>().ok()?;
    let second = time_parts.next()?.parse::<u8>().ok()?;
    if time_parts.next().is_some() {
        return None;
    }

    Some(PrimitiveDateTime::new(
        Date::from_calendar_date(year, month, day).ok()?,
        Time::from_hms_nano(hour, minute, second, frac).ok()?,
    ))
}

/// Precision used when emitting an RFC 3339 timestamp.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Precision {
    /// No fractional seconds, as chrono's `SecondsFormat::Secs`.
    Secs,
    /// Millisecond precision, as chrono's `SecondsFormat::Millis`.
    Millis,
    /// Microsecond precision, as chrono's `SecondsFormat::Micros`.
    Micros,
    /// Nanosecond precision, as chrono's `SecondsFormat::Nanos`.
    Nanos,
    /// chrono's `SecondsFormat::AutoSi`: 0/3/6/9 digits.
    Auto,
}

/// Formats a UTC instant as RFC 3339 with an explicit `+00:00` offset.
pub fn rfc3339(dt: UtcDateTime) -> String {
    format_rfc3339(dt, Precision::Auto, '+')
}

/// Formats a UTC instant as RFC 3339 with a trailing `Z`.
pub fn rfc3339_z(dt: UtcDateTime, precision: Precision) -> String {
    format_rfc3339(dt, precision, 'Z')
}

fn format_rfc3339(dt: UtcDateTime, precision: Precision, zone: char) -> String {
    let ns = dt.nanosecond();
    let frac = match precision {
        Precision::Secs => String::new(),
        Precision::Millis => format!(".{:03}", ns / 1_000_000),
        Precision::Micros => format!(".{:06}", ns / 1_000),
        Precision::Nanos => format!(".{:09}", ns),
        Precision::Auto => fraction(ns),
    };
    let zone_text = if zone == 'Z' {
        "Z".to_string()
    } else {
        "+00:00".to_string()
    };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{}",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second(),
        frac,
        zone_text
    )
}

/// Extension methods preserving chrono's `DateTime<Utc>` call shape.
pub trait DateTimeExt: Sized {
    /// Builds an instant from Unix milliseconds, like chrono's associated fn.
    fn from_timestamp_millis(millis: i64) -> Option<Self>;
    /// Builds an instant from Unix seconds and nanoseconds.
    fn from_timestamp(secs: i64, nsecs: u32) -> Option<Self>;
    /// Parses an RFC 3339 timestamp.
    fn parse_from_rfc3339(value: &str) -> Result<Self, ParseError>;
    /// Returns Unix milliseconds.
    fn timestamp_millis(&self) -> i64;
    /// Returns Unix seconds.
    fn timestamp(&self) -> i64;
    /// Returns Unix nanoseconds when representable.
    fn timestamp_nanos_opt(&self) -> Option<i64>;
    /// Formats with chrono's default RFC 3339 representation.
    fn to_rfc3339(&self) -> String;
    /// Formats RFC 3339 with explicit precision and zone style.
    fn to_rfc3339_opts(&self, precision: Precision, use_z: bool) -> String;
    /// Formats as chrono-compatible RFC 3339 with `+00:00`.
    fn format_apollo(&self) -> String;
    /// Formats as `YYYY-MM-DD HH:MM:SS`.
    fn format_space(&self) -> String;
    /// Formats as `YYYYMMDDHHMMSS`.
    fn format_compact(&self) -> String;
    /// Returns the naive UTC datetime.
    fn naive_utc(&self) -> PrimitiveDateTime;
    /// Adds a signed duration.
    fn checked_add_signed(self, duration: Duration) -> Option<Self>;
}

impl DateTimeExt for UtcDateTime {
    #[inline]
    fn from_timestamp_millis(millis: i64) -> Option<Self> {
        from_millis(millis)
    }

    #[inline]
    fn from_timestamp(secs: i64, nsecs: u32) -> Option<Self> {
        UtcDateTime::from_unix_timestamp(secs)
            .ok()
            .and_then(|dt| dt.checked_add(Duration::nanoseconds(i64::from(nsecs))))
    }

    #[inline]
    fn parse_from_rfc3339(value: &str) -> Result<Self, ParseError> {
        let dt = OffsetDateTime::parse(value, &Rfc3339)
            .map_err(|e| ParseError(e.to_string()))?
            .to_offset(UtcOffset::UTC);
        Ok(UtcDateTime::new(dt.date(), dt.time()))
    }

    #[inline]
    fn timestamp_millis(&self) -> i64 {
        millis(*self)
    }

    #[inline]
    fn timestamp(&self) -> i64 {
        secs(*self)
    }

    #[inline]
    fn timestamp_nanos_opt(&self) -> Option<i64> {
        nanos_opt(*self)
    }

    #[inline]
    fn to_rfc3339(&self) -> String {
        rfc3339(*self)
    }

    #[inline]
    fn to_rfc3339_opts(&self, precision: Precision, use_z: bool) -> String {
        if use_z {
            rfc3339_z(*self, precision)
        } else {
            format_rfc3339(*self, precision, '+')
        }
    }

    #[inline]
    fn format_apollo(&self) -> String {
        rfc3339(*self)
    }

    #[inline]
    fn format_space(&self) -> String {
        let dt = naive(*self);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            dt.year(),
            u8::from(dt.month()),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second()
        )
    }

    #[inline]
    fn format_compact(&self) -> String {
        format!(
            "{:04}{:02}{:02}{:02}{:02}{:02}",
            self.year(),
            u8::from(self.month()),
            self.day(),
            self.hour(),
            self.minute(),
            self.second()
        )
    }

    #[inline]
    fn naive_utc(&self) -> PrimitiveDateTime {
        naive(*self)
    }

    #[inline]
    fn checked_add_signed(self, duration: Duration) -> Option<Self> {
        self.checked_add(duration)
    }
}

/// Extension methods preserving chrono's `NaiveDateTime` call shape.
pub trait NaiveDateTimeExt: Sized {
    /// Parses the naive datetime formats used by the historical code.
    fn parse_from_str(value: &str, format: &str) -> Result<Self, ParseError>;
    /// Formats as `YYYY-MM-DDTHH:MM:SS[.fraction]+00:00`.
    fn format_apollo(&self) -> String;
    /// Formats as second-precision `YYYY-MM-DDTHH:MM:SS+00:00`.
    fn format_apollo_secs(&self) -> String;
    /// Formats as `YYYYMMDDHHMMSS`.
    fn format_compact(&self) -> String;
    /// Formats as `YYYY-MM-DD HH:MM:SS`.
    fn format_space(&self) -> String;
    /// Interprets the naive datetime as UTC.
    fn and_utc(self) -> UtcDateTime;
}

impl NaiveDateTimeExt for PrimitiveDateTime {
    #[inline]
    fn parse_from_str(value: &str, _format: &str) -> Result<Self, ParseError> {
        parse_naive(value).ok_or_else(|| ParseError("invalid datetime".to_string()))
    }

    #[inline]
    fn format_apollo(&self) -> String {
        format!(
            "{}T{:02}:{:02}:{:02}{}+00:00",
            date_part(*self),
            self.hour(),
            self.minute(),
            self.second(),
            fraction(self.nanosecond())
        )
    }

    #[inline]
    fn format_apollo_secs(&self) -> String {
        format_naive_secs_plus00(*self)
    }

    #[inline]
    fn format_compact(&self) -> String {
        format!(
            "{:04}{:02}{:02}{:02}{:02}{:02}",
            self.year(),
            u8::from(self.month()),
            self.day(),
            self.hour(),
            self.minute(),
            self.second()
        )
    }

    #[inline]
    fn format_space(&self) -> String {
        format!(
            "{} {:02}:{:02}:{:02}",
            date_part(*self),
            self.hour(),
            self.minute(),
            self.second()
        )
    }

    #[inline]
    fn and_utc(self) -> UtcDateTime {
        utc(self)
    }
}

fn date_part(dt: PrimitiveDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        dt.year(),
        u8::from(dt.month()),
        dt.day()
    )
}

/// Serde adapter producing chrono-compatible naive datetime strings.
pub mod naive_serde {
    use super::{format_naive, parse_naive};
    use serde::{Deserialize, Deserializer, Serializer};
    use time::PrimitiveDateTime;

    /// Serializes a naive datetime in chrono's serde representation.
    pub fn serialize<S>(value: &PrimitiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format_naive(*value))
    }

    /// Deserializes a naive datetime from chrono's or `time`'s representation.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<PrimitiveDateTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_naive(&value).ok_or_else(|| serde::de::Error::custom("invalid datetime"))
    }

    /// Same adapter for optional naive datetimes.
    pub mod option {
        use super::serialize as ser;
        use serde::{Deserialize, Deserializer, Serializer};
        use time::PrimitiveDateTime;

        /// Serializes an optional naive datetime.
        pub fn serialize<S>(
            value: &Option<PrimitiveDateTime>,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match value {
                Some(value) => ser(value, serializer),
                None => serializer.serialize_none(),
            }
        }

        /// Deserializes an optional naive datetime.
        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<PrimitiveDateTime>, D::Error>
        where
            D: Deserializer<'de>,
        {
            Option::<String>::deserialize(deserializer)?
                .map(|value| {
                    super::parse_naive(&value)
                        .ok_or_else(|| serde::de::Error::custom("invalid datetime"))
                })
                .transpose()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Wire {
        #[serde(with = "naive_serde")]
        created_at: PrimitiveDateTime,
        #[serde(with = "naive_serde::option")]
        updated_at: Option<PrimitiveDateTime>,
    }

    #[test]
    fn naive_serde_matches_chrono_shape() {
        let dt = from_millis(1_763_829_296_123).unwrap();
        let naive = naive(dt);
        assert_eq!(format_naive(naive), "2025-11-22T16:34:56.123");
        let wire = Wire {
            created_at: naive,
            updated_at: Some(naive),
        };
        let json = serde_json::to_string(&wire).unwrap();
        assert_eq!(
            json,
            r#"{"created_at":"2025-11-22T16:34:56.123","updated_at":"2025-11-22T16:34:56.123"}"#
        );
        let back: Wire = serde_json::from_str(&json).unwrap();
        assert_eq!(back.created_at, naive);
        assert_eq!(back.updated_at, Some(naive));
    }

    #[test]
    fn naive_parser_accepts_both_separators_and_omits_zero_fraction() {
        let epoch = default_naive();
        assert_eq!(format_naive(epoch), "1970-01-01T00:00:00");
        assert_eq!(parse_naive("1970-01-01 00:00:00.0"), Some(epoch));
        assert_eq!(parse_naive("1970-01-01T00:00:00"), Some(epoch));
    }

    #[test]
    fn rfc3339_matches_chrono_default_and_explicit_variants() {
        let dt = from_millis(1_763_829_296_123).unwrap();
        assert_eq!(rfc3339(dt), "2025-11-22T16:34:56.123+00:00");
        assert_eq!(rfc3339_z(dt, Precision::Millis), "2025-11-22T16:34:56.123Z");
        assert_eq!(
            rfc3339_z(from_secs(0).unwrap(), Precision::Secs),
            "1970-01-01T00:00:00Z"
        );
    }

    /// Golden bytes captured from chrono 0.4.45. The console audit display
    /// used `format("%Y-%m-%d %H:%M:%S")`, which drops sub-second precision;
    /// these helpers must keep that shape exactly.
    #[test]
    fn display_helpers_match_chrono_golden_bytes() {
        let dt = from_millis(1_763_829_296_123).unwrap();
        let naive_dt = naive(dt);
        assert_eq!(DateTimeExt::format_space(&dt), "2025-11-22 16:34:56");
        assert_eq!(
            NaiveDateTimeExt::format_space(&naive_dt),
            "2025-11-22 16:34:56"
        );
        assert_eq!(DateTimeExt::format_compact(&dt), "20251122163456");
        assert_eq!(
            NaiveDateTimeExt::format_apollo(&naive_dt),
            "2025-11-22T16:34:56.123+00:00"
        );
        assert_eq!(
            NaiveDateTimeExt::format_apollo_secs(&naive_dt),
            "2025-11-22T16:34:56+00:00"
        );
    }
}
