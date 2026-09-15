use std::sync::Arc;
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::AppState;

// ============ Telegram Bot API helpers ============

fn get_bot_token() -> Option<String> {
    std::env::var("TELEGRAM_BOT_TOKEN").ok().filter(|v| !v.is_empty())
}

fn get_chat_id() -> Option<String> {
    std::env::var("TELEGRAM_CHAT_ID").ok().filter(|v| !v.is_empty())
}

fn build_client() -> reqwest::Client {
    let mut builder = reqwest::Client::builder();
    if let Ok(proxy_url) = std::env::var("HTTP_PROXY") {
        if !proxy_url.is_empty() {
            match reqwest::Proxy::all(&proxy_url) {
                Ok(proxy) => {
                    tracing::info!("Telegram 使用代理: {}", proxy_url);
                    builder = builder.proxy(proxy);
                }
                Err(e) => {
                    tracing::warn!("Telegram 代理配置无效 ({}): {}", proxy_url, e);
                }
            }
        }
    }
    builder.build().unwrap_or_default()
}

async fn send_message(text: &str, reply_markup: Option<Value>) -> Option<i64> {
    let token = match get_bot_token() {
        Some(t) => t,
        None => { warn!("send_message: TELEGRAM_BOT_TOKEN 未设置"); return None; }
    };
    let chat_id = match get_chat_id() {
        Some(c) => c,
        None => { warn!("send_message: TELEGRAM_CHAT_ID 未设置"); return None; }
    };
    let client = build_client();
    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);

    let mut body = json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "HTML",
    });
    if let Some(markup) = reply_markup {
        body["reply_markup"] = markup;
    }

    match client.post(&url).json(&body).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(data) => {
                if data["ok"].as_bool().unwrap_or(false) {
                    data["result"]["message_id"].as_i64()
                } else {
                    warn!("Telegram sendMessage 失败: {}", data);
                    None
                }
            }
            Err(e) => { warn!("Telegram 解析响应失败: {}", e); None }
        },
        Err(e) => { warn!("Telegram 请求失败: {}", e); None }
    }
}

async fn edit_message(message_id: i64, text: &str) -> bool {
    let token = match get_bot_token() { Some(t) => t, None => return false };
    let chat_id = match get_chat_id() { Some(c) => c, None => return false };
    let client = build_client();
    let url = format!("https://api.telegram.org/bot{}/editMessageText", token);

    let body = json!({
        "chat_id": chat_id,
        "message_id": message_id,
        "text": text,
        "parse_mode": "HTML",
    });

    match client.post(&url).json(&body).send().await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(data) => data["ok"].as_bool().unwrap_or(false),
            Err(e) => { warn!("Telegram editMessage 解析失败: {}", e); false }
        },
        Err(e) => { warn!("Telegram editMessage 请求失败: {}", e); false }
    }
}

// ============ 积分通知函数 ============

/// 发送积分申请审批请求（带 inline keyboard）
pub async fn send_approval_request(child_name: &str, request_type: &str, points: i64, request_id: i64) {
    let type_label = match request_type {
        "tutoring" => "补课",
        "homework" => "作业",
        _ => "其他",
    };
    let text = format!(
        "📋 <b>积分申请</b>\n\n孩子: {}\n类型: {}\n积分: {}\n\n请审批：",
        child_name, type_label, points
    );
    let markup = json!({
        "inline_keyboard": [[
            {"text": "✅ 同意", "callback_data": format!("approve:{}", request_id)},
            {"text": "❌ 拒绝", "callback_data": format!("reject:{}", request_id)},
        ]]
    });
    send_message(&text, Some(markup)).await;
}

/// 发送兑换成功通知
pub async fn send_exchange_notification(child_name: &str, points: i64, minutes: i64) {
    let text = format!(
        "🎮 <b>积分兑换成功</b>\n\n孩子: {}\n消耗: {} 积分\n获得: {} 分钟平板时间",
        child_name, points, minutes
    );
    send_message(&text, None).await;
}

/// 发送拒绝通知
pub async fn send_rejection_notification(child_name: &str, request_type: &str, points: i64) {
    let type_label = match request_type {
        "tutoring" => "补课",
        "homework" => "作业",
        _ => "其他",
    };
    let text = format!(
        "❌ <b>积分申请被拒绝</b>\n\n孩子: {}\n类型: {}\n积分: {}",
        child_name, type_label, points
    );
    send_message(&text, None).await;
}

