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

    fn new(entry: HttpPingerEntry) -> Result<Self>
    where
        Self: Sized;

    fn address(&self) -> &str;

    fn url(&self) -> &url::Url;

    fn method(&self) -> &Method;
}

#[derive(Debug, Clone)]
pub struct PingResponse {
    pub url: String,
    pub ip: String,
    pub send_time: Instant,
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
}

fn wrap_soft_err<E: Display>(pinger: &impl AsyncHttpPinger, e: E, begin: Instant) -> PingResponse {
    PingResponse {
        url: pinger.url().to_string(),
        ip: pinger.address().to_owned(),
        send_time: begin,
        result: PingResult::Failure(e.to_string()),
    }
}
