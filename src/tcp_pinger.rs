use crate::config::TcpPingerEntry;
use anyhow::Result;
use hickory_resolver::name_server::TokioConnectionProvider;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::net::TcpSocket;
use tokio_rustls::rustls::pki_types::ServerName;
use url::Url;

type Resolver = hickory_resolver::Resolver<TokioConnectionProvider>;

#[derive(Debug, Clone)]
pub struct TcpPingResult<'pinger> {
    pub address: (ServerName<'pinger>, u16),
    pub resolved_ip: IpAddr,
    pub newly_resolved: bool,
    pub send_time: Instant,
    pub response: TcpPingResponse,
}

#[derive(Debug, Clone)]
pub enum TcpPingResponse {
    Success {
        endpoint: SocketAddr,
        resolve_time: Option<Duration>,
        established_time: Duration,
    },
    Failure(String),
    Timeout,
}

#[derive(Debug, Clone, Copy)]
enum ResolvePolicy {
    Always,
    Resolved(IpAddr),
}

pub struct TcpPinger {
    host: ServerName<'static>,
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

    fn wrap_soft_err<E: std::fmt::Display>(&self, e: E, begin: Instant) -> Result<TcpPingResult> {
        Ok(TcpPingResult {
            address: (self.host.clone(), self.port),
            resolved_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            newly_resolved: false,
            send_time: begin,
            response: TcpPingResponse::Failure(e.to_string()),
        })
    }

    fn wrap_timeout(&self, begin: Instant) -> Result<TcpPingResult> {
        Ok(TcpPingResult {
            address: (self.host.clone(), self.port),
            resolved_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            newly_resolved: false,
            send_time: begin,
            response: TcpPingResponse::Timeout,
        })
    }

    async fn resolve_addr(&self) -> Result<IpAddr> {
        let host = &self.host;

        match host {
            ServerName::IpAddress(ip) => Ok(IpAddr::from(*ip)),
            ServerName::DnsName(name) => {
                let ip = self.resolver.lookup_ip(name.as_ref()).await?;
                if let Some(ip) = ip.iter().next() {
                    Ok(ip)
                } else {
                    Err(anyhow::anyhow!(
                        "No IP addresses found for host: {}",
                        name.as_ref()
                    ))
                }
            }
            _ => unreachable!("unexpected ServerName variant"),
        }
    }

    pub async fn new(
        TcpPingerEntry {
            host,
            port,
            always_resolve,
        }: TcpPingerEntry,
        timeout: Duration,
        resolver: Resolver,
    ) -> Result<Self> {
        let host = ServerName::try_from(host)?;

        let resolve = match host.clone() {
            ServerName::IpAddress(ip) => ResolvePolicy::Resolved(IpAddr::from(ip)),
            ServerName::DnsName(name) => {
                if always_resolve {
                    ResolvePolicy::Always
                } else {
                    let ip = resolver.lookup_ip(name.as_ref()).await?;
                    if let Some(ip) = ip.iter().next() {
                        ResolvePolicy::Resolved(ip)
                    } else {
                        return Err(anyhow::anyhow!(
                            "No IP addresses found for host: {}",
                            name.as_ref()
                        ));
                    }
                }
            }
            _ => unreachable!("unexpected ServerName variant"),
        };

        Ok(Self {
            host,
            port,
            timeout,
            resolver,
            policy: resolve,
        })
    }

    async fn ping_inner(&self) -> Result<TcpPingResult> {
        let mut resolve_time: Option<Duration> = None;
        let begin = Instant::now();
        let resolved_ip = match &self.policy {
            ResolvePolicy::Always => match self.resolve_addr().await {
                Ok(ip) => {
                    resolve_time = Some(begin.elapsed());
                    ip
                }
                Err(e) => return self.wrap_soft_err(e, begin),
            },
            ResolvePolicy::Resolved(ip) => *ip,
        };
        let socket_addr = SocketAddr::new(resolved_ip, self.port);
        let socket = match resolved_ip {
            IpAddr::V4(_) => TcpSocket::new_v4()?,
            IpAddr::V6(_) => TcpSocket::new_v6()?,
        };

        if let Err(e) = socket.connect(socket_addr).await {
            return self.wrap_soft_err(e, begin);
        }

        let established_time = begin.elapsed();
        Ok(TcpPingResult {
            address: (self.host.clone(), self.port),
            resolved_ip,
            newly_resolved: matches!(self.policy, ResolvePolicy::Always),
            send_time: begin,
            response: TcpPingResponse::Success {
                endpoint: socket_addr,
                resolve_time,
                established_time,
            },
        })
    }

    pub async fn ping(&self) -> Result<TcpPingResult> {
        let task_submission_time = Instant::now();
        let result =
            tokio::time::timeout(self.timeout, async move { self.ping_inner().await }).await;

        match result {
            Ok(Ok(res)) => Ok(res),
            Ok(Err(e)) => {
                // This is not a soft error, but a failure to ping
                panic!(
                    "fatal error occurs when pinging {}: {}",
                    self.host.to_str(),
                    e
                );
            }
            Err(_) => self.wrap_timeout(task_submission_time),
        }
    }
}
