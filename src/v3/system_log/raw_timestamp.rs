use super::*;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RawTimestamp {
    Partial(u8, u8, NaiveTime),
    FixedOffset(DateTime<FixedOffset>),
}

impl RawTimestamp {
    pub(crate) fn into_timestamp(
        self,
        successor: Option<Timestamp>,
        now: DateTime<FixedOffset>,
    ) -> Result<Timestamp, EntryParseError> {
        let (m, d, hms, reference) = match (self, successor) {
            (RawTimestamp::FixedOffset(dt), _) => {
                // Nothing to cook
                return Ok(Timestamp::FixedOffset(dt));
            }
            (RawTimestamp::Partial(m, d, hms), Some(Timestamp::Naive(later))) => {
                // Cook the partial timestamp with "later" as the reference time
                (m, d, hms, later)
            }
            (RawTimestamp::Partial(m, d, hms), _) => {
                // Cook the partial timestamp, but we'll have to use "now" as the reference time
                (m, d, hms, now.naive_utc())
            }
        };

        // (md, hms) is likely near "reference". It could have the same year, the previous year, or
        // the next year.
        // We want to pick whichever is closest to our reference timestamp.
        let reference_timestamp = reference.timestamp();

        // Try the current year
        let current_year = with_year(reference.year(), m, d, hms);
        match current_year {
            Some(ts) if (ts.timestamp() - reference_timestamp).abs() < 180 * 86400 => {
                // This is within 180 days of our reference. It's definitely the closest.
                return Ok(Timestamp::Naive(ts));
            }
            _ => {
                // The current year may or may not be the closest
                // Try adjacent years
                let mut options = Vec::with_capacity(3);
                if let Some(ts) = current_year {
                    options.push(ts);
                }
                if let Some(ts) = with_year(reference.year() - 1, m, d, hms) {
                    options.push(ts);
                }
                if let Some(ts) = with_year(reference.year() + 1, m, d, hms) {
                    options.push(ts);
                }

                // Which is closest?
                // That is, which has the smallest difference relative to our reference time?
                options.sort_by_key(|dt| (dt.timestamp() - reference_timestamp).abs());

                // Pick that one
                options
                    .into_iter()
                    .next()
                    .map(Timestamp::Naive)
                    .ok_or(EntryParseError)
            }
        }
    }

    pub(crate) fn parse_new(timestamp: &str) -> Result<Self, EntryParseError> {
        if timestamp.len() != 29 {
            return Err(EntryParseError);
        }
        let year = parse_year(&timestamp[0..4])?;
        let month = parse_dash_month_dash(&timestamp[4..8])?;
        let day = parse_day_t(&timestamp[8..11])?;
        let hour = parse_hour(&timestamp[11..13])?;
        let minute = parse_colon_minute(&timestamp[13..16])?;
        let (second, millis) = parse_colon_second(&timestamp[16..19])?;
        if &timestamp[19..20] != "." {
            return Err(EntryParseError);
        }
        let millis = millis + u32::from_str(&timestamp[20..23]).map_err(|_| EntryParseError)?;
        let offset = parse_offset(&timestamp[23..])?;

        NaiveDate::from_ymd_opt(year, month as _, day as _)
            .and_then(|d| d.and_hms_milli_opt(hour, minute, second, millis))
            .and_then(|dt| FixedOffset::east(offset).from_local_datetime(&dt).single())
            .map(RawTimestamp::FixedOffset)
            .ok_or(EntryParseError)
    }

    pub(crate) fn parse_old(timestamp: &str) -> Result<Self, EntryParseError> {
        if timestamp.len() != 15 {
            return Err(EntryParseError);
        }

        let month = parse_month_space(&timestamp[0..4])?;
        let day = parse_day_space(&timestamp[4..7])?;
        let hour = parse_hour(&timestamp[7..9])?;
        let minute = parse_colon_minute(&timestamp[9..12])?;
        let (second, millis) = parse_colon_second(&timestamp[12..15])?;

        let hms = NaiveTime::from_hms_milli(hour, minute, second, millis);

        Ok(RawTimestamp::Partial(month, day, hms))
    }
}

fn parse_year(s: &str) -> Result<i32, EntryParseError> {
    i32::from_str(s).map_err(|_| EntryParseError)
}

