//! The VAPIX recording API at `/axis-cgi/record/*`.

use crate::v4::disk_management::DiskId;
use crate::*;
use chrono::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::num::NonZeroU32;
use std::str::FromStr;

// Docs:
// https://www.axis.com/vapix-library/subjects/t10037719/section/t10004596/display?section=t10004596-t10004656
// https://www.axis.com/vapix-library/subjects/t10037719/section/t10004596/display?section=t10004596-t10047558

/// A device's recordings interface.
pub struct Recordings<'a, T: Transport> {
    device: &'a Client<T>,
    supports_continuous_recording: bool,
    supports_playback_over_rtsp: bool,
    supports_exporting: bool,
}

impl<'a, T: Transport> Recordings<'a, T> {
    pub(crate) async fn new(device: &'a Client<T>) -> Result<Option<Recordings<'a, T>>> {
        let params = device
            .parameters()
            .list(Some(
                &[
                    "Properties.API.HTTP.Version",
                    "Properties.LocalStorage",
                    "Properties.API.RTSP.Version",
                ][..],
            ))
            .await?;

        // We require Properties.API.HTTP.Version=3 and Properties.LocalStorage.LocalStorage=yes
        // in order to have a Recording interface
        if params
            .get("Properties.API.HTTP.Version")
            .map(String::as_str)
            != Some("3")
            || params
                .get("Properties.LocalStorage.LocalStorage")
                .map(String::as_str)
                != Some("yes")
        {
            return Ok(None);
        };

        // We may also support continuous recording
        let supports_continuous_recording = params
            .get("Properties.LocalStorage.ContinuousRecording")
            .map(String::as_str)
            == Some("yes")
            && params
                .get("Properties.LocalStorage.ContinuousRecordingProfiles")
                .and_then(|s| u32::from_str(&s).ok())
                .map(|num| num > 0)
                .unwrap_or(false);

        // And we may also support playback over RTSP
        let supports_playback_over_rtsp = params
            .get("Properties.API.RTSP.Version")
            .map(|s| s.as_str() >= "2.01")
            .unwrap_or(false);

        // And we may also support exporting recordings
        let supports_exporting = params
            .get("Properties.LocalStorage.ExportRecording")
            .map(String::as_str)
            == Some("yes");

        Ok(Some(Self {
            device,
            supports_continuous_recording,
            supports_playback_over_rtsp,
            supports_exporting,
        }))
    }

    pub async fn list_recordings(
        &self,
        _request: ListRecordingsRequest,
    ) -> Result<ListRecordingsResponse> {
        todo!()
    }
}

mod iso8601;
mod optional_iso8601;

mod list_cgi;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ListRecordingsRequest {
    pub event_id: Option<EventId>,
    pub disk_id: Option<DiskId>,
    pub source: Option<Source>,

    pub earliest_timestamp: Option<DateTime<Utc>>,
    pub latest_timestamp: Option<DateTime<Utc>>,

    pub pagination: Pagination,
    pub sort: Sort,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ListRecordingsResponse {
    /// The number of recordings which match the request.
    pub count: u64,
    /// The total number of recordings
    pub overall_total_recordings: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct Pagination {
    /// The number of records to return.
    pub page_size: Option<u64>,
    /// The offset of the first record.
    pub offset: Option<u64>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Source {
    /// The video channel number.
    ///
    /// The meaning of this value is product-dependent. Channels are always numbered sequentially
    /// starting from 1.
    ChannelNumber(NonZeroU32),
    /// A synthetic quad stream, available on 4-channel devices.
    Quad,
}

impl Default for Source {
    fn default() -> Self {
        Self::ChannelNumber(NonZeroU32::new(1).unwrap())
    }
}

impl Serialize for Source {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            Self::ChannelNumber(num) => serializer.serialize_str(&num.get().to_string()),
            Self::Quad => serializer.serialize_str("Quad"),
        }
    }
}

impl<'de> Deserialize<'de> for Source {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, Visitor};
        use std::convert::TryFrom;
        use std::fmt;

        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Source;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string containing a channel number (1 … 2^32) or \"Quad\"")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match v {
                    "Quad" => Ok(Source::Quad),
                    other => u32::from_str(other)
                        .ok()
                        .and_then(|v| NonZeroU32::try_from(v).ok())
                        .map(Source::ChannelNumber)
                        .ok_or_else(|| E::custom(format!("invaid channel number: {:?}", other))),
                }
            }
        }

        deserializer.deserialize_str(V)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sort {
    /// Results sorted by recording start time ascending, meaning the earliest start times first.
    EarliestFirst,
    /// Results sorted by recording start time descending, meaning the latest start times first.
    LatestFirst,
}

impl Default for Sort {
    fn default() -> Self {
        // This is arbitrary but it matches the web UI's default
        Sort::LatestFirst
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub id: RecordingId,
    pub disk_id: DiskId,
    pub start_time: DateTime<FixedOffset>,
    pub end_time: DateTime<FixedOffset>,
    pub source: Source,
    pub event_id: EventId,
    pub video: Option<Video>,
    pub audio: Option<Audio>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub video_type: VideoType,
    pub width: u32,
    pub height: u32,
    pub framerate: Framerate,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Framerate {
    pub numerator: NonZeroU32,
    pub denominator: NonZeroU32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum VideoType {
    /// Motion JPEG video with MIME type `image/jpeg`.
    #[serde(rename = "image/jpeg")]
    Mjpeg,

    /// Legacy MPEG-4 part 2 video with MIME type `video/MP4V-ES`. Not to be confused with `H264`.
    #[serde(rename = "video/MP4V-ES")]
    Mpeg4Part2,

    /// H.264 video with MIME type `video/x-h264`, also known as MPEG-4 part 10 and AVC.
    #[serde(rename = "video/x-h264")]
    H264,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Audio {
    pub audio_type: AudioType,
    pub bitrate: u32,
    pub sample_rate: u32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AudioType {
    /// `audio/mpeg`
    #[serde(rename = "audio/mpeg")]
    Mpeg,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecordingType {
    Triggered,
    Scheduled,
    Continuous,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecordingStatus {
    Unknown,
    Recording,
    Completed,
}

string_type!(pub struct EventId);
string_type!(pub struct RecordingId);
