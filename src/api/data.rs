use std::sync::Arc;
use axum::{Json, extract::State};
use serde_json::{json, Value};
use crate::AppState;

pub async fn collect_api_data(state: &AppState) -> Result<Value, String> {
    let tablets = state.config.get_tablets();
    let mac_ip = crate::ros::arp::get_mac_ip_map(&state.ros).await;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let mut devices = Vec::new();
    for (mac, tablet) in &tablets {
        let mac_upper = mac.to_uppercase();
        let ip = if tablet.ip.is_empty() { mac_ip.get(&mac_upper).cloned().unwrap_or_default() } else { tablet.ip.clone() };
        let online = !ip.is_empty();
        let (active_min, video_min, game_min) = state.db.get_daily_active_minutes(&mac_upper, &today);
        let limit_sec = state.config.get_current_limit(&mac_upper, &state.config.get_day_type());
        let usage_sec = active_min * 60;

        devices.push(json!({
            "mac": mac_upper,
            "name": tablet.name,
            "ip": ip,
            "online": online,
            "daily_active_min": active_min,
            "daily_video_min": video_min,
            "daily_game_min": game_min,
            "limit_sec": limit_sec,
            "usage_sec": usage_sec,
        }));
    }

    let config = state.config.get_all();
    // 查询所有设备今日的 video_windows
    let mut all_windows = Vec::new();
    for (mac, _) in &tablets {
        let mac_upper = mac.to_uppercase();
        let windows = state.db.get_video_windows(&mac_upper, &today);
        all_windows.extend(windows.into_iter().map(|w| {
            serde_json::to_value(w).unwrap_or(serde_json::Value::Null)
        }));
    }

    Ok(json!({
        "devices": devices,
        "config": config,
        "video_windows": all_windows,
    }))
}

pub async fn get_data(State(state): State<Arc<AppState>>) -> Json<Value> {
    match collect_api_data(&state).await {
        Ok(data) => Json(data),
        Err(e) => Json(json!({"error": e}))
    }
}

pub async fn get_weekly_stats(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = state.db.get_weekly_stats();
    let mut result = serde_json::Map::new();
    for (date, mac, active_min) in rows {
        let entry = result.entry(date).or_insert_with(|| json!({}));
        entry.as_object_mut().unwrap().insert(mac, json!(active_min));
    }
    Json(json!({"weekly": result}))
}

pub async fn get_recent_activity(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = state.db.get_recent_activity(20);
    let tablets = state.config.get_tablets();
    let mut device_map: std::collections::HashMap<String, Vec<Value>> =
        std::collections::HashMap::new();
    for (ts, mac, status, activity_type) in rows {
        let entry = device_map.entry(mac).or_insert_with(Vec::new);
        // 提取 HH:mm
        let time = ts
            .split('T')
            .nth(1)
            .and_then(|s| s.get(..5))
            .unwrap_or("")
            .to_string();
        entry.push(json!({"time": time, "status": status, "type": activity_type}));
    }
    let devices: Vec<Value> = tablets
        .iter()
        .map(|(mac, tablet)| {
            let mac_upper = mac.to_uppercase();
            let minutes = device_map.get(&mac_upper).cloned().unwrap_or_default();
            json!({
                "mac": mac_upper,
                "name": tablet.name,
                "minutes": minutes,
            })
        })
        .collect();
    Json(json!({"devices": devices}))
}

pub async fn get_video_windows(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let mac = params.get("mac").map(|s| s.as_str()).unwrap_or("");
    let date = params.get("date").map(|s| s.as_str()).unwrap_or("");
    if mac.is_empty() || date.is_empty() {
        return Json(json!({"error": "mac and date required"}));
    }
    let windows = state.db.get_video_windows(mac, date);
    Json(json!({"windows": windows}))
}
