use clap::Parser;
use serde::{Deserialize, Serialize};

/// HTTP client implementation to use
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum HttpPinger {
    Hyper,
    Reqwest,
}

/// HTTP endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpPingerEntry {
    pub url: String,
    pub method: String,
}

/// HTTP ping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpPingerConfig {
    pub pinger: HttpPinger,
    pub retries: u8,
    pub timeout_millis: u64,
    pub interval_millis: u64,
    pub entries: Vec<HttpPingerEntry>,
}

/// TCP endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpPingerEntry {
    pub host: String,
    pub port: u16,
}

/// TCP ping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpPingerConfig {
    pub retries: u8,
    pub timeout_millis: u64,
    pub interval_millis: u64,
    pub entries: Vec<TcpPingerEntry>,
}

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingerConfig {
    pub http: HttpPingerConfig,
    pub tcp: TcpPingerConfig,
    pub dns_timeout_millis: u64,
    pub measure_dns_stats: bool,
}

/// Command line arguments
#[derive(Debug, Clone, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Configuration file path
    #[arg(short, long)]
    pub config: String,

    /// Enable debug mode
    #[arg(short, long, default_value_t = false)]
    pub debug: bool,

    /// Metrics server bind address
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: String,

    /// Metrics server port
    #[arg(long, default_value_t = 3000)]
    pub port: u16,
}
