use std::collections::HashMap;
use anyhow::Result;
use tracing::info;
use chrono::{Datelike, Timelike};
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

    pub fn get_day_type(&self) -> String {
        let now = chrono::Local::now();
        let ymd_tuple = (now.year() as u16, now.month() as u8, now.day() as u8);
        let day_kind = chinese_holiday::chinese_holiday(ymd_tuple);
        let is_holiday = day_kind.is_holiday() && !day_kind.is_workday();
        let is_weekend = now.weekday() == chrono::Weekday::Sat || now.weekday() == chrono::Weekday::Sun;

        if is_holiday { "holiday".to_string() }
        else if is_weekend { "weekend".to_string() }
        else { "workday".to_string() }
    }

    pub fn get_time_mode(&self, day_type: &str) -> String {
        self.get(&format!("{}_TIME_MODE", day_type.to_uppercase()))
            .unwrap_or_else(|| {
                if day_type == "workday" { "block".to_string() }
                else { "allow".to_string() }
            })
    }

    pub fn get_time_config(&self, day_type: &str) -> (String, String) {
        let start = self.get(&format!("{}_FREE_START", day_type.to_uppercase()))
            .unwrap_or_else(|| "08:00".to_string());
        let end = self.get(&format!("{}_FREE_END", day_type.to_uppercase()))
            .unwrap_or_else(|| "21:00".to_string());
        (start, end)
    }

    /// 基础限额（不含 override），用于状态显示和比较
    pub fn get_device_limit(&self, mac: &str, day_type: &str) -> i64 {
        let key = format!("{}_LIMIT", day_type.to_uppercase());
        let default_limit = self.get_i64("DEFAULT_LIMIT", 70);
        let limit = self.get_i64(&key, default_limit);
        limit * 60 // 分钟转秒
    }

    /// 当前实际限额（含 override），用于 adjust 计算
    pub fn get_current_limit(&self, mac: &str, day_type: &str) -> i64 {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let override_key = format!("LIMIT_OVERRIDE_{}_{}", mac.to_uppercase(), today);
        if let Some(override_val) = self.get_i64_opt(&override_key) {
            // override_val >= 0 都是有效值（0 表示用户把时间减到0了）
            return override_val;
        }
        self.get_device_limit(mac, day_type)
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

    pub fn is_in_time_window(&self, start_str: &str, end_str: &str) -> bool {
        let now = chrono::Local::now();
        let parse = |s: &str| -> i64 {
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() == 2 {
                parts[0].parse::<i64>().unwrap_or(0) * 60 + parts[1].parse::<i64>().unwrap_or(0)
            } else { 0 }
        };
        let start = parse(start_str);
        let end = parse(end_str);
        let now_min = now.hour() as i64 * 60 + now.minute() as i64;
        if start > end {
            now_min >= start || now_min < end
        } else {
            start <= now_min && now_min < end
        }
    }
}
