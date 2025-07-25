use anyhow::Result;
use hickory_resolver::name_server::TokioConnectionProvider;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::net::TcpSocket;
use url::Url;

type Resolver = hickory_resolver::Resolver<TokioConnectionProvider>;

#[derive(Debug, Clone)]
pub struct TcpPingResponse {
    pub address: String,
    pub resolved_ip: IpAddr,
    pub newly_resolved: bool,
    pub send_time: Instant,
    pub result: TcpPingResult,
}

#[derive(Debug, Clone)]
pub enum TcpPingResult {
    Success {
        address: SocketAddr,
        resolve_time: Option<Duration>,
        established_time: Duration,
    },
    Failure(String),
}

#[derive(Debug, Clone, Copy)]
enum ResolvePolicy {
    Always,
    Resolved(IpAddr),
}

pub struct TcpPinger {
    host: String,
    port: u16,
    timeout: Duration,
    resolver: Resolver,
    policy: ResolvePolicy,
}

impl TcpPinger {
    fn normalize_address(address: &str) -> Result<Url> {
        let url = Url::parse(address)?;
        if url.host_str().is_none() {
            return Err(anyhow::anyhow!(
                "Address must contain a valid host: {}",
                address
            ));
        }
        if url.port().is_none() {
            return Err(anyhow::anyhow!(
                "Address must contain a valid port: {}",
                address
            ));
        }
        if !url.scheme().is_empty() {
            return Err(anyhow::anyhow!(
                "Address should not contain a scheme: {}",
                address
            ));
        }
        if !url.path().is_empty() {
            return Err(anyhow::anyhow!(
                "Address should not contain a path: {}",
                address
            ));
        }
        if !url.query().is_none() {
            return Err(anyhow::anyhow!(
                "Address should not contain a query: {}",
                address
            ));
        }
        if !url.fragment().is_none() {
            return Err(anyhow::anyhow!(
                "Address should not contain a fragment: {}",
                address
            ));
        }
        if !url.username().is_empty() || !url.password().is_none() {
            return Err(anyhow::anyhow!(
                "Address should not contain credentials: {}",
                address
            ));
        }
        Ok(url)
    }

    fn is_ip(address: &Url) -> bool {
        if let Some(host) = address.host_str() {
            host.parse::<IpAddr>().is_ok()
        } else {
            false
        }
    }

    async fn resolve_addr(&self) -> Result<IpAddr> {
        let host = &self.host;
        let ip = self.resolver.lookup_ip(host).await?;
        if let Some(ip) = ip.iter().next() {
            Ok(ip)
        } else {
            Err(anyhow::anyhow!("No IP addresses found for host: {}", host))
        }
    }

    pub async fn new(
        address: String,
        timeout: Duration,
        always_resolve: bool,
        resolver: Resolver,
    ) -> Result<Self> {
        let address = Self::normalize_address(&address)?;
        let is_ip = Self::is_ip(&address);
        let host = address.host_str().unwrap().to_string();
        let port = address.port().unwrap();

        let resolve = if is_ip {
            ResolvePolicy::Resolved(address.host_str().unwrap().parse::<IpAddr>()?)
        } else if always_resolve {
            ResolvePolicy::Always
        } else {
            let ip = resolver.lookup_ip(&host).await?;
            if let Some(ip) = ip.iter().next() {
                ResolvePolicy::Resolved(ip)
            } else {
                return Err(anyhow::anyhow!("No IP addresses found for host: {}", host));
            }
        };

        Ok(Self {
            host,
            port,
            timeout,
            resolver,
            policy: resolve,
        })
    }

    pub async fn ping(&self) -> Result<TcpPingResult> {
        let begin = Instant::now();
        let mut resolve_time = None;
        let resolved_ip = match &self.policy {
            ResolvePolicy::Always => match self.resolve_addr().await {
                Ok(_) if begin.elapsed() > self.timeout => {
                    return Ok(TcpPingResult::Failure("Resolution timed out".to_string()));
                }
                Ok(ip) => {
                    resolve_time = Some(begin.elapsed());
                    ip
                }
                Err(e) => return Ok(TcpPingResult::Failure(e.to_string())),
            },
            ResolvePolicy::Resolved(ip) => *ip,
        };
        let socket_addr = SocketAddr::new(resolved_ip, self.port);
        let socket = match resolved_ip {
            IpAddr::V4(_) => TcpSocket::new_v4()?,
            IpAddr::V6(_) => TcpSocket::new_v6()?,
        };

        if let Err(e) = socket.connect(socket_addr).await {
            return Ok(TcpPingResult::Failure(e.to_string()));
        }

        let established_time = begin.elapsed();
        Ok(TcpPingResult::Success {
            resolve_time,
            address: socket_addr,
            established_time,
        })
    }
}
