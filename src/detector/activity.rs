use std::collections::HashMap;
use std::sync::Mutex;
use serde_json;
use tracing::{debug, warn};
use crate::models::DomainClassification;
use super::domain_rules::classify_domains;

const DECAY_WINDOWS: i32 = 3;
const BW_THRESHOLD: i64 = 50_000;
const IDLE_TRAFFIC: i64 = 5_000;
const CONN_THRESHOLD: i32 = 2;
const SILENCE_THRESHOLD: i32 = 2;

struct DeviceState {
    idle_count: i32,
    silence_count: i32,
    last_active_ts: Option<f64>,
}

pub struct ActivityDetector {
    states: Mutex<HashMap<String, DeviceState>>,
    prev_bytes: Mutex<HashMap<String, i64>>,
    dns_cache: Mutex<HashMap<String, Vec<(f64, Vec<String>)>>>,
}

impl ActivityDetector {
    pub fn new() -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
            prev_bytes: Mutex::new(HashMap::new()),
            dns_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn calc_bytes_delta(&self, mac: &str, current: i64) -> i64 {
        let mut prev = self.prev_bytes.lock().unwrap();
        let prev_val = prev.get(mac).copied().unwrap_or(0);
        prev.insert(mac.to_string(), current);
        (current - prev_val).max(0)
    }

    pub fn add_dns(&self, mac: &str, ts: f64, domains: Vec<String>) {
        let mut cache = self.dns_cache.lock().unwrap();
        let entry = cache.entry(mac.to_string()).or_insert_with(Vec::new);
        entry.push((ts, domains));
        entry.retain(|(t, _)| *t >= ts - 60.0);
    }

    pub fn get_cached_domains(&self, mac: &str) -> Vec<String> {
        let cache = self.dns_cache.lock().unwrap();
        let mut result = Vec::new();
        if let Some(entries) = cache.get(mac) {
            for (_, domains) in entries {
                result.extend(domains.iter().cloned());
            }
        }
        result.sort();
        result.dedup();
        result
    }

    pub fn detect_and_record(&self, mac: &str, ip: &str, ts: f64,
        dns_domains: Vec<String>, conn_count: i32, bytes_delta: i64,
        db: &crate::db::Database) -> crate::models::ActivityResult
    {
        let minute_ts = format_time(ts);
        let date_str = format_date(ts);

        self.add_dns(mac, ts, dns_domains.clone());

        let cls = classify_domains(&dns_domains);
        let has_dns = cls.category != "NONE";

        let has_traffic = bytes_delta > BW_THRESHOLD;
        let has_conns = conn_count >= CONN_THRESHOLD;
        let signal = has_dns && (has_traffic || has_conns);

        let is_silent = bytes_delta < IDLE_TRAFFIC && !has_dns && conn_count < CONN_THRESHOLD;

        let mut states = self.states.lock().unwrap();
        let state = states.entry(mac.to_string()).or_insert_with(|| DeviceState {
            idle_count: 0,
            silence_count: 0,
            last_active_ts: None,
        });

        let status = if signal {
            state.idle_count = 0;
            state.silence_count = 0;
            state.last_active_ts = Some(ts);
            "ACTIVE".to_string()
        } else if is_silent {
            state.idle_count = DECAY_WINDOWS;
            state.silence_count += 1;
            if state.silence_count >= SILENCE_THRESHOLD {
                state.last_active_ts = None;
            }
            "IDLE".to_string()
        } else if state.last_active_ts.is_some() && state.idle_count < DECAY_WINDOWS {
            state.idle_count += 1;
            "ACTIVE".to_string()
        } else {
            state.idle_count = DECAY_WINDOWS;
            "IDLE".to_string()
        };

        let activity_type = if status == "ACTIVE" {
            cls.category.to_lowercase()
        } else {
            "none".to_string()
        };

        let _ = db.insert_video_window(
            &minute_ts, &date_str, mac, ip,
            bytes_delta, self.get_cached_domains(mac).len() as i64,
            &serde_json::to_string(&dns_domains.iter().take(10).collect::<Vec<_>>()).unwrap_or_else(|_| "[]".to_string()),
            &serde_json::to_string(&cls.domains).unwrap_or_else(|_| "[]".to_string()),
            if status == "ACTIVE" { 100 } else { 0 },
            &status, &activity_type,
        );

        let _ = db.upsert_session(mac, &date_str, &minute_ts, &status, &cls.name,
            &serde_json::to_string(&cls.domains).unwrap_or_else(|_| "[]".to_string()));

        crate::models::ActivityResult {
            mac: mac.to_string(),
            ip: ip.to_string(),
            status,
            category: cls.category,
            name: cls.name,
            bytes_delta,
            conn_count,
            has_dns,
            has_traffic,
            has_conns,
            signal,
            is_silent,
        }
    }
}

fn format_time(ts: f64) -> String {
    let dt = chrono::DateTime::from_timestamp(ts as i64, 0)
        .unwrap_or_default()
        .with_timezone(&chrono::Local);
    dt.format("%Y-%m-%dT%H:%M:00").to_string()
}

fn format_date(ts: f64) -> String {
    let dt = chrono::DateTime::from_timestamp(ts as i64, 0)
        .unwrap_or_default()
        .with_timezone(&chrono::Local);
    dt.format("%Y-%m-%d").to_string()
}
