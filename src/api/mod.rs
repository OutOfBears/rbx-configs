use std::sync::Arc;

use reqwest::Client;
use reqwest::cookie::Jar;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};

use crate::api::middleware::{RobloxAuthMiddleware, RobloxRateLimitMiddleware};

pub mod configs;
mod middleware;
pub mod model;

macro_rules! headers {
	($($key:expr => $value:expr),* $(,)?) => {{
		let mut headers = reqwest::header::HeaderMap::new();
		$(
			headers.insert(
				reqwest::header::HeaderName::from_static($key),
				reqwest::header::HeaderValue::from_str($value).unwrap(),
			);
		)*
		headers
	}};
}

lazy_static::lazy_static! {
    static ref JAR: Arc<Jar> = Arc::new(Jar::default());

    static ref API_CLIENT: ClientWithMiddleware = {
        let retry_policy = ExponentialBackoff::builder()
                .build_with_max_retries(5);

        let client = Client::builder()
            .user_agent(format!("rbx-configs/{}", env!("CARGO_PKG_VERSION")))
            .cookie_provider(Arc::clone(&JAR))
            .cookie_store(true)
            .default_headers(headers! {
                "cache-control" => "no-cache",
                "pragma" => "no-cache",
                "referrer" => "https://create.roblox.com",
                "origin" => "https://create.roblox.com",
                "priority" => "u=1, i",
            })
            .build().unwrap();

        ClientBuilder::new(client)
            .with(RobloxAuthMiddleware::new())
            .with(RobloxRateLimitMiddleware::new().with_max_429_retries(5))
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build()
    };
}

pub async fn set_cookie(token: String) {
    let url = "https://www.roblox.com/".parse().unwrap();

    JAR.add_cookie_str(
        &format!(
            ".ROBLOSECURITY={}; Domain=.roblox.com; Path=/; Secure; HttpOnly",
            token
        ),
        &url,
    );
}
