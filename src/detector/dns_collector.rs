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

    /// 从 DNS 服务获取最近的查询日志，按 IP 过滤，返回域名列表。
    /// 兼容两种后端：
    ///   - mosdns 全家桶：`GET /api/v1/audit/logs` → 顶层数组，query_time 为 RFC3339 字符串，answers 为字符串数组
    ///   - LocalDNS：`GET /api/v2/audit/logs` → `{"logs": [...]}`，query_time 为毫秒时间戳，answers 为 `[{"data": "..."}]`
    pub async fn fetch_recent_domains(&self, client: &reqwest::Client, mac: &str, ip: &str) -> Vec<String> {
        let logs = match self.fetch_logs(client, mac).await {
            Some(v) => v,
            None => return vec![],
        };

        // 过滤：只保留最近 1 分钟 + 匹配设备 IP
        let cutoff = chrono::Local::now() - chrono::Duration::minutes(1);
        let mut domains = Vec::new();
        let mut answers = Vec::new();

        for entry in &logs {
            // 时间过滤：兼容 RFC3339 字符串与毫秒时间戳
            let fresh = match entry.get("query_time") {
                Some(Value::String(s)) => chrono::DateTime::parse_from_rfc3339(s)
                    .map(|ts| ts >= cutoff)
                    .unwrap_or(true),
                Some(Value::Number(n)) => {
                    use chrono::TimeZone;
                    n.as_i64()
                        .and_then(|ms| chrono::Local.timestamp_millis_opt(ms).single())
                        .map(|ts| ts >= cutoff)
                        .unwrap_or(true)
                }
                _ => true,
            };
            if !fresh {
                continue;
            }

            // IP 过滤：mosdns 可能返回 ::ffff:10.1.1.97，LocalDNS 直接返回 10.1.1.97
            let client_ip = entry.get("client_ip").and_then(|v| v.as_str()).unwrap_or("");
            let pure_ip = client_ip.strip_prefix("::ffff:").unwrap_or(client_ip);
            if pure_ip != ip {
                continue;
            }

            // 提取域名，过滤反向 DNS 查询噪音
            if let Some(domain) = entry.get("query_name").and_then(|v| v.as_str()) {
                let d = domain.to_lowercase();
                if d.is_empty() || domains.contains(&d) {
                    continue;
                }
                if d.ends_with(".ip6.arpa") || d.ends_with(".in-addr.arpa") {
                    continue;
                }
                domains.push(d);
            }

            // 提取应答 IP：兼容字符串数组与 [{"data": "..."}] 两种形态
            if let Some(ans_arr) = entry.get("answers").and_then(|v| v.as_array()) {
                for ans in ans_arr {
                    let raw = match ans {
                        Value::String(s) => Some(s.as_str()),
                        Value::Object(_) => ans.get("data").and_then(|v| v.as_str()),
                        _ => None,
                    };
                    if let Some(raw) = raw {
                        let clean_ip = raw.strip_prefix("::ffff:").unwrap_or(raw);
                        if !clean_ip.is_empty() && !answers.iter().any(|a| a == clean_ip) {
                            answers.push(clean_ip.to_string());
                        }
                    }
                }
            }
        }

        // 缓存应答 IP 供自动发现使用
        *self.last_answers.lock().unwrap() = answers;

        domains
    }

    /// 先试 LocalDNS 的 v2 接口，失败则回落到 mosdns 的 v1 接口
    async fn fetch_logs(&self, client: &reqwest::Client, mac: &str) -> Option<Vec<Value>> {
        let timeout = std::time::Duration::from_secs(10);

        let v2_url = format!("{}/api/v2/audit/logs?limit=200", self.mosdns_url);
        match client.get(&v2_url).timeout(timeout).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(data) = resp.json::<Value>().await {
                    if let Some(arr) = data.get("logs").and_then(|v| v.as_array()) {
                        return Some(arr.clone());
                    }
                }
            }
            Ok(resp) => warn!("DNS 采集 v2 返回 {} ({})", resp.status(), mac),
            Err(e) => warn!("DNS 采集 v2 失败 ({}): {}", mac, e),
        }

        let v1_url = format!("{}/api/v1/audit/logs?limit=200", self.mosdns_url);
        match client.get(&v1_url).timeout(timeout).send().await {
            Ok(resp) if resp.status().is_success() => match resp.json::<Value>().await {
                Ok(Value::Array(arr)) => Some(arr),
                Ok(_) => {
                    warn!("DNS 采集 v1 返回结构非数组 ({})", mac);
                    None
                }
                Err(e) => {
                    warn!("DNS JSON 解析失败 ({}): {}", mac, e);
                    None
                }
            },
            Ok(resp) => {
                warn!("DNS 采集 v1 返回 {} ({})", resp.status(), mac);
                None
            }
            Err(e) => {
                warn!("DNS 采集 v1 失败 ({}): {}", mac, e);
                None
            }
        }
    }
}
