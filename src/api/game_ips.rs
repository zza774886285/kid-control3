use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::detector::ip_registry::GameIpRegistry;
use crate::AppState;

#[derive(Deserialize)]
pub struct AddIpRequest {
    pub addr: String,
    pub label: Option<String>,
}

#[derive(Deserialize)]
pub struct RemoveIpRequest {
    pub addr: String,
}

#[derive(Deserialize)]
pub struct AddDomainRequest {
    pub domain: String,
}

#[derive(Deserialize)]
pub struct RemoveDomainRequest {
    pub domain: String,
}

#[derive(Serialize)]
pub struct GameIpListResponse {
    pub domains: Vec<String>,
    pub ips: Vec<IpEntryResponse>,
    pub total: usize,
}

#[derive(Serialize)]
pub struct IpEntryResponse {
    pub addr: String,
    pub source: String,
    pub label: Option<String>,
    pub first_seen: Option<String>,
}

#[derive(Serialize)]
pub struct OpResponse {
    pub ok: bool,
    pub message: String,
}

/// GET /api/game-ips — 获取游戏 IP 注册表
pub async fn get_game_ips(
    State(state): State<Arc<AppState>>,
) -> Json<GameIpListResponse> {
    let registry = state.ip_registry.read().await;
    let ips: Vec<IpEntryResponse> = registry.ips.iter().map(|e| IpEntryResponse {
        addr: e.addr.clone(),
        source: e.source.clone(),
        label: e.label.clone(),
        first_seen: e.first_seen.clone(),
    }).collect();
    let total = ips.len();
    Json(GameIpListResponse {
        domains: registry.domains.clone(),
        ips,
        total,
    })
}

/// POST /api/game-ips/ip — 手动添加 IP
pub async fn add_game_ip(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddIpRequest>,
) -> Json<OpResponse> {
    let mut registry = state.ip_registry.write().await;
    let added = registry.add_manual(&req.addr, req.label.as_deref());
    if added {
        registry.save(&state.registry_path);
        Json(OpResponse { ok: true, message: format!("已添加: {}", req.addr) })
    } else {
        Json(OpResponse { ok: false, message: format!("已存在: {}", req.addr) })
    }
}

/// DELETE /api/game-ips/ip — 删除 IP
pub async fn remove_game_ip(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RemoveIpRequest>,
) -> Json<OpResponse> {
    let mut registry = state.ip_registry.write().await;
    let removed = registry.remove(&req.addr);
    if removed {
        registry.save(&state.registry_path);
        Json(OpResponse { ok: true, message: format!("已删除: {}", req.addr) })
    } else {
        Json(OpResponse { ok: false, message: format!("未找到: {}", req.addr) })
    }
}

/// POST /api/game-ips/domain — 添加域名模式
pub async fn add_game_domain(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddDomainRequest>,
) -> Json<OpResponse> {
    let mut registry = state.ip_registry.write().await;
    let added = registry.add_domain(&req.domain);
    if added {
        registry.save(&state.registry_path);
        Json(OpResponse { ok: true, message: format!("已添加域名: {}", req.domain) })
    } else {
        Json(OpResponse { ok: false, message: format!("已存在: {}", req.domain) })
    }
}

/// DELETE /api/game-ips/domain — 删除域名模式
pub async fn remove_game_domain(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RemoveDomainRequest>,
) -> Json<OpResponse> {
    let mut registry = state.ip_registry.write().await;
    let removed = registry.remove_domain(&req.domain);
    if removed {
        registry.save(&state.registry_path);
        Json(OpResponse { ok: true, message: format!("已删除域名: {}", req.domain) })
    } else {
        Json(OpResponse { ok: false, message: format!("未找到: {}", req.domain) })
    }
}