fn parse_day_t(s: &str) -> Result<u8, EntryParseError> {
    Ok(match s {
        "01T" => 1,
        "02T" => 2,
        "03T" => 3,
        "04T" => 4,
        "05T" => 5,
        "06T" => 6,
        "07T" => 7,
        "08T" => 8,
        "09T" => 9,
        "10T" => 10,
        "11T" => 11,
        "12T" => 12,
        "13T" => 13,
        "14T" => 14,
        "15T" => 15,
        "16T" => 16,
        "17T" => 17,
        "18T" => 18,
        "19T" => 19,
        "20T" => 20,
        "21T" => 21,
        "22T" => 22,
        "23T" => 23,
        "24T" => 24,
        "25T" => 25,
        "26T" => 26,
        "27T" => 27,
        "28T" => 28,
        "29T" => 29,
        "30T" => 30,
        "31T" => 31,
        _ => return Err(EntryParseError),
    })
}

fn parse_dash_month_dash(s: &str) -> Result<u8, EntryParseError> {
    Ok(match s {
        "-01-" => 1,
        "-02-" => 2,
        "-03-" => 3,
        "-04-" => 4,
        "-05-" => 5,
        "-06-" => 6,
        "-07-" => 7,
        "-08-" => 8,
        "-09-" => 9,
        "-10-" => 10,
        "-11-" => 11,
        "-12-" => 12,
        _ => return Err(EntryParseError),
    })
}

fn parse_month_space(s: &str) -> Result<u8, EntryParseError> {
    Ok(match s {
        "Jan " => 1,
        "Feb " => 2,
        "Mar " => 3,
        "Apr " => 4,
        "May " => 5,
        "Jun " => 6,
        "Jul " => 7,
        "Aug " => 8,
        "Sep " => 9,
        "Oct " => 10,
        "Nov " => 11,
        "Dec " => 12,
        _ => return Err(EntryParseError),
    })
}

fn parse_day_space(s: &str) -> Result<u8, EntryParseError> {
    Ok(match s {
        " 1 " => 1,
        " 2 " => 2,
        " 3 " => 3,
        " 4 " => 4,
        " 5 " => 5,
        " 6 " => 6,
        " 7 " => 7,
        " 8 " => 8,
        " 9 " => 9,
        "10 " => 10,
        "11 " => 11,
        "12 " => 12,
        "13 " => 13,
        "14 " => 14,
        "15 " => 15,
        "16 " => 16,
        "17 " => 17,
        "18 " => 18,
        "19 " => 19,
        "20 " => 20,
        "21 " => 21,
        "22 " => 22,
        "23 " => 23,
        "24 " => 24,
        "25 " => 25,
        "26 " => 26,
        "27 " => 27,
        "28 " => 28,
        "29 " => 29,
        "30 " => 30,
        "31 " => 31,
        _ => return Err(EntryParseError),
    })
}

fn parse_hour(s: &str) -> Result<u32, EntryParseError> {
    Ok(match s {
        "00" => 0,
        "01" => 1,
        "02" => 2,
        "03" => 3,
        "04" => 4,
        "05" => 5,
        "06" => 6,
        "07" => 7,
        "08" => 8,
        "09" => 9,
        "10" => 10,
        "11" => 11,
        "12" => 12,
        "13" => 13,
        "14" => 14,
        "15" => 15,
        "16" => 16,
        "17" => 17,
        "18" => 18,
        "19" => 19,
        "20" => 20,
        "21" => 21,
        "22" => 22,
        "23" => 23,
        _ => return Err(EntryParseError),
    })
}

