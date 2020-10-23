use super::Transport;
use futures::prelude::*;
use futures::task::Context;
use http::Request;
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

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
    type Output = HyperResponseFuture;
    type Body = HyperBody;
    type Chunk = hyper::body::Bytes;

    fn roundtrip(&self, request: Request<Vec<u8>>) -> Self::Output {
        let (parts, body) = request.into_parts();
        let request = hyper::Request::from_parts(parts, body.into());
        HyperResponseFuture(self.0.request(request))
    }
}

#[pin_project]
pub struct HyperResponseFuture(#[pin] hyper::client::ResponseFuture);

impl Future for HyperResponseFuture {
    type Output = Result<http::Response<HyperBody>, crate::transport::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().0.poll(cx) {
            Poll::Ready(Ok(resp)) => {
                // Wrap the body before returning
                let (parts, body) = resp.into_parts();
                let resp = http::Response::from_parts(parts, HyperBody(body));
                Poll::Ready(Ok(resp))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(crate::transport::Error::new(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[pin_project]
pub struct HyperBody(#[pin] hyper::body::Body);

impl Stream for HyperBody {
    type Item = Result<hyper::body::Bytes, crate::transport::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().0.poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => Poll::Ready(Some(Ok(chunk))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(crate::transport::Error::new(e)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
