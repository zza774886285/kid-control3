use serde_json::json;
use crate::ros::RosClient;
use tracing::info;

pub async fn get_address_list_count(client: &RosClient, list_name: &str, ip: &str) -> i32 {
    let query = format!("/ip/firewall/address-list?list={}&address={}", list_name, ip);
    let data = match client.get(&query).await {
        Some(val) => val,
        None => return -1,
    };
    match data.as_array() {
        Some(arr) => arr.len() as i32,
        None => -1,
    }
}

pub async fn block_ip(client: &RosClient, ip: &str, list_name: &str) {
    let body = json!({
        "list": list_name,
        "address": ip,
        "comment": "kid-control3",
    });
    client.put("/ip/firewall/address-list", &body).await;
    info!("IPv4 封禁 {} 加入 address-list {}", ip, list_name);
}

pub async fn unblock_ip(client: &RosClient, ip: &str, list_name: &str) {
    let query = format!("/ip/firewall/address-list?list={}&address={}", list_name, ip);
    let data = match client.get(&query).await {
        Some(val) => val,
        None => return,
    };
    let entries = match data.as_array() {
        Some(arr) => arr,
        None => return,
    };
    for entry in entries {
        if let Some(entry_id) = entry.get(".id").and_then(|v| v.as_str()) {
            let delete_path = format!("/ip/firewall/address-list/{}", entry_id);
            client.delete(&delete_path).await;
        }
    }
    info!("IPv4 解封 {} 从 address-list {}", ip, list_name);
}
