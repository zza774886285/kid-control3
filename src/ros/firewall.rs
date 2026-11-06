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

pub async fn block_ip(client: &RosClient, ip: &str, list_name: &str) -> bool {
    let existing = get_address_list_count(client, list_name, ip).await;
    if existing > 0 {
        return true;
    }
    if existing < 0 {
        return false;
    }
    let body = json!({
        "list": list_name,
        "address": ip,
        "comment": "kid-control3",
    });
    let ok = client.put("/ip/firewall/address-list", &body).await.is_some();
    if ok {
        info!("IPv4 封禁 {} 加入 address-list {}", ip, list_name);
    }
    ok
}

pub async fn unblock_ip(client: &RosClient, ip: &str, list_name: &str) -> bool {
    let query = format!("/ip/firewall/address-list?list={}&address={}", list_name, ip);
    let data = match client.get(&query).await {
        Some(val) => val,
        None => return false,
    };
    let entries = match data.as_array() {
        Some(arr) => arr,
        None => return false,
    };
    let mut ok = true;
    for entry in entries {
        if let Some(entry_id) = entry.get(".id").and_then(|v| v.as_str()) {
            let delete_path = format!("/ip/firewall/address-list/{}", entry_id);
            if client.delete(&delete_path).await.is_none() {
                ok = false;
            }
        }
    }
    if ok {
        info!("IPv4 解封 {} 从 address-list {}", ip, list_name);
    }
    ok
}
