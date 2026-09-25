use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};

// ============ 设备配置 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabletConfig {
    pub name: String,
    #[serde(default)]
    pub mac: String,
    pub ip: String,
    #[serde(default)]
    pub ipv6_comment: String,
}

// ============ 数据库行类型 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoWindow {
    pub ts: String,
    pub date: String,
    pub mac: String,
    pub ip: String,
    pub download_bytes: i64,
    pub dns_total_queries: i64,
    pub video_domains: String,
    pub video_platforms: String,
    pub video_score: i64,
    pub video_status: String,
    pub activity_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSession {
    pub id: i64,
    pub mac: String,
    pub date: String,
    pub session_start: String,
    pub session_end: String,
    pub platform: String,
    pub total_download: i64,
    pub avg_score: i64,
    pub domains: String,
    pub status: String,
}

// ============ API 响应 ============

#[derive(Debug, Serialize)]
pub struct ApiData {
    pub devices: Vec<DeviceInfo>,
    pub config: serde_json::Value,
    pub video_windows: Vec<VideoWindow>,
    pub video_stats: VideoStats,
}

#[derive(Debug, Serialize)]
pub struct DeviceInfo {
    pub mac: String,
    pub name: String,
    pub ip: String,
    pub online: bool,
    pub daily_active_min: i64,
    pub daily_video_min: i64,
    pub daily_game_min: i64,
    pub limit_sec: i64,
    pub usage_sec: i64,
}

#[derive(Debug, Serialize, Default)]
pub struct VideoStats {
    pub total_active_min: i64,
    pub total_video_min: i64,
    pub total_game_min: i64,
}

// ============ 活动检测 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainClassification {
    pub category: String,
    pub name: String,
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityResult {
    pub mac: String,
    pub ip: String,
    pub status: String,
    pub category: String,
    pub name: String,
    pub bytes_delta: i64,
    pub conn_count: i32,
    pub has_dns: bool,
    pub has_traffic: bool,
    pub has_conns: bool,
    pub signal: bool,
    pub is_silent: bool,
}

// ============ 积分系统 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub tablet_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointRequest {
    pub id: i64,
    pub user_id: i64,
    pub request_type: String,
    pub points: i64,
    pub status: String,
    pub admin_note: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointTransaction {
    pub id: i64,
    pub user_id: i64,
    pub tx_type: String,
    pub points: i64,
    pub balance_after: i64,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct PointsBalance {
    pub user_id: i64,
    pub username: String,
    pub display_name: String,
    pub balance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyQuota {
    pub id: i64,
    pub user_id: i64,
    pub week_start: String,
    pub tutoring_used: i64,
    pub homework_used: i64,
    pub other_used: i64,
    pub tutoring_count: i64,
    pub homework_count: i64,
    pub other_count: i64,
    pub tutoring_max: i64,
    pub homework_max: i64,
    pub other_max: i64,
}

// ============ 控制状态 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlStatus {
    pub mac: String,
    pub name: String,
    pub ip: String,
    pub online: bool,
    pub blocked: bool,
    pub paused: bool,
    pub switch_enabled: Option<bool>,
    pub usage_sec: i64,
    pub limit_sec: i64,
    pub reason: String,
}
