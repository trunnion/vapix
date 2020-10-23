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

pub struct Error(Box<dyn std::error::Error + Send>);

impl Error {
    pub fn new<E: std::error::Error + Send + 'static>(error: E) -> Self {
        Error(Box::new(error))
    }

    pub fn is<T: std::error::Error + 'static>(&self) -> bool {
        self.0.is::<T>()
    }

    pub fn downcast<T: std::error::Error + 'static>(self) -> Result<Box<T>, Self> {
        self.0.downcast().map_err(Self)
    }

    pub fn downcast_ref<T: std::error::Error + 'static>(&self) -> Option<&T> {
        self.0.downcast_ref()
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
