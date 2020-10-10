use crate::{v3, v4, Error, Transport};
use futures::StreamExt;
use std::convert::TryInto;

mod authentication;

// todo:
//   * /axis-cgi/admin/systemlog.cgi
//   * /axis-cgi/admin/accesslog.cgi
//   * /axis-cgi/serverreport.cgi

#[derive(Debug, Clone)]
pub struct Device<T: Transport> {
    scheme: http::uri::Scheme,
    authority: http::uri::Authority,
    authentication: authentication::Authentication,
    transport: T,
}

impl<T: Transport> Device<T> {
    pub fn new<U>(transport: T, uri: U) -> Self
    where
        U: Into<http::Uri>,
    {
        let parts = uri.into().into_parts();
        let scheme = parts.scheme.unwrap(); // fixme
        let authority = parts.authority.unwrap(); // fixme

        let userinfo = authority.as_str().rsplitn(2, '@').nth(1);
        let (username, password) = {
            let mut parts = userinfo.unwrap_or("").splitn(2, ':');
            (parts.next(), parts.next())
        };
        let authentication = authentication::Authentication::new(
            username.unwrap_or("root"),
            password.unwrap_or("pass"),
        );

        Self {
            scheme,
            authority,
            authentication,
            transport,
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_transport<F: FnOnce(T) -> T2, T2: Transport>(
        self,
        replacer: F,
    ) -> Device<T2> {
        Device {
            scheme: self.scheme,
            authority: self.authority,
            authentication: self.authentication,
            transport: replacer(self.transport),
        }
    }

    pub(crate) fn uri_for(&self, path_and_query: &str) -> http::Result<http::Uri> {
        http::Uri::builder()
            .authority(self.authority.clone())
            .scheme(self.scheme.clone())
            .path_and_query(path_and_query)
            .build()
    }

    pub(crate) fn uri_for_args<P>(&self, path_and_query: &str, params: P) -> http::Result<http::Uri>
    where
        P: serde::Serialize,
    {
        let path_and_query: http::uri::PathAndQuery = path_and_query.try_into()?;
        let path = path_and_query.path();
        let query = path_and_query.query();

        let params_as_query = serde_urlencoded::to_string(params).unwrap();

        let combined_path_and_query = match query {
            Some(hardcoded) => path.to_string() + "?" + &params_as_query + "&" + hardcoded,
            None => path.to_string() + "?" + &params_as_query,
        };

        self.uri_for(&combined_path_and_query)
    }

    fn add_authorization_header(&self, request: &mut http::Request<Vec<u8>>) {
        if let Some(value) = self.authentication.authorization_for(
            request.method(),
            request.uri().path_and_query().unwrap(),
            request.body().as_slice(),
        ) {
            request.headers_mut().insert(
                http::header::AUTHORIZATION,
                http::HeaderValue::from_str(&value).unwrap(),
            );
        }
    }

    pub(crate) async fn roundtrip(
        &self,
        req: http::Request<Vec<u8>>,
        expected_content_type: &'static str,
    ) -> Result<(http::response::Parts, Vec<u8>), Error<T::Error>> {
        // Build a retry request before sending the first request
        // Split it into parts
        let (mut parts, body) = req.into_parts();
        parts.headers.insert(
            http::header::ACCEPT,
            http::HeaderValue::from_str(expected_content_type).unwrap(),
        );

        // Make retry parts
        let retry_parts = {
            let (mut retry_parts, _) = http::Request::new(()).into_parts();
            retry_parts.method = parts.method.clone();
            retry_parts.uri = parts.uri.clone();
            retry_parts.version = parts.version;
            retry_parts.headers = parts.headers.clone();
            retry_parts
        };

        // Assemble a second request in case we need to retry
        let mut second_request = http::Request::from_parts(retry_parts, body.clone());

        // Reassemble the original request, adding authorization
        let mut request = http::Request::from_parts(parts, body);
        self.add_authorization_header(&mut request);

        // Make the request
        let response: http::Response<T::Body> = self
            .transport
            .roundtrip(request)
            .await
            .map_err(Error::TransportError)?;

        // See if we should retry
        let (response_parts, response_body) = response.into_parts();

        // Retry as needed
        let (response_parts, response_body) = if self.authentication.should_retry(&response_parts) {
            // Update the second request
            self.add_authorization_header(&mut second_request);

            // Send the second request
            let response: http::Response<_> = self
                .transport
                .roundtrip(second_request)
                .await
                .map_err(Error::TransportError)?;

            // See if authentication wants to retry, but… don't
            let (response_parts, response_body) = response.into_parts();
            self.authentication.should_retry(&response_parts);

            // Use the second response
            (response_parts, response_body)
        } else {
            // Use the original response
            (response_parts, response_body)
        };

        // Read the whole body, even if we'll discard it below
        // This helps with connection reuse (HTTP/1.1 can't abort mid-response) and is necessary for
        // eavesdropping in test
        let response_body = response_body
            .fold(Ok(<Vec<u8>>::new()), |state, chunk| async {
                let mut buf = match state {
                    Ok(b) => b,
                    Err(e) => return Err(e),
                };

                match chunk {
                    Ok(c) => {
                        buf.extend(c.as_ref());
                        Ok(buf)
                    }
                    Err(e) => Err(e),
                }
            })
            .await
            .map_err(crate::Error::TransportError)?;

        // Are we 200 OK?
        if response_parts.status != http::status::StatusCode::OK {
            return Err(Error::BadStatusCodeError(response_parts.status));
        }

        // Is this the right content type?
        let content_type_value = match response_parts.headers.get(http::header::CONTENT_TYPE) {
            Some(v) => v,
            None => return Err(Error::BadContentTypeError(None)),
        };
        let content_type = content_type_value
            .to_str()
            .map_err(|_| Error::BadContentTypeError(Some(content_type_value.clone())))?;

        let left = content_type.splitn(2, ';').next().unwrap();
        if left != expected_content_type {
            return Err(Error::BadContentTypeError(Some(content_type_value.clone())));
        }

        // Success
        Ok((response_parts, response_body))
    }

    /// Access `Parameters` directly, without testing for support. Subsequent calls may fail if the
    /// device does not actually support the `Parameters` interface.
    ///
    /// Alternately, `.services()` probes for support of this interface and provides an
    /// `Option<Parameters>` if the device indicates support. This is the better path for newer
    /// devices, but ancient devices may support `.parameters()` without supporting `.services()`,
    /// so both paths are necessary.
    pub fn parameters(&self) -> v3::Parameters<T> {
        v3::Parameters::new(self, "1.0".to_string())
    }

    /// Discover which VAPIX services the device supports.
    ///
    /// Requires firmware >= 8.50.
    pub async fn services(&self) -> Result<v4::Services<'_, T>, Error<T::Error>> {
        v4::Services::new(self).await
    }

    /// Return the applications interface, if supported by the device.
    pub async fn applications(&self) -> Result<Option<v3::Applications<'_, T>>, Error<T::Error>> {
        let params = self
            .parameters()
            .list(Some(&["Properties.EmbeddedDevelopment.Version"]))
            .await?;

        Ok(params
            .into_iter()
            .find_map(|(k, v)| {
                if k == "Properties.EmbeddedDevelopment.Version" {
                    Some(v)
                } else {
                    None
                }
            })
            .map(|ver| v3::Applications::new(self, ver)))
    }

