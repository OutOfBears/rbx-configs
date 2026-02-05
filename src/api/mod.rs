use std::sync::Arc;

use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use tokio::sync::Mutex;

use crate::api::middleware::{RobloxAuthMiddleware, RobloxRateLimitMiddleware};

pub mod configs;
mod middleware;
pub mod model;

lazy_static::lazy_static! {
    static ref COOKIE: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    static ref API_CLIENT: ClientWithMiddleware = {
        let retry_policy = ExponentialBackoff::builder()
                .build_with_max_retries(5);

        let client = Client::builder()
            .user_agent(format!("rbx_config/{}", env!("CARGO_PKG_VERSION")))
            .build().unwrap();

        ClientBuilder::new(client)
            .with(RobloxAuthMiddleware::new())
            .with(RobloxRateLimitMiddleware::new().with_max_429_retries(5))
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build()
    };
}

pub async fn set_cookie(token: String) {
    let mut guard = COOKIE.lock().await;
    *guard = Some(token);
}
