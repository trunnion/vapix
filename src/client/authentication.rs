use std::sync::Mutex;

#[derive(Debug)]
pub(crate) struct Authentication {
    username: String,
    password: String,
    prompt: Mutex<Option<digest_auth::WwwAuthenticateHeader>>,
}

impl Clone for Authentication {
    fn clone(&self) -> Self {
        Authentication {
            username: self.username.clone(),
            password: self.password.clone(),
            prompt: Mutex::new(self.prompt.lock().unwrap().as_ref().cloned()),
        }
    }
}

impl Authentication {
    pub fn new(username: &str, password: &str) -> Self {
        Self {
            username: username.to_owned(),
            password: password.to_owned(),
            prompt: Mutex::new(None),
        }
    }

    pub fn should_retry(&self, parts: &http::response::Parts) -> bool {
        // get the header as bytes
        let header = match parts.headers.get(http::header::WWW_AUTHENTICATE) {
            Some(value) => value.as_bytes(),
            None => return false,
        };

        // get it as a string
        let header = match std::str::from_utf8(header) {
            Ok(str) => str,
            Err(_) => return false,
        };

        // parse it as a digest prompt
        let header = match digest_auth::WwwAuthenticateHeader::parse(header) {
            Ok(header) => header,
            Err(_) => return false,
        };

        // store this header
        self.prompt.lock().unwrap().replace(header);

        // we've updated our WWW-Authenticate header
        // the requester should retry iff the response has a 401 code
        parts.status == 401
    }

