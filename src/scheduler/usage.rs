//! 有效用量计算：扣除「大人模式」（PAUSE_<MAC>=true）期间的时长。
//!
//! 背景：时间线判定会因大人模式强制记 IDLE，但额度用量走 DPI 的连接级
//! 区间并集（`dpi_source::cached`），那条路径不认 PAUSE，导致暂停期间
//! 用量照涨。这里在用量出口处做冻结/扣减：
//!
//! - 进入暂停：记下当时用量作为基线，暂停期间一律返回基线（冻结）
//! - 退出暂停：把「当前全量 - 基线」累加进扣除额，之后按 全量 - 扣除额 返回
//!
//! 两个键都按日期隔离：`PAUSE_BASE_<MAC>_<DATE>`、`PAUSE_DEDUCT_<MAC>_<DATE>`。

use crate::config::ConfigManager;
use crate::db::Database;

/// 用量三元组（分钟）：(活跃, 视频, 游戏)
type Usage = (i64, i64, i64);

fn base_key(mac: &str, date: &str) -> String {
    format!("PAUSE_BASE_{}_{}", mac, date)
}

fn deduct_key(mac: &str, date: &str) -> String {
    format!("PAUSE_DEDUCT_{}_{}", mac, date)
}

fn read_base(config: &ConfigManager, mac: &str, date: &str) -> Option<Usage> {
    let raw = config.get(&base_key(mac, date))?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some((
        v.get("a")?.as_i64()?,
        v.get("v").and_then(|x| x.as_i64()).unwrap_or(0),
        v.get("g").and_then(|x| x.as_i64()).unwrap_or(0),
    ))
}

fn write_base(config: &ConfigManager, mac: &str, date: &str, u: Usage) {
    let s = serde_json::json!({"a": u.0, "v": u.1, "g": u.2}).to_string();
    let _ = config.set(&base_key(mac, date), &s);
}

fn is_paused(config: &ConfigManager, mac: &str) -> bool {
    config
        .get(&format!("PAUSE_{}", mac))
        .map(|v| v == "true")
        .unwrap_or(false)
}

/// 全量用量扣掉累计扣除额；分项不得超过总量，避免出现「视频 88 分 > 总时长 60 分」
fn apply_deduct(u: Usage, deduct: i64) -> Usage {
    let a = (u.0 - deduct).max(0);
    (a, u.1.min(a), u.2.min(a))
}

/// 取某设备某天的有效用量（大人模式期间已冻结/扣除）。
///
/// 只在 `date` 为今天时才有 DPI 路径的偏差问题；历史日期回退 video_windows，
/// 暂停期间本来就记 IDLE，不做额外处理。
pub fn effective_minutes(
    db: &Database,
    config: &ConfigManager,
    mac: &str,
    date: &str,
) -> Usage {
    let mac = &mac.to_uppercase();
    let raw = db.get_daily_active_minutes(mac, date);
    let paused = is_paused(config, mac);

    if paused {
        // 暂停中：有基线就冻结在基线（基线存的是「有效用量」，已含历史扣除），
        // 没基线就立刻按当前有效用量记一个
        return match read_base(config, mac, date) {
            Some(base) => base,
            None => {
                let base = apply_deduct(raw, config.get_i64(&deduct_key(mac, date), 0));
                write_base(config, mac, date, base);
                base
            }
        };
    }

    // 不在暂停中：若存在未结算的基线，先结算。
    // 基线是有效值，所以新扣除额 = 当前全量 - 基线（不是累加，基线已含历史扣除）
    if let Some(base) = read_base(config, mac, date) {
        let deduct = (raw.0 - base.0).max(0);
        let _ = config.set(&deduct_key(mac, date), &deduct.to_string());
        let _ = config.delete(&base_key(mac, date));
        return apply_deduct(raw, deduct);
    }

    let deduct = config.get_i64(&deduct_key(mac, date), 0);
    if deduct > 0 {
        return apply_deduct(raw, deduct);
    }
    raw
}

#[cfg(test)]
mod tests {
    use super::{apply_deduct, effective_minutes};
    use crate::config::ConfigManager;
    use crate::db::Database;
    use std::sync::Arc;

    const MAC: &str = "AA:BB:CC:DD:EE:01";

    fn setup(tag: &str) -> (Arc<Database>, ConfigManager) {
        let path = std::env::temp_dir().join(format!("kc3-usage-test-{}.db", tag));
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
        }
        let db = Arc::new(Database::new(path.to_str().expect("tmp path")).expect("打开测试数据库"));
        let config = ConfigManager::new(db.clone());
        (db, config)
    }

    /// 插入 n 条 ACTIVE 窗口（同一分钟内只算 1 分钟，所以 ts 按分钟递增）
    fn add_active_minutes(db: &Database, date: &str, start_hour: u32, n: u32) {
        for i in 0..n {
            let ts = format!("{}T{:02}:{:02}:00", date, start_hour, i);
            db.insert_video_window(&ts, date, MAC, "10.1.1.97", 0, 1, "", "", 1, "ACTIVE", "tencent_video")
                .expect("插入窗口");
        }
    }

    #[test]
    fn deduct_reduces_total_and_caps_parts() {
        let u = (120, 88, 13);
        let (a, v, g) = apply_deduct(u, 30);
        assert_eq!(a, 90);
        assert!(v <= a && g <= a);
    }

    #[test]
    fn deduct_never_goes_negative() {
        assert_eq!(apply_deduct((10, 5, 5), 100).0, 0);
    }

    /// 暂停期间用量冻结：基线之后新增的活动不计入
    #[test]
    fn paused_freezes_usage() {
        let (db, config) = setup("freeze");
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        add_active_minutes(&db, &today, 10, 5);

        config.set(&format!("PAUSE_{}", MAC), "true").unwrap();
        assert_eq!(effective_minutes(&db, &config, MAC, &today).0, 5);

        // 暂停期间又玩了 7 分钟
        add_active_minutes(&db, &today, 11, 7);
        assert_eq!(effective_minutes(&db, &config, MAC, &today).0, 5);
    }

    /// 退出暂停后，暂停期间那段被扣掉，不跳涨
    #[test]
    fn unpause_deducts_paused_span() {
        let (db, config) = setup("deduct");
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        add_active_minutes(&db, &today, 10, 5);
        config.set(&format!("PAUSE_{}", MAC), "true").unwrap();
        assert_eq!(effective_minutes(&db, &config, MAC, &today).0, 5);

        add_active_minutes(&db, &today, 11, 7); // 暂停期间 7 分钟
        config.set(&format!("PAUSE_{}", MAC), "false").unwrap();
        assert_eq!(effective_minutes(&db, &config, MAC, &today).0, 5);

        // 恢复后正常计时
        add_active_minutes(&db, &today, 12, 3);
        assert_eq!(effective_minutes(&db, &config, MAC, &today).0, 8);
    }

    /// 一天内多次开关也能正确累计
    #[test]
    fn multiple_pause_cycles_accumulate() {
        let (db, config) = setup("cycles");
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        add_active_minutes(&db, &today, 10, 2);

        for (pause_hour, paused_min) in [(11u32, 4u32), (13, 6)] {
            config.set(&format!("PAUSE_{}", MAC), "true").unwrap();
            assert_eq!(effective_minutes(&db, &config, MAC, &today).0, 2);
            add_active_minutes(&db, &today, pause_hour, paused_min);
            config.set(&format!("PAUSE_{}", MAC), "false").unwrap();
            assert_eq!(effective_minutes(&db, &config, MAC, &today).0, 2);
        }
    }
}
