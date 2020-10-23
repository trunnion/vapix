use serde::Deserialize;
use std::fmt;

/// An error returned by the `vapix` crate.
#[derive(Debug)]
pub enum Error {
    /// An HTTP request failed.
    HttpRequestFailed(Box<dyn std::error::Error + Send + 'static>),
    /// An HTTP request returned a response which could not be parsed.
    UnparseableResponseError(UnparseableResponseError),
    /// The API call returned a structured error.
    ApiError(ApiError),
    /// The device does not support this feature.
    UnsupportedFeature,
    /// An error which isn't yet properly itemized.
    Other(&'static str),
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::HttpRequestFailed(te) => write!(f, "HTTP request failed: {}", te),
            Error::UnparseableResponseError(e) => write!(f, "unparseable response: {:?}", e),
            Error::ApiError(e) => write!(f, "JSON API error: {:?}", e),
            Error::UnsupportedFeature => write!(f, "this device does not support that feature"),
            Error::Other(e) => write!(f, "error: {}", e),
        }
    }
}

impl From<crate::transport::Error> for Error {
    fn from(e: crate::transport::Error) -> Self {
        Error::HttpRequestFailed(e.into_inner())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::UnparseableResponseError(UnparseableResponseError::JsonDeError(e))
    }
}

impl From<quick_xml::DeError> for Error {
    fn from(e: quick_xml::DeError) -> Self {
        Error::UnparseableResponseError(UnparseableResponseError::XmlDeError(e))
    }
}

impl From<ApiError> for Error {
    fn from(e: ApiError) -> Self {
        Error::ApiError(e)
    }
}

#[derive(Debug)]
pub enum UnparseableResponseError {
    /// JSON deserialization failed.
    JsonDeError(serde_json::Error),
    /// XML deserialization failed.
    XmlDeError(quick_xml::DeError),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ApiError {
    InvalidParameter,
    AccessForbidden,
    UnsupportedHttpMethod,
    UnsupportedApiVersion,
    UnsupportedApiMethod,
    InvalidJsonFormat,
    RequiredParameterIsMissing,
    InternalError,
    OtherError(Box<RawJsonApiError>),
}

impl From<RawJsonApiError> for ApiError {
    fn from(e: RawJsonApiError) -> Self {
        match e.code {
            1000 => ApiError::InvalidParameter,
            2001 => ApiError::AccessForbidden,
            2002 => ApiError::UnsupportedHttpMethod,
            2003 => ApiError::UnsupportedApiVersion,
            2004 => ApiError::UnsupportedApiMethod,
            4000 => ApiError::InvalidJsonFormat,
            4002 => ApiError::RequiredParameterIsMissing,
            8000 => ApiError::InternalError,
            _ => ApiError::OtherError(Box::new(e)),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RawJsonApiError {
    pub code: u32,
    pub message: Option<String>,
}

impl From<RawJsonApiError> for Error {
    fn from(e: RawJsonApiError) -> Self {
        Error::ApiError(e.into())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HttpStatusCodeError(pub http::StatusCode);
impl std::error::Error for HttpStatusCodeError {}
impl fmt::Display for HttpStatusCodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "HTTP response returned status code {}", self.0)
    }
}
impl From<HttpStatusCodeError> for Error {
    fn from(e: HttpStatusCodeError) -> Self {
        Error::HttpRequestFailed(Box::new(e))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HttpContentTypeError(Option<Vec<u8>>, &'static str);
impl HttpContentTypeError {
    pub fn new(actual: Option<&http::header::HeaderValue>, expected: &'static str) -> Self {
        Self(actual.map(|v| v.as_bytes().to_vec()), expected)
    }
}
impl std::error::Error for HttpContentTypeError {}
impl fmt::Display for HttpContentTypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.0 {
            None => write!(f, "HTTP response had no Content-Type:, expected {}", self.1),
            Some(v) => write!(
                f,
                "HTTP response had Content-Type: {:?}, expected {}",
                String::from_utf8_lossy(&v),
                self.1
            ),
        }
    }
}
impl From<HttpContentTypeError> for Error {
    fn from(e: HttpContentTypeError) -> Self {
        Error::HttpRequestFailed(Box::new(e))
    }
}

pub(crate) trait ResultExt {
    fn map_404_to_unsupported_feature(self) -> Self;
}

impl<T> ResultExt for std::result::Result<T, Error> {
    fn map_404_to_unsupported_feature(self) -> Self {
        match self {
            Err(Error::HttpRequestFailed(e))
                if e.downcast_ref() == Some(&HttpStatusCodeError(http::StatusCode::NOT_FOUND)) =>
            {
                Err(Error::UnsupportedFeature)
            }
            other => other,
        }
    }
}
