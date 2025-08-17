use crate::Resolve;
use crate::metric::PingMetrics;
use crate::metric::ResolveLabel;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub trait TimeReporter: Debug {
    fn report_time(&self, name: String, time: Duration, ok: bool);
}

impl TimeReporter for PingMetrics {
    fn report_time(&self, name: String, time: Duration, ok: bool) {
        let label = ResolveLabel { host: name };
        let time = time.as_micros() as f64;
        if ok {
            self.resolve_time_us.get_or_create(&label).observe(time);
        } else {
            self.resolve_failure.get_or_create(&label).inc();
        }
    }
}

#[derive(Debug)]
pub struct TimedResolver<R, T>
where
    R: Resolve + Send + Sync,
    T: TimeReporter + Send + Sync + 'static,
{
    resolver: R,
    reporter: Arc<T>,
}

impl<R: Resolve + Send + Sync, T: TimeReporter + Send + Sync> reqwest::dns::Resolve
    for TimedResolver<R, T>
{
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let str_name = String::from(name.as_str());
        let fut = self.resolver.resolve(name);
        let reporter = self.reporter.clone();

        Box::pin(async move {
            let begin = Instant::now();
            let result = fut.await;
            match &result {
                Ok(_) => reporter.report_time(str_name, begin.elapsed(), true),
                Err(_) => reporter.report_time(str_name, begin.elapsed(), false),
            }

            result
        })
    }
}

impl<R: Resolve + Send + Sync, T: TimeReporter + Send + Sync> Resolve for TimedResolver<R, T> {}

impl<R, T> TimedResolver<R, T>
where
    R: Resolve + Send + Sync,
    T: TimeReporter + Send + Sync + 'static,
{
    pub fn new(resolver: R, reporter: Arc<T>) -> Self {
        Self { resolver, reporter }
    }
}
