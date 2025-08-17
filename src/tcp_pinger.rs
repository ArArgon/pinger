use crate::config::TcpPingerEntry;
use crate::resolver::{Resolve, resolve_str};
use anyhow::Result;
use std::fmt::Debug;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpSocket;
use tokio_rustls::rustls::pki_types::ServerName;
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct TcpPingResult {
    pub address: (ServerName<'static>, u16),
    pub resolved_ip: IpAddr,
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

#[derive(Debug)]
pub struct TcpPinger {
    host: ServerName<'static>,
    port: u16,
    timeout: Duration,
    resolver: Arc<dyn Resolve>,
    policy: ResolvePolicy,
}

impl TcpPinger {
    fn wrap_soft_err<E: std::fmt::Display>(&self, e: E, begin: Instant) -> Result<TcpPingResult> {
        Ok(TcpPingResult {
            address: (self.host.clone(), self.port),
            resolved_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            send_time: begin,
            response: TcpPingResponse::Failure(e.to_string()),
        })
    }

    fn wrap_timeout(&self, begin: Instant) -> Result<TcpPingResult> {
        Ok(TcpPingResult {
            address: (self.host.clone(), self.port),
            resolved_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            send_time: begin,
            response: TcpPingResponse::Timeout,
        })
    }

    #[instrument(fields(host = %self.host.to_str(), port = %self.port), skip(self))]
    async fn resolve_addr(&self) -> Result<IpAddr> {
        let host = &self.host;

        match host {
            ServerName::IpAddress(ip) => Ok(IpAddr::from(*ip)),
            ServerName::DnsName(name) => {
                Ok(resolve_str(self.resolver.as_ref(), name.as_ref()).await?)
            }
            _ => unreachable!("unexpected ServerName variant"),
        }
    }

    pub async fn new(
        TcpPingerEntry { host, port }: TcpPingerEntry,
        timeout: Duration,
        measure_dns: bool,
        resolver: Arc<dyn Resolve>,
    ) -> Result<Self> {
        let host = ServerName::try_from(host)?;

        let resolve = match host.clone() {
            ServerName::IpAddress(ip) => ResolvePolicy::Resolved(IpAddr::from(ip)),
            ServerName::DnsName(name) => {
                if measure_dns {
                    ResolvePolicy::Always
                } else {
                    ResolvePolicy::Resolved(resolve_str(resolver.as_ref(), name.as_ref()).await?)
                }
            }
            _ => unreachable!("unexpected ServerName variant"),
        };

        Ok(Self {
            host,
            port,
            timeout,
            resolver: resolver as _,
            policy: resolve,
        })
    }

    #[instrument(fields(host = %self.host.to_str(), port = %self.port), skip(self))]
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
            send_time: begin,
            response: TcpPingResponse::Success {
                endpoint: socket_addr,
                resolve_time,
                established_time,
            },
        })
    }

    #[instrument(fields(host = %self.host.to_str(), port = %self.port), skip(self))]
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
