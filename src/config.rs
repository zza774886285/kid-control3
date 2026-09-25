use std::collections::HashMap;
use anyhow::Result;
use crate::db::Database;

pub struct ConfigManager {
    db: std::sync::Arc<Database>,
}

impl ConfigManager {
    pub fn new(db: std::sync::Arc<Database>) -> Self {
        Self { db }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.db.get_config(key)
    }

    pub fn get_or(&self, key: &str, default: &str) -> String {
        self.get(key).unwrap_or_else(|| default.to_string())
    }

    pub fn get_i64(&self, key: &str, default: i64) -> i64 {
        self.get(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        self.db.delete_config(key)
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.db.set_config(key, value)
    }

    pub fn get_all(&self) -> HashMap<String, String> {
        self.db.get_all_config()
    }

    pub fn get_tablets(&self) -> HashMap<String, crate::models::TabletConfig> {
        let raw = self.get("TABLETS").unwrap_or_else(|| "{}".to_string());
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn get_daily_limit(&self) -> i64 {
        self.get_i64("DAILY_LIMIT", 60) * 60
    }

    /// 当前实际限额（含 override），用于 adjust 计算
    pub fn get_current_limit(&self, mac: &str) -> i64 {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let override_key = format!("LIMIT_OVERRIDE_{}_{}", mac.to_uppercase(), today);
        if let Some(override_val) = self.get_i64_opt(&override_key) {
            // override_val >= 0 都是有效值（0 表示用户把时间减到0了）
            return override_val;
        }
        self.get_daily_limit()
    }

    pub fn get_i64_opt(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// 按用户积分配置（优先）+ 全局默认回退
    pub fn get_points_config_for_user(&self, user_id: i64) -> serde_json::Value {
        let key = format!("POINTS_CONFIG_USER_{}", user_id);
        self.get(&key)
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_else(|| {
                serde_json::json!({
                    "tutoring": 30,
                    "homework": 30,
                    "other": 30,
                })
            })
    }

    pub fn set_points_config_for_user(&self, user_id: i64, config: &serde_json::Value) {
        let key = format!("POINTS_CONFIG_USER_{}", user_id);
        if let Ok(val) = serde_json::to_string(config) {
            let _ = self.set(&key, &val);
        }
    }
}
