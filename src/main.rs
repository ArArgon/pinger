use crate::config::{HttpPinger, PingerConfig};
use crate::http_pinger::AsyncHttpPinger;
use crate::http_pinger::hyper_pinger::HyperPinger;
use crate::http_pinger::reqwest_pinger::ReqwestPinger;
use crate::metric::{PingMetrics, SharedMetrics};
use crate::metrics_server::start_metrics_server;
use crate::tcp_pinger::TcpPinger;
use anyhow::Result;
use resolver::Resolve;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

mod config;
mod http_pinger;
mod metric;
mod metrics_server;
mod resolver;
mod tcp_pinger;

// Enum to hold different HTTP pinger types
enum HttpPingerImpl {
    Hyper(HyperPinger),
    Reqwest(ReqwestPinger),
}

impl HttpPingerImpl {
    async fn ping(&self) -> Result<crate::http_pinger::PingResponse> {
        match self {
            HttpPingerImpl::Hyper(pinger) => pinger.ping().await,
            HttpPingerImpl::Reqwest(pinger) => pinger.ping().await,
        }
    }
}

async fn load_config() -> Result<PingerConfig> {
    // Try to load from config file, fallback to default config
    let config = tokio::fs::read_to_string("config.json").await?;
    serde_json::from_str(&config).map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = load_config().await?;

    // Initialize metrics
    let metrics: SharedMetrics = Arc::new(PingMetrics::default());

    // Start metrics server in background with configurable host and port
    let metrics_server_handle = {
        let metrics_clone = Arc::clone(&metrics);
        let host = config.metrics.host.clone();
        let port = config.metrics.port;
        tokio::spawn(async move {
            start_metrics_server(metrics_clone, &host, port)
                .await
                .unwrap()
        })
    };

    let resolver = resolver::build_resolver(&config, Arc::clone(&metrics));
    let mut ping_tasks: Vec<JoinHandle<()>> = Vec::new();

    // Create HTTP ping tasks
    if !config.http.entries.is_empty() {
        let http_timeout = Duration::from_millis(config.http.timeout_millis);
        let http_interval = Duration::from_millis(config.http.interval_millis);

        for entry in config.http.entries {
            let pinger_result = match config.http.pinger {
                HttpPinger::Hyper => {
                    HyperPinger::new(entry, http_timeout, Arc::clone(&resolver) as _)
                        .map(HttpPingerImpl::Hyper)
                }
                HttpPinger::Reqwest => {
                    ReqwestPinger::new(entry, http_timeout, Arc::clone(&resolver) as _)
                        .map(HttpPingerImpl::Reqwest)
                }
            };

            match pinger_result {
                Ok(pinger) => {
                    let metrics_clone: SharedMetrics = Arc::clone(&metrics);
                    let task = tokio::spawn(async move {
                        loop {
                            match pinger.ping().await {
                                Ok(response) => {
                                    println!("HTTP Ping response: {:?}", response);
                                    metrics_clone.record_http_ping(&response);
                                }
                                Err(e) => {
                                    eprintln!("HTTP Ping error: {}", e);
                                }
                            }
                            tokio::time::sleep(http_interval).await;
                        }
                    });
                    ping_tasks.push(task);
                }
                Err(e) => {
                    eprintln!("Failed to create HTTP pinger: {}", e);
                }
            }
        }
    }

    // Create TCP ping tasks
    if !config.tcp.entries.is_empty() {
        let tcp_timeout = Duration::from_millis(config.tcp.timeout_millis);
        let tcp_interval = Duration::from_millis(config.tcp.interval_millis);

        for entry in config.tcp.entries {
            match TcpPinger::new(
                entry,
                tcp_timeout,
                config.measure_dns_stats,
                resolver.clone(),
            )
            .await
            {
                Ok(pinger) => {
                    let metrics_clone: SharedMetrics = Arc::clone(&metrics);
                    let task = tokio::spawn(async move {
                        loop {
                            match pinger.ping().await {
                                Ok(response) => {
                                    println!("TCP Ping response: {:?}", response);
                                    metrics_clone.as_ref().record_tcp_ping(&response);
                                }
                                Err(e) => {
                                    eprintln!("TCP Ping error: {}", e);
                                }
                            }
                            tokio::time::sleep(tcp_interval).await;
                        }
                    });
                    ping_tasks.push(task);
                }
                Err(e) => {
                    eprintln!("Failed to create TCP pinger: {}", e);
                }
            }
        }
    }

    println!("Started {} ping tasks", ping_tasks.len());
    println!(
        "Metrics server running on http://{}:{}/metrics",
        config.metrics.host, config.metrics.port
    );

    // Wait for all tasks (runs indefinitely)
    for task in ping_tasks {
        let _ = task.await;
    }

    // Wait for metrics server
    let _ = metrics_server_handle.await;

    Ok(())
}
