use crate::config::{Args, HttpPinger, PingerConfig};
use crate::http_pinger::AsyncHttpPinger;
use crate::http_pinger::hyper_pinger::HyperPinger;
use crate::http_pinger::reqwest_pinger::ReqwestPinger;
use crate::metric::{PingMetrics, SharedMetrics};
use crate::metrics_server::start_metrics_server;
use crate::tcp_pinger::TcpPinger;
use anyhow::Result;
use clap::Parser;
use resolver::Resolve;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{error, info};

mod config;
mod http_pinger;
mod metric;
mod metrics_server;
mod resolver;
mod tcp_pinger;

/// Enum to hold different HTTP pinger types
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

/// Load configuration from file
async fn load_config(config_path: &str) -> Result<PingerConfig> {
    let config_content = tokio::fs::read_to_string(config_path).await?;
    serde_json::from_str(&config_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))
}

/// Create HTTP ping task
fn create_http_ping_task(
    entry: crate::config::HttpPingerEntry,
    timeout: Duration,
    interval: Duration,
    resolver: Arc<dyn Resolve>,
    metrics: SharedMetrics,
    pinger_type: HttpPinger,
) -> Result<JoinHandle<()>> {
    let pinger_result = match pinger_type {
        HttpPinger::Hyper => {
            HyperPinger::new(entry, timeout, Arc::clone(&resolver) as _).map(HttpPingerImpl::Hyper)
        }
        HttpPinger::Reqwest => ReqwestPinger::new(entry, timeout, Arc::clone(&resolver) as _)
            .map(HttpPingerImpl::Reqwest),
    };

    match pinger_result {
        Ok(pinger) => {
            let task = tokio::spawn(async move {
                let mut tick = tokio::time::interval(interval);
                loop {
                    match pinger.ping().await {
                        Ok(response) => {
                            info!(name: "httping", "Response: {:?}", response);
                            metrics.record_http_ping(&response);
                        }
                        Err(e) => {
                            error!("HTTP Ping error: {}", e);
                        }
                    }
                    tick.tick().await;
                }
            });
            Ok(task)
        }
        Err(e) => {
            error!("Failed to create HTTP pinger: {}", e);
            Err(anyhow::anyhow!("HTTP pinger creation failed: {}", e))
        }
    }
}

/// Create TCP ping task
async fn create_tcp_ping_task(
    entry: crate::config::TcpPingerEntry,
    timeout: Duration,
    interval: Duration,
    measure_dns_stats: bool,
    resolver: Arc<dyn Resolve>,
    metrics: SharedMetrics,
) -> Result<JoinHandle<()>> {
    match TcpPinger::new(entry, timeout, measure_dns_stats, resolver).await {
        Ok(pinger) => {
            let task = tokio::spawn(async move {
                loop {
                    match pinger.ping().await {
                        Ok(response) => {
                            info!(name: "tcping", "Response: {:?}", response);
                            metrics.record_tcp_ping(&response);
                        }
                        Err(e) => {
                            error!("TCP Ping error: {}", e);
                        }
                    }
                    tokio::time::sleep(interval).await;
                }
            });
            Ok(task)
        }
        Err(e) => {
            error!("Failed to create TCP pinger: {}", e);
            Err(anyhow::anyhow!("TCP pinger creation failed: {}", e))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.debug {
        tracing_subscriber::fmt::init();
    }

    // Load configuration
    let config = load_config(&args.config).await?;

    // Initialize metrics
    let metrics: SharedMetrics = Arc::new(PingMetrics::default());

    // Start metrics server in background with CLI configurable host and port
    let metrics_server_handle = {
        let metrics_clone = Arc::clone(&metrics);
        let host = args.bind.clone();
        let port = args.port;
        tokio::spawn(async move {
            start_metrics_server(metrics_clone, &host, port)
                .await
                .unwrap()
        })
    };

    let resolver = resolver::build_resolver(&config, Arc::clone(&metrics))?;
    let mut ping_tasks: Vec<JoinHandle<()>> = Vec::new();

    // Create HTTP ping tasks
    if !config.http.entries.is_empty() {
        let http_timeout = Duration::from_millis(config.http.timeout_millis);
        let http_interval = Duration::from_millis(config.http.interval_millis);

        if http_interval < http_timeout {
            error!("HTTP interval is less than timeout, which is not allowed");
            return Err("HTTP interval is less than timeout, which is not allowed".into());
        }

        for entry in config.http.entries {
            match create_http_ping_task(
                entry,
                http_timeout,
                http_interval,
                Arc::clone(&resolver),
                Arc::clone(&metrics),
                config.http.pinger,
            ) {
                Ok(task) => ping_tasks.push(task),
                Err(e) => error!("Failed to create HTTP ping task: {}", e),
            }
        }
    }

    // Create TCP ping tasks
    if !config.tcp.entries.is_empty() {
        let tcp_timeout = Duration::from_millis(config.tcp.timeout_millis);
        let tcp_interval = Duration::from_millis(config.tcp.interval_millis);

        if tcp_interval < tcp_timeout {
            error!("TCP interval is less than timeout, which is not allowed");
            return Err("TCP interval is less than timeout, which is not allowed".into());
        }

        for entry in config.tcp.entries {
            match create_tcp_ping_task(
                entry,
                tcp_timeout,
                tcp_interval,
                config.measure_dns_stats,
                Arc::clone(&resolver),
                Arc::clone(&metrics),
            )
            .await
            {
                Ok(task) => ping_tasks.push(task),
                Err(e) => error!("Failed to create TCP ping task: {}", e),
            }
        }
    }

    println!("Started {} ping tasks", ping_tasks.len());
    println!(
        "Metrics server running on http://{}:{}/metrics",
        args.bind, args.port
    );

    // Wait for all tasks (runs indefinitely)
    for task in ping_tasks {
        let _ = task.await;
    }

    // Wait for metrics server
    let _ = metrics_server_handle.await;

    Ok(())
}
