use crate::{http_pinger, tcp_pinger};
use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::histogram::{Histogram, exponential_buckets_range};
use prometheus_client::registry::Registry;
use std::sync::Arc;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelValue)]
pub enum PingStatus {
    Success,
    Timeout,
    Failure,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelValue)]
pub enum FailureType {
    Dns,
    Other,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct HttpPingLabel {
    pub url: String,
    pub method: String,
    pub ip: Option<String>,
    pub status: PingStatus,
    pub status_code: Option<u32>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct HttpPingFailureLabel {
    pub url: String,
    pub method: String,
    pub failure_type: FailureType,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct TcpPingLabel {
    pub host: String,
    pub port: u32,
    pub resolved_ip: String,
    pub response: PingStatus,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct TcpPingFailureLabel {
    pub host: String,
    pub port: u32,
    pub failure_type: FailureType,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct ResolveLabel {
    pub host: String,
}

#[derive(Debug)]
pub struct PingMetrics {
    pub registry: Registry,

    // HTTP metrics - Gauge-based individual ping results
    pub http_ping_response_time_us: Family<HttpPingLabel, Histogram>,
    pub http_ping_failure: Family<HttpPingLabel, Counter>,

    // TCP metrics - Gauge-based individual ping results
    pub tcp_ping_response_time_us: Family<TcpPingLabel, Histogram>,
    pub tcp_ping_failure: Family<TcpPingLabel, Counter>,

    // DNS metrics
    pub resolve_time_us: Family<ResolveLabel, Histogram>,
    pub resolve_failure: Family<ResolveLabel, Counter>,
}

pub type SharedMetrics = Arc<PingMetrics>;

impl PingMetrics {
    fn default_histogram() -> Histogram {
        Histogram::new(exponential_buckets_range(100.0, 2e6, 20))
    }
}

impl Default for PingMetrics {
    fn default() -> Self {
        let mut registry = Registry::default();

        let http_ping_failure = Family::<HttpPingLabel, Counter>::default();
        let tcp_ping_failure = Family::<TcpPingLabel, Counter>::default();
        let resolve_failure = Family::<ResolveLabel, Counter>::default();

        let http_ping_response_time_us =
            Family::<HttpPingLabel, Histogram>::new_with_constructor(Self::default_histogram);
        let tcp_ping_response_time_us =
            Family::<TcpPingLabel, Histogram>::new_with_constructor(Self::default_histogram);
        let tcp_ping_resolve_time_us =
            Family::<TcpPingLabel, Histogram>::new_with_constructor(Self::default_histogram);
        let resolve_time_us =
            Family::<ResolveLabel, Histogram>::new_with_constructor(Self::default_histogram);

        registry.register(
            "http_ping_failure",
            "Failure number of HTTP ping requests",
            http_ping_failure.clone(),
        );
        registry.register(
            "tcp_ping_failure",
            "Failure number of TCP ping requests",
            tcp_ping_failure.clone(),
        );
        registry.register(
            "http_ping_response_time_us",
            "HTTP ping response time in us - updates with each ping",
            http_ping_response_time_us.clone(),
        );
        registry.register(
            "tcp_ping_response_time_us",
            "TCP ping response time in us - updates with each ping",
            tcp_ping_response_time_us.clone(),
        );
        registry.register(
            "tcp_ping_resolve_time_us",
            "TCP ping resolve time in us - updates with each ping",
            tcp_ping_resolve_time_us.clone(),
        );
        registry.register(
            "resolve_time_us",
            "DNS resolve time - present when DNS is timed",
            resolve_time_us.clone(),
        );
        registry.register(
            "resolve_failure",
            "DNS resolution error count - present when DNS is timed",
            resolve_failure.clone(),
        );

        Self {
            registry,
            http_ping_failure,
            http_ping_response_time_us,
            tcp_ping_response_time_us,
            tcp_ping_failure,
            resolve_time_us,
            resolve_failure,
        }
    }
}

impl PingMetrics {
    pub fn record_http_ping(&self, response: &http_pinger::PingResponse) {
        let label = HttpPingLabel::from(response.clone());

        // Record individual ping response time in us
        if let http_pinger::PingResult::Success { response_time, .. } = &response.result {
            self.http_ping_response_time_us
                .get_or_create(&label)
                .observe(response_time.as_micros() as f64);
        } else {
            // Record failure count
            self.http_ping_failure.get_or_create(&label).inc();
        }
    }

    pub fn record_tcp_ping(&self, result: &tcp_pinger::TcpPingResult) {
        let label = TcpPingLabel::from(result.clone());

        // Record duration if available - convert to us for higher precision
        if let tcp_pinger::TcpPingResponse::Success {
            established_time, ..
        } = &result.response
        {
            self.tcp_ping_response_time_us
                .get_or_create(&label)
                .observe(established_time.as_micros() as f64);
        } else {
            // Record failure count
            self.tcp_ping_failure.get_or_create(&label).inc();
        }
    }
}
impl From<http_pinger::PingResponse> for HttpPingLabel {
    fn from(response: http_pinger::PingResponse) -> Self {
        let http_pinger::PingResponse {
            url,
            ip,
            result,
            method,
            ..
        } = response;
        let response = match &result {
            http_pinger::PingResult::Success { .. } => PingStatus::Success,
            http_pinger::PingResult::Failure(_) => PingStatus::Failure,
            http_pinger::PingResult::Timeout => PingStatus::Timeout,
        };

        let status_code = match result {
            http_pinger::PingResult::Success { http_status, .. } => Some(http_status as u32),
            _ => None,
        };

        HttpPingLabel {
            url,
            method: method.to_string(),
            ip,
            status: response,
            status_code,
        }
    }
}

impl From<tcp_pinger::TcpPingResult> for TcpPingLabel {
    fn from(result: tcp_pinger::TcpPingResult) -> Self {
        let tcp_pinger::TcpPingResult {
            address: (host, port),
            resolved_ip,
            response,
            ..
        } = result;
        TcpPingLabel {
            host: String::from(host.to_str()),
            port: port.into(),
            resolved_ip: resolved_ip.to_string(),
            response: match response {
                tcp_pinger::TcpPingResponse::Success { .. } => PingStatus::Success,
                tcp_pinger::TcpPingResponse::Failure(_) => PingStatus::Failure,
                tcp_pinger::TcpPingResponse::Timeout => PingStatus::Timeout,
            },
        }
    }
}
