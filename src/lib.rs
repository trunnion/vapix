use futures::prelude::*;
use http::{Request, Response};
use std::convert::TryFrom;
use std::future::Future;

pub trait Transport {
    type Error;
    type Output: Future<Output = Result<http::Response<Self::Body>, Self::Error>>;
    type Body: Stream<Item = Result<Self::Chunk, Self::Error>>;
    type Chunk: AsRef<[u8]>;
    fn roundtrip(&self, request: http::Request<Vec<u8>>) -> Self::Output;
}

#[derive(Debug)]
pub enum Error<T: Transport> {
    TransportError(T::Error),
}

mod authentication;

pub(crate) use authentication::Authentication;

pub struct Device<T: Transport> {
    scheme: http::uri::Scheme,
    authority: http::uri::Authority,
    authentication: Authentication,
    transport: T,
}

impl<T: Transport> Device<T> {
    pub fn new<U>(transport: T, uri: U) -> Self
    where
        U: Into<http::Uri>,
    {
        let transport = transport.into();
        let parts = uri.into().into_parts();
        let scheme = parts.scheme.unwrap(); // fixme
        let authority = parts.authority.unwrap(); // fixme

        let userinfo = authority.as_str().rsplitn(2, '@').skip(1).next();
        let (username, password) = {
            let mut parts = userinfo.unwrap_or("").splitn(2, ':');
            (parts.next(), parts.next())
        };
        let authentication =
            Authentication::new(username.unwrap_or("root"), password.unwrap_or("pass"));

        Self {
            scheme,
            authority,
            authentication,
            transport,
        }
    }

    fn uri_for<PQ>(&self, path_and_query: PQ) -> http::Result<http::Uri>
    where
        http::uri::PathAndQuery: TryFrom<PQ>,
        <http::uri::PathAndQuery as TryFrom<PQ>>::Error: Into<http::Error>,
    {
        http::Uri::builder()
            .authority(self.authority.clone())
            .scheme(self.scheme.clone())
            .path_and_query(path_and_query)
            .build()
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

    async fn roundtrip(
        &self,
        req: http::Request<Vec<u8>>,
    ) -> Result<http::Response<T::Body>, T::Error> {
        // build a retry request before sending the first request
        // split it into parts
        let (parts, body) = req.into_parts();

        // make retry parts
        let retry_parts = {
            let (mut retry_parts, _) = http::Request::new(()).into_parts();
            retry_parts.method = parts.method.clone();
            retry_parts.uri = parts.uri.clone();
            retry_parts.version = parts.version.clone();
            retry_parts.headers = parts.headers.clone();
            retry_parts
        };

        // assemble a second request in case we need to retry
        let mut second_request = http::Request::from_parts(retry_parts, body.clone());

        // reassemble the original request, adding authorization
        let mut request = http::Request::from_parts(parts, body);
        self.add_authorization_header(&mut request);

        // make the request
        let response: http::Response<T::Body> = self.transport.roundtrip(request).await?;

        // see if we should retry
        let (response_parts, response_body) = response.into_parts();
        if self.authentication.should_retry(&response_parts) {
            // update the second request
            self.add_authorization_header(&mut second_request);

            // send the second request
            let response = self.transport.roundtrip(second_request).await?;

            // see if authentication wants to retry, but… don't
            let (response_parts, response_body) = response.into_parts();
            self.authentication.should_retry(&response_parts);

            // return the second response
            Ok(http::Response::from_parts(response_parts, response_body))
        } else {
            // return the original response
            Ok(http::Response::from_parts(response_parts, response_body))
        }
    }

    // https://www.axis.com/vapix-library/subjects/t10037719/section/t10132180/display
    async fn get_property(&self, property: &str) -> Result<String, T::Error> {
        let body = r#"
{
  "apiVersion": "1.0",
  "context": "Client defined request ID",
  "method": "getProperties",
  "params": {
    "propertyList": [
      "Brand",
      "ProdNbr",
      "Version"
    ]
  }
}
        "#
        .as_bytes()
        .to_vec();

        let mut req = http::Request::builder()
            .method(http::method::Method::POST)
            .uri(self.uri_for("/axis-cgi/basicdeviceinfo.cgi").unwrap());

        let (resp, resp_body) = self.roundtrip(req.body(body).unwrap()).await?.into_parts();
        println!("status: {:?}", resp.status);
        println!("headers: {:?}", resp.headers);
        let resp_body = resp_body
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
            .await?;

        println!("body: {:?}", std::str::from_utf8(resp_body.as_slice()));

        Ok("".into())
    }
}

#[cfg(feature = "hyper")]
mod hyper;

#[cfg(feature = "hyper")]
pub use self::hyper::HyperTransport;

#[cfg(test)]
mod tests {
    use crate::HyperTransport;
    use hyper;
    use tokio;

    #[tokio::test]
    async fn smoke() {
        let transport: HyperTransport<_> = hyper::Client::new().into();
        let uri: hyper::Uri = "http://root:root@172.16.4.30/".parse().unwrap();
        let d = crate::Device::new(transport, uri);
        let resp = d.get_property("moo").await.unwrap();
        assert_eq!(resp, "");
    }
}
