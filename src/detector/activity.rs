use std::collections::HashMap;
use std::sync::Mutex;
use serde_json;
use tracing::{debug, warn};
use crate::models::DomainClassification;
use super::domain_rules::classify_domains;

const DECAY_WINDOWS: i32 = 10;
const BW_THRESHOLD: i64 = 50_000;
/// DPI 判定窗口（秒）：最后一条目标 App 连接结束后这么多秒内仍算 ACTIVE。
/// 取 180：DPI 的 access_log 先在内存聚合再批量落库，本身有 1–2 分钟延迟，
/// 加上缓存刷新与本采集的错位，120 秒会卡在边界导致 ACTIVE/IDLE 抖动。
const ACTIVE_WINDOW: i64 = 180;

struct DeviceState {
    idle_count: i32,
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
        dst_ips: &[String],
        force_idle: bool,
        ip_registry: &crate::detector::ip_registry::GameIpRegistry,
        db: &crate::db::Database) -> crate::models::ActivityResult
    {
        let minute_ts = format_time(ts);
        let date_str = format_date(ts);

        self.add_dns(mac, ts, dns_domains.clone());

        let cls = classify_domains(&dns_domains);
        let has_dns = !force_idle && cls.category != "NONE";

        let has_traffic = !force_idle && bytes_delta > BW_THRESHOLD;
        let has_conns = !force_idle && conn_count >= 2;
        // 连接表信号：目标 IP 匹配游戏 IP 注册表
        let has_game_conn = !force_idle && dst_ips.iter().any(|ip| ip_registry.is_game_ip(ip));
        // 双信号：DNS 命中 或 连接表命中游戏 IP
        let signal = has_dns || has_game_conn;

        let mut states = self.states.lock().unwrap();
        let state = states.entry(mac.to_string()).or_insert_with(|| DeviceState {
            idle_count: 0,
            last_active_ts: None,
        });

        // DPI 优先（B 口径）：最近一次目标 App 连接结束后 ACTIVE_WINDOW 秒内算 ACTIVE，不做衰减。
        // 窗口取 120 秒而不是 60：DPI 缓存和本采集都是 60 秒一轮，两者不同频时
        // 会读到上一轮的缓存，60 秒窗口会判不出活动，时间线出现「ACTIVE/IDLE 跳变」。
        // DPI 不可用（未配置/过期/查询失败）时 last_activity 返回 None，回落到下面的 DNS + 衰减逻辑。
        let dpi_hit = crate::detector::dpi_source::last_activity(mac);
        let dpi_active = dpi_hit
            .as_ref()
            .map(|(_, end)| ts as i64 - *end <= ACTIVE_WINDOW)
            .unwrap_or(false);

        let status = if force_idle {
            state.idle_count = DECAY_WINDOWS;
            state.last_active_ts = None;
            "IDLE".to_string()
        } else if dpi_hit.is_some() {
            // DPI 在线：只认真实连接，空档立即算 IDLE（时间线不留尾巴）
            if dpi_active {
                state.idle_count = 0;
                state.last_active_ts = Some(ts);
                "ACTIVE".to_string()
            } else {
                state.idle_count = DECAY_WINDOWS;
                state.last_active_ts = None;
                "IDLE".to_string()
            }
        } else if signal {
            state.idle_count = 0;
            state.last_active_ts = Some(ts);
            "ACTIVE".to_string()
        } else if state.last_active_ts.is_some() && state.idle_count < DECAY_WINDOWS {
            // 衰减路径：之前活跃过，衰减计数器还没满 → 继续算 ACTIVE
            state.idle_count += 1;
            "ACTIVE".to_string()
        } else {
            // 超过衰减窗口，或从未活跃过 → IDLE
            state.idle_count = DECAY_WINDOWS;
            "IDLE".to_string()
        };

        let activity_type = if status == "ACTIVE" {
            match (&dpi_hit, dpi_active) {
                (Some((app, _)), true) => app.to_lowercase(),
                _ => cls.category.to_lowercase(),
            }
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
            is_silent: false,
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
