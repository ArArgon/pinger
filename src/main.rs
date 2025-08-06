use crate::config::HttpPingerEntry;
use crate::http_pinger::hyper_pinger::HyperPinger;
use crate::http_pinger::AsyncHttpPinger;
use crate::metric::PingMetrics;
use crate::metrics_server::{start_metrics_server, SharedMetrics};
use anyhow::Result;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::Resolver;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

mod config;
mod http_pinger;
mod metric;
mod metrics_server;
mod tcp_pinger;

fn build_resolver() -> Resolver<TokioConnectionProvider> {
    let mut options = ResolverOpts::default();
    options.cache_size = 0; // Disable caching for testing purposes

    Resolver::builder_with_config(ResolverConfig::new(), TokioConnectionProvider::default())
        .with_options(options)
        .build()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize metrics
    let metrics: SharedMetrics = Arc::new(PingMetrics::default());

    // Start metrics server in background
    let metrics_server_handle = {
        let metrics_clone = Arc::clone(&metrics);
        tokio::spawn(async move {
            if let Err(e) = start_metrics_server(metrics_clone, 3000).await {
                eprintln!("Metrics server error: {}", e);
            }
        })
    };

    let resolver = build_resolver();

    let urls = vec![
        ("https://www.google.com", hyper::Method::HEAD),
        ("https://www.rust-lang.org", hyper::Method::HEAD),
        ("https://www.github.com", hyper::Method::HEAD),
        ("https://www.youtube.com", hyper::Method::HEAD),
        ("https://www.wikipedia.org", hyper::Method::HEAD),
        ("https://www.stackoverflow.com", hyper::Method::HEAD),
        ("https://www.reddit.com", hyper::Method::HEAD),
        ("https://www.x.com", hyper::Method::HEAD),
        ("https://www.facebook.com", hyper::Method::HEAD),
        ("https://www.linkedin.com", hyper::Method::HEAD),
    ];

    let pingers = urls
        .iter()
        .map(|(url, method)| {
            HyperPinger::new(
                HttpPingerEntry {
                    url: url.to_string(),
                    method: method.to_string(),
                },
                Duration::from_secs(10),
            )
        })
        .collect::<Result<Vec<_>>>()?;

    // Create ping tasks with metrics recording
    let ping_tasks: Vec<JoinHandle<()>> = pingers
        .into_iter()
        .map(|pinger| {
            let metrics_clone = Arc::clone(&metrics);
            tokio::spawn(async move {
                loop {
                    metrics_clone.inc_active_pings();

                    match pinger.ping().await {
                        Ok(response) => {
                            println!("Ping response: {:?}", response);
                            metrics_clone.record_http_ping(&response, None);
                        }
                        Err(e) => {
                            eprintln!("Ping error: {}", e);
                        }
                    }

                    metrics_clone.dec_active_pings();
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            })
        })
        .collect();

    println!("Started {} ping tasks", ping_tasks.len());
    println!("Metrics server running on http://localhost:3000/metrics");

    // Wait for all tasks (runs indefinitely)
    for task in ping_tasks {
        let _ = task.await;
    }

    // Wait for metrics server
    let _ = metrics_server_handle.await;

    Ok(())
}
