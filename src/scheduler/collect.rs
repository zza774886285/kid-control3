use std::sync::Arc;
use std::collections::HashMap;
use tracing::{info, error, debug};
use tokio::sync::RwLock;
use crate::ros::RosClient;
use crate::ros::arp::get_mac_ip_map;
use crate::ros::firewall as fw;
use crate::ros::ipv6;
use crate::detector::activity::ActivityDetector;
use crate::detector::dns_collector::DnsCollector;
use crate::config::ConfigManager;
use crate::db::Database;

/// 防火墙状态缓存：记录每个设备当前的封禁状态，避免重复操作 ROS
static FW_STATE: once_cell::sync::Lazy<RwLock<HashMap<String, bool>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(HashMap::new()));

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
    let day_type = config.get_day_type();
    let tablets = config.get_tablets();
    let block_list = config.get("BLOCK_LIST").unwrap_or_else(|| "block-tablet".to_string());

    info!("开始管控检查 | 日期类型: {} | 时间: {}", day_type, now.format("%H:%M:%S"));

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
            continue;
        }

        // 检查开关
        let switch_key = format!("SWITCH_{}", mac_upper);
        let switch_enabled = config.get(&switch_key).map(|v| v == "true");

        let time_mode = config.get_time_mode(&day_type);
        let (start, end) = config.get_time_config(&day_type);
        let in_window = config.is_in_time_window(&start, &end);
        let time_block = match time_mode.as_str() {
            "all" => false,
            "allow" => !in_window,
            _ => in_window, // "block" mode
        };

        if switch_enabled == Some(false) {
            info!("  {}: 开关关闭 → 强制封禁", name);
            block(ros, &ip, &tablet.ipv6_comment, &block_list, &mac_upper).await;
            continue;
        }

        if switch_enabled == Some(true) && !time_block {
            // 先检查每日限额（含 override），即使在允许时段内也要检查
            let limit_sec = config.get_current_limit(&mac_upper, &day_type);
            if limit_sec >= 0 {
                let today = now.format("%Y-%m-%d").to_string();
                let (active_min, _, _) = db.get_daily_active_minutes(&mac_upper, &today);
                let usage_sec = active_min * 60;
                if usage_sec >= limit_sec {
                    info!("  {}: 超限 ({}s >= {}s) → 封禁", name, usage_sec, limit_sec);
                    block(ros, &ip, &tablet.ipv6_comment, &block_list, &mac_upper).await;
                    continue;
                }
            }
            info!("  {}: 开关开启 + {} → 放行", name, if in_window { "允许时段" } else { "非禁止时段" });
            unblock(ros, &ip, &tablet.ipv6_comment, &block_list, &mac_upper).await;
            continue;
        }

        // 检查每日限额（含 override）
        let limit_sec = config.get_current_limit(&mac_upper, &day_type);
        if limit_sec >= 0 {
            let today = now.format("%Y-%m-%d").to_string();
            let (active_min, _, _) = db.get_daily_active_minutes(&mac_upper, &today);
            let usage_sec = active_min * 60;
            if usage_sec >= limit_sec {
                info!("  {}: 超限 ({}s >= {}s) → 封禁", name, usage_sec, limit_sec);
                block(ros, &ip, &tablet.ipv6_comment, &block_list, &mac_upper).await;
                continue;
            }
        }

        if time_block {
            info!("  {}: 时间限制 → 封禁", name);
            block(ros, &ip, &tablet.ipv6_comment, &block_list, &mac_upper).await;
        } else {
            info!("  {}: 允许 → 放行", name);
            unblock(ros, &ip, &tablet.ipv6_comment, &block_list, &mac_upper).await;
        }
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
) {
    let tablets = config.get_tablets();

    // IP 已在配置中固定时跳过 ARP 查询
    let need_arp = tablets.values().any(|t| t.ip.is_empty());
    let mac_ip = if need_arp { get_mac_ip_map(ros).await } else { HashMap::new() };

    let now = chrono::Local::now().timestamp() as f64;

    for (mac, tablet) in &tablets {
        let mac_upper = mac.to_uppercase();
        let ip = if tablet.ip.is_empty() { mac_ip.get(&mac_upper).cloned().unwrap_or_default() } else { tablet.ip.clone() };
        if ip.is_empty() { continue; }

        let dns_domains = dns_collector.fetch_recent_domains(&http_client, &mac_upper, &ip).await;
        let (bytes_total, conn_count) = crate::ros::arp::get_connection_detail_by_ip(ros, &ip).await;
        let bytes_delta = detector.calc_bytes_delta(&mac_upper, bytes_total);

        let result = detector.detect_and_record(
            &mac_upper, &ip, now, dns_domains, conn_count, bytes_delta, db,
        );
        debug!("数据采集 {} ({}) → {} (bytes={}, conns={})", tablet.name, mac_upper, result.status, result.bytes_delta, result.conn_count);
    }
}
