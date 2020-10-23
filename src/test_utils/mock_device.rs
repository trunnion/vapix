use futures::Stream;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};

pub fn mock_device<F, B, E>(f: F) -> crate::Device<impl crate::Transport>
where
    F: FnMut(http::Request<Vec<u8>>) -> Result<http::Response<B>, E>,
    B: IntoIterator<Item = Vec<u8>>,
    B::IntoIter: Unpin,
    E: std::error::Error + Send + 'static,
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
    E: std::error::Error + Send + 'static,
{
    type Output = TransportAdapterOutput<B::IntoIter>;
    type Body = TransportAdapterBody<B::IntoIter>;
    type Chunk = Vec<u8>;

    fn roundtrip(&self, request: http::Request<Vec<u8>>) -> Self::Output {
        let result = self.0.lock().unwrap()(request);
        TransportAdapterOutput(Some(
            result
                .map(|resp| {
                    let (resp, body) = resp.into_parts();
                    let body_iter = body.into_iter();
                    http::Response::from_parts(resp, TransportAdapterBody(Some(Ok(body_iter))))
                })
                .map_err(|e| crate::transport::Error::new(e)),
        ))
    }
}

struct TransportAdapterOutput<B>(
    Option<Result<http::Response<TransportAdapterBody<B>>, crate::transport::Error>>,
);

impl<B: Iterator<Item = Vec<u8>> + Unpin> Future for TransportAdapterOutput<B> {
    type Output = Result<http::Response<TransportAdapterBody<B>>, crate::transport::Error>;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.0.take().expect("poll() once").map_err(|e| e.into()))
    }
}

struct TransportAdapterBody<B>(Option<Result<B, crate::transport::Error>>);

impl<B: Iterator<Item = Vec<u8>> + Unpin> Stream for TransportAdapterBody<B> {
    type Item = Result<Vec<u8>, crate::transport::Error>;

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
