use crate::{Client, Error, Transport};
use serde::{Deserialize, Serialize};

pub(crate) struct JsonService<'a, T: Transport> {
    device: &'a Client<T>,
    uri: http::Uri,
    api_version: String,
}

impl<'a, T: Transport> JsonService<'a, T> {
    pub fn new(device: &'a Client<T>, path_and_query: &str, api_version: String) -> Self {
        Self {
            device,
            uri: device.uri_for(path_and_query).unwrap(),
            api_version,
        }
    }

    async fn inner<RQ, RS>(&self, method: &str, request: Option<RQ>) -> Result<RS, Error>
    where
        RQ: serde::Serialize,
        RS: serde::de::DeserializeOwned,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Req<'a, RQ> {
            api_version: &'a str,
            method: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            params: Option<&'a RQ>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp<RS> {
            error: Option<crate::error::RawJsonApiError>,
            data: Option<RS>,
        }

        let req = {
            let json_request = Req {
                api_version: &self.api_version,
                method,
                params: request.as_ref(),
            };
            let json_request = serde_json::to_vec(&json_request).unwrap();

            http::Request::builder()
                .method(http::method::Method::POST)
                .uri(&self.uri)
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(json_request)
                .unwrap()
        };

        let (_resp, resp_body) = self.device.roundtrip(req, "application/json").await?;

        let resp_body: Resp<RS> = serde_json::from_slice(resp_body.as_slice())?;

        if let Some(e) = resp_body.error {
            return Err(e.into());
        }
        if let Some(d) = resp_body.data {
            Ok(d)
        } else {
            Err(Error::Other("response included neither `data` nor `error`"))
        }
    }

    pub async fn call_method<RQ, RS>(&self, method: &str, params: RQ) -> Result<RS, Error>
    where
        RQ: serde::Serialize,
        RS: serde::de::DeserializeOwned,
    {
        self.inner(method, Some(params)).await
    }

    pub async fn call_method_bare<RS>(&self, method: &str) -> Result<RS, Error>
    where
        RS: serde::de::DeserializeOwned,
    {
        let params: Option<()> = None;
        self.inner(method, params).await
    }
}
