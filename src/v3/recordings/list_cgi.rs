//! `list.cgi` request and response data structures. [Docs].
//!
//! [Docs]: https://www.axis.com/vapix-library/subjects/t10037719/section/t10004596/display?section=t10004596-t10004652

use super::*;

#[derive(Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Request<'a> {
    #[serde(rename = "listentity")]
    list_entity: &'a str,
    #[serde(
        default,
        rename = "recordingid",
        skip_serializing_if = "Option::is_none"
    )]
    recording_id: Option<&'a str>,
    #[serde(
        default,
        rename = "maxnumberofresults",
        skip_serializing_if = "Option::is_none"
    )]
    max_number_of_results: Option<u64>,
    #[serde(
        default,
        rename = "startatresultnumber",
        skip_serializing_if = "Option::is_none"
    )]
    start_at_result_number: Option<u64>,
    #[serde(default, rename = "eventid", skip_serializing_if = "Option::is_none")]
    event_id: Option<&'a str>,
    #[serde(default, rename = "diskid", skip_serializing_if = "Option::is_none")]
    disk_id: Option<&'a str>,
    #[serde(
        default,
        rename = "starttime",
        with = "optional_iso8601",
        skip_serializing_if = "Option::is_none"
    )]
    start_time: Option<DateTime<FixedOffset>>,
    #[serde(
        default,
        rename = "stoptime",
        with = "optional_iso8601",
        skip_serializing_if = "Option::is_none"
    )]
    stop_time: Option<DateTime<FixedOffset>>,
    #[serde(default, rename = "sortorder", skip_serializing_if = "Option::is_none")]
    sort_order: Option<&'a str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<Source>,
}

impl<'a> From<&'a ListRecordingsRequest> for Request<'a> {
    fn from(req: &'a ListRecordingsRequest) -> Self {
        Request {
            list_entity: "recordingid",
            recording_id: None,
            max_number_of_results: req.pagination.page_size,
            start_at_result_number: req.pagination.offset,
            event_id: req.event_id.as_ref().map(|id| id.into()),
            disk_id: req.disk_id.as_ref().map(|id| id.into()),
            start_time: req
                .earliest_timestamp
                .map(|dt| dt.with_timezone(&FixedOffset::west(0))),
            stop_time: req
                .latest_timestamp
                .map(|dt| dt.with_timezone(&FixedOffset::west(0))),
            sort_order: Some(match req.sort {
                Sort::EarliestFirst => "ascending",
                Sort::LatestFirst => "descending",
            }),
            source: req.source,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Response {
    #[serde(rename = "recordings")]
    Recordings(Recordings),
}

#[derive(Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Recordings {
    #[serde(rename = "totalnumberofrecordings")]
    pub total_number_of_recordings: u64,
    #[serde(rename = "numberofrecordings")]
    pub number_of_recordings: u64,
    #[serde(rename = "recording")]
    pub recordings: Vec<Recording>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Recording {
    #[serde(rename = "diskid")]
    pub disk_id: DiskId,
    #[serde(rename = "recordingid")]
    pub id: RecordingId,
    #[serde(rename = "starttime", with = "iso8601")]
    pub start_time: DateTime<FixedOffset>,
    #[serde(rename = "starttimelocal", with = "iso8601")]
    pub start_time_local: DateTime<FixedOffset>,
    #[serde(rename = "stoptime", with = "optional_iso8601")]
    pub end_time: Option<DateTime<FixedOffset>>,
    #[serde(rename = "stoptimelocal", with = "optional_iso8601")]
    pub end_time_local: Option<DateTime<FixedOffset>>,
    #[serde(rename = "recordingtype")]
    pub recording_type: RecordingType,
    #[serde(rename = "eventtrigger")]
    pub event_trigger: String,
    #[serde(rename = "eventid")]
    pub event_id: EventId,
    #[serde(rename = "recordingstatus")]
    pub recording_status: RecordingStatus,
    #[serde(rename = "source")]
    pub source: Source,

    // schema says maxoccurs="unbounded", so this is a Vec
    #[serde(rename = "video")]
    pub video: Vec<Video>,

    // schema says maxoccurs="unbounded", so this is a Vec
    #[serde(rename = "audio")]
    pub audio: Vec<Audio>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Video {
    #[serde(rename = "mimetype")]
    pub video_type: VideoType,
    pub width: u32,
    pub height: u32,
    pub framerate: Framerate,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Audio {
    #[serde(rename = "mimetype")]
    pub audio_type: AudioType,
    pub bitrate: u32,
    pub sample_rate: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    #[test]
    fn request_serde() {
        let req = Request {
            list_entity: "recording",
            recording_id: Some("id"),
            max_number_of_results: Some(123),
            start_at_result_number: Some(45),
            event_id: Some("eid"),
            disk_id: None,
            start_time: Some(
                Utc.timestamp(1234567890, 0)
                    .with_timezone(&FixedOffset::west(3600)),
            ),
            stop_time: None,
            sort_order: Some("ascending"),
            source: Some(Source::ChannelNumber(5.try_into().unwrap())),
        };
        let str = "listentity=recording\
        &recordingid=id\
        &maxnumberofresults=123\
        &startatresultnumber=45\
        &eventid=eid\
        &starttime=2009-02-13T22%3A31%3A30-01%3A00\
        &sortorder=ascending\
        &source=5";
        assert_eq!(&serde_urlencoded::to_string(&req).unwrap(), str);
        assert_eq!(&serde_urlencoded::from_str::<Request>(str).unwrap(), &req);

        let req = Request {
            list_entity: "event",
            source: Some(Source::Quad),
            ..Default::default()
        };
        let str = "listentity=event\
        &source=Quad";
        assert_eq!(&serde_urlencoded::to_string(&req).unwrap(), str);
        assert_eq!(&serde_urlencoded::from_str::<Request>(str).unwrap(), &req);
    }
}
