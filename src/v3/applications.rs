//! The VAPIX application interface at `/axis-cgi/applications/*`.

use crate::*;

/// A device's application management API.
pub struct Applications<'a, T: Transport>(&'a Device<T>, String);

impl<'a, T: Transport> Applications<'a, T> {
    /// Instantiate with the value from `Properties.EmbeddedDevelopment.Version`
    pub(crate) fn new(device: &'a Device<T>, embedded_development_version: String) -> Self {
        Self(device, embedded_development_version)
    }

    pub async fn upload(&self, application_package_data: &[u8]) -> Result<(), Error<T::Error>> {
        let mut request_body = b"--fileboundary\r\n\
        Content-Disposition: form-data; name=\"packfil\"; filename=\"application.eap\"\r\n\
        Content-Type: application/octet-stream\r\n\
        \r\n"
            .to_vec();
        request_body.extend_from_slice(application_package_data);
        request_body.extend_from_slice(b"\r\n--fileboundary--\r\n\r\n");

        let req = http::Request::builder()
            .method(http::Method::POST)
            .uri(self.0.uri_for("/axis-cgi/applications/upload.cgi").unwrap())
            .header(
                http::header::CONTENT_TYPE,
                "multipart/form-data; boundary=fileboundary",
            )
            .header(
                http::header::CONTENT_LENGTH,
                format!("{}", request_body.len()),
            )
            .body(request_body)
            .unwrap();

        let (_resp, resp_body) = self.0.roundtrip(req, "text/plain").await?;

        let resp_body =
            std::str::from_utf8(resp_body.as_slice()).map_err(|_| Error::Other("invalid UTF-8"))?;

        if resp_body.starts_with("OK") {
            Ok(())
        } else {
            // TODO: smuggle out the error value
            Err(Error::Other("application upload failed"))
        }
    }
}
