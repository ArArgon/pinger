mod hickory_wrapper;
mod timed_resolver;

use crate::config::PingerConfig;
use crate::metric::SharedMetrics;
use hickory_wrapper::build;
use reqwest::dns::Name;
use std::fmt::Debug;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use timed_resolver::TimedResolver;

pub trait Resolve: reqwest::dns::Resolve + Debug {}

pub fn build_resolver(
    config: &PingerConfig,
    metric: SharedMetrics,
) -> anyhow::Result<Arc<dyn Resolve>> {
    let hickory = build(
        if config.measure_dns_stats { 0 } else { 10 },
        10,
        Duration::from_millis(config.dns_timeout_millis),
    )?;

    if config.measure_dns_stats {
        Ok(Arc::new(TimedResolver::new(hickory, Arc::clone(&metric))))
    } else {
        Ok(Arc::new(hickory))
    }
}

pub async fn resolve_str(resolver: &dyn Resolve, name: &str) -> anyhow::Result<IpAddr> {
    let sock_addr = resolver
        .resolve(Name::from_str(name)?)
        .await
        .map_err(|e| anyhow::anyhow!(e))?
        .next()
        .ok_or(anyhow::anyhow!("no dns record for {}", name))?;
    Ok(sock_addr.ip())
}
