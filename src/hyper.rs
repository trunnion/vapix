use super::Transport;
use http::Request;

pub struct HyperTransport<C = ::hyper::client::HttpConnector, B = ::hyper::body::Body>(
    hyper::Client<C, B>,
);

impl<C, B> HyperTransport<C, B> {
    pub fn new(client: hyper::Client<C, B>) -> Self {
        Self(client)
    }

    pub fn into_inner(self) -> hyper::Client<C, B> {
        self.0
    }
}

impl<C, B> Into<HyperTransport<C, B>> for hyper::Client<C, B> {
    fn into(self) -> HyperTransport<C, B> {
        HyperTransport(self)
    }
}

impl Default for HyperTransport<::hyper::client::HttpConnector, ::hyper::body::Body> {
    fn default() -> Self {
        Self(hyper::Client::new())
    }
}

impl<C, B> Transport for HyperTransport<C, B>
where
    C: hyper::client::connect::Connect + Clone + Send + Sync + 'static,
    B: hyper::body::HttpBody + Send + 'static + From<Vec<u8>>,
    B::Data: Send,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Error = hyper::error::Error;
    type Output = hyper::client::ResponseFuture;
    type Body = hyper::body::Body;
    type Chunk = hyper::body::Bytes;

    fn roundtrip(&self, request: Request<Vec<u8>>) -> Self::Output {
        let (parts, body) = request.into_parts();
        let request = hyper::Request::from_parts(parts, body.into());
        self.0.request(request)
    }
}
