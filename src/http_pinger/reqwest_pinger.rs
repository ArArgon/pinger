use crate::config::HttpPingerEntry;
use crate::http_pinger::{AsyncHttpPinger, PingResponse, PingResult};
use crate::resolver::Resolve;
use async_trait::async_trait;
use hyper::Method;
use reqwest::redirect::Policy;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::instrument;

#[derive(Debug, Clone)]
pub(crate) struct ReqwestPinger {
    url: url::Url,
    address: String,
    method: Method,
    timeout: Duration,
    reqwest_client: reqwest::Client,
}

impl ReqwestPinger {
    #[instrument]
    async fn ping_inner(&self) -> anyhow::Result<PingResponse> {
        let builder = self
            .reqwest_client
            .request(self.method.clone(), self.url.clone());
        let begin = Instant::now();
        match builder.send().await {
            Ok(response) => {
                let response_time = begin.elapsed();
                let status = response.status();
                Ok(PingResponse {
                    url: self.url.to_string(),
                    ip: Some(response.remote_addr().unwrap().to_string()),
                    send_time: begin,
                    method: self.method.clone(),
                    result: PingResult::Success {
                        http_status: status.as_u16(),
                        response_time,
                        version: response.version(),
                    },
                })
            }
            Err(e) => Ok(self.wrap_soft_err(e, begin)),
        }
    }
}

#[async_trait]
impl AsyncHttpPinger for ReqwestPinger {
    #[instrument]
    async fn ping(&self) -> anyhow::Result<PingResponse> {
        use tokio::time::timeout;
        let task_submission_time = Instant::now();
        let result = timeout(self.timeout, self.ping_inner()).await;

        match result {
            Ok(res) => res,
            Err(_) => Ok(PingResponse {
                url: self.url.to_string(),
                ip: None,
                method: self.method.clone(),
                send_time: task_submission_time,
                result: PingResult::Timeout,
            }),
        }
    }
    fn new(
        HttpPingerEntry { url, method }: HttpPingerEntry,
        timeout: Duration,
        resolver: Arc<dyn Resolve>,
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

        let builder = reqwest::Client::builder()
            .connect_timeout(timeout)
            .pool_max_idle_per_host(0)
            .no_hickory_dns()
            .dns_resolver2(resolver as Arc<dyn reqwest::dns::Resolve>)
            .redirect(Policy::none());

        Ok(ReqwestPinger {
            url,
            address: format!("{}:{}", host, port),
            method,
            timeout,
            reqwest_client: builder.build()?,
        })
    }

    fn url(&self) -> &url::Url {
        &self.url
    }

    fn method(&self) -> &Method {
        &self.method
    }
}
