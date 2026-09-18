use std::collections::HashSet;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use chrono::Local;

/// 单条 IP/CIDR 记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameIpEntry {
    pub addr: String,
    pub source: String,       // "manual" | "auto"
    pub label: Option<String>,
    pub first_seen: Option<String>,
}

/// 游戏 IP 注册表：自动发现 + 手动管理
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameIpRegistry {
    pub domains: Vec<String>,
    pub ips: Vec<GameIpEntry>,
}

impl GameIpRegistry {
    /// 从 JSON 文件加载，不存在则返回默认值
    pub fn load(path: &PathBuf) -> Self {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(reg) => return reg,
                    Err(e) => warn!("游戏IP表JSON解析失败: {}，使用默认值", e),
                },
                Err(e) => warn!("游戏IP表读取失败: {}，使用默认值", e),
            }
        }
        Self::default_registry()
    }

    /// 保存到 JSON 文件
    pub fn save(&self, path: &PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    warn!("游戏IP表保存失败: {}", e);
                }
            }
            Err(e) => warn!("游戏IP表序列化失败: {}", e),
        }
    }

    /// 检查一个 IP 是否匹配游戏服务器（精确匹配或 CIDR 包含）
    pub fn is_game_ip(&self, ip: &str) -> bool {
        for entry in &self.ips {
            if ip_match(ip, &entry.addr) {
                return true;
            }
        }
        false
    }

    /// 从 DNS 解析结果中自动发现新 IP
    /// domains: 本次 DNS 查询到的域名列表
    /// answers: 本次 DNS 应答中的 IP 列表
    pub fn auto_discover_from_dns(&mut self, domains: &[String], answers: &[String]) -> bool {
        let mut added = false;
        // 检查域名是否匹配已知游戏域名模式
        let is_game_domain = domains.iter().any(|d| {
            let dl = d.to_lowercase();
            self.domains.iter().any(|pattern| domain_match(&dl, pattern))
        });
        if !is_game_domain {
            return false;
        }
        let today = Local::now().format("%Y-%m-%d").to_string();
        for ip in answers {
            if ip.is_empty() || ip == "::" { continue; }
            // 已存在则跳过
            if self.ips.iter().any(|e| e.addr == *ip) { continue; }
            info!("自动发现游戏IP: {} (来自DNS)", ip);
            self.ips.push(GameIpEntry {
                addr: ip.to_string(),
                source: "auto".to_string(),
                label: None,
                first_seen: Some(today.clone()),
            });
            added = true;
        }
        added
    }

    /// 从连接表自动发现新 IP：如果设备当前被判定在玩游戏，
    /// 那么它连接的目标 IP 中未在表里的，就是候选游戏服务器
    pub fn auto_discover_from_conn(&mut self, dst_ips: &[String], is_playing: bool) -> bool {
        if !is_playing { return false; }
        let mut added = false;
        let today = Local::now().format("%Y-%m-%d").to_string();
        for ip in dst_ips {
            if ip.is_empty() { continue; }
            // 排除内网 IP
            if ip.starts_with("10.") || ip.starts_with("192.168.") || ip.starts_with("172.") {
                continue;
            }
            if self.ips.iter().any(|e| e.addr == *ip) { continue; }
            info!("自动发现游戏IP: {} (来自连接表)", ip);
            self.ips.push(GameIpEntry {
                addr: ip.to_string(),
                source: "auto".to_string(),
                label: None,
                first_seen: Some(today.clone()),
            });
            added = true;
        }
        added
    }

    /// 手动添加 IP
    pub fn add_manual(&mut self, addr: &str, label: Option<&str>) -> bool {
        if self.ips.iter().any(|e| e.addr == addr) {
            return false;
        }
        let today = Local::now().format("%Y-%m-%d").to_string();
        self.ips.push(GameIpEntry {
            addr: addr.to_string(),
            source: "manual".to_string(),
            label: label.map(|s| s.to_string()),
            first_seen: Some(today),
        });
        true
    }

    /// 删除 IP
    pub fn remove(&mut self, addr: &str) -> bool {
        let before = self.ips.len();
        self.ips.retain(|e| e.addr != addr);
        self.ips.len() < before
    }

    /// 添加域名模式
    pub fn add_domain(&mut self, domain: &str) -> bool {
        let d = domain.to_lowercase();
        if self.domains.contains(&d) { return false; }
        self.domains.push(d);
        true
    }

    /// 删除域名模式
    pub fn remove_domain(&mut self, domain: &str) -> bool {
        let d = domain.to_lowercase();
        let before = self.domains.len();
        self.domains.retain(|x| *x != d);
        self.domains.len() < before
    }

    /// 获取所有自动发现的 IP 供审核
    pub fn auto_ips(&self) -> Vec<&GameIpEntry> {
        self.ips.iter().filter(|e| e.source == "auto").collect()
    }

    /// 获取所有手动添加的 IP
    pub fn manual_ips(&self) -> Vec<&GameIpEntry> {
        self.ips.iter().filter(|e| e.source == "manual").collect()
    }

    fn default_registry() -> Self {
        Self {
            domains: vec![
                "preternatural.cn".to_string(),
                "anticheatexpert.com".to_string(),
            ],
            ips: vec![
                GameIpEntry {
                    addr: "114.117.128.0/23".to_string(),
                    source: "manual".to_string(),
                    label: Some("超自然登录服".to_string()),
                    first_seen: None,
                },
                GameIpEntry {
                    addr: "122.9.145.84".to_string(),
                    source: "manual".to_string(),
                    label: Some("超自然战斗服".to_string()),
                    first_seen: None,
                },
                GameIpEntry {
                    addr: "120.72.63.210".to_string(),
                    source: "manual".to_string(),
                    label: Some("超自然战斗服".to_string()),
                    first_seen: None,
                },
                GameIpEntry {
                    addr: "211.95.155.0/24".to_string(),
                    source: "manual".to_string(),
                    label: Some("超自然HTTPS".to_string()),
                    first_seen: None,
                },
                GameIpEntry {
                    addr: "119.188.85.103".to_string(),
                    source: "manual".to_string(),
                    label: Some("超自然HTTPS".to_string()),
                    first_seen: None,
                },
            ],
        }
    }
}

