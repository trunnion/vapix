use futures::Stream;
use std::future::Future;

/// An HTTP transport.
pub trait Transport {
    /// The error type returned by this transport.
    type Error: std::error::Error;

    /// The `Future` representing the part of the roundtrip leading to the response headers.
    type Output: Future<Output = Result<http::Response<Self::Body>, Self::Error>>;

    /// The `Future` representing the part of the roundtrip which streams the response body.
    type Body: Stream<Item = Result<Self::Chunk, Self::Error>>;

    /// The type representing a chunk of the response body.
    type Chunk: AsRef<[u8]>;

    /// Perform an HTTP roundtrip.
    fn roundtrip(&self, request: http::Request<Vec<u8>>) -> Self::Output;
}
