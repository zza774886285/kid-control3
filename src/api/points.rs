use std::sync::Arc;
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{json, Value};
use crate::AppState;

#[derive(Deserialize)]
pub struct EarnPointsRequest {
    pub user_id: i64,
    pub request_type: String,
    pub points: i64,
    pub admin_note: Option<String>,
}

#[derive(Deserialize)]
pub struct ApproveRequest {
    pub request_id: i64,
    pub action: String,
    pub note: Option<String>,
}

#[derive(Deserialize)]
pub struct ExchangeRequest {
    pub user_id: i64,
    pub points: i64,
}

pub async fn get_points_balance(State(state): State<Arc<AppState>>) -> Json<Value> {
    let users = state.db.get_all_users();
    let balances: Vec<Value> = users.iter().map(|u| {
        let balance = state.db.get_user_points_balance(u.id);
        json!({
            "user_id": u.id,
            "username": u.username,
            "display_name": u.display_name,
            "balance": balance,
        })
    }).collect();
    Json(json!({"balances": balances}))
}

pub async fn earn_points(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EarnPointsRequest>,
) -> Json<Value> {
    let conn = state.db.get_conn();
    conn.execute(
        "INSERT INTO point_requests (user_id, request_type, points) VALUES (?1, ?2, ?3)",
        rusqlite::params![req.user_id, req.request_type, req.points],
    ).unwrap();
    Json(json!({"ok": true, "message": "积分申请已提交"}))
}

pub async fn get_pending_requests(State(state): State<Arc<AppState>>) -> Json<Value> {
    let requests = state.db.get_pending_requests();
    Json(json!({"requests": requests}))
}

pub async fn approve_request(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApproveRequest>,
) -> Json<Value> {
    if req.action != "approve" && req.action != "reject" {
        return Json(json!({"ok": false, "error": "无效操作"}));
    }
    state.db.approve_request(req.request_id, &req.action, req.note.as_deref());
    Json(json!({"ok": true}))
}

pub async fn exchange_points(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExchangeRequest>,
) -> Json<Value> {
    let balance = state.db.get_user_points_balance(req.user_id);
    if balance < req.points {
        return Json(json!({"ok": false, "error": "积分不足"}));
    }
    let minutes = req.points;
    let user = state.db.get_user_by_id(req.user_id);
    if let Some(u) = user {
        if let Some(tablet_key) = &u.tablet_key {
            let tablets = state.config.get_tablets();
            if let Some(tablet) = tablets.get(tablet_key) {
                let mac = tablet.mac.clone();
                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                let key = format!("LIMIT_OVERRIDE_{}_{}", mac.to_uppercase(), today);
                let current: i64 = state.config.get(&key).and_then(|v| v.parse().ok()).unwrap_or(0);
                let new_val = current + minutes * 60;
                let _ = state.config.set(&key, &new_val.to_string());
                state.db.record_exchange(req.user_id, req.points, minutes, &mac);
                return Json(json!({"ok": true, "message": format!("兑换成功！获得{}分钟平板时间", minutes)}));
            }
        }
    }
    Json(json!({"ok": false, "error": "用户未绑定设备"}))
}

pub async fn get_my_points(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let user_id: i64 = params.get("user_id").and_then(|v| v.parse().ok()).unwrap_or(0);
    let balance = state.db.get_user_points_balance(user_id);
    let history = state.db.get_point_history(user_id);
    Json(json!({"balance": balance, "history": history}))
}
