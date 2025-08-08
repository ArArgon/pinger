use crate::{http_pinger, tcp_pinger};
use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelValue)]
pub enum PingResponse {
    Success,
    Timeout,
    Failure,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct HttpPingLabel {
    pub url: String,
    pub method: String,
    pub ip: Option<String>,
    pub response: PingResponse,
    pub version: Option<String>,
    pub status_code: Option<u32>, // Changed to String for better label encoding
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct TcpPingLabel {
    pub host: String,
    pub port: u32, // Changed to String for better label encoding
    pub resolved_ip: String,
    pub newly_resolved: bool,
    pub response: PingResponse,
}

pub struct PingMetrics {
    pub registry: Registry,

    // HTTP metrics - Gauge-based individual ping results
    pub http_ping_response_time_us: Family<HttpPingLabel, Gauge>,
    pub http_ping_failure: Family<HttpPingLabel, Counter>,

    // TCP metrics - Gauge-based individual ping results
    pub tcp_ping_response_time_us: Family<TcpPingLabel, Gauge>,
    pub tcp_ping_resolve_time_us: Family<TcpPingLabel, Gauge>,
    pub tcp_ping_failure: Family<TcpPingLabel, Counter>,
}

impl Default for PingMetrics {
    fn default() -> Self {
        let mut registry = Registry::default();

        let http_ping_failure = Family::<HttpPingLabel, Counter>::default();
        let tcp_ping_failure = Family::<TcpPingLabel, Counter>::default();

        let http_ping_response_time_us = Family::<HttpPingLabel, Gauge>::default();
        let tcp_ping_response_time_us = Family::<TcpPingLabel, Gauge>::default();
        let tcp_ping_resolve_time_us = Family::<TcpPingLabel, Gauge>::default();

        // Register metrics with millisecond precision naming
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

        // Register PRIMARY metrics for individual ping results (fine-grained data)
        registry.register(
            "http_ping_response_time_milliseconds",
            "Individual HTTP ping response time in milliseconds - updates with each ping",
            http_ping_response_time_us.clone(),
        );
        registry.register(
            "tcp_ping_response_time_milliseconds",
            "Individual TCP ping response time in milliseconds - updates with each ping",
            tcp_ping_response_time_us.clone(),
        );
        registry.register(
            "tcp_ping_resolve_time_milliseconds",
            "Individual TCP ping resolve time in milliseconds - updates with each ping",
            tcp_ping_resolve_time_us.clone(),
        );

        // Register other metrics
        registry.register(
            "http_ping_failure",
            "Failure number of HTTP ping requests",
            http_ping_failure.clone(),
        );

        Self {
            registry,
            http_ping_failure,
            http_ping_response_time_us,
            tcp_ping_response_time_us,
            tcp_ping_resolve_time_us,
            tcp_ping_failure,
        }
    }
}

impl PingMetrics {
    pub fn record_http_ping(&self, response: &http_pinger::PingResponse) {
        let label = HttpPingLabel::from(response.clone());

        // Record individual ping response time in milliseconds
        if let http_pinger::PingResult::Success { response_time, .. } = &response.result {
            self.http_ping_response_time_us
                .get_or_create(&label)
                .set(response_time.as_micros() as i64);
        } else {
            // Record failure count
            self.http_ping_failure.get_or_create(&label).inc();
        }
    }

    pub fn record_tcp_ping(&self, result: &tcp_pinger::TcpPingResult) {
        let label = TcpPingLabel::from(result.clone());

        // Record duration if available - convert to milliseconds for higher precision
        if let tcp_pinger::TcpPingResponse::Success {
            established_time,
            resolve_time,
            ..
        } = &result.response
        {
            // NEW: Record current TCP response time
            self.tcp_ping_response_time_us
                .get_or_create(&label)
                .set(established_time.as_micros() as i64);
            if let Some(resolve_time) = resolve_time {
                self.tcp_ping_resolve_time_us
                    .get_or_create(&label)
                    .set(resolve_time.as_micros() as i64);
            }
        } else {
            // Record failure count
            self.tcp_ping_failure.get_or_create(&label).inc();
        }
    }

    // NEW: Get current response times
    pub async fn get_current_response_times(&self) -> std::collections::HashMap<String, u64> {
        // This would require iterating through gauge families, which is complex
        // For now, return a simple response
        std::collections::HashMap::new()
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
            http_pinger::PingResult::Success { .. } => PingResponse::Success,
            http_pinger::PingResult::Failure(_) => PingResponse::Failure,
            http_pinger::PingResult::Timeout => PingResponse::Timeout,
        };

        let (version, status_code) = match result {
            http_pinger::PingResult::Success {
                version,
                http_status,
                ..
            } => (Some(format!("{:?}", version)), Some(http_status as u32)),
            _ => (None, None),
        };

        HttpPingLabel {
            url,
            method: method.to_string(),
            ip,
            response,
            version,
            status_code,
        }
    }
}

impl From<tcp_pinger::TcpPingResult> for TcpPingLabel {
    fn from(result: tcp_pinger::TcpPingResult) -> Self {
        let tcp_pinger::TcpPingResult {
            address: (host, port),
            resolved_ip,
            newly_resolved,
            response,
            ..
        } = result;
        TcpPingLabel {
            host: String::from(host.to_str()),
            port: port.into(),
            resolved_ip: resolved_ip.to_string(),
            newly_resolved,
            response: match response {
                tcp_pinger::TcpPingResponse::Success { .. } => PingResponse::Success,
                tcp_pinger::TcpPingResponse::Failure(_) => PingResponse::Failure,
                tcp_pinger::TcpPingResponse::Timeout => PingResponse::Timeout,
            },
        }
    }
}
