use std::sync::Arc;
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::warn;
use crate::AppState;

#[derive(Deserialize)]
pub struct ApplyPointsRequest {
    pub user_id: i64,
    pub request_type: String,
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

#[derive(Deserialize)]
pub struct SetPointsRequest {
    pub user_id: i64,
    pub points: i64,
    pub description: Option<String>,
}

/// GET /api/points/balance — 所有用户积分余额
pub async fn get_points_balance(State(state): State<Arc<AppState>>) -> Json<Value> {
    let users = state.db.get_all_users();
    let balances: Vec<Value> = users.iter().map(|u| {
        let balance = state.db.get_user_points_balance(u.id);
        let quota = state.db.get_weekly_quota(u.id);
        json!({
            "user_id": u.id,
            "username": u.username,
            "display_name": u.display_name,
            "balance": balance,
            "weekly_quota": quota,
        })
    }).collect();
    Json(json!({ "balances": balances }))
}

/// POST /api/points/apply — 孩子申请积分（检查周额度 + 发 Telegram 通知）
/// 积分值由 request_type + 设备积分配置自动确定
pub async fn apply_points(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApplyPointsRequest>,
) -> Json<Value> {
    // 校验 request_type
    if !["tutoring", "homework", "other"].contains(&req.request_type.as_str()) {
        return Json(json!({ "ok": false, "error": "无效的申请类型" }));
    }

    // 检查本周额度（每种类型每周只能申请一次）
    if state.db.is_quota_used(req.user_id, &req.request_type) {
        return Json(json!({ "ok": false, "error": "该类型本周已申请过，请下周再试" }));
    }

    // 获取用户信息
    let user = match state.db.get_user_by_id(req.user_id) {
        Some(u) => u,
        None => return Json(json!({ "ok": false, "error": "用户不存在" })),
    };

    // 从积分配置获取该类型对应的积分值
    let points = if let Some(tablet_key) = &user.tablet_key {
        let config = state.config.get_points_config(tablet_key);
        match req.request_type.as_str() {
            "tutoring" => config["tutoring"].as_i64().unwrap_or(60),
            "homework" => config["homework"].as_i64().unwrap_or(30),
            "other" => config["other"].as_i64().unwrap_or(30),
            _ => 30,
        }
    } else {
        30 // 未绑定设备时的默认值
    };

    if points <= 0 {
        return Json(json!({ "ok": false, "error": "积分配置无效" }));
    }

    // 创建 point_requests 记录
    let conn = state.db.get_conn();
    let insert_result = conn.execute(
        "INSERT INTO point_requests (user_id, request_type, points, created_at) VALUES (?1, ?2, ?3, datetime('now', '+8 hours'))",
        rusqlite::params![req.user_id, req.request_type, points],
    );
    if let Err(e) = insert_result {
        warn!("apply_points INSERT 失败: {}", e);
        return Json(json!({ "ok": false, "error": format!("申请失败: {}", e) }));
    }
    // 获取刚插入的 request_id
    let request_id: i64 = conn.query_row(
        "SELECT last_insert_rowid()",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    drop(conn); // 释放锁

    // 异步发送 Telegram 通知（fire and forget）
    let child_name = user.display_name.clone();
    let req_type = req.request_type.clone();
    tokio::spawn(async move {
        crate::telegram::send_approval_request(&child_name, &req_type, points, request_id).await;
    });

    Json(json!({ "ok": true, "message": "积分申请已提交，等待管理员审批", "request_id": request_id, "points": points }))
}

/// POST /api/points/approve — 管理员审批（HTTP 路径，备用）
pub async fn approve_request(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApproveRequest>,
) -> Json<Value> {
    if req.action != "approve" && req.action != "reject" {
        return Json(json!({ "ok": false, "error": "无效操作" }));
    }
    // 获取申请详情
    let request_info = state.db.get_request_by_id(req.request_id);
    if let Some(ref r) = request_info {
        if req.action == "approve" {
            // 更新 weekly_quota
            state.db.update_weekly_quota(r.user_id, &r.request_type);
            // 记录积分交易（earn）
            let balance = state.db.get_user_points_balance(r.user_id);
            let new_balance = balance + r.points;
            let conn = state.db.get_conn();
            let _ = conn.execute(
                "INSERT INTO point_transactions (user_id, tx_type, points, balance_after, description, request_id, created_at) \
                 VALUES (?1, 'earn', ?2, ?3, ?4, ?5, datetime('now', '+8 hours'))",
                rusqlite::params![r.user_id, r.points, new_balance, format!("{} +{}分", r.request_type, r.points), r.id],
            );
            drop(conn);
        }
    }
    state.db.approve_request(req.request_id, &req.action, req.note.as_deref());
    Json(json!({ "ok": true }))
}

/// POST /api/points/exchange — 兑换积分
pub async fn exchange_points(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ExchangeRequest>,
) -> Json<Value> {
    let balance = state.db.get_user_points_balance(req.user_id);
    if balance < req.points {
        return Json(json!({ "ok": false, "error": "积分不足" }));
    }

    // 1积分 = 1分钟
    let minutes = req.points;

    let user = match state.db.get_user_by_id(req.user_id) {
        Some(u) => u,
        None => return Json(json!({ "ok": false, "error": "用户不存在" })),
    };

    if let Some(tablet_key) = &user.tablet_key {
        let tablets = state.config.get_tablets();
        if let Some(tablet) = tablets.get(tablet_key) {
            let mac = tablet.mac.clone();
            let day_type = state.config.get_day_type();
            let current = state.config.get_current_limit(&mac.to_uppercase(), &day_type);
            let new_val = current + minutes * 60;
            let key = format!("LIMIT_OVERRIDE_{}_{}", mac.to_uppercase(),
                chrono::Local::now().format("%Y-%m-%d"));
            let _ = state.config.set(&key, &new_val.to_string());

            // 扣除积分（record_exchange 内部处理余额检查和记录）
            state.db.record_exchange(req.user_id, req.points, minutes, &mac);

            // 异步发送 Telegram 兑换通知
            let child_name = user.display_name.clone();
            tokio::spawn(async move {
                crate::telegram::send_exchange_notification(&child_name, req.points, minutes).await;
            });

            return Json(json!({
                "ok": true,
                "message": format!("兑换成功！获得 {} 分钟平板时间", minutes),
                "minutes": minutes,
            }));
        }
    }
    Json(json!({ "ok": false, "error": "用户未绑定设备" }))
}

/// GET /api/points/my — 个人积分历史
pub async fn get_my_points(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let user_id: i64 = params.get("user_id").and_then(|v| v.parse().ok()).unwrap_or(0);
    let balance = state.db.get_user_points_balance(user_id);
    let history = state.db.get_point_history(user_id);
    Json(json!({ "balance": balance, "history": history }))
}

/// GET /api/points/pending — 待审批列表
pub async fn get_pending_requests(State(state): State<Arc<AppState>>) -> Json<Value> {
    let requests = state.db.get_pending_requests();
    Json(json!({ "requests": requests }))
}

/// GET /api/points/recent — 最近交易记录
pub async fn get_recent_transactions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let transactions = state.db.get_recent_transactions();
    Json(json!({ "transactions": transactions }))
}

/// GET /api/points/config — 积分配置（前端用）
pub async fn get_points_config(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let tablet_key = params.get("tablet_key").cloned().unwrap_or_default();
    let config = state.config.get_points_config(&tablet_key);
    Json(json!({ "config": config }))
}

/// POST /api/points/set — 管理员手动设置积分
pub async fn set_points(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetPointsRequest>,
) -> Json<Value> {
    let user = match state.db.get_user_by_id(req.user_id) {
        Some(u) => u,
        None => return Json(json!({ "ok": false, "error": "用户不存在" })),
    };

    let balance = state.db.get_user_points_balance(req.user_id);
    let new_balance = balance + req.points; // 可正可负

    // 检查不能扣成负数
    if new_balance < 0 {
        return Json(json!({ "ok": false, "error": format!("积分不足，当前 {} 分，不能调整为 {} 分", balance, new_balance) }));
    }

    let desc = req.description.unwrap_or_else(|| {
        if req.points > 0 {
            format!("管理员手动增加 {} 积分", req.points)
        } else {
            format!("管理员手动扣除 {} 积分", req.points.abs())
        }
    });

    let conn = state.db.get_conn();
    let tx_type = if req.points >= 0 { "earn" } else { "spend" };
    conn.execute(
        "INSERT INTO point_transactions (user_id, tx_type, points, balance_after, description, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now', '+8 hours'))",
        rusqlite::params![req.user_id, tx_type, req.points.abs(), new_balance, desc],
    ).unwrap();
    drop(conn);

    Json(json!({
        "ok": true,
        "message": format!("已为 {} 设置积分，当前余额: {}", user.display_name, new_balance),
        "balance": new_balance,
    }))
}
