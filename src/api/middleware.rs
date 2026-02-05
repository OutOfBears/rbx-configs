use http::HeaderValue;
use log::{debug, info, warn};
use reqwest::{
    Request, Response, StatusCode,
    cookie::{self, CookieStore},
};
use reqwest_middleware::{Middleware, Next, Result};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

use crate::api::{API_CLIENT, model::ErrorResponse};

#[derive(Debug, Default)]
struct RateState {
    remaining: Option<u64>,
    reset_after_secs: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct RobloxRateLimitMiddleware {
    state: Arc<Mutex<RateState>>,
    max_429_retries: usize,
    cushion_ms: u64,
}

#[derive(Clone, Debug)]
pub struct RobloxAuthMiddleware {
    seen_etag: Arc<Mutex<bool>>,
    csrf_token: Arc<Mutex<Option<String>>>,
}

impl RobloxRateLimitMiddleware {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(RateState::default())),
            max_429_retries: 5,
            cushion_ms: 75,
        }
    }

    pub fn with_max_429_retries(mut self, n: usize) -> Self {
        self.max_429_retries = n;
        self
    }

    async fn ingest_headers(&self, resp: &Response) {
        let remaining = resp
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok());

        let reset_secs = resp
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok());

        let mut st = self.state.lock().await;
        if remaining.is_some() {
            st.remaining = remaining;
        }
        if reset_secs.is_some() {
            st.reset_after_secs = reset_secs;
        }
    }

    fn retry_wait_from_headers(resp: &Response) -> Duration {
        let secs = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .or_else(|| {
                resp.headers()
                    .get("x-ratelimit-reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.trim().parse::<u64>().ok())
            })
            .unwrap_or(1);

        Duration::from_secs(secs)
    }
}

impl RobloxAuthMiddleware {
    pub fn new() -> Self {
        Self {
            seen_etag: Arc::new(Mutex::new(false)),
            csrf_token: Arc::new(Mutex::new(None)),
        }
    }

    async fn set_seen(&self, seen: bool) {
        let mut lock = self.seen_etag.lock().await;
        *lock = seen;
    }

    async fn has_seen(&self) -> bool {
        let lock = self.seen_etag.lock().await;
        (*lock).clone()
    }

    pub async fn get_csrf_token(&self) -> Option<String> {
        let token_lock = self.csrf_token.lock().await;
        (*token_lock).clone()
    }

    pub async fn set_csrf_token(&self, token: String) {
        let mut token_lock = self.csrf_token.lock().await;
        *token_lock = Some(token);
    }
}

#[async_trait::async_trait]
impl Middleware for RobloxAuthMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut http::Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        if let Some(csrf_token) = self.get_csrf_token().await {
            req.headers_mut()
                .insert("x-csrf-token", HeaderValue::from_str(&csrf_token).unwrap());
        }

        if let Some(cookie_header) = super::JAR.cookies(&req.url()) {
            req.headers_mut().insert("cookie", cookie_header);
        }

        let resp = next
            .clone()
            .run(req.try_clone().unwrap(), extensions)
            .await?;

        let mut did_update_csrf = false;

        if let Some(new_token) = resp.headers().get("x-csrf-token") {
            if let Ok(token_str) = new_token.to_str() {
                self.set_csrf_token(token_str.to_string()).await;
                did_update_csrf = true;
                debug!("Updated CSRF token from response headers");
            }
        }

        if resp.status() == StatusCode::FORBIDDEN {
            if did_update_csrf {
                debug!("Retrying request with new CSRF token...");
                return Self::handle(self, req, extensions, next).await;
            }
        }

        if resp.status() == StatusCode::BAD_REQUEST {
            let status = resp.status();
            let body: ErrorResponse = resp.json().await?;

            if body.message == "ETagMismatch" {
                let seen = self.has_seen().await;
                if !seen {
                    self.set_seen(true).await;
                    warn!("Waiting for roblox etag to propagate...");
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
                let resp = Self::handle(self, req, extensions, next).await;

                if !seen {
                    self.set_seen(false).await;
                }

                return resp;
            };

            return Err(reqwest_middleware::Error::Middleware(anyhow::anyhow!(
                "Request failed with status {}: {}",
                status,
                body.message
            )));
        }

        Ok(resp)
    }
}

#[async_trait::async_trait]
impl Middleware for RobloxRateLimitMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut http::Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        let mut req = req;
        for attempt in 0..=self.max_429_retries {
            let req_clone = req.try_clone();

            let resp = next.clone().run(req, extensions).await?;

            if !resp.status().is_success() {
                debug!("request failed with status {}", resp.status());
            }

            if resp.status() != StatusCode::TOO_MANY_REQUESTS {
                return Ok(resp);
            }

            if attempt >= self.max_429_retries {
                return Ok(resp);
            }

            let wait = Self::retry_wait_from_headers(&resp);

            warn!(
                "Rate limited on attempt {}, retrying after {} seconds...",
                attempt + 1,
                wait.as_secs()
            );

            tokio::time::sleep(wait + Duration::from_millis(self.cushion_ms)).await;

            if let Some(cloned) = req_clone {
                req = cloned;
            } else {
                return Ok(resp);
            }
        }

        unreachable!()
    }
}