    pub fn authorization_for(
        &self,
        method: &http::Method,
        path_and_query: &http::uri::PathAndQuery,
        body: &[u8],
    ) -> Option<String> {
        let ctx = digest_auth::AuthContext::new_with_method(
            &self.username,
            &self.password,
            path_and_query.as_str(),
            if !body.is_empty() { Some(body) } else { None },
            match method.as_str() {
                "GET" => digest_auth::HttpMethod::GET,
                "HEAD" => digest_auth::HttpMethod::HEAD,
                "POST" => digest_auth::HttpMethod::POST,
                "DELETE" => digest_auth::HttpMethod::OTHER("DELETE"),
                "TRACE" => digest_auth::HttpMethod::OTHER("TRACE"),
                "CONNECT" => digest_auth::HttpMethod::OTHER("CONNECT"),
                "PATCH" => digest_auth::HttpMethod::OTHER("PATCH"),
                _ => digest_auth::HttpMethod::POST, // :(
            },
        );

        self.prompt
            .lock()
            .unwrap()
            .as_mut()
            .and_then(|prompt| prompt.respond(&ctx).ok())
            .map(|h| h.to_header_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUTH_HEADER_1: &'static str = r#"Digest realm="AXIS_ACCC8EF7DE6B", nonce="h20V+wGvBQA=b6c0ce8666d2d4b2688858d7a31386d9d337e072", algorithm=MD5, qop="auth""#;
    const AUTH_HEADER_2: &'static str = r#"Digest realm="AXIS_ACCC8EF7D108", nonce="kNEbGQKvBQA=cc9e12e5fbcf71667f58f1a4dbbbc952a1cd7d97", algorithm=MD5, qop="auth""#;

    #[test]
    fn usage() {
        // instantiate an Authentication object
        let auth = Authentication::new("user", "pass");

        // it cannot provide an authorization header out of the box
        assert_eq!(
            auth.authorization_for(
                &http::Method::GET,
                &http::uri::PathAndQuery::from_static("/foo?bar"),
                &[]
            ),
            None
        );

        // if we give it a 200 OK response, it doesn't tell us to retry or update its prompt
        let resp = http::Response::builder()
            .status(http::StatusCode::OK)
            .body(())
            .unwrap();
        assert_eq!(auth.should_retry(&resp.into_parts().0), false);
        assert!(auth.prompt.lock().unwrap().is_none());

        // if we give it a 401 Unauthorized response, it again does neither
        let resp = http::Response::builder()
            .status(http::StatusCode::UNAUTHORIZED)
            .body(())
            .unwrap();
        assert_eq!(auth.should_retry(&resp.into_parts().0), false);
        assert!(auth.prompt.lock().unwrap().is_none());

        // if we give it a 401 Unauthorized response with a non-String header, it again does neither
        let resp = http::Response::builder()
            .status(http::StatusCode::UNAUTHORIZED)
            .header(
                http::header::WWW_AUTHENTICATE,
                http::HeaderValue::from_bytes(&b"\xfe\xfd"[..]).unwrap(),
            )
            .body(())
            .unwrap();
        assert_eq!(auth.should_retry(&resp.into_parts().0), false);
        assert!(auth.prompt.lock().unwrap().is_none());

        // if we give it a 401 Unauthorized response with a bogus header, it again does neither
        let resp = http::Response::builder()
            .status(http::StatusCode::UNAUTHORIZED)
            .header(
                http::header::WWW_AUTHENTICATE,
                http::HeaderValue::from_static("bogus"),
            )
            .body(())
            .unwrap();
        assert_eq!(auth.should_retry(&resp.into_parts().0), false);
        assert!(auth.prompt.lock().unwrap().is_none());

        // finally, if we give it a 401 Unauthorized response with a valid header, it does both
        let resp = http::Response::builder()
            .status(http::StatusCode::UNAUTHORIZED)
            .header(
                http::header::WWW_AUTHENTICATE,
                http::HeaderValue::from_static(AUTH_HEADER_1),
            )
            .body(())
            .unwrap();
        assert_eq!(auth.should_retry(&resp.into_parts().0), true);
        assert!(auth.prompt.lock().unwrap().is_some());

        // take the prompt
        let prompt = auth.prompt.lock().unwrap().as_ref().cloned().unwrap();

        // it can now provide authorization
        assert_ne!(
            auth.authorization_for(
                &http::Method::GET,
                &http::uri::PathAndQuery::from_static("/foo?bar"),
                &[]
            ),
            None
        );

        // providing authorization modifies internal state
        assert_ne!(auth.prompt.lock().unwrap().as_ref().unwrap(), &prompt);

        // finally, any response with a WWW-Authenticate can update state, even if it wouldn't
        // prompt a retry
        let prompt = auth.prompt.lock().unwrap().as_ref().cloned().unwrap();
        let resp = http::Response::builder()
            .status(http::StatusCode::OK)
            .header(
                http::header::WWW_AUTHENTICATE,
                http::HeaderValue::from_static(AUTH_HEADER_2),
            )
            .body(())
            .unwrap();
        assert_eq!(auth.should_retry(&resp.into_parts().0), false);
        assert_ne!(auth.prompt.lock().unwrap().as_ref().unwrap(), &prompt);
    }

    #[test]
    fn clone() {
        // make an Authentication
        let auth = Authentication::new("user", "pass");
        let resp = http::Response::builder()
            .status(http::StatusCode::OK)
            .header(
                http::header::WWW_AUTHENTICATE,
                http::HeaderValue::from_static(AUTH_HEADER_1),
            )
            .body(())
            .unwrap();
        auth.should_retry(&resp.into_parts().0);
        assert!(auth.prompt.lock().unwrap().is_some());

        // clone it
        let auth2 = auth.clone();

        // they share a prompt
        assert_eq!(
            auth.prompt.lock().unwrap().as_ref(),
            auth2.prompt.lock().unwrap().as_ref()
        );

        // authenticate a request
        auth.authorization_for(
            &http::Method::OPTIONS,
            &http::uri::PathAndQuery::from_static("/doesnt_matter"),
            &[],
        );

        // they no longer share a prompt
        assert_ne!(
            auth.prompt.lock().unwrap().as_ref(),
            auth2.prompt.lock().unwrap().as_ref()
        );
    }
}
