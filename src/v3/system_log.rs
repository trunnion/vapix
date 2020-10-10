//! The VAPIX system log interface at `/axis-cgi/systemlog.cgi`.

use crate::*;
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::str::FromStr;

/// A device's system log interface.
pub struct SystemLog<'a, T: Transport>(&'a Device<T>);

impl<'a, T: Transport> SystemLog<'a, T> {
    pub(crate) fn new(device: &'a Device<T>) -> Self {
        Self(device)
    }

    // Retrieve system log information.
    //
    // The level of information included in the log is set in the `Log.System` parameter group.
    pub async fn entries(&self) -> Result<Entries, Error<T::Error>> {
        let req = http::request::Builder::new()
            .method(http::Method::GET)
            .uri(self.0.uri_for("/axis-cgi/systemlog.cgi").unwrap())
            .body(Vec::new())
            .unwrap();

        let (resp, body) = self.0.roundtrip(req, "text/plain").await?;

        // Use the HTTP Date: header returned with the logs to help parse the log timestamps
        // If that's missing or un-parseable, use our system time
        // Clock drift isn't that big of a problem until we get to ±6 months.
        let now = resp
            .headers
            .get(http::header::DATE)
            .and_then(|v| v.to_str().ok())
            .and_then(|d| DateTime::parse_from_rfc2822(d).ok())
            .unwrap_or(Local::now().into());

        Ok(Entries::new(
            String::from_utf8_lossy(body.as_slice()).into_owned(),
            now,
        ))
    }
}

/// A set of system log entries returned from the API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entries {
    buffer: String,
    generated_at: DateTime<FixedOffset>,
}

impl Entries {
    /// Instantiate `Entries` from a response `String` and the timestamp at which it was produced
    /// by the API.
    ///
    /// `generated_at` is sourced from the HTTP `Date:` response header. It is used to help
    /// interpret certain incomplete timestamp formats.
    pub fn new(buffer: String, generated_at: DateTime<FixedOffset>) -> Self {
        Self {
            buffer,
            generated_at,
        }
    }

    /// Iterate over the `Entries`.
    pub fn iter(&self) -> EntriesIter {
        EntriesIter(self.buffer.rsplit('\n'), None, self.generated_at)
    }
}

/// An `Iterator` which parses `Entry` records.
pub struct EntriesIter<'a>(
    std::str::RSplit<'a, char>,
    Option<Timestamp>,
    DateTime<FixedOffset>,
);

impl<'a> Iterator for EntriesIter<'a> {
    type Item = Result<Entry<'a>, EntryParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Parse the lines into a raw entries
        while let Some(line) = self.0.next() {
            // Trim trailing \r
            let line = if line.ends_with('\r') {
                &line[0..line.len() - 1]
            } else {
                line
            };

            let result = match line {
                _ if line.is_empty() => continue,
                _ if line.starts_with("----- ") && line.ends_with(" -----") => continue,
                line => RawEntry::parse(line),
            };

            return Some(
                result
                    .and_then(|raw_entry| raw_entry.cook(self.1, self.2))
                    .map(|entry| {
                        self.1 = Some(entry.timestamp);
                        entry
                    }),
            );
        }
        None
    }
}

impl<'a> IntoIterator for &'a Entries {
    type Item = Result<Entry<'a>, EntryParseError>;
    type IntoIter = EntriesIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Emergency,
    Alert,
    Critical,
    Error,
    Warning,
    Notice,
    Info,
    Debug,
    Repeated,
}

