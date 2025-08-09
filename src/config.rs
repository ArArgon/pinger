use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum HttpPinger {
    Hyper,
    Reqwest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpPingerEntry {
    pub url: String,
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpPingerConfig {
    pub pinger: HttpPinger,
    pub retries: u8,
    pub timeout_millis: u64,
    pub interval_millis: u64,
    pub entries: Vec<HttpPingerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpPingerEntry {
    pub always_resolve: bool,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpPingerConfig {
    pub retries: u8,
    pub timeout_millis: u64,
    pub interval_millis: u64,
    pub entries: Vec<TcpPingerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingerConfig {
    pub http: HttpPingerConfig,
    pub tcp: TcpPingerConfig,
    pub metrics: MetricsServerConfig,
}
