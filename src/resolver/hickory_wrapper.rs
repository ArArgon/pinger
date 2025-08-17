use crate::Resolve;
use hickory_resolver::Resolver;
use hickory_resolver::config::ResolverOpts;
use hickory_resolver::lookup_ip::LookupIpIntoIter;
use hickory_resolver::name_server::TokioConnectionProvider;
use reqwest::dns::Addrs;
use std::net::SocketAddr;
use std::time::Duration;
use tracing::info;

#[derive(Debug, Clone)]
pub struct HickoryWrapper(Resolver<TokioConnectionProvider>);

struct SocketAddrIter {
    iter: LookupIpIntoIter,
}

impl Iterator for SocketAddrIter {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ip| SocketAddr::new(ip, 0))
    }
}

impl reqwest::dns::Resolve for HickoryWrapper {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let resolver = self.0.clone();
        Box::pin(async move {
            let result = resolver.lookup_ip(name.as_str()).await?;
            let iter: Addrs = Box::new(SocketAddrIter {
                iter: result.into_iter(),
            });
            Ok(iter)
        })
    }
}

impl Resolve for HickoryWrapper {}

pub fn build(
    cache_size: usize,
    num_concurrent_reqs: usize,
    timeout: Duration,
) -> anyhow::Result<HickoryWrapper> {
    let mut options = ResolverOpts::default();
    options.cache_size = cache_size;
    options.num_concurrent_reqs = num_concurrent_reqs;
    options.timeout = timeout;

    let hickory = Resolver::builder(TokioConnectionProvider::default())?
        .with_options(options)
        .build();

    info!("Hickory DNS config: {:?}", hickory.config());
    Ok(HickoryWrapper(hickory))
}
