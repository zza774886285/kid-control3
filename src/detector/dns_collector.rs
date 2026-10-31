use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::warn;

/// 同一次采集周期里多台设备复用同一份 DNS 快照的时间
const SNAPSHOT_TTL: Duration = Duration::from_secs(15);

/// 每次抓取的记录条数。atomdns 的审计条数是全客户端共享的，实测 200 条只覆盖
/// 约 55 秒（比下面 1 分钟的时间窗还短，会系统性漏条），500 条覆盖约 170 秒。
const FETCH_LIMIT: usize = 500;

pub struct DnsCollector {
    pub mosdns_url: String,
    last_answers: Mutex<Vec<String>>,
    /// 共用快照：采集循环每轮会给每台设备各调一次，复用同一份响应避免重复拉取
    snapshot: Mutex<Option<(Instant, Arc<Vec<Value>>)>>,
}

impl DnsCollector {
    pub fn new(mosdns_url: &str) -> Self {
        Self {
            mosdns_url: mosdns_url.to_string(),
            last_answers: Mutex::new(Vec::new()),
            snapshot: Mutex::new(None),
        }
    }

    /// 获取上次 DNS 查询的应答 IP 列表
    pub fn get_last_answers(&self) -> Vec<String> {
        self.last_answers.lock().unwrap().clone()
    }

    /// 从 DNS 程序（atomdns）获取最近的审计日志，按 IP 过滤，返回域名列表
    pub async fn fetch_recent_domains(&self, client: &reqwest::Client, mac: &str, ip: &str) -> Vec<String> {
        let logs = self.recent_logs(client, mac).await;

        // 1. 过滤：只保留最近 1 分钟 + 匹配设备 IP
        let now = chrono::Local::now();
        let cutoff = now - chrono::Duration::minutes(1);
        let mut domains = Vec::new();
        let mut answers = Vec::new();

        for entry in logs.iter() {
            // 时间过滤
            if let Some(query_time) = entry.get("query_time").and_then(|v| v.as_str()) {
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(query_time) {
                    if ts < cutoff { continue; }
                }
            }

            // IP 过滤：Go 版 mosdns 返回 ::ffff:192.0.2.10 这种形式，atomdns 返回纯 IPv4
            let client_ip = entry.get("client_ip").and_then(|v| v.as_str()).unwrap_or("");
            let pure_ip = client_ip.strip_prefix("::ffff:").unwrap_or(client_ip);
            if pure_ip != ip { continue; }

            // 提取域名，过滤反向DNS查询噪音
            if let Some(domain) = entry.get("query_name").and_then(|v| v.as_str()) {
                let d = domain.to_lowercase();
                if d.is_empty() || domains.contains(&d) { continue; }
                if d.ends_with(".ip6.arpa") || d.ends_with(".in-addr.arpa") { continue; }
                domains.push(d);
            }

            // 提取 DNS 应答中的 IP 地址
            if let Some(ans_arr) = entry.get("answers").and_then(|v| v.as_array()) {
                for ans in ans_arr {
                    // atomdns 的 answers 是 [{"type":"A","ttl":2,"data":"1.2.3.4"}]，旧版是字符串数组
                    let ip_str = match ans.as_str() {
                        Some(s) => Some(s),
                        None => ans.get("data").and_then(|v| v.as_str()),
                    };
                    if let Some(ip_str) = ip_str {
                        let clean_ip = ip_str.strip_prefix("::ffff:").unwrap_or(ip_str).to_string();
                        if !clean_ip.is_empty() && !answers.contains(&clean_ip) {
                            answers.push(clean_ip);
                        }
                    }
                }
            }
        }

        // 缓存应答 IP 供自动发现使用
        *self.last_answers.lock().unwrap() = answers;

        domains
    }

    /// 取一份审计日志快照：`SNAPSHOT_TTL` 内复用，过期才重新拉。
    async fn recent_logs(&self, client: &reqwest::Client, mac: &str) -> Arc<Vec<Value>> {
        // 先看缓存快照（这里不能跨 await 持锁）
        if let Some(logs) = self.cached_snapshot() {
            return logs;
        }
        let Some(logs) = self.query_logs(client, mac).await else {
            return Arc::new(Vec::new());
        };
        let logs = Arc::new(logs);
        *self.snapshot.lock().unwrap() = Some((Instant::now(), Arc::clone(&logs)));
        logs
    }

    fn cached_snapshot(&self) -> Option<Arc<Vec<Value>>> {
        let guard = self.snapshot.lock().unwrap();
        match guard.as_ref() {
            Some((at, logs)) if at.elapsed() < SNAPSHOT_TTL => Some(Arc::clone(logs)),
            _ => None,
        }
    }

    /// 拉一次审计日志。失败只 warn，返回 None 让调用方按「无数据」处理。
    async fn query_logs(&self, client: &reqwest::Client, mac: &str) -> Option<Vec<Value>> {
        let url = format!("{}/api/v2/audit/logs?limit={}", self.mosdns_url, FETCH_LIMIT);
        let resp = match client.get(&url).timeout(Duration::from_secs(10)).send().await {
            Ok(r) => r,
            Err(e) => {
                warn!("DNS 采集失败 ({}): {}", mac, e);
                return None;
            }
        };
        let data: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                warn!("DNS JSON 解析失败 ({}): {}", mac, e);
                return None;
            }
        };
        // atomdns 返回 {"pagination":…,"logs":[…]}；旧版 Go mosdns 返回裸数组，两种都认
        if let Some(arr) = data.as_array() {
            return Some(arr.clone());
        }
        match data.get("logs").and_then(|v| v.as_array()) {
            Some(arr) => Some(arr.clone()),
            None => {
                warn!("DNS 响应里没有 logs 数组 ({})", mac);
                None
            }
        }
    }
}
