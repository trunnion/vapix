use futures::Stream;
use std::fmt;
use std::future::Future;

/// An HTTP transport.
pub trait Transport {
    /// The `Future` representing the part of the roundtrip leading to the response headers.
    type Output: Future<Output = Result<http::Response<Self::Body>, Error>>;

    /// The `Future` representing the part of the roundtrip which streams the response body.
    type Body: Stream<Item = Result<Self::Chunk, Error>>;

    /// The type representing a chunk of the response body.
    type Chunk: AsRef<[u8]>;

    /// Perform an HTTP roundtrip.
    fn roundtrip(&self, request: http::Request<Vec<u8>>) -> Self::Output;
}

/// An error returned by a `vapix::Transport`.
///
/// This is a newtype around a `Box<dyn std::error::Error + …>` just to make transport-related
/// interfaces more clear.
pub struct Error(Box<dyn std::error::Error + Send + 'static>);

impl Error {
    /// Create a `transport::Error` from a generic `std::error::Error`.
    pub fn new<E: std::error::Error + Send + 'static>(error: E) -> Self {
        Error(Box::new(error))
    }

    /// Consume the `transport::Error`, returning the `Box<dyn std::error::Error>`.
    pub fn into_inner(self) -> Box<dyn std::error::Error + Send + 'static> {
        self.0
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