impl AsRef<str> for Level {
    fn as_ref(&self) -> &str {
        match self {
            Level::Emergency => "[ EMERG   ]",
            Level::Alert => "[ ALERT   ]",
            Level::Critical => "[ CRIT    ]",
            Level::Error => "[ ERR     ]",
            Level::Warning => "[ WARNING ]",
            Level::Notice => "[ NOTICE  ]",
            Level::Info => "[ INFO    ]",
            Level::Debug => "[ DEBUG   ]",
            Level::Repeated => "[REPEATED ]",
        }
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Source<'a> {
    None,
    Name(&'a str),
    NameAndPid(&'a str, u32),
}

impl<'a> Source<'a> {
    pub fn is_none(&self) -> bool {
        match self {
            Source::None => true,
            _ => false,
        }
    }

    pub fn is_some(&self) -> bool {
        match self {
            Source::None => false,
            _ => true,
        }
    }

    fn from_str(s: &'a str) -> Result<Self, ()> {
        match s.find('[') {
            Some(start) if s.ends_with(']') => {
                let name = &s[0..start];
                let pid = &s[start + 1..s.len() - 1];
                let pid = u32::from_str(pid).map_err(|_| ())?;
                Ok(Source::NameAndPid(name, pid))
            }
            _ if !s.is_empty() => Ok(Source::Name(s)),
            _ => Err(()),
        }
    }
}

impl<'a> std::fmt::Display for Source<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Source::None => Ok(()),
            Source::Name(s) => f.write_str(s),
            Source::NameAndPid(name, pid) => write!(f, "{}[{}]", name, pid),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Timestamp {
    Naive(NaiveDateTime),
    FixedOffset(DateTime<FixedOffset>),
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Timestamp::Naive(dt) => dt.format("%Y-%m-%dT%H:%M:%S").fmt(f),
            Timestamp::FixedOffset(dt) => dt.format("%Y-%m-%dT%H:%M:%S%.3f%:z").fmt(f),
        }
    }
}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Timestamp::Naive(a), Timestamp::Naive(b)) => a.partial_cmp(b),
            (Timestamp::FixedOffset(a), Timestamp::FixedOffset(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

mod raw_timestamp;
use raw_timestamp::RawTimestamp;

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry<'a> {
    pub timestamp: Timestamp,
    pub hostname: &'a str,
    pub level: Level,
    pub source: Source<'a>,
    pub message: &'a str,
}

#[derive(Debug, Clone, PartialEq)]
struct RawEntry<'a> {
    pub timestamp: RawTimestamp,
    pub hostname: &'a str,
    pub level: Level,
    pub source: Source<'a>,
    pub message: &'a str,
}

#[derive(Debug, PartialEq)]
pub struct EntryParseError;

impl<'a> RawEntry<'a> {
    fn cook(
        self,
        successor: Option<Timestamp>,
        now: DateTime<FixedOffset>,
    ) -> Result<Entry<'a>, EntryParseError> {
        let Self {
            timestamp,
            hostname,
            level,
            source,
            message,
        } = self;

        let timestamp = timestamp.into_timestamp(successor, now)?;

        Ok(Entry {
            timestamp,
            hostname,
            level,
            source,
            message,
        })
    }

    fn parse(s: &'a str) -> Result<Self, EntryParseError> {
        Self::parse_old(s).or_else(|_| Self::parse_new(s))
    }

    fn parse_old(s: &'a str) -> Result<Self, EntryParseError> {
        if s.len() < 30 {
            return Err(EntryParseError);
        }

        let level = &s[0..11];
        let timestamp = &s[11..26];
        let rest = &s[27..];

        if &s[26..27] != " " {
            return Err(EntryParseError);
        }

        let level = match level {
            "<EMERG   > " => Level::Emergency,
            "<ALERT   > " => Level::Alert,
            "<CRITICAL> " => Level::Critical,
            "<ERR     > " => Level::Error,
            "<WARNING > " => Level::Warning,
            "<NOTICE  > " => Level::Notice,
            "<INFO    > " => Level::Info,
            "<DEBUG   > " => Level::Debug,
            "<REPEATED> " => Level::Repeated,
            _ => return Err(EntryParseError),
        };

        let timestamp = RawTimestamp::parse_old(timestamp)?;

        let (hostname, rest) = rest
            .find(' ')
            .map(|i| rest.split_at(i))
            .ok_or(EntryParseError)?;
        let rest = &rest[1..];

        let (source, message) = parse_rest(rest)?;

        Ok(Self {
            timestamp,
            hostname,
            level,
            source,
            message,
        })
    }

    fn parse_new(s: &'a str) -> Result<Self, EntryParseError> {
        let (timestamp, hostname, rest) = {
            let mut i = s.splitn(3, ' ');
            (
                i.next().ok_or(EntryParseError)?,
                i.next().ok_or(EntryParseError)?,
                i.next().ok_or(EntryParseError)?,
            )
        };

        let timestamp = RawTimestamp::parse_new(timestamp)?;

        if rest.len() < 13 {
            return Err(EntryParseError);
        }
        let (level, rest) = rest.split_at(12);
        let level = match level {
            "[ EMERG   ] " => Level::Emergency,
            "[ ALERT   ] " => Level::Alert,
            "[ CRIT    ] " => Level::Critical,
            "[ ERR     ] " => Level::Error,
            "[ WARNING ] " => Level::Warning,
            "[ NOTICE  ] " => Level::Notice,
            "[ INFO    ] " => Level::Info,
            "[ DEBUG   ] " => Level::Debug,
            _ => return Err(EntryParseError),
        };

        let (source, message) = parse_rest(rest)?;

        Ok(Self {
            timestamp,
            hostname,
            level,
            source,
            message,
        })
    }
}

