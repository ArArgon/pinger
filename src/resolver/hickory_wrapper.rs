use std::time::Duration;

use crate::Resolve;
use hickory_resolver::Resolver;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::lookup_ip::LookupIpIntoIter;
use hickory_resolver::name_server::TokioConnectionProvider;
use reqwest::dns::Addrs;
use std::net::SocketAddr;

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

impl Resolve for HickoryWrapper {
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

pub fn build(cache_size: usize, num_concurrent_reqs: usize, timeout: Duration) -> HickoryWrapper {
    let mut options = ResolverOpts::default();
    options.cache_size = cache_size;
    options.num_concurrent_reqs = num_concurrent_reqs;
    options.timeout = timeout;

    HickoryWrapper(
        Resolver::builder_with_config(
            ResolverConfig::default(),
            TokioConnectionProvider::default(),
        )
        .with_options(options)
        .build(),
    )
}
