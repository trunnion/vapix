use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub device_info: DeviceInfo,
    #[serde(with = "serde_transaction_map")]
    pub transactions: HashMap<RecordedHttpRequest, RecordedHttpResponse>,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            device_info: DeviceInfo::default(),
            transactions: HashMap::new(),
        }
    }
}

mod serde_transaction_map {
    use super::*;
    use serde::de::SeqAccess;
    use serde::ser::SerializeSeq;

    pub(crate) fn serialize<S>(
        map: &HashMap<RecordedHttpRequest, RecordedHttpResponse>,
        ser: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let sorted_requests = {
            let mut vec = map.keys().collect::<Vec<_>>();
            vec.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
            vec
        };

        let mut seq = ser.serialize_seq(Some(map.len()))?;
        for req in sorted_requests {
            let resp = &map[req];
            let tx = RecordedTransaction {
                request: req.clone(),
                response: resp.clone(),
            };
            seq.serialize_element(&tx)?;
        }
        seq.end()
    }

    pub(crate) fn deserialize<'de, D>(
        de: D,
    ) -> Result<HashMap<RecordedHttpRequest, RecordedHttpResponse>, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = HashMap<RecordedHttpRequest, RecordedHttpResponse>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a sequence of recorded transactions")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, <A as SeqAccess<'de>>::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut map = HashMap::new();
                while let Some(tx) = seq.next_element::<RecordedTransaction>()? {
                    map.insert(tx.request, tx.response);
                }
                Ok(map)
            }
        }

        de.deserialize_seq(V)
    }
}

impl Recording {
    pub fn find<'a>(&'a self, req: &http::Request<Vec<u8>>) -> Option<&'a RecordedHttpResponse> {
        let req = RecordedHttpRequest::new(req);
        self.transactions.get(&req)
    }
}

pub fn fixture_dir() -> std::path::PathBuf {
    let root = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("fixtures").join("recordings")
}

pub fn fixture_filename(device_info: &DeviceInfo) -> std::path::PathBuf {
    let filename = PathBuf::from(format!(
        "{} v{}.json",
        &device_info.serial_number, &device_info.firmware_version,
    ));
    fixture_dir().join(&filename)
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecordedTransaction {
    pub request: RecordedHttpRequest,
    pub response: RecordedHttpResponse,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct RecordedHttpRequest {
    pub method: String,
    pub path: String,
    pub headers: RecordedHeaders,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<RecordedBody>,
}

impl RecordedHttpRequest {
    pub fn new<T: AsRef<[u8]>>(req: &http::Request<T>) -> Self {
        let body = req.body().as_ref();
        let body = match (req.method(), std::str::from_utf8(body)) {
            (&http::Method::GET, _) => None,
            (_, Ok(str)) => Some(RecordedBody::String(str.to_string())),
            (_, Err(_)) => Some(RecordedBody::Binary(body.to_vec())),
        };

        Self {
            method: req.method().to_string(),
            path: req
                .uri()
                .path_and_query()
                .map(|pq| pq.to_string())
                .unwrap_or("".into()),
            headers: RecordedHeaders::new(req.headers()),
            body,
        }
    }

    fn sort_key(&self) -> (&str, &str, &[u8]) {
        (
            &self.path,
            &self.method,
            self.body.as_ref().map(|b| b.as_slice()).unwrap_or(&[]),
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecordedHttpResponse {
    pub status_code: u16,
    pub headers: RecordedHeaders,
    pub body: RecordedBody,
}

impl RecordedHttpResponse {
    pub fn http_response_builder(&self) -> http::response::Builder {
        let mut b = http::response::Builder::new()
            .status(http::StatusCode::from_u16(self.status_code).unwrap());
        for (key, value) in &self.headers {
            b = b.header(key, value);
        }
        b
    }
}

pub struct RecordedHttpResponseBuilder {
    pub status_code: u16,
    pub headers: RecordedHeaders,
    pub body: Vec<u8>,
}

impl RecordedHttpResponseBuilder {
    pub fn new(resp: &http::response::Parts) -> Self {
        Self {
            status_code: resp.status.as_u16(),
            headers: RecordedHeaders::new(&resp.headers),
            body: Vec::new(),
        }
    }

    pub fn add_body_chunk(&mut self, body: &[u8]) {
        self.body.extend_from_slice(body);
    }

    pub fn build(self) -> RecordedHttpResponse {
        RecordedHttpResponse {
            status_code: self.status_code,
            headers: self.headers,
            body: match std::str::from_utf8(&self.body) {
                Ok(str) => RecordedBody::String(str.to_string()),
                Err(_) => RecordedBody::Binary(self.body),
            },
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct RecordedHeaders {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

impl RecordedHeaders {
    pub fn new(headers: &http::HeaderMap) -> Self {
        Self {
            accept: headers
                .get(http::header::ACCEPT)
                .map(|v| v.to_str().unwrap().to_string()),
            content_type: headers
                .get(http::header::CONTENT_TYPE)
                .map(|v| v.to_str().unwrap().to_string()),
        }
    }
}

impl<'a> IntoIterator for &'a RecordedHeaders {
    type Item = (http::header::HeaderName, http::header::HeaderValue);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let mut h = Vec::new();

        fn header(
            h: &mut Vec<(http::header::HeaderName, http::header::HeaderValue)>,
            name: http::header::HeaderName,
            value: &Option<String>,
        ) {
            if let Some(value) = value {
                h.push((name, http::header::HeaderValue::from_str(value).unwrap()))
            }
        }

        header(&mut h, http::header::ACCEPT, &self.accept);
        header(&mut h, http::header::CONTENT_TYPE, &self.content_type);

        h.into_iter()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordedBody {
    String(String),
    Binary(Vec<u8>),
}

impl RecordedBody {
    pub fn as_slice(&self) -> &[u8] {
        match &self {
            RecordedBody::String(s) => s.as_bytes(),
            RecordedBody::Binary(v) => v.as_slice(),
        }
    }
}

impl Hash for RecordedBody {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state)
    }
}
