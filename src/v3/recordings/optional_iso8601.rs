use chrono::prelude::*;
use serde::{Deserializer, Serializer};

pub fn serialize<S, Tz: TimeZone>(
    date_time: &Option<DateTime<Tz>>,
    serializer: S,
) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
where
    S: Serializer,
    Tz::Offset: std::fmt::Display,
{
    match date_time {
        None => serializer.serialize_str(""),
        Some(dt) => serializer.serialize_str(&dt.to_rfc3339()),
    }
}

pub fn deserialize<'de, D>(
    deserializer: D,
) -> Result<Option<DateTime<FixedOffset>>, <D as Deserializer<'de>>::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct V;
    impl<'de> Visitor<'de> for V {
        type Value = Option<DateTime<FixedOffset>>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an ISO8601 timestamp or an empty string")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v == "" {
                return Ok(None);
            }
            DateTime::parse_from_rfc3339(v)
                .map(|dt| Some(dt))
                .map_err(|e| E::custom(format!("invaild timestamp {:?}: {}", v, e)))
        }
    }

    let visitor = V;
    deserializer.deserialize_str(visitor)
}
