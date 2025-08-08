use crate::config::HttpPingerEntry;
use crate::http_pinger::{AsyncHttpPinger, PingResponse, PingResult};
use async_trait::async_trait;
use http_body_util::Empty;
use hyper::body::{Body, Bytes, Incoming};
use hyper::{Method, Request, Response, Version};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::ops::Add;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

#[derive(Debug, Clone)]
pub(crate) struct HyperPinger {
    url: url::Url,
    address: String,
    method: Method,
    timeout: Duration,
    tls_config: Arc<ClientConfig>,
}

struct Connect {
    peer_address: SocketAddr,
    begin: Instant,
    res: Pin<Box<dyn Future<Output = anyhow::Result<Response<Incoming>, hyper::Error>> + Send>>,
    handle: JoinHandle<anyhow::Result<(), hyper::Error>>,
}

impl HyperPinger {
    async fn connect_tls<B>(&self, req: Request<B>) -> anyhow::Result<Connect>
    where
        B: Body + Send + 'static,
        <B as Body>::Error: std::error::Error + Send + Sync + 'static,
        <B as Body>::Data: Send + Sync + 'static,
    {
        let host = self.url.host().unwrap().to_owned();
        let connector = TlsConnector::from(self.tls_config.clone());

        let begin = Instant::now();
        let dns_name = ServerName::try_from(host.to_string())?;
        let tcp = TcpStream::connect(&self.address).await?;
        let peer_address = tcp.peer_addr()?;
        let stream = connector.connect(dns_name, tcp).await?;

        let io = TokioIo::new(stream);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

        // Spawn the connection future to handle incoming responses
        let handle = tokio::spawn(async move { conn.await });
        let res = sender.send_request(req);
        Ok(Connect {
            begin,
            peer_address,
            res: Box::pin(res),
            handle,
        })
    }

    async fn connect_http<B>(&self, req: Request<B>) -> anyhow::Result<Connect>
    where
        B: Body + Send + 'static,
        <B as Body>::Error: std::error::Error + Send + Sync + 'static,
        <B as Body>::Data: Send + Sync + 'static,
    {
        let begin = Instant::now();
        let tcp = TcpStream::connect(&self.address).await?;
        let peer_address = tcp.peer_addr()?;
        let io = TokioIo::new(tcp);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

        // Spawn the connection future to handle incoming responses
        let handle = tokio::spawn(async move { conn.await });
        let res = sender.send_request(req);
        Ok(Connect {
            begin,
            peer_address,
            res: Box::pin(res),
            handle,
        })
    }

    fn build_request(&self) -> anyhow::Result<Request<Empty<Bytes>>, anyhow::Error> {
        Ok(hyper::Request::builder()
            .method(self.method.clone())
            .header(hyper::header::HOST, self.url.authority())
            .uri(self.url.as_str())
            .body(Empty::<Bytes>::new())?)
    }

    async fn ping_inner(&self) -> anyhow::Result<PingResponse> {
        let req = self.build_request()?;
        let conn_result = if self.url.scheme() == "https" {
            self.connect_tls(req).await
        } else {
            self.connect_http(req).await
        };

        let Connect {
            begin,
            res,
            handle,
            peer_address,
        } = match conn_result {
            Ok(result) => result,
            Err(e) => return Ok(self.wrap_soft_err(e, Instant::now())),
        };

        if let Err(e) = handle.await {
            return Err(anyhow::anyhow!("Connection error: {}", e));
        }

        match res.await {
            Ok(response) => {
                let response_time = begin.elapsed();
                let status = response.status();
                Ok(PingResponse {
                    url: self.url.to_string(),
                    ip: Some(peer_address.ip().to_string()),
                    send_time: begin,
                    method: self.method.clone(),
                    result: PingResult::Success {
                        http_status: status.as_u16(),
                        response_time,
                        version: Version::HTTP_11,
                    },
                })
            }
            Err(e) => Err(anyhow::anyhow!("Failed to send request: {}", e)),
        }
    }
}

#[async_trait]
impl AsyncHttpPinger for HyperPinger {
    async fn ping(&self) -> anyhow::Result<PingResponse> {
        use tokio::time::{timeout_at, Instant as TokioInstant};

        let begin = Instant::now();
        let result = timeout_at(
            TokioInstant::from(begin.add(self.timeout)),
            self.ping_inner(),
        )
        .await;

        match result {
            Ok(res) => res,
            Err(_) => Ok(PingResponse {
                url: self.url.to_string(),
                ip: None,
                send_time: begin,
                method: self.method.clone(),
                result: PingResult::Timeout,
            }),
        }
    }
    fn new(
        HttpPingerEntry { url, method }: HttpPingerEntry,
        timeout: Duration,
    ) -> anyhow::Result<Self> {
        let method = Method::from_str(&method)
            .map_err(|e| anyhow::anyhow!("Invalid HTTP method: {}: {}", method, e))?;
        let url = url.trim().to_string().parse::<url::Url>()?;
        let host = url
            .host()
            .map(|h| h.to_string())
            .ok_or_else(|| anyhow::anyhow!("Invalid URL: Host is missing in {}", url))?;
        let port = match url.port_or_known_default() {
            Some(p) => p,
            None => return Err(anyhow::anyhow!("Unsupported URL scheme: {}", url.scheme())),
        };

        // TLS setup
        let mut root_cert_store = RootCertStore::empty();
        root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = ClientConfig::builder()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();

        Ok(HyperPinger {
            url,
            address: format!("{}:{}", host, port),
            method,
            timeout,
            tls_config: Arc::new(config),
        })
    }

    fn address(&self) -> &str {
        &self.address
    }

    fn url(&self) -> &url::Url {
        &self.url
    }

    fn method(&self) -> &Method {
        &self.method
    }
}