// ============ Telegram 轮询 ============

/// 后台轮询 Telegram getUpdates，处理 callback_query
pub async fn poll_telegram_updates(state: Arc<AppState>) {
    let token = match get_bot_token() {
        Some(t) => t,
        None => {
            info!("TELEGRAM_BOT_TOKEN 未设置，跳过 Telegram 轮询");
            return;
        }
    };
    let chat_id = match get_chat_id() {
        Some(c) => c,
        None => {
            info!("TELEGRAM_CHAT_ID 未设置，跳过 Telegram 轮询");
            return;
        }
    };

    info!("Telegram 轮询任务启动");
    let client = build_client();
    let url = format!("https://api.telegram.org/bot{}/getUpdates", token);
    let mut offset: i64 = 0;

    loop {
        let body = json!({
            "offset": offset,
            "timeout": 0,
            "allowed_updates": ["callback_query"],
        });

        match client.post(&url).json(&body).send().await {
            Ok(resp) => match resp.json::<Value>().await {
                Ok(data) => {
                    if data["ok"].as_bool().unwrap_or(false) {
                        if let Some(updates) = data["result"].as_array() {
                            for update in updates {
                                if let Some(update_id) = update["update_id"].as_i64() {
                                    offset = update_id + 1;
                                }
                                if let Some(cb) = update.get("callback_query") {
                                    process_callback(&state, &client, &token, &chat_id, cb).await;
                                }
                            }
                        }
                    }
                }
                Err(e) => warn!("Telegram getUpdates 解析失败: {}", e),
            },
            Err(e) => warn!("Telegram getUpdates 请求失败: {}", e),
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

async fn process_callback(
    state: &AppState,
    client: &reqwest::Client,
    token: &str,
    _chat_id: &str,
    cb: &Value,
) {
    let callback_id = cb["id"].as_str().unwrap_or("");
    let message_id = cb["message"]["message_id"].as_i64().unwrap_or(0);
    let data = cb["data"].as_str().unwrap_or("");

    // 解析 callback_data: "approve:123" 或 "reject:123"
    let parts: Vec<&str> = data.splitn(2, ':').collect();
    if parts.len() != 2 {
        warn!("无效的 callback_data: {}", data);
        return;
    }
    let action = parts[0];
    let request_id: i64 = match parts[1].parse() {
        Ok(id) => id,
        Err(_) => { warn!("无效的 request_id: {}", parts[1]); return; }
    };

    // 获取申请详情（用于通知和 weekly_quota 更新）
    let request_info = state.db.get_request_by_id(request_id);

    let result_text = if action == "approve" {
        if let Some(ref req) = request_info {
            // 更新 weekly_quota
            state.db.update_weekly_quota(req.user_id, &req.request_type);
            // 审批通过（更新 status + 记录 point_transactions）
            state.db.approve_request(request_id, "approve", None);
            let type_label = match req.request_type.as_str() {
                "tutoring" => "补课",
                "homework" => "作业",
                _ => "其他",
            };
            format!("✅ 已同意 — {} {}积分", type_label, req.points)
        } else {
            state.db.approve_request(request_id, "approve", None);
            "✅ 已同意".to_string()
        }
    } else if action == "reject" {
        if let Some(ref req) = request_info {
            state.db.approve_request(request_id, "reject", Some("管理员拒绝"));
            let type_label = match req.request_type.as_str() {
                "tutoring" => "补课",
                "homework" => "作业",
                _ => "其他",
            };
            // 发送拒绝通知给管理员（群组消息已可见）
            let user = state.db.get_user_by_id(req.user_id);
            if let Some(u) = user {
                send_rejection_notification(&u.display_name, &req.request_type, req.points).await;
            }
            format!("❌ 已拒绝 — {} {}积分", type_label, req.points)
        } else {
            state.db.approve_request(request_id, "reject", Some("管理员拒绝"));
            "❌ 已拒绝".to_string()
        }
    } else {
        warn!("未知的 callback action: {}", action);
        return;
    };

    // 编辑原消息，显示结果
    if message_id > 0 {
        edit_message(message_id, &result_text).await;
    }

    // 回答 callback query（消除加载动画）
    let answer_url = format!("https://api.telegram.org/bot{}/answerCallbackQuery", token);
    let _ = client.post(&answer_url).json(&json!({
        "callback_query_id": callback_id,
    })).send().await;
}
