use crate::config::HttpPingerEntry;
use crate::http_pinger::hyper_pinger::HyperPinger;
use crate::http_pinger::AsyncHttpPinger;
use anyhow::Result;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::Resolver;
use tokio::task::JoinHandle;

mod config;
mod http_pinger;
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
            HyperPinger::new(HttpPingerEntry {
                url: url.to_string(),
                method: method.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let interval = std::time::Duration::from_millis(10000);
    let retry_interval = std::time::Duration::from_millis(50);
    let handles: Vec<JoinHandle<Result<http_pinger::PingResponse>>> = pingers
        .into_iter()
        .map(|pinger| {
            tokio::spawn(async move {
                loop {
                    for retry in 0..3 {
                        match pinger.ping().await {
                            Ok(response) => {
                                println!("{:?}", response);
                                break;
                            }
                            Err(e) => {
                                eprintln!("Error pinging {}: {}", pinger.address(), e);
                            }
                        }
                        if retry < 2 {
                            tokio::time::sleep(retry_interval).await;
                        } else {
                            break; // Exit the retry loop after 3 attempts
                        }
                    }

                    tokio::time::sleep(interval).await;
                }
            })
        })
        .collect();

    for handle in handles {
        match handle.await {
            Ok(Ok(response)) => println!("Ping response: {:?}", response),
            Ok(Err(e)) => eprintln!("Error in ping task: {}", e),
            Err(e) => eprintln!("Task join error: {}", e),
        }
    }

    Ok(())
}
