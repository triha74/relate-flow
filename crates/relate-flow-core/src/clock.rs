use chrono::{DateTime, Duration, SecondsFormat, Utc};
use std::sync::Mutex;

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub struct FixedClock(pub DateTime<Utc>);

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

/// A clock tests can move forward. Lease expiry uses it.
pub struct ManualClock {
    now: Mutex<DateTime<Utc>>,
}

impl ManualClock {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            now: Mutex::new(now),
        }
    }

    pub fn advance(&self, by: std::time::Duration) {
        let mut guard = self.now.lock().expect("clock lock");
        *guard += Duration::from_std(by).unwrap_or_else(|_| Duration::zero());
    }
}

impl Clock for ManualClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().expect("clock lock")
    }
}

pub fn stamp(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn parse_stamp(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

pub fn add_duration(time: DateTime<Utc>, duration: std::time::Duration) -> DateTime<Utc> {
    time + Duration::from_std(duration).unwrap_or_else(|_| Duration::zero())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_is_rfc3339_utc() {
        let t = DateTime::parse_from_rfc3339("2026-11-10T22:15:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(stamp(t), "2026-11-10T22:15:00Z");
        assert_eq!(parse_stamp("2026-11-10T22:15:00Z"), Some(t));
    }
}
