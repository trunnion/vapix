use futures::Stream;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};

pub fn mock_device<F, B, E>(f: F) -> crate::Device<impl crate::Transport<Error = E>>
where
    F: FnMut(http::Request<Vec<u8>>) -> Result<http::Response<B>, E>,
    B: IntoIterator<Item = Vec<u8>>,
    B::IntoIter: Unpin,
    E: std::error::Error + Unpin,
{
    let t = TransportAdapter(Mutex::new(f));
    let uri = http::Uri::from_static("http://1.2.3.4");
    crate::Device::new(t, uri)
}

struct TransportAdapter<F>(Mutex<F>);

impl<F, E, B> crate::Transport for TransportAdapter<F>
where
    F: FnMut(http::Request<Vec<u8>>) -> Result<http::Response<B>, E>,
    B: IntoIterator<Item = Vec<u8>>,
    B::IntoIter: Unpin,
    E: std::error::Error + Unpin,
{
    type Error = E;
    type Output = TransportAdapterOutput<B::IntoIter, E>;
    type Body = TransportAdapterBody<B::IntoIter, E>;
    type Chunk = Vec<u8>;

    fn roundtrip(&self, request: http::Request<Vec<u8>>) -> Self::Output {
        let result = self.0.lock().unwrap()(request);
        TransportAdapterOutput(Some(result.map(|resp| {
            let (resp, body) = resp.into_parts();
            let body_iter = body.into_iter();
            http::Response::from_parts(resp, TransportAdapterBody(Some(Ok(body_iter))))
        })))
    }
}

struct TransportAdapterOutput<B, E>(Option<Result<http::Response<TransportAdapterBody<B, E>>, E>>);

impl<B: Iterator<Item = Vec<u8>> + Unpin, E: Unpin> Future for TransportAdapterOutput<B, E> {
    type Output = Result<http::Response<TransportAdapterBody<B, E>>, E>;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.0.take().expect("poll() once"))
    }
}

struct TransportAdapterBody<B, E>(Option<Result<B, E>>);

impl<B: Iterator<Item = Vec<u8>> + Unpin, E: Unpin> Stream for TransportAdapterBody<B, E> {
    type Item = Result<Vec<u8>, E>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.0.take() {
            Some(Ok(mut i)) => match i.next() {
                Some(chunk) => {
                    self.0 = Some(Ok(i));
                    Poll::Ready(Some(Ok(chunk)))
                }
                None => Poll::Ready(None),
            },
            Some(Err(e)) => Poll::Ready(Some(Err(e))),
            None => Poll::Ready(None),
        }
    }
}
