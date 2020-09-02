use std::sync::Mutex;

pub(crate) struct Authentication {
    username: String,
    password: String,
    prompt: Mutex<Option<digest_auth::WwwAuthenticateHeader>>,
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

        eprintln!("< WWW-Authenticate: {}", header);

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
            if body.len() > 0 { Some(body) } else { None },
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
            .map(|h| {
                eprintln!("> Authorization: {}", &h);
                h
            })
    }
}
