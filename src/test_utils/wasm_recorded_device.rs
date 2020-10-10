use crate::*;
use futures::Future;
use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct TestDevice {
    pub device_info: DeviceInfo,
    pub device: Device<TestDeviceTransport>,
}

pub fn test_with_devices<FN, F>(_f: FN)
where
    FN: Fn(TestDevice) -> F,
    F: Future<Output = Result<(), Error<TestDeviceTransportError>>> + Send + 'static,
{
    unimplemented!()
}

pub struct TestDeviceTransport;

impl Default for TestDeviceTransport {
    fn default() -> Self {
        Self
    }
}

impl Transport for TestDeviceTransport {
    type Error = TestDeviceTransportError;
    type Output = TestDeviceTransportOutput;
    type Body = TestDeviceTransportBody;
    type Chunk = Vec<u8>;

    fn roundtrip(&self, _request: http::Request<Vec<u8>>) -> Self::Output {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct TestDeviceTransportError;
impl std::error::Error for TestDeviceTransportError {}

impl fmt::Display for TestDeviceTransportError {
    fn fmt(&self, _: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

pub struct TestDeviceTransportOutput;

impl Future for TestDeviceTransportOutput {
    type Output = Result<http::Response<TestDeviceTransportBody>, TestDeviceTransportError>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        unimplemented!()
    }
}

pub struct TestDeviceTransportBody;

impl futures::Stream for TestDeviceTransportBody {
    type Item = Result<Vec<u8>, TestDeviceTransportError>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unimplemented!()
    }
}
