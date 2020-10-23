use crate::*;
use futures::future::Shared;
use futures::task::Context;
use futures::FutureExt;
use lazy_static::lazy_static;
use pin_project::pin_project;
use std::convert::TryFrom;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::Poll;

mod recording;
use recording::*;

mod eavesdrop;
use eavesdrop::*;

/// A test device, either pre-recorded or live.
pub struct TestDevice {
    pub device_info: DeviceInfo,
    pub device: Device<TestDeviceTransport>,
    device_guard: Option<tokio::sync::OwnedMutexGuard<Option<Device<TestDeviceTransport>>>>,
    writer: Option<Writer>,
}

pub fn test_with_devices<FN, F>(f: FN)
where
    FN: Fn(TestDevice) -> F,
    F: Future<Output = Result<(), Error>> + Send + 'static,
{
    tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // Read all the devices from disk
            let mut devices = fixture_test_devices();

            // Add the live device, if any
            if let Some(factory) = LIVE_TEST_DEVICE_INSTANCE_FACTORY.clone().await {
                devices.push(factory.get().await);
            }

            let mut failures = 0usize;
            let mut successes = 0usize;
            for (device_info, join_handle) in devices.into_iter().map(|test_device| {
                (
                    test_device.device_info.clone(),
                    tokio::spawn(f(test_device)),
                )
            }) {
                match join_handle.await {
                    Ok(Ok(())) => {
                        successes += 1;
                    }
                    Ok(Err(Error::UnsupportedFeature)) => {
                        // acceptable failure mode
                    }
                    Ok(Err(Error::TransportError(e)))
                        if e.downcast_ref()
                            == Some(&TestDeviceTransportError::RequestNotRecorded) =>
                    {
                        eprintln!(
                            "serial {:?} version {} is missing a recording",
                            device_info.serial_number, device_info.firmware_version,
                        );
                    }
                    Ok(Err(e)) => {
                        eprintln!(
                            "error for serial {:?} version {}: {}",
                            device_info.serial_number, device_info.firmware_version, e
                        );
                        failures += 1;
                    }
                    Err(e) => {
                        eprintln!(
                            "failure for serial {:?} version {}: {}",
                            device_info.serial_number, device_info.firmware_version, e
                        );
                        failures += 1;
                    }
                }
            }

            assert_ne!(successes, 0, "test must succeed for at least one device");
            assert_eq!(
                failures, 0,
                "no test device may panic or return an unexpected error"
            );
        });
}

lazy_static! {
    static ref FIXTURE_RECORDINGS: Vec<Arc<Recording>> = load_fixture_recordings();
    static ref LIVE_TEST_DEVICE_INSTANCE_FACTORY: Shared<Pin<Box<dyn Future<Output = Option<LiveTestDeviceInstanceFactory>> + Send>>> =
        LiveTestDeviceInstanceFactory::new().boxed().shared();
}

fn fixture_test_devices() -> Vec<TestDevice> {
    let uri = http::Uri::from_static("http://test.device.local");

    FIXTURE_RECORDINGS
        .iter()
        .map(|recording| {
            let recording = recording.clone();

            let device_info = recording.device_info.clone();
            let transport = TestDeviceTransport(TestDeviceTransportInner::Recording(recording));

            TestDevice {
                device_info,
                device: crate::Device::new(transport, uri.clone()),
                device_guard: None,
                writer: None,
            }
        })
        .collect()
}

fn load_fixture_recordings() -> Vec<Arc<Recording>> {
    std::fs::read_dir(fixture_dir())
        .expect("read fixture dir")
        .map(|entry| entry.expect("read fixture dir entry"))
        .map(|entry| entry.path())
        .filter(|path| path.extension().map(|s| s == "json") == Some(true))
        .map(|path| {
            std::thread::spawn(move || {
                // This is wasteful, but it appears fs::read + ::from_slice is about 10 times faster
                // than File::open + ::from_reader. Buffering? &shrug;
                let bytes = std::fs::read(&path).expect("read fixture file");
                serde_json::from_slice(&bytes)
                    .map(Arc::new)
                    .map_err(|e| format!("parse fixture {}: {}", path.to_string_lossy(), e))
            })
        })
        .map(|thread| thread.join().unwrap().unwrap())
        .collect()
}

#[derive(Clone)]
struct LiveTestDeviceInstanceFactory {
    device_info: DeviceInfo,
    device: Arc<tokio::sync::Mutex<Option<Device<TestDeviceTransport>>>>,
    writer: Writer,
}

impl LiveTestDeviceInstanceFactory {
    /// If `RECORD_DEVICE_URI` is absent, returns `None`. Otherwise, returns a
    /// `LiveTestDeviceInstanceFactory` capable of providing `TestDevice`s which communicate with a
    /// live device.
    ///
    /// Errors communicating with `RECORD_DEVICE_URI` cause a panic.
    pub async fn new() -> Option<Self> {
        let uri = match std::env::var("RECORD_DEVICE_URI") {
            Err(std::env::VarError::NotPresent) => return None,
            other => other,
        }
        .expect("RECORD_DEVICE_URI must be valid UTF-8");

        let uri = http::Uri::try_from(uri).expect("RECORD_DEVICE_URI must be a valid URI");

        let (device_info, device, writer) = new_eavesdrop_device(uri)
            .await
            .expect("RECORD_DEVICE_URI must be an Axis device");

        let device = device
            .replace_transport(|e| TestDeviceTransport(TestDeviceTransportInner::Eavesdrop(e)));

        Some(Self {
            device_info,
            device: Arc::new(tokio::sync::Mutex::new(Some(device))),
            writer,
        })
    }

