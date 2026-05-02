use std::time::Duration;

use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct HttpJsonResponse {
    pub status: u16,
    pub body: Value,
}

pub trait LlmHttpTransport: Send + Sync {
    fn post_json(
        &self,
        url: &str,
        headers: Vec<(String, String)>,
        body: Value,
    ) -> AppResult<HttpJsonResponse>;
}

pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new() -> AppResult<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| {
                AppError::LlmProvider(sanitize_provider_message(&error.to_string()))
            })?;
        Ok(Self { client })
    }
}

impl LlmHttpTransport for ReqwestTransport {
    fn post_json(
        &self,
        url: &str,
        headers: Vec<(String, String)>,
        body: Value,
    ) -> AppResult<HttpJsonResponse> {
        let mut request = self.client.post(url).json(&body);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = request.send().map_err(|error| {
            AppError::LlmProvider(sanitize_provider_message(&error.to_string()))
        })?;
        let status = response.status().as_u16();
        let body = response.json::<Value>().unwrap_or_else(|_| json!({}));
        Ok(HttpJsonResponse { status, body })
    }
}

pub fn sanitize_provider_message(message: &str) -> String {
    message
        .split_whitespace()
        .filter(|part| {
            !part.starts_with("sk-")
                && !part.starts_with("AIza")
                && !part.to_ascii_lowercase().starts_with("authorization:")
                && !part.to_ascii_lowercase().starts_with("x-goog-api-key:")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
pub mod test_support {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::*;

    pub struct RecordingTransport {
        response: HttpJsonResponse,
        requests: Mutex<Vec<Value>>,
    }

    impl RecordingTransport {
        pub fn new(body: Value) -> Self {
            Self {
                response: HttpJsonResponse { status: 200, body },
                requests: Mutex::new(Vec::new()),
            }
        }

        pub fn with_status(status: u16, body: Value) -> Self {
            Self {
                response: HttpJsonResponse { status, body },
                requests: Mutex::new(Vec::new()),
            }
        }

        pub fn last_body(&self) -> Option<Value> {
            self.requests.lock().expect("requests").last().cloned()
        }

        pub fn call_count(&self) -> usize {
            self.requests.lock().expect("requests").len()
        }
    }

    impl LlmHttpTransport for RecordingTransport {
        fn post_json(
            &self,
            _url: &str,
            _headers: Vec<(String, String)>,
            body: Value,
        ) -> AppResult<HttpJsonResponse> {
            self.requests.lock().expect("requests").push(body);
            Ok(self.response.clone())
        }
    }

    pub struct SequenceTransport {
        responses: Mutex<VecDeque<HttpJsonResponse>>,
        requests: Mutex<Vec<Value>>,
    }

    impl SequenceTransport {
        pub fn new(responses: Vec<HttpJsonResponse>) -> Self {
            Self {
                responses: Mutex::new(VecDeque::from(responses)),
                requests: Mutex::new(Vec::new()),
            }
        }

        pub fn single_success(body: Value) -> Self {
            Self::new(vec![HttpJsonResponse { status: 200, body }])
        }

        pub fn unused() -> Self {
            Self::new(Vec::new())
        }

        pub fn call_count(&self) -> usize {
            self.requests.lock().expect("requests").len()
        }
    }

    impl LlmHttpTransport for SequenceTransport {
        fn post_json(
            &self,
            _url: &str,
            _headers: Vec<(String, String)>,
            body: Value,
        ) -> AppResult<HttpJsonResponse> {
            self.requests.lock().expect("requests").push(body);
            self.responses
                .lock()
                .expect("responses")
                .pop_front()
                .ok_or_else(|| AppError::LlmProvider("no fake response queued".into()))
        }
    }
}
