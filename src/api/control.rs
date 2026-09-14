use std::sync::Arc;
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;
use crate::ros::firewall as fw;
use crate::ros::ipv6;

#[derive(Deserialize)]
pub struct AdjustRequest {
    pub mac: String,
    pub delta: i64, // 秒
}

#[derive(Deserialize)]
pub struct SwitchRequest {
    pub mac: String,
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct PauseRequest {
    pub mac: String,
    pub paused: bool,
}

pub async fn kid_adjust(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AdjustRequest>,
) -> Json<Value> {
    let mac_upper = req.mac.to_uppercase();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let key = format!("LIMIT_OVERRIDE_{}_{}", mac_upper, today);

    let current: i64 = state.config.get(&key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let new_val = (current + req.delta).max(0);
    let day_type = state.config.get_day_type();
    if new_val == 0 {
        // 值为0时删除 override，回退到基础限额
        let _ = state.config.delete(&key);
        let base_limit = state.config.get_device_limit(&mac_upper, &day_type);
        Json(json!({"ok": true, "new_limit_sec": base_limit}))
    } else {
        match state.config.set(&key, &new_val.to_string()) {
            Ok(()) => Json(json!({"ok": true, "new_limit_sec": new_val})),
            Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
        }
    }
}

pub async fn switch_device(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SwitchRequest>,
) -> Json<Value> {
    let mac_upper = req.mac.to_uppercase();
    // 从 ARP 表获取设备 IP
    let mac_ip = crate::ros::arp::get_mac_ip_map(&state.ros).await;
    let ip = match mac_ip.get(&mac_upper) {
        Some(ip) => ip.clone(),
        None => return Json(json!({"ok": false, "error": "设备离线，无法操作"})),
    };
    let block_list = state.config.get("BLOCK_LIST").unwrap_or_else(|| "block-tablet".to_string());
    if req.enabled {
        // 恢复：从封禁列表移除
        fw::unblock_ip(&state.ros, &ip, &block_list).await;
        ipv6::set_ipv6_filter_disabled(&state.ros, &ip, false).await;
    } else {
        // 断网：加入封禁列表
        fw::block_ip(&state.ros, &ip, &block_list).await;
        ipv6::set_ipv6_filter_disabled(&state.ros, &ip, true).await;
    }
    // 同时更新配置状态
    let key = format!("SWITCH_{}", mac_upper);
    let val = if req.enabled { "true" } else { "false" };
    let _ = state.config.set(&key, val);
    Json(json!({"ok": true, "blocked": !req.enabled}))
}

pub async fn pause_device(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PauseRequest>,
) -> Json<Value> {
    let mac_upper = req.mac.to_uppercase();
    let key = format!("PAUSE_{}", mac_upper);
    let val = if req.paused { "true" } else { "false" };
    match state.config.set(&key, val) {
        Ok(()) => Json(json!({"ok": true})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn get_control_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let tablets = state.config.get_tablets();
    let mac_ip = crate::ros::arp::get_mac_ip_map(&state.ros).await;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let day_type = state.config.get_day_type();

    let mut statuses = Vec::new();
    for (mac, tablet) in &tablets {
        let mac_upper = mac.to_uppercase();
        let ip = if tablet.ip.is_empty() { mac_ip.get(&mac_upper).cloned().unwrap_or_default() } else { tablet.ip.clone() };
        let online = !ip.is_empty();
        let paused = state.config.get(&format!("PAUSE_{}", mac_upper))
            .map(|v| v == "true").unwrap_or(false);
        let switch_enabled = state.config.get(&format!("SWITCH_{}", mac_upper))
            .map(|v| v == "true");
        let (active_min, _, _) = state.db.get_daily_active_minutes(&mac_upper, &today);
        let usage_sec = active_min * 60;
        let limit_sec = state.config.get_device_limit(&mac_upper, &day_type);
        let block_list = state.config.get("BLOCK_LIST").unwrap_or_else(|| "block-tablet".to_string());
        let blocked = if online {
            let count = fw::get_address_list_count(&state.ros, &block_list, &ip).await;
            count > 0
        } else { false };

        statuses.push(json!({
            "mac": mac_upper,
            "name": tablet.name,
            "ip": ip,
            "online": online,
            "blocked": blocked,
            "paused": paused,
            "switch_enabled": switch_enabled,
            "usage_sec": usage_sec,
            "limit_sec": limit_sec,
        }));
    }
    Json(json!({"devices": statuses, "day_type": day_type}))
}
