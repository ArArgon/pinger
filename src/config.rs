use clap::Parser;
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
    pub dns_timeout_millis: u64,
    pub measure_dns_stats: bool,
}

#[derive(Debug, Clone, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Config file path
    #[arg(short, long)]
    pub config: String,

    /// Enable debug mode
    #[arg(short, long, default_value_t = false)]
    pub debug: bool,
}
