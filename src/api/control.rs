use std::sync::Arc;
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;
use crate::ros::firewall as fw;

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

    let current = state.config.get_current_limit(&mac_upper);
    let usage_sec = state.db.get_daily_active_minutes(&mac_upper, &today).0 * 60;
    let new_val = (current.max(usage_sec) + req.delta).max(0);
    let base_limit = state.config.get_daily_limit();

    if new_val == base_limit {
        // 等于基础限额时删除 override
        let _ = state.config.delete(&key);
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
    // 获取设备的 ipv6_comment
    let tablets = state.config.get_tablets();
    let ipv6_comment = tablets.get(&mac_upper)
        .map(|t| t.ipv6_comment.clone())
        .unwrap_or_default();
    if req.enabled {
        // 恢复：IPv4 address-list + IPv6 filter 规则
        fw::unblock_ip(&state.ros, &ip, &block_list).await;
        if !ipv6_comment.is_empty() {
            crate::ros::ipv6::unblock_ipv6(&state.ros, &ipv6_comment).await;
        }
    } else {
        // 断网：IPv4 address-list + IPv6 filter 规则
        fw::block_ip(&state.ros, &ip, &block_list).await;
        if !ipv6_comment.is_empty() {
            crate::ros::ipv6::block_ipv6(&state.ros, &ipv6_comment).await;
        }
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
        Ok(()) => {
            // 立即生效：大人模式开启时解封，关闭时不操作（由管控周期决定）
            if req.paused {
                let mac_ip = crate::ros::arp::get_mac_ip_map(&state.ros).await;
                let tablets = state.config.get_tablets();
                let block_list = state.config.get("BLOCK_LIST").unwrap_or_else(|| "block-tablet".to_string());
                if let Some(ip) = mac_ip.get(&mac_upper) {
                    fw::unblock_ip(&state.ros, ip, &block_list).await;
                    if let Some(tablet) = tablets.get(&mac_upper) {
                        if !tablet.ipv6_comment.is_empty() {
                            crate::ros::ipv6::unblock_ipv6(&state.ros, &tablet.ipv6_comment).await;
                        }
                    }
                }
            }
            Json(json!({"ok": true}))
        },
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn get_control_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let tablets = state.config.get_tablets();
    let mac_ip = crate::ros::arp::get_mac_ip_map(&state.ros).await;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

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
        let limit_sec = state.config.get_current_limit(&mac_upper);
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
    Json(json!({"devices": statuses}))
}