    /// Provide a `TestDevice` accessing the live device.
    ///
    /// Blocks until any previous `TestDevice` returned from `get()` is dropped.
    pub async fn get(&self) -> TestDevice {
        let mut device_guard = self.device.clone().lock_owned().await;
        TestDevice {
            device_info: self.device_info.clone(),
            device: device_guard.take().expect("must contain device"),
            device_guard: Some(device_guard),
            writer: Some(self.writer.clone()),
        }
    }
}

impl Drop for TestDevice {
    fn drop(&mut self) {
        if let Some(guard) = self.device_guard.as_mut() {
            let mut device = Device::new(
                TestDeviceTransport::default(),
                http::Uri::from_static("http://1.2.3.4"),
            );
            std::mem::swap(&mut device, &mut self.device);
            guard.replace(device);
        }

        if let Some(writer) = self.writer.as_mut() {
            writer.write_as_fixture();
        }
    }
}

/// A `Transport` which backed by either a `Recording` of a historical device or an
/// `EavesdropTransport` communicating with a live device.  
pub struct TestDeviceTransport(TestDeviceTransportInner);

impl Default for TestDeviceTransport {
    fn default() -> Self {
        Self(TestDeviceTransportInner::Recording(Arc::new(
            Recording::default(),
        )))
    }
}

enum TestDeviceTransportInner {
    Recording(Arc<Recording>),
    Eavesdrop(EavesdropTransport<HyperTransport>),
}

impl Transport for TestDeviceTransport {
    type Output = TestDeviceTransportOutput;
    type Body = TestDeviceTransportBody;
    type Chunk = Vec<u8>;

    fn roundtrip(&self, request: http::Request<Vec<u8>>) -> Self::Output {
        match &self.0 {
            TestDeviceTransportInner::Recording(r) => {
                TestDeviceTransportOutput(TestDeviceTransportOutputInner::Recording(
                    r.find(&request).map(|resp| resp.clone()),
                ))
            }
            TestDeviceTransportInner::Eavesdrop(t) => {
                if let Some(recorded) = t.get(&request) {
                    TestDeviceTransportOutput(TestDeviceTransportOutputInner::Recording(Some(
                        recorded,
                    )))
                } else {
                    TestDeviceTransportOutput(TestDeviceTransportOutputInner::Eavesdrop(Box::new(
                        t.roundtrip(request),
                    )))
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum TestDeviceTransportError {
    RequestNotRecorded,
    LiveError(crate::transport::Error),
}

impl PartialEq for TestDeviceTransportError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (&Self::RequestNotRecorded, &Self::RequestNotRecorded) => true,
            _ => false,
        }
    }
}

impl std::error::Error for TestDeviceTransportError {}

impl fmt::Display for TestDeviceTransportError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TestDeviceTransportError::RequestNotRecorded => write!(f, "request not recorded"),
            TestDeviceTransportError::LiveError(e) => write!(f, "live error: {}", e),
        }
    }
}

#[pin_project]
pub struct TestDeviceTransportOutput(#[pin] TestDeviceTransportOutputInner);

#[pin_project]
enum TestDeviceTransportOutputInner {
    Recording(#[pin] Option<RecordedHttpResponse>),
    Eavesdrop(#[pin] Box<EavesdropTransportOutput<HyperTransport>>),
}

impl Future for TestDeviceTransportOutput {
    type Output = Result<http::Response<TestDeviceTransportBody>, crate::transport::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().0.project() {
            __TestDeviceTransportOutputInnerProjection::Recording(mut o) => match o.take() {
                Some(resp) => {
                    let resp = resp
                        .http_response_builder()
                        .body(TestDeviceTransportBody(
                            TestDeviceTransportBodyInner::Recording(Some(
                                resp.body.as_slice().to_vec(),
                            )),
                        ))
                        .unwrap();
                    Poll::Ready(Ok(resp))
                }
                None => Poll::Ready(Err(crate::transport::Error::new(
                    TestDeviceTransportError::RequestNotRecorded,
                ))),
            },
            __TestDeviceTransportOutputInnerProjection::Eavesdrop(o) => {
                o.poll(cx).map(|r| match r {
                    Ok(h) => {
                        let (parts, body) = h.into_parts();
                        let body = TestDeviceTransportBody(
                            TestDeviceTransportBodyInner::Eavesdrop(Box::new(body)),
                        );
                        Ok(http::Response::from_parts(parts, body))
                    }
                    Err(e) => Err(crate::transport::Error::new(
                        TestDeviceTransportError::LiveError(e),
                    )),
                })
            }
        }
    }
}

#[pin_project]
pub struct TestDeviceTransportBody(#[pin] TestDeviceTransportBodyInner);

#[pin_project]
enum TestDeviceTransportBodyInner {
    Recording(#[pin] Option<Vec<u8>>),
    Eavesdrop(#[pin] Box<EavesdropTransportBody<HyperTransport>>),
}

impl futures::Stream for TestDeviceTransportBody {
    type Item = Result<Vec<u8>, crate::transport::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().0.project() {
            __TestDeviceTransportBodyInnerProjection::Recording(mut b) => match b.take() {
                Some(bytes) => Poll::Ready(Some(Ok(bytes))),
                None => Poll::Ready(None),
            },
            __TestDeviceTransportBodyInnerProjection::Eavesdrop(b) => match b.poll_next(cx) {
                Poll::Ready(Some(Ok(c))) => Poll::Ready(Some(Ok(c.as_ref().to_vec()))),
                Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(crate::transport::Error::new(
                    TestDeviceTransportError::LiveError(e),
                )))),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}
