use serde_json::Value;
use tracing::{info, warn};
use std::collections::HashMap;

pub struct DnsCollector {
    mosdns_url: String,
}

impl DnsCollector {
    pub fn new(mosdns_url: &str) -> Self {
        Self { mosdns_url: mosdns_url.to_string() }
    }

    pub async fn fetch_recent_domains(&self, client: &reqwest::Client, mac: &str, ip: &str) -> Vec<String> {
        let url = format!("{}/api/v1/audit/logs?ip={}&limit=100", self.mosdns_url, ip);
        let resp = match client.get(&url).timeout(std::time::Duration::from_secs(5)).send().await {
            Ok(r) => r,
            Err(e) => {
                warn!("DNS 采集失败 ({}): {}", mac, e);
                return vec![];
            }
        };
        let data: Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => return vec![],
        };
        let logs = match data.get("logs").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => return vec![],
        };
        let mut domains = Vec::new();
        for log in logs {
            if let Some(q) = log.get("query").and_then(|v| v.as_str()) {
                let parts: Vec<&str> = q.splitn(2, ' ').collect();
                if parts.len() > 0 {
                    domains.push(parts[0].to_string());
                }
            }
        }
        domains
    }
}
