use http::HeaderValue;
use reqwest::{Request, Response, StatusCode};
use reqwest_middleware::{Middleware, Next, Result};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

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
    cookie: Arc<Mutex<Option<String>>>,
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

    async fn preflight_wait(&self) {
        loop {
            let wait = {
                let st = self.state.lock().await;
                match (st.remaining, st.reset_after_secs) {
                    (Some(0), Some(secs)) if secs > 0 => Some(Duration::from_secs(secs)),
                    _ => None,
                }
            };

            if let Some(d) = wait {
                tokio::time::sleep(d + Duration::from_millis(self.cushion_ms)).await;
                continue;
            }

            break;
        }
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
            cookie: super::COOKIE.clone(),
            csrf_token: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn get_cookie(&self) -> Option<String> {
        let cookie_lock = self.cookie.lock().await;
        cookie_lock.clone()
    }

    pub async fn get_csrf_token(&self) -> Option<String> {
        let token_lock = self.csrf_token.lock().await;
        token_lock.clone()
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
        if let Some(cookie) = self.get_cookie().await {
            req.headers_mut().insert(
                "Cookie",
                HeaderValue::from_str(&format!(".ROBLOSECURITY={}", cookie)).unwrap(),
            );
        }

        if let Some(csrf_token) = self.get_csrf_token().await {
            req.headers_mut()
                .insert("x-csrf-token", HeaderValue::from_str(&csrf_token).unwrap());
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
            }
        }

        if resp.status() == StatusCode::FORBIDDEN {
            if did_update_csrf {
                return Self::handle(self, req, extensions, next).await;
            }
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
        self.preflight_wait().await;

        let mut req = req;
        for attempt in 0..=self.max_429_retries {
            let req_clone = req.try_clone();
            let resp = next.clone().run(req, extensions).await?;

            self.ingest_headers(&resp).await;

            if resp.status() != StatusCode::TOO_MANY_REQUESTS {
                return Ok(resp);
            }

            if attempt >= self.max_429_retries {
                return Ok(resp);
            }

            let wait = Self::retry_wait_from_headers(&resp);
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
