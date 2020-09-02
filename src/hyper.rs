use super::Transport;
use http::Request;

pub struct HyperTransport<C>(hyper::Client<C>);

impl<C> HyperTransport<C> {
    pub fn new(client: hyper::Client<C>) -> Self {
        Self(client)
    }

    pub fn into_inner(self) -> hyper::Client<C> {
        self.0
    }
}

impl<C> Into<HyperTransport<C>> for hyper::Client<C> {
    fn into(self) -> HyperTransport<C> {
        HyperTransport::new(self)
    }
}

impl<C> Transport for HyperTransport<C>
where
    C: hyper::client::connect::Connect + Clone + Send + Sync + 'static,
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
