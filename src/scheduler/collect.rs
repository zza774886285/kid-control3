use std::sync::Arc;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, error, debug, warn};
use tokio::sync::RwLock;
use crate::ros::RosClient;
use crate::ros::arp::get_mac_ip_map;
use crate::ros::connections::get_device_connections;
use crate::ros::firewall as fw;
use crate::ros::ipv6;
use crate::detector::activity::ActivityDetector;
use crate::detector::dns_collector::DnsCollector;
use crate::detector::ip_registry::GameIpRegistry;
use crate::config::ConfigManager;
use crate::db::Database;

/// 防火墙状态缓存：记录每个设备当前的封禁状态，避免重复操作 ROS
static FW_STATE: once_cell::sync::Lazy<RwLock<HashMap<String, bool>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(HashMap::new()));

pub async fn load_block_states(config: &ConfigManager) {
    let restored: HashMap<String, bool> = config
        .get_all()
        .into_iter()
        .filter_map(|(key, value)| {
            let mac = key.strip_prefix("BLOCK_STATE_")?;
            match value.as_str() {
                "true" => Some((mac.to_uppercase(), true)),
                "false" => Some((mac.to_uppercase(), false)),
                _ => None,
            }
        })
        .collect();

    if !restored.is_empty() {
        let mut cache = FW_STATE.write().await;
        for (mac, blocked) in restored {
            cache.insert(mac, blocked);
        }
    }
}

async fn persist_block_state(config: &ConfigManager, mac_upper: &str) {
    let blocked = {
        let cache = FW_STATE.read().await;
        cache.get(mac_upper).copied()
    };
    let Some(blocked) = blocked else {
        return;
    };
    let key = format!("BLOCK_STATE_{}", mac_upper);
    let value = if blocked { "true" } else { "false" };
    if config.get(&key).as_deref() == Some(value) {
        return;
    }
    if let Err(e) = config.set(&key, value) {
        warn!("持久化封禁状态失败 {}: {}", key, e);
    }
}

/// 统一封禁：IPv4 address-list + IPv6 filter 规则（带状态缓存）
async fn block(ros: &RosClient, ip: &str, ipv6_comment: &str, list_name: &str, cache_key: &str) {
    let mut cache = FW_STATE.write().await;
    if cache.get(cache_key) == Some(&true) {
        debug!("  已封禁，跳过: {}", cache_key);
        return;
    }
    drop(cache);
    fw::block_ip(ros, ip, list_name).await;
    if !ipv6_comment.is_empty() {
        ipv6::block_ipv6(ros, ipv6_comment).await;
    }
    FW_STATE.write().await.insert(cache_key.to_string(), true);
}

/// 统一解封：IPv4 address-list + IPv6 filter 规则（带状态缓存）
async fn unblock(ros: &RosClient, ip: &str, ipv6_comment: &str, list_name: &str, cache_key: &str) {
    let mut cache = FW_STATE.write().await;
    if cache.get(cache_key) == Some(&false) {
        debug!("  已解封，跳过: {}", cache_key);
        return;
    }
    drop(cache);
    fw::unblock_ip(ros, ip, list_name).await;
    if !ipv6_comment.is_empty() {
        ipv6::unblock_ipv6(ros, ipv6_comment).await;
    }
    FW_STATE.write().await.insert(cache_key.to_string(), false);
}

pub async fn run_control_cycle(
    ros: &RosClient,
    config: &ConfigManager,
    db: &Arc<Database>,
    detector: &ActivityDetector,
    dns_collector: &DnsCollector,
) {
    let now = chrono::Local::now();
    let tablets = config.get_tablets();
    let block_list = config.get("BLOCK_LIST").unwrap_or_else(|| "block-tablet".to_string());

    info!("开始管控检查 | 时间: {}", now.format("%H:%M:%S"));

    // 只有当某个设备 IP 为空时才查 ARP（通常 IP 已在配置中固定，跳过查询）
    let need_arp = tablets.values().any(|t| t.ip.is_empty());
    let mac_ip = if need_arp { get_mac_ip_map(ros).await } else { HashMap::new() };

    for (mac, tablet) in &tablets {
        let mac_upper = mac.to_uppercase();
        let ip = if tablet.ip.is_empty() { mac_ip.get(&mac_upper).cloned().unwrap_or_default() } else { tablet.ip.clone() };
        let name = &tablet.name;

        if ip.is_empty() {
            info!("  {} ({}) 未在线，跳过", name, mac);
            continue;
        }

        // 检查大人模式
        let paused = config.get(&format!("PAUSE_{}", mac_upper))
            .map(|v| v == "true")
            .unwrap_or(false);
        if paused {
            info!("  {}: 大人模式 → 放行", name);
            unblock(ros, &ip, &tablet.ipv6_comment, &block_list, &mac_upper).await;
            persist_block_state(config, &mac_upper).await;
            continue;
        }

        // 检查开关
        let switch_key = format!("SWITCH_{}", mac_upper);
        let switch_enabled = config.get(&switch_key).map(|v| v == "true");

        if switch_enabled == Some(false) {
            info!("  {}: 开关关闭 → 封禁", name);
            block(ros, &ip, &tablet.ipv6_comment, &block_list, &mac_upper).await;
            persist_block_state(config, &mac_upper).await;
            continue;
        }

        // 检查每日限额（含 override）
        let limit_sec = config.get_current_limit(&mac_upper);
        if limit_sec >= 0 {
            let today = now.format("%Y-%m-%d").to_string();
            let (active_min, _, _) = db.get_daily_active_minutes(&mac_upper, &today);
            let usage_sec = active_min * 60;
            if usage_sec >= limit_sec {
                info!("  {}: 超限 ({}s >= {}s) → 封禁", name, usage_sec, limit_sec);
                block(ros, &ip, &tablet.ipv6_comment, &block_list, &mac_upper).await;
                persist_block_state(config, &mac_upper).await;
                continue;
            }
        }

        info!("  {}: 放行", name);
        unblock(ros, &ip, &tablet.ipv6_comment, &block_list, &mac_upper).await;
        persist_block_state(config, &mac_upper).await;
    }

    info!("管控执行完成");
}

