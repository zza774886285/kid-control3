use std::sync::Arc;
use axum::{Json, extract::State};
use serde_json::{json, Value};
use crate::AppState;

pub async fn get_settings(State(state): State<Arc<AppState>>) -> Json<Value> {
    let config = state.config.get_all();
    Json(json!({"settings": config}))
}

pub async fn post_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Some(obj) = body.as_object() {
        for (key, val) in obj {
            let s = match val {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            if let Err(e) = state.config.set(key, &s) {
                return Json(json!({"ok": false, "error": e.to_string()}));
            }
        }
        Json(json!({"ok": true}))
    } else {
        Json(json!({"ok": false, "error": "expected object"}))
    }
}

pub async fn get_vacation(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mode = state.config.get("VACATION_MODE").unwrap_or_else(|| "false".to_string());
    Json(json!({"vacation_mode": mode == "true"}))
}

pub async fn set_vacation(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let val = if enabled { "true" } else { "false" };
    match state.config.set("VACATION_MODE", val) {
        Ok(()) => Json(json!({"ok": true, "vacation_mode": enabled})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}
