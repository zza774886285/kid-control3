use std::sync::Arc;
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;
use crate::ros::firewall as fw;
use crate::scheduler::collect::apply_now;

#[derive(Deserialize)]
pub struct AdjustRequest {
    pub mac: String,
    #[serde(default)]
    pub delta: i64,
    /// 直接设定今日剩余分钟数（优先于 delta）
    #[serde(default)]
    pub set_remaining_min: Option<i64>,
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
    let new_val = match req.set_remaining_min {
        Some(mins) => (usage_sec + mins.max(0) * 60).max(0),
        None => (current.max(usage_sec) + req.delta).max(0),
    };
    let base_limit = state.config.get_daily_limit();

    let result = if new_val == base_limit {
        state.config.delete(&key).map(|_| base_limit)
    } else {
        state.config
            .set(&key, &new_val.to_string())
            .map(|_| new_val)
    };

    match result {
        Ok(limit_sec) => {
            apply_now(&state.ros, &state.config, &state.db, &mac_upper).await;
            Json(json!({"ok": true, "new_limit_sec": limit_sec}))
        }
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

pub async fn switch_device(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SwitchRequest>,
) -> Json<Value> {
    let mac_upper = req.mac.to_uppercase();
    let key = format!("SWITCH_{}", mac_upper);
    let val = if req.enabled { "true" } else { "false" };
    match state.config.set(&key, val) {
        Ok(()) => {
            apply_now(&state.ros, &state.config, &state.db, &mac_upper).await;
            Json(json!({"ok": true, "blocked": !req.enabled}))
        }
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
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
            apply_now(&state.ros, &state.config, &state.db, &mac_upper).await;
            Json(json!({"ok": true}))
        }
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
