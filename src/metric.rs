use crate::{http_pinger, tcp_pinger};
use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub status_code: Option<String>, // Changed to String for better label encoding
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct TcpPingLabel {
    pub host: String,
    pub port: String, // Changed to String for better label encoding
    pub resolved_ip: String,
    pub newly_resolved: String, // Changed to String for better label encoding
    pub response: PingResponse,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PingRecord {
    pub timestamp: u64,
    pub url: String,
    pub response_time_ms: u64,
    pub status_code: Option<u16>,
    pub ip: Option<String>,
    pub error: Option<String>,
}

pub struct PingMetrics {
    pub registry: Registry,

    // HTTP metrics - Gauge-based individual ping results
    pub http_ping_response_time_ms: Family<HttpPingLabel, Gauge>,
    pub http_ping_total: Family<HttpPingLabel, Counter>,

    // TCP metrics - Gauge-based individual ping results
    pub tcp_ping_response_time_ms: Family<TcpPingLabel, Gauge>,
    pub tcp_ping_total: Family<TcpPingLabel, Counter>,

    // General metrics
    pub active_pings: Gauge,
    pub last_ping_timestamp: Gauge,

    // Recent ping history for detailed analysis (last 1000 pings)
    pub recent_http_pings: Mutex<VecDeque<PingRecord>>,
    pub recent_tcp_pings: Mutex<VecDeque<PingRecord>>,
}

impl Default for PingMetrics {
    fn default() -> Self {
        let mut registry = Registry::default();

        let http_ping_total = Family::<HttpPingLabel, Counter>::default();
        let tcp_ping_total = Family::<TcpPingLabel, Counter>::default();

        let http_ping_response_time_ms = Family::<HttpPingLabel, Gauge>::default();
        let tcp_ping_response_time_ms = Family::<TcpPingLabel, Gauge>::default();

        let active_pings = Gauge::default();
        let last_ping_timestamp = Gauge::default();

        // Register metrics with millisecond precision naming
        registry.register(
            "http_ping_total",
            "Total number of HTTP ping requests",
            http_ping_total.clone(),
        );
        registry.register(
            "tcp_ping_total",
            "Total number of TCP ping requests",
            tcp_ping_total.clone(),
        );
        registry.register(
            "active_pings",
            "Number of currently active ping operations",
            active_pings.clone(),
        );
        registry.register(
            "last_ping_timestamp_seconds",
            "Timestamp of the last ping operation",
            last_ping_timestamp.clone(),
        );

        // Register PRIMARY metrics for individual ping results (fine-grained data)
        registry.register(
            "http_ping_response_time_milliseconds",
            "Individual HTTP ping response time in milliseconds - updates with each ping",
            http_ping_response_time_ms.clone(),
        );
        registry.register(
            "tcp_ping_response_time_milliseconds",
            "Individual TCP ping response time in milliseconds - updates with each ping",
            tcp_ping_response_time_ms.clone(),
        );

        // Register other metrics
        registry.register(
            "http_ping_total",
            "Total number of HTTP ping requests",
            http_ping_total.clone(),
        );

        Self {
            registry,
            http_ping_total,
            http_ping_response_time_ms,
            tcp_ping_response_time_ms,
            tcp_ping_total,
            active_pings,
            last_ping_timestamp,
            recent_http_pings: Mutex::new(VecDeque::with_capacity(1000)),
            recent_tcp_pings: Mutex::new(VecDeque::with_capacity(1000)),
        }
    }
}

impl PingMetrics {
    pub fn record_http_ping(
        &self,
        response: &http_pinger::PingResponse,
        _response_size: Option<usize>,
    ) {
        let label = HttpPingLabel::from(response.clone());

        // Record individual ping response time in milliseconds
        if let http_pinger::PingResult::Success { response_time, .. } = &response.result {
            self.http_ping_response_time_ms
                .get_or_create(&label)
                .set(response_time.as_millis() as i64);
        }

        // Record total count
        self.http_ping_total.get_or_create(&label).inc();

        // Update timestamp
        self.last_ping_timestamp.set(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        );

        // Update recent ping history
        if let http_pinger::PingResult::Success {
            response_time,
            http_status,
            ..
        } = &response.result
        {
            let mut recent_pings = self.recent_http_pings.lock().unwrap();
            if recent_pings.len() == 1000 {
                recent_pings.pop_front();
            }
            recent_pings.push_back(PingRecord {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                url: response.url.clone(),
                response_time_ms: response_time.as_millis() as u64,
                status_code: Some(*http_status),
                ip: response.ip.clone(),
                error: None,
            });
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
            self.tcp_ping_response_time_ms
                .get_or_create(&label)
                .set(established_time.as_millis() as i64);
        }

        // Record total count
        self.tcp_ping_total.get_or_create(&label).inc();

        // Update timestamp
        self.last_ping_timestamp.set(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        );

        // NEW: Update recent TCP ping history
        if let tcp_pinger::TcpPingResponse::Success {
            established_time,
            resolve_time,
            ..
        } = &result.response
        {
            let mut recent_pings = self.recent_tcp_pings.lock().unwrap();
            if recent_pings.len() == 1000 {
                recent_pings.pop_front();
            }
            recent_pings.push_back(PingRecord {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                url: result.address.0.to_str().to_string(),
                response_time_ms: established_time.as_millis() as u64,
                status_code: None,
                ip: Some(result.resolved_ip.to_string()),
                error: None,
            });
        }
    }

    pub fn inc_active_pings(&self) {
        self.active_pings.inc();
    }

    pub fn dec_active_pings(&self) {
        self.active_pings.dec();
    }

    // NEW: Methods to access detailed ping history
    pub fn get_recent_http_pings(&self, limit: Option<usize>) -> Vec<PingRecord> {
        let recent_pings = self.recent_http_pings.lock().unwrap();
        let take = limit.unwrap_or(recent_pings.len());
        recent_pings.iter().rev().take(take).cloned().collect()
    }

    pub fn get_recent_tcp_pings(&self, limit: Option<usize>) -> Vec<PingRecord> {
        let recent_pings = self.recent_tcp_pings.lock().unwrap();
        let take = limit.unwrap_or(recent_pings.len());
        recent_pings.iter().rev().take(take).cloned().collect()
    }

    // NEW: Combined method for API endpoint
    pub async fn get_recent_pings(&self) -> Vec<PingRecord> {
        let mut all_pings = Vec::new();

        // Get HTTP pings
        if let Ok(http_pings) = self.recent_http_pings.try_lock() {
            all_pings.extend(http_pings.iter().cloned());
        }

        // Get TCP pings
        if let Ok(tcp_pings) = self.recent_tcp_pings.try_lock() {
            all_pings.extend(tcp_pings.iter().cloned());
        }

        // Sort by timestamp (newest first)
        all_pings.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        all_pings
    }

    // NEW: Get ping statistics for API
    pub async fn get_ping_stats(&self, url: &str, last_n: usize) -> Result<PingStats, String> {
        Ok(self.get_recent_stats(url, last_n))
    }

    // NEW: Get current response times
    pub async fn get_current_response_times(&self) -> std::collections::HashMap<String, u64> {
        // This would require iterating through gauge families, which is complex
        // For now, return a simple response
        std::collections::HashMap::new()
    }

    // NEW: Get statistics from recent pings
    pub fn get_recent_stats(&self, url: &str, last_n: usize) -> PingStats {
        let recent_pings = self.recent_http_pings.lock().unwrap();
        let filtered_pings: Vec<&PingRecord> = recent_pings
            .iter()
            .filter(|ping| ping.url == url)
            .rev()
            .take(last_n)
            .collect();

        if filtered_pings.is_empty() {
            return PingStats::default();
        }

        let response_times: Vec<u64> = filtered_pings.iter().map(|p| p.response_time_ms).collect();
        let mut sorted_times = response_times.clone();
        sorted_times.sort();

        let sum: u64 = response_times.iter().sum();
        let count = response_times.len();
        let avg = sum as f64 / count as f64;
        let min = *sorted_times.first().unwrap_or(&0);
        let max = *sorted_times.last().unwrap_or(&0);

        // Calculate percentiles
        let p50_idx = count / 2;
        let p95_idx = (count as f64 * 0.95) as usize;
        let p99_idx = (count as f64 * 0.99) as usize;

        PingStats {
            count,
            avg_ms: avg,
            min_ms: min,
            max_ms: max,
            p50_ms: sorted_times.get(p50_idx).copied().unwrap_or(0),
            p95_ms: sorted_times
                .get(p95_idx.min(count - 1))
                .copied()
                .unwrap_or(0),
            p99_ms: sorted_times
                .get(p99_idx.min(count - 1))
                .copied()
                .unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PingStats {
    pub count: usize,
    pub avg_ms: f64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
}

impl From<http_pinger::PingResponse> for HttpPingLabel {
    fn from(response: http_pinger::PingResponse) -> Self {
        let (version, status_code) = match &response.result {
            http_pinger::PingResult::Success {
                version,
                http_status,
                ..
            } => (
                Some(format!("{:?}", version)),
                Some(http_status.to_string()),
            ),
            _ => (None, None),
        };

        HttpPingLabel {
            url: response.url,
            method: "HEAD".to_string(), // Default method, should be extracted from request
            ip: response.ip,
            response: match response.result {
                http_pinger::PingResult::Success { .. } => PingResponse::Success,
                http_pinger::PingResult::Failure(_) => PingResponse::Failure,
                http_pinger::PingResult::Timeout => PingResponse::Timeout,
            },
            version,
            status_code,
        }
    }
}

impl From<tcp_pinger::TcpPingResult> for TcpPingLabel {
    fn from(result: tcp_pinger::TcpPingResult) -> Self {
        TcpPingLabel {
            host: result.address.0.to_str().to_string(),
            port: result.address.1.to_string(),
            resolved_ip: result.resolved_ip.to_string(),
            newly_resolved: result.newly_resolved.to_string(),
            response: match result.response {
                tcp_pinger::TcpPingResponse::Success { .. } => PingResponse::Success,
                tcp_pinger::TcpPingResponse::Failure(_) => PingResponse::Failure,
                tcp_pinger::TcpPingResponse::Timeout => PingResponse::Timeout,
            },
        }
    }
}