pub async fn run_data_collection(
    ros: &RosClient,
    config: &ConfigManager,
    db: &Arc<Database>,
    http_client: &reqwest::Client,
    detector: &ActivityDetector,
    dns_collector: &DnsCollector,
    ip_registry: &Arc<RwLock<GameIpRegistry>>,
    registry_path: &PathBuf,
) {
    let tablets = config.get_tablets();

    let now = chrono::Local::now().timestamp() as f64;

    for (mac, tablet) in &tablets {
        let mac_upper = mac.to_uppercase();
        let ip = if tablet.ip.is_empty() { continue } else { tablet.ip.clone() };
        let force_idle = {
            let state = FW_STATE.read().await;
            state.get(&mac_upper).copied().unwrap_or(false)
        };

        let dns_domains = dns_collector.fetch_recent_domains(&http_client, &mac_upper, &ip).await;

        // 用连接表替代旧的 get_connection_detail_by_ip，同时获取目标 IP 列表
        let conn_detail = get_device_connections(ros, &ip).await;
        let bytes_delta = detector.calc_bytes_delta(&mac_upper, conn_detail.total_bytes);

        // 读取 IP 注册表（只读锁，快速释放）
        let registry_snapshot = ip_registry.read().await.clone();

        let result = detector.detect_and_record(
            &mac_upper, &ip, now,
            dns_domains.clone(), conn_detail.conn_count, bytes_delta,
            &conn_detail.dst_ips, force_idle, &registry_snapshot, db,
        );

        // 自动发现：DNS 应答中的新 IP + 连接表中的新 IP
        let mut registry = ip_registry.write().await;
        let dns_answers = dns_collector.get_last_answers();
        let changed1 = registry.auto_discover_from_dns(&dns_domains, &dns_answers);
        let changed2 = registry.auto_discover_from_conn(&conn_detail.dst_ips, result.status == "ACTIVE");
        if changed1 || changed2 {
            registry.save(registry_path);
        }
        drop(registry);

        debug!("数据采集 {} ({}) → {} (bytes={}, conns={}, game_conn={})",
            tablet.name, mac_upper, result.status, result.bytes_delta, result.conn_count,
            conn_detail.dst_ips.iter().any(|ip| registry_snapshot.is_game_ip(ip)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detector::ip_registry::GameIpRegistry;

    const MAC: &str = "AA:BB:CC:DD:EE:FF";

    fn test_db(tag: &str) -> Arc<Database> {
        let path = std::env::temp_dir().join(format!("kc3-collect-test-{}.db", tag));
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
        }
        Arc::new(Database::new(path.to_str().expect("tmp path")).expect("打开测试数据库"))
    }

    /// 不存在的路径 → 默认注册表，避免测试依赖磁盘文件
    fn test_registry() -> GameIpRegistry {
        GameIpRegistry::load(&std::env::temp_dir().join("kc3-absent-registry.json"))
    }

    fn ts_at(hour: u32, minute: u32) -> f64 {
        chrono::Local::now()
            .date_naive()
            .and_hms_opt(hour, minute, 0)
            .and_then(|dt| dt.and_local_timezone(chrono::Local).single())
            .expect("构造测试时间戳")
            .timestamp() as f64
    }

    fn collect_once(
        detector: &ActivityDetector,
        db: &Database,
        ts: f64,
        dns: Vec<String>,
        force_idle: bool,
    ) -> String {
        detector
            .detect_and_record(
                MAC, "10.1.1.9", ts, dns, 5, 200_000, &[], force_idle, &test_registry(), db,
            )
            .status
    }

    /// 有信号时正常计入使用时间
    #[test]
    fn signal_counts_as_active() {
        let db = test_db("active");
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let detector = ActivityDetector::new();

        let status = collect_once(
            &detector, &db, ts_at(10, 0), vec!["www.xiaohongshu.com".to_string()], false);
        assert_eq!(status, "ACTIVE");
        assert_eq!(db.get_daily_active_minutes(MAC, &today).0, 1);
    }

    /// 封禁期间（force_idle）即使仍有 DNS 信号，也不计入使用时间
    #[test]
    fn force_idle_does_not_count_toward_usage() {
        let db = test_db("force-idle");
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let detector = ActivityDetector::new();

        let status = collect_once(
            &detector, &db, ts_at(10, 0), vec!["www.xiaohongshu.com".to_string()], true);
        assert_eq!(status, "IDLE");
        assert_eq!(db.get_daily_active_minutes(MAC, &today).0, 0);
    }

    /// 封禁期间不靠 10 周期衰减窗口继续计时；解封后无信号即为空闲
    #[test]
    fn force_idle_resets_decay_window() {
        let db = test_db("decay");
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let detector = ActivityDetector::new();
        let dns = vec!["www.xiaohongshu.com".to_string()];

        assert_eq!(collect_once(&detector, &db, ts_at(10, 0), dns.clone(), false), "ACTIVE");
        assert_eq!(collect_once(&detector, &db, ts_at(10, 1), dns, true), "IDLE");
        // 解封后没有信号：不应沿用封禁前的活跃状态
        assert_eq!(collect_once(&detector, &db, ts_at(10, 2), vec![], false), "IDLE");
        assert_eq!(db.get_daily_active_minutes(MAC, &today).0, 1);
    }
}
