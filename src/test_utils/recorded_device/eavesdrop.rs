use super::*;
use crate::{Client, Error, HyperTransport};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Writer(Arc<EavesdropRecorder>);

impl Writer {
    pub fn write_as_fixture(&self) {
        self.0.write_as_fixture()
    }
}

pub async fn new_eavesdrop_device(
    uri: http::Uri,
) -> Result<
    (
        DeviceInfo,
        Client<EavesdropTransport<HyperTransport>>,
        Writer,
    ),
    Error,
> {
    let device = Client::new(HyperTransport::default(), uri.clone());
    let device_info = DeviceInfo::retrieve(&device).await;
    let device_info = device_info?;

    let recorder = EavesdropRecorder::new(device_info.clone());
    let recorder = Arc::new(recorder);

    let writer = Writer(recorder.clone());

    Ok((
        device_info,
        device.replace_transport(move |_| EavesdropTransport {
            recorder,
            transport: HyperTransport::default(),
        }),
        writer,
    ))
}

pub struct EavesdropRecorder {
    recording: Mutex<Option<Recording>>,
}

impl EavesdropRecorder {
    fn new(device_info: DeviceInfo) -> Self {
        Self {
            recording: Mutex::new(Some(Recording {
                device_info,
                transactions: HashMap::new(),
            })),
        }
    }

    fn get(&self, req: &RecordedHttpRequest) -> Option<RecordedHttpResponse> {
        self.recording
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .transactions
            .get(req)
            .cloned()
    }

    fn push(&self, transaction: RecordedTransaction) {
        self.recording
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .transactions
            .insert(transaction.request, transaction.response);
    }

    fn write_as_fixture(&self) {
        let lock = self.recording.lock().unwrap();
        let recording = lock.as_ref().unwrap();

        let f = std::fs::File::create(fixture_filename(&recording.device_info)).expect("open file");

        serde_json::to_writer_pretty(&f, recording).expect("write fixture");

        f.sync_all().expect("write fixture");
    }
}

impl Drop for EavesdropRecorder {
    fn drop(&mut self) {
        self.write_as_fixture()
    }
}

pub struct EavesdropTransport<T: Transport> {
    recorder: Arc<EavesdropRecorder>,
    transport: T,
}

impl<T: Transport> EavesdropTransport<T> {
    pub fn get(&self, request: &http::Request<Vec<u8>>) -> Option<RecordedHttpResponse> {
        let request = RecordedHttpRequest::new(&request);
        self.recorder.get(&request)
    }
}

impl<T: Transport> Transport for EavesdropTransport<T> {
    type Output = EavesdropTransportOutput<T>;
    type Body = EavesdropTransportBody<T>;
    type Chunk = T::Chunk;

    fn roundtrip(&self, request: http::Request<Vec<u8>>) -> Self::Output {
        let recorded_request = RecordedHttpRequest::new(&request);

        let future = self.transport.roundtrip(request);

        EavesdropTransportOutput {
            inner: future,
            req: Some((self.recorder.clone(), recorded_request)),
        }
    }
}

#[pin_project]
pub struct EavesdropTransportOutput<T: Transport> {
    #[pin]
    inner: T::Output,
    req: Option<(Arc<EavesdropRecorder>, RecordedHttpRequest)>,
}

impl<T: Transport> Future for EavesdropTransportOutput<T> {
    type Output = Result<http::Response<EavesdropTransportBody<T>>, crate::transport::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.inner.poll(cx) {
            Poll::Ready(Ok(resp)) => {
                let (resp, body) = resp.into_parts();

                let (recorder, req) = this.req.take().unwrap();
                let resp_builder = RecordedHttpResponseBuilder::new(&resp);
                let body = EavesdropTransportBody {
                    inner: body,
                    state: Some((recorder, req, resp_builder)),
                };

                Poll::Ready(Ok(http::Response::from_parts(resp, body)))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[pin_project]
pub struct EavesdropTransportBody<T: Transport> {
    #[pin]
    inner: T::Body,
    state: Option<(
        Arc<EavesdropRecorder>,
        RecordedHttpRequest,
        RecordedHttpResponseBuilder,
    )>,
}

impl<T: Transport> futures::Stream for EavesdropTransportBody<T> {
    type Item = Result<T::Chunk, crate::transport::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match this.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                this.state
                    .as_mut()
                    .unwrap()
                    .2
                    .add_body_chunk(chunk.as_ref());
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(e))) => {
                this.state.take();
                Poll::Ready(Some(Err(e.into())))
            }
            Poll::Ready(None) => {
                // done
                let (recorder, request, response) = this.state.take().unwrap();
                let response = response.build();
                let tx = RecordedTransaction { request, response };
                recorder.push(tx);

                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
