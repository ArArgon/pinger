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
use tokio::signal;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
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
    #[inline]
    async fn ping(&self) -> Result<crate::http_pinger::PingResponse> {
        match self {
            HttpPingerImpl::Hyper(pinger) => pinger.ping().await,
            HttpPingerImpl::Reqwest(pinger) => pinger.ping().await,
        }
    }
}

/// Load configuration from file
async fn load_config(config_path: &str) -> Result<PingerConfig> {
    let path = std::path::Path::new(config_path);

    let config_content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path.display(), e))?;
    let ext = path
        .file_name()
        .ok_or(anyhow::anyhow!("Failed to get file name"))?
        .to_str()
        .ok_or(anyhow::anyhow!("Failed to decode file name"))?
        .split(".")
        .last()
        .ok_or(anyhow::anyhow!("Failed to get file extension"))?;
    match ext {
        "json" => serde_json::from_str(&config_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e)),
        "yaml" => serde_yaml::from_str(&config_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e)),
        "toml" => toml::from_str(&config_content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e)),
        _ => anyhow::bail!("Unsupported file extension: {}", ext),
    }
}

/// Create HTTP ping task
#[allow(clippy::too_many_arguments)]
fn create_http_ping_task(
    entry: crate::config::HttpPingerEntry,
    timeout: Duration,
    interval: Duration,
    retries: u8,
    resolver: Arc<dyn Resolve>,
    metrics: SharedMetrics,
    pinger_type: HttpPinger,
    cancel: CancellationToken,
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
                    tokio::select! {
                        _ = cancel.cancelled() => {
                            break;
                        }
                        _ = tick.tick() => {
                            for _ in 0..retries {
                                match pinger.ping().await {
                                    Ok(response) => {
                                        info!(name: "httping", "Response: {:?}", response);
                                        metrics.record_http_ping(&response);
                                        break;
                                    }
                                    Err(e) => {
                                        error!("HTTP Ping error: {}", e);
                                    }
                                }
                            }
                        }
                    }
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
#[allow(clippy::too_many_arguments)]
async fn create_tcp_ping_task(
    entry: crate::config::TcpPingerEntry,
    timeout: Duration,
    interval: Duration,
    measure_dns_stats: bool,
    retries: u8,
    resolver: Arc<dyn Resolve>,
    metrics: SharedMetrics,
    cancel: CancellationToken,
) -> Result<JoinHandle<()>> {
    match TcpPinger::new(entry, timeout, measure_dns_stats, resolver).await {
        Ok(pinger) => {
            let mut tick = tokio::time::interval(interval);
            let task = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => { break; }
                        _ = tick.tick() => {
                            for _ in 0..retries {
                                match pinger.ping().await {
                                    Ok(response) => {
                                        info!(name: "tcping", "Response: {:?}", response);
                                        metrics.record_tcp_ping(&response);
                                        break;
                                    }
                                    Err(e) => {
                                        error!("TCP Ping error: {}", e);
                                    }
                                }
                            }
                        }
                    }
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

fn cancel_handler() -> (CancellationToken, JoinHandle<()>) {
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    let cancel_task = tokio::spawn(async move {
        signal::ctrl_c()
            .await
            .expect("Failed to register Ctrl+C handler");
        info!("Received interrupt signal, cancelling tasks");
        cancel_clone.cancel();
    });
    (cancel, cancel_task)
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

    // Ctrl+C to cancel all tasks
    let (cancel, cancel_task) = cancel_handler();

    // Start metrics server in background with CLI configurable host and port
    let metrics_server_handle = tokio::spawn(start_metrics_server(
        Arc::clone(&metrics),
        args.bind.clone(),
        args.port,
        cancel.clone(),
    ));

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
                config.http.retries,
                Arc::clone(&resolver),
                Arc::clone(&metrics),
                config.http.pinger,
                cancel.clone(),
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
                config.tcp.retries,
                Arc::clone(&resolver),
                Arc::clone(&metrics),
                cancel.clone(),
            )
            .await
            {
                Ok(task) => ping_tasks.push(task),
                Err(e) => error!("Failed to create TCP ping task: {}", e),
            }
        }
    }

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

    // Wait for cancel task
    let _ = cancel_task.await;

    Ok(())
}
