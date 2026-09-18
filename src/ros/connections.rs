use std::collections::HashSet;
use serde_json::Value;
use crate::ros::client::RosClient;

/// 连接详情：字节数 + 连接数 + 目标 IP 列表
pub struct ConnectionDetail {
    pub total_bytes: i64,
    pub conn_count: i32,
    pub dst_ips: Vec<String>,
}

/// 从 ROS 获取指定设备的连接详情（按 src-address 过滤）
/// 返回该设备当前所有出站连接的字节数、连接数和目标 IP 列表
pub async fn get_device_connections(client: &RosClient, ip: &str) -> ConnectionDetail {
    let query = format!("/ip/connection?src-address~={}", ip);
    let data = match client.get(&query).await {
        Some(val) => val,
        None => return ConnectionDetail { total_bytes: 0, conn_count: 0, dst_ips: vec![] },
    };
    let arr = match data.as_array() {
        Some(a) => a,
        None => return ConnectionDetail { total_bytes: 0, conn_count: 0, dst_ips: vec![] },
    };

    let mut total_bytes = 0i64;
    let mut conn_count = 0i32;
    let mut dst_ips = HashSet::new();

    for conn in arr {
        // 累计字节数
        if let Some(orig) = conn.get("orig-bytes").and_then(|v| v.as_str()) {
            total_bytes += orig.parse::<i64>().unwrap_or(0);
        }
        if let Some(repl) = conn.get("repl-bytes").and_then(|v| v.as_str()) {
            total_bytes += repl.parse::<i64>().unwrap_or(0);
        }
        conn_count += 1;

        // 提取目标 IP（dst-address 格式可能是 "IP:PORT" 或纯 IP）
        if let Some(dst) = conn.get("dst-address").and_then(|v| v.as_str()) {
            let ip_part = dst.split(':').next().unwrap_or(dst);
            if !ip_part.is_empty() {
                dst_ips.insert(ip_part.to_string());
            }
        }
    }

    let mut ips: Vec<String> = dst_ips.into_iter().collect();
    ips.sort();
    ConnectionDetail { total_bytes, conn_count, dst_ips: ips }
}
