use serde_json::Value;
use std::sync::Mutex;
use tracing::warn;

pub struct DnsCollector {
    pub mosdns_url: String,
    last_answers: Mutex<Vec<String>>,
}

impl DnsCollector {
    pub fn new(mosdns_url: &str) -> Self {
        Self {
            mosdns_url: mosdns_url.to_string(),
            last_answers: Mutex::new(Vec::new()),
        }
    }

    /// 获取上次 DNS 查询的应答 IP 列表
    pub fn get_last_answers(&self) -> Vec<String> {
        self.last_answers.lock().unwrap().clone()
    }

    /// 从 MOSDNS 获取最近的 DNS 日志，按 IP 过滤，返回域名列表
    pub async fn fetch_recent_domains(&self, client: &reqwest::Client, mac: &str, ip: &str) -> Vec<String> {
        // 1. 拉最近1000条DNS记录（MOSDNS不支持client_ip过滤）
        let url = format!("{}/api/v1/audit/logs?limit=200", self.mosdns_url);
        let resp = match client.get(&url).timeout(std::time::Duration::from_secs(10)).send().await {
            Ok(r) => r,
            Err(e) => {
                warn!("DNS 采集失败 ({}): {}", mac, e);
                return vec![];
            }
        };
        let data: Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                warn!("DNS JSON 解析失败 ({}): {}", mac, e);
                return vec![];
            }
        };
        let logs = match data.as_array() {
            Some(arr) => arr,
            None => return vec![],
        };

        // 2. 过滤：只保留最近 2 分钟 + 匹配设备 IP
        let now = chrono::Local::now();
        let cutoff = now - chrono::Duration::minutes(1);
        let mut domains = Vec::new();
        let mut answers = Vec::new();

        for entry in logs {
            // 时间过滤
            if let Some(query_time) = entry.get("query_time").and_then(|v| v.as_str()) {
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(query_time) {
                    if ts < cutoff { continue; }
                }
            }

            // IP 过滤：MOSDNS 返回 ::ffff:10.1.1.97 格式
            let client_ip = entry.get("client_ip").and_then(|v| v.as_str()).unwrap_or("");
            let pure_ip = if client_ip.starts_with("::ffff:") {
                &client_ip[7..]
            } else {
                client_ip
            };
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
                    if let Some(ip_str) = ans.as_str() {
                        let clean_ip = if ip_str.starts_with("::ffff:") {
                            ip_str[7..].to_string()
                        } else {
                            ip_str.to_string()
                        };
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
}
