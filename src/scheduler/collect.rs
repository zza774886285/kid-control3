use std::sync::Arc;
use tracing::{info, error, debug};
use crate::ros::RosClient;
use crate::ros::arp::get_mac_ip_map;
use crate::ros::firewall as fw;
use crate::ros::ipv6;
use crate::detector::activity::ActivityDetector;
use crate::detector::dns_collector::DnsCollector;
use crate::config::ConfigManager;
use crate::db::Database;

/// 统一封禁：IPv4 address-list + IPv6 filter 规则
async fn block(ros: &RosClient, ip: &str, ipv6_comment: &str, list_name: &str) {
    fw::block_ip(ros, ip, list_name).await;
    if !ipv6_comment.is_empty() {
        ipv6::block_ipv6(ros, ipv6_comment).await;
    }
}

/// 统一解封：IPv4 address-list + IPv6 filter 规则
async fn unblock(ros: &RosClient, ip: &str, ipv6_comment: &str, list_name: &str) {
    fw::unblock_ip(ros, ip, list_name).await;
    if !ipv6_comment.is_empty() {
        ipv6::unblock_ipv6(ros, ipv6_comment).await;
    }
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

    let mac_ip = get_mac_ip_map(ros).await;
    info!("ROS 连接成功，获取到 {} 条 ARP", mac_ip.len());

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .no_proxy()
        .build()
        .unwrap_or_default();

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
            unblock(ros, &ip, &tablet.ipv6_comment, &block_list).await;
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
            block(ros, &ip, &tablet.ipv6_comment, &block_list).await;
            continue;
        }

        if switch_enabled == Some(true) && !time_block {
            info!("  {}: 开关开启 + {} → 放行", name, if in_window { "允许时段" } else { "非禁止时段" });
            unblock(ros, &ip, &tablet.ipv6_comment, &block_list).await;
            continue;
        }

        // 检查每日限额（含 override）
        let limit_sec = config.get_current_limit(&mac_upper, &day_type);
        if limit_sec > 0 {
            let today = now.format("%Y-%m-%d").to_string();
            let (active_min, _, _) = db.get_daily_active_minutes(&mac_upper, &today);
            let usage_sec = active_min * 60;
            if usage_sec >= limit_sec {
                info!("  {}: 超限 ({}s >= {}s) → 封禁", name, usage_sec, limit_sec);
                block(ros, &ip, &tablet.ipv6_comment, &block_list).await;
                continue;
            }
        }

        if time_block {
            info!("  {}: 时间限制 → 封禁", name);
            block(ros, &ip, &tablet.ipv6_comment, &block_list).await;
        } else {
            info!("  {}: 允许 → 放行", name);
            unblock(ros, &ip, &tablet.ipv6_comment, &block_list).await;
        }
    }

    info!("管控执行完成");
}

pub async fn run_data_collection(
    ros: &RosClient,
    config: &ConfigManager,
    db: &Arc<Database>,
    detector: &ActivityDetector,
    dns_collector: &DnsCollector,
) {
    let tablets = config.get_tablets();
    let mac_ip = get_mac_ip_map(ros).await;

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .no_proxy()
        .build()
        .unwrap_or_default();

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
