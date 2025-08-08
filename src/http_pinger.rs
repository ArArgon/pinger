pub mod hyper_pinger;
pub mod reqwest_pinger;

use crate::config::HttpPingerEntry;
use anyhow::Result;
use async_trait::async_trait;
use hyper::Method;
use std::fmt::Display;
use std::time::{Duration, Instant};

#[async_trait]
pub trait AsyncHttpPinger {
    async fn ping(&self) -> Result<PingResponse>;

    fn new(entry: HttpPingerEntry, timeout: Duration) -> Result<Self>
    where
        Self: Sized;

    fn address(&self) -> &str;

    fn url(&self) -> &url::Url;

    fn method(&self) -> &Method;

    fn wrap_soft_err<E: Display>(&self, e: E, begin: Instant) -> PingResponse {
        PingResponse {
            url: self.url().to_string(),
            ip: None,
            send_time: begin,
            method: self.method().clone(),
            result: PingResult::Failure(e.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PingResponse {
    pub url: String,
    pub ip: Option<String>,
    pub send_time: Instant,
    pub method: Method,
    pub result: PingResult,
}

#[derive(Debug, Clone)]
pub enum PingResult {
    Success {
        http_status: u16,
        response_time: Duration,
        version: hyper::Version,
    },
    Failure(String),
    Timeout,
}