fn parse_rest(rest: &str) -> Result<(Source, &str), EntryParseError> {
    // We might have "{source}: {message}", or we might just have "{message}".
    // Parse assuming we have both, and backtrack as needed.
    let (source, message) = match rest.find(": ").map(|i| rest.split_at(i)) {
        Some((source, message)) => match Source::from_str(source) {
            Ok(source) => (source, &message[2..]),
            Err(_) => (Source::None, rest),
        },
        None => (Source::None, rest),
    };

    Ok((source, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries() {
        crate::test_with_devices(|test_device| async move {
            let entries = test_device.device.system_log().entries().await?;
            let parsed = entries
                .iter()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| Error::Other("couldn't parse an entry"))?;
            assert!(!parsed.is_empty());
            Ok(())
        })
    }

    fn partial_timestamp(month: u8, day: u8, hour: u32, minute: u32, second: u32) -> RawTimestamp {
        RawTimestamp::Partial(month, day, NaiveTime::from_hms(hour, minute, second))
    }

    fn full_timestamp(
        ymd: (i32, u32, u32),
        hmsms: (u32, u32, u32, u32),
        offset_secs: i32,
    ) -> RawTimestamp {
        let ymd = NaiveDate::from_ymd(ymd.0, ymd.1, ymd.2);
        let hms = NaiveTime::from_hms_milli(hmsms.0, hmsms.1, hmsms.2, hmsms.3);
        RawTimestamp::FixedOffset(
            FixedOffset::east(offset_secs)
                .from_local_datetime(&NaiveDateTime::new(ymd, hms))
                .unwrap(),
        )
    }

    #[test]
    fn parse_5_50_4_9() {
        assert_eq!(RawEntry::parse(
            "<REPEATED> Nov 14 06:08:29 axis-00408cb99b33 last CRITICAL  message repeated 4 times"
        ), Ok(RawEntry {
            timestamp: partial_timestamp(11, 14, 6, 8, 29),
            hostname: "axis-00408cb99b33",
            level: Level::Repeated,
            source: Source::None,
            message: "last CRITICAL  message repeated 4 times"
        }));

        assert_eq!(RawEntry::parse(
            "<CRITICAL> Nov 14 06:07:54 axis-00408cb99b33 kernel: CIFS VFS: Send error in SessSetup = -13"
        ), Ok(RawEntry {
            timestamp: partial_timestamp(11, 14, 6, 7, 54),
            hostname: "axis-00408cb99b33",
            level: Level::Critical,
            source: Source::Name("kernel"),
            message: "CIFS VFS: Send error in SessSetup = -13"
        }));
    }

    #[test]
    fn parse_5_51_7() {
        assert_eq!(
            RawEntry::parse(
                r"<INFO    > Oct  9 15:41:26 axis-00408cfb6888 syslogd[23459]: 1.4.1: restart."
            ),
            Ok(RawEntry {
                timestamp: partial_timestamp(10, 9, 15, 41, 26),
                hostname: "axis-00408cfb6888",
                level: Level::Info,
                source: Source::NameAndPid("syslogd", 23459),
                message: "1.4.1: restart.",
            })
        );
    }

    #[test]
    fn parse_9_80_2_2_entry() {
        assert_eq!(
            RawEntry::parse(
                "2020-10-09T10:30:02.425-05:00 axis-accc8ef7d108 [ INFO    ] systemd[1]: Started Rotate log files."
            ),
            Ok(RawEntry {
                timestamp: full_timestamp((2020, 10, 9), (10, 30, 2, 425), -5 * 3600),
                hostname: "axis-accc8ef7d108",
                level: Level::Info,
                source: Source::NameAndPid("systemd", 1),
                message: "Started Rotate log files."
            })
        );

        assert_eq!(
            RawEntry::parse(
                "2020-10-08T22:16:11.027-05:00 axis-accc8ef7d108 [ WARNING ] [    5.501068][    T1] systemd[1]: /usr/lib/systemd/system/imagectrl-data.service:2: Unknown key name \'Desription\' in section \'Unit\', ignoring."
            ),
            Ok(RawEntry {
                timestamp: full_timestamp((2020, 10, 8), (22, 16, 11, 27), -5 * 3600),
                hostname: "axis-accc8ef7d108",
                level: Level::Warning,
                source: Source::None,
                message:
                "[    5.501068][    T1] systemd[1]: /usr/lib/systemd/system/imagectrl-data.service:2: Unknown key name \'Desription\' in section \'Unit\', ignoring."
            })
        );

        assert_eq!(
            RawEntry::parse(
                "2020-10-08T22:16:11.033-05:00 axis-accc8ef7d108 [ WARNING ] kernel: [    7.050105][  T126] artpec_5: module license 'Proprietary' taints kernel."
            ),
            Ok(RawEntry {
                timestamp: full_timestamp((2020, 10, 8), (22, 16, 11, 33), -5 * 3600),
                hostname: "axis-accc8ef7d108",
                level: Level::Warning,
                source: Source::Name("kernel"),
                message:
                "[    7.050105][  T126] artpec_5: module license 'Proprietary' taints kernel."
            })
        );
    }
}
