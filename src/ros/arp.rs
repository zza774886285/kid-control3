use std::collections::HashMap;
use serde_json::Value;
use crate::ros::client::RosClient;

pub async fn get_mac_ip_map(client: &RosClient) -> HashMap<String, String> {
    let data = match client.get("/ip/arp").await {
        Some(val) => val,
        None => {
            tracing::error!("get_mac_ip_map: client.get 返回 None");
            return HashMap::new();
        }
    };
    tracing::debug!("get_mac_ip_map: ROS 返回类型={}, 是数组={}", data, data.is_array());
    let arr = match data.as_array() {
        Some(a) => a,
        None => {
            tracing::error!("get_mac_ip_map: 返回数据不是数组: {}", &data.to_string()[..200.min(data.to_string().len())]);
            return HashMap::new();
        }
    };
    let mut map = HashMap::new();
    for entry in arr {
        if let (Some(mac), Some(ip)) = (
            entry.get("mac-address").and_then(|v| v.as_str()),
            entry.get("address").and_then(|v| v.as_str()),
        ) {
            map.insert(mac.to_uppercase(), ip.to_string());
        }
    }
    map
}

pub async fn get_connection_detail_by_ip(client: &RosClient, ip: &str) -> (i64, i32) {
    let query = format!("/ip/connection?dst-address~={}", ip);
    let data = match client.get(&query).await {
        Some(val) => val,
        None => return (0, 0),
    };
    let arr = match data.as_array() {
        Some(a) => a,
        None => return (0, 0),
    };
    let mut total_bytes = 0i64;
    let mut conn_count = 0i32;
    for conn in arr {
        if let Some(orig) = conn.get("orig-bytes").and_then(|v| v.as_str()) {
            total_bytes += orig.parse::<i64>().unwrap_or(0);
        }
        if let Some(repl) = conn.get("repl-bytes").and_then(|v| v.as_str()) {
            total_bytes += repl.parse::<i64>().unwrap_or(0);
        }
        conn_count += 1;
    }
    (total_bytes, conn_count)
}