/// 精确 IP 或 CIDR 前缀匹配
fn ip_match(ip: &str, pattern: &str) -> bool {
    if pattern.contains('/') {
        // CIDR 匹配
        cidr_match(ip, pattern)
    } else {
        ip == pattern
    }
}

/// 简单 CIDR 匹配（支持 /8 /16 /23 /24 等常见掩码）
fn cidr_match(ip: &str, cidr: &str) -> bool {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 { return ip == cidr; }
    let prefix = parts[0];
    let mask_bits: u32 = match parts[1].parse() { Ok(v) => v, Err(_) => return false };
    let ip_num = match parse_ipv4(ip) { Some(v) => v, None => return false };
    let prefix_num = match parse_ipv4(prefix) { Some(v) => v, None => return false };
    if mask_bits == 0 { return true; }
    let mask = !0u32 << (32 - mask_bits);
    (ip_num & mask) == (prefix_num & mask)
}

fn parse_ipv4(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 { return None; }
    let mut result = 0u32;
    for p in parts {
        let octet: u32 = p.parse().ok()?;
        result = (result << 8) | octet;
    }
    Some(result)
}

/// 域名模式匹配：`*.preternatural.cn` 匹配 `file.preternatural.cn`
fn domain_match(domain: &str, pattern: &str) -> bool {
    let p = pattern.trim_start_matches("*.");
    domain == p || domain.ends_with(&format!(".{}", p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cidr_match() {
        assert!(cidr_match("114.117.128.5", "114.117.128.0/23"));
        assert!(cidr_match("114.117.129.50", "114.117.128.0/23"));
        assert!(!cidr_match("114.117.130.5", "114.117.128.0/23"));
        assert!(cidr_match("211.95.155.129", "211.95.155.0/24"));
        assert!(!cidr_match("211.95.156.1", "211.95.155.0/24"));
    }

    #[test]
    fn test_domain_match() {
        assert!(domain_match("file.preternatural.cn", "*.preternatural.cn"));
        assert!(domain_match("login.preternatural.cn", "*.preternatural.cn"));
        assert!(!domain_match("other.example.com", "*.preternatural.cn"));
    }

    #[test]
    fn test_registry_ops() {
        let mut reg = GameIpRegistry::default_registry();
        assert!(reg.is_game_ip("122.9.145.84"));
        assert!(reg.is_game_ip("114.117.129.50"));
        assert!(!reg.is_game_ip("8.8.8.8"));

        // 添加/删除手动 IP
        assert!(reg.add_manual("1.2.3.4", Some("test")));
        assert!(reg.is_game_ip("1.2.3.4"));
        assert!(!reg.add_manual("1.2.3.4", None)); // 重复
        assert!(reg.remove("1.2.3.4"));
        assert!(!reg.is_game_ip("1.2.3.4"));
    }
}