fn parse_colon_minute(s: &str) -> Result<u32, EntryParseError> {
    Ok(match s {
        ":00" => 0,
        ":01" => 1,
        ":02" => 2,
        ":03" => 3,
        ":04" => 4,
        ":05" => 5,
        ":06" => 6,
        ":07" => 7,
        ":08" => 8,
        ":09" => 9,
        ":10" => 10,
        ":11" => 11,
        ":12" => 12,
        ":13" => 13,
        ":14" => 14,
        ":15" => 15,
        ":16" => 16,
        ":17" => 17,
        ":18" => 18,
        ":19" => 19,
        ":20" => 20,
        ":21" => 21,
        ":22" => 22,
        ":23" => 23,
        ":24" => 24,
        ":25" => 25,
        ":26" => 26,
        ":27" => 27,
        ":28" => 28,
        ":29" => 29,
        ":30" => 30,
        ":31" => 31,
        ":32" => 32,
        ":33" => 33,
        ":34" => 34,
        ":35" => 35,
        ":36" => 36,
        ":37" => 37,
        ":38" => 38,
        ":39" => 39,
        ":40" => 40,
        ":41" => 41,
        ":42" => 42,
        ":43" => 43,
        ":44" => 44,
        ":45" => 45,
        ":46" => 46,
        ":47" => 47,
        ":48" => 48,
        ":49" => 49,
        ":50" => 50,
        ":51" => 51,
        ":52" => 52,
        ":53" => 53,
        ":54" => 54,
        ":55" => 55,
        ":56" => 56,
        ":57" => 57,
        ":58" => 58,
        ":59" => 59,
        _ => return Err(EntryParseError),
    })
}

fn parse_colon_second(s: &str) -> Result<(u32, u32), EntryParseError> {
    Ok(match s {
        ":00" => (0, 0),
        ":01" => (1, 0),
        ":02" => (2, 0),
        ":03" => (3, 0),
        ":04" => (4, 0),
        ":05" => (5, 0),
        ":06" => (6, 0),
        ":07" => (7, 0),
        ":08" => (8, 0),
        ":09" => (9, 0),
        ":10" => (10, 0),
        ":11" => (11, 0),
        ":12" => (12, 0),
        ":13" => (13, 0),
        ":14" => (14, 0),
        ":15" => (15, 0),
        ":16" => (16, 0),
        ":17" => (17, 0),
        ":18" => (18, 0),
        ":19" => (19, 0),
        ":20" => (20, 0),
        ":21" => (21, 0),
        ":22" => (22, 0),
        ":23" => (23, 0),
        ":24" => (24, 0),
        ":25" => (25, 0),
        ":26" => (26, 0),
        ":27" => (27, 0),
        ":28" => (28, 0),
        ":29" => (29, 0),
        ":30" => (30, 0),
        ":31" => (31, 0),
        ":32" => (32, 0),
        ":33" => (33, 0),
        ":34" => (34, 0),
        ":35" => (35, 0),
        ":36" => (36, 0),
        ":37" => (37, 0),
        ":38" => (38, 0),
        ":39" => (39, 0),
        ":40" => (40, 0),
        ":41" => (41, 0),
        ":42" => (42, 0),
        ":43" => (43, 0),
        ":44" => (44, 0),
        ":45" => (45, 0),
        ":46" => (46, 0),
        ":47" => (47, 0),
        ":48" => (48, 0),
        ":49" => (49, 0),
        ":50" => (50, 0),
        ":51" => (51, 0),
        ":52" => (52, 0),
        ":53" => (53, 0),
        ":54" => (54, 0),
        ":55" => (55, 0),
        ":56" => (56, 0),
        ":57" => (57, 0),
        ":58" => (58, 0),
        ":59" => (59, 0),
        ":60" => (60, 1000),
        _ => return Err(EntryParseError),
    })
}

fn parse_offset(s: &str) -> Result<i32, EntryParseError> {
    Ok(match s {
        "Z" => 0,
        s if s.len() == 6 => {
            let neg = match &s[0..1] {
                "-" => -1,
                "+" => 1,
                _ => return Err(EntryParseError),
            };
            let hour = parse_hour(&s[1..3])? as i32;
            let minute = parse_colon_minute(&s[3..6])? as i32;
            (neg * (hour * 60) + minute) * 60
        }
        _ => return Err(EntryParseError),
    })
}

fn with_year(year: i32, month: u8, day: u8, hms: NaiveTime) -> Option<NaiveDateTime> {
    NaiveDate::from_ymd_opt(year, month as _, day as _).map(|d| NaiveDateTime::new(d, hms))
}

#[cfg(test)]
mod tests {
    use crate::v3::system_log::raw_timestamp::RawTimestamp;
    use chrono::NaiveTime;

    #[test]
    fn parse_old() {
        assert_eq!(
            RawTimestamp::parse_old("Oct 10 00:19:57"),
            Ok(RawTimestamp::Partial(
                10,
                10,
                NaiveTime::from_hms(0, 19, 57)
            ))
        );
    }
}