    /// Return the system log interface for this device.
    pub fn system_log(&self) -> v3::SystemLog<'_, T> {
        v3::SystemLog::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn authentication() {
        let username = "dark_helmet_man";
        let password = "1-2-3-4-5";
        let realm = "shadow";
        let nonce = "xkcd-221";

        let mut device: Device<_> = crate::mock_device(|req| {
            let ctx = digest_auth::AuthContext {
                username: username.into(),
                password: password.into(),
                uri: req
                    .uri()
                    .path_and_query()
                    .map(|pq| pq.as_str())
                    .unwrap_or("/")
                    .into(),
                body: None,
                method: digest_auth::HttpMethod::GET,
                cnonce: None,
            };

            if let Some(authorization) = req.headers().get(http::header::AUTHORIZATION) {
                // parse what the client gave us
                let mut authorization =
                    digest_auth::AuthorizationHeader::parse(authorization.to_str().unwrap())
                        .unwrap();

                // extract the original response digest
                let original_response = authorization.response.clone();

                // hardcode our own bits
                authorization.realm = realm.into();
                authorization.nonce = nonce.into();

                // calculate a new response
                authorization.digest(&ctx);

                // ensure it matches
                assert_eq!(authorization.response, original_response);

                // pass back 200 OK
                http::Response::builder()
                    .status(http::StatusCode::OK)
                    .header(
                        http::header::CONTENT_TYPE,
                        http::HeaderValue::from_static("text/plain"),
                    )
                    .body(vec![b"great success".to_vec()])
            } else {
                let header = digest_auth::WwwAuthenticateHeader {
                    domain: None,
                    realm: realm.into(),
                    nonce: nonce.into(),
                    opaque: None,
                    stale: false,
                    algorithm: Default::default(),
                    qop: Some(vec![digest_auth::Qop::AUTH]),
                    userhash: false,
                    charset: digest_auth::Charset::UTF8,
                    nc: 0,
                };
                http::Response::builder()
                    .status(http::StatusCode::UNAUTHORIZED)
                    .header(
                        http::header::WWW_AUTHENTICATE,
                        http::HeaderValue::from_str(&header.to_string()).unwrap(),
                    )
                    .body(vec![vec![]])
            }
        });

        // specify authentication, since mock_device() doesn't
        device.authentication = authentication::Authentication::new(username, password);

        // make a roundtrip
        let response = device
            .roundtrip(
                http::Request::builder()
                    .uri(device.uri_for("/whatever").unwrap())
                    .body(vec![])
                    .unwrap(),
                "text/plain",
            )
            .await
            .unwrap();

        // we should make two requests, ultimately submitting the right digest and getting 200 OK
        assert_eq!(response.0.status, http::StatusCode::OK);
    }
}
