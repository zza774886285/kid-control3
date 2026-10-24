//! DPI 数据源：从 ROS-DPI 的 MySQL `access_log` 读取连接记录，
//! 按「App + 区间并集」算出设备当天的使用时长，供 db.rs 直接取用。
//!
//! 设计要点：
//! - 后台每 60 秒刷新一次缓存，db.rs 的 get_daily_active_minutes 同步读缓存（签名不变，调用方零改动）
//! - 缓存超过 STALE_SEC 未更新、或 DPI_MYSQL_URL 未配置、或查询失败 → 返回 None，调用方回退原有 video_windows 计数
//! - 「娱乐」范围由 DPI_VIDEO_APPS / DPI_GAME_APPS 环境变量控制（逗号分隔 app id），默认覆盖家里在用的

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDate};
use once_cell::sync::Lazy;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::Row;
use tracing::{info, warn};

/// 缓存超过这个秒数没用更新过，就认为 DPI 不可用，回退原逻辑
const STALE_SEC: u64 = 300;
/// 刷新间隔。取 30 秒而不是 60：access_log 有落库延迟，
/// 刷新快一点能让活动判定用上更新的数据，减少时间线抖动。
const REFRESH_SEC: u64 = 30;

struct Cache {
    /// mac(大写) -> (active_min, video_min, game_min)
    data: HashMap<String, (i64, i64, i64)>,
    /// mac(大写) -> [(start, end, app_id)]，今天全部目标 App 的原始区间。
    /// 活动判定用「这一分钟是否落在某条连接区间内」，和时长口径同源，不怕落库延迟
    spans: HashMap<String, Vec<(i64, i64, String)>>,
    ts: Option<Instant>,
}

static CACHE: Lazy<RwLock<Cache>> = Lazy::new(|| {
    RwLock::new(Cache {
        data: HashMap::new(),
        spans: HashMap::new(),
        ts: None,
    })
});

/// 同步读缓存。None 表示 DPI 数据不可用，调用方应回退到 video_windows。
pub fn cached(mac: &str) -> Option<(i64, i64, i64)> {
    let c = CACHE.read().ok()?;
    let ts = c.ts?;
    if ts.elapsed() > Duration::from_secs(STALE_SEC) {
        return None;
    }
    c.data.get(&mac.to_uppercase()).copied()
}

/// DPI 数据是否可用（缓存新鲜）。false 时调用方应回退到原来的 DNS 轮询逻辑。
pub fn available() -> bool {
    match CACHE.read() {
        Ok(c) => c
            .ts
            .map(|t| t.elapsed() <= Duration::from_secs(STALE_SEC))
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// 时刻 `at` 是否落在该设备某条目标 App 连接的区间内；落在则返回那个 app_id。
/// 多条重叠时取结束时间最晚的一条（通常是正在进行的那条长连接）。
pub fn covering(mac: &str, at: i64) -> Option<String> {
    let c = CACHE.read().ok()?;
    let fresh = c.ts.map(|t| t.elapsed() <= Duration::from_secs(STALE_SEC))?;
    if !fresh {
        return None;
    }
    c.spans
        .get(&mac.to_uppercase())?
        .iter()
        .filter(|(s, e, _)| *s <= at && at <= *e)
        .max_by_key(|(_, e, _)| *e)
        .map(|(_, _, app)| app.clone())
}

/// 当前数据源状态，供日志/调试用
pub fn status() -> (bool, usize) {
    match CACHE.read() {
        Ok(c) => {
            let fresh = c
                .ts
                .map(|t| t.elapsed() <= Duration::from_secs(STALE_SEC))
                .unwrap_or(false);
            (fresh, c.data.len())
        }
        Err(_) => (false, 0),
    }
}

fn env_list(key: &str, default: &[&str]) -> HashSet<String> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => default.iter().map(|s| s.to_string()).collect(),
    }
}

/// 区间并集总秒数（重叠只算一次）
fn union_seconds(mut iv: Vec<(i64, i64)>) -> i64 {
    if iv.is_empty() {
        return 0;
    }
    iv.sort_unstable();
    let mut total = 0i64;
    let mut cur_s = iv[0].0;
    let mut cur_e = iv[0].1;
    for &(s, e) in iv.iter().skip(1) {
        if s <= cur_e {
            if e > cur_e {
                cur_e = e;
            }
        } else {
            total += cur_e - cur_s;
            cur_s = s;
            cur_e = e;
        }
    }
    total + (cur_e - cur_s)
}

/// 合并重叠区间，并且把间隔 <= gap 秒的相邻区间也连起来。
/// 用途：游戏切场景、换服重连、短暂加载会断开几秒到一两分钟，
/// 如果只按纯并集算，玩一小时可能只记半小时；把这种小间隙并进会话内更贴近真实使用。
fn merge_with_gap(iv: Vec<(i64, i64)>, gap: i64) -> Vec<(i64, i64)> {
    if iv.is_empty() {
        return Vec::new();
    }
    let mut sorted = iv;
    sorted.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::with_capacity(sorted.len());
    for (s, e) in sorted {
        if let Some(last) = out.last_mut() {
            // 重叠 或 间隙在容忍范围内 → 合并
            if s <= last.1 + gap {
                if e > last.1 {
                    last.1 = e;
                }
                continue;
            }
        }
        out.push((s, e));
    }
    out
}

fn secs_to_min(sec: i64) -> i64 {
    // 四舍五入到分钟，避免 3599s 变成 59 分钟
    (sec + 30) / 60
}

/// 后台循环：定期把 DPI 的当天数据算成分钟数写进缓存
pub async fn run_loop() {
    let url = match std::env::var("DPI_MYSQL_URL") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => {
            info!("DPI 数据源未配置（DPI_MYSQL_URL 为空）→ 使用原有 DNS 轮询统计");
            return;
        }
    };

    let video_apps = env_list(
        "DPI_VIDEO_APPS",
        &[
            "xiaohongshu",
            "douyin",
            "kuaishou",
            "tencent_video",
            "iqiyi",
            "youku",
            "bilibili",
            "hongguo",
            "mgtv",
            "ixigua",
        ],
    );
    let game_apps = env_list(
        "DPI_GAME_APPS",
        &["game_egg", "game_preternatural", "game_miga"],
    );

    // 游戏断断续续：间隔 <= gap 秒视为同一段使用（切场景/重连不算退出）
    let gap: i64 = std::env::var("DPI_GAP_SEC")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(120);

    info!(
        "DPI 数据源已启用：视频类 {} 个 App，游戏类 {} 个 App，断档容忍 {} 秒",
        video_apps.len(),
        game_apps.len(),
        gap
    );

    let pool = match MySqlPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            warn!("DPI 数据库首次连接失败（将每 {}s 重试）：{}", REFRESH_SEC, e);
            // 不 return，交给下面的循环重试
            MySqlPoolOptions::new()
                .max_connections(1)
                .connect_lazy(&url)
                .expect("DPI_MYSQL_URL 格式不正确")
        }
    };

    loop {
        match refresh(&pool, &video_apps, &game_apps, gap).await {
            Ok(n) => info!("DPI 时长已刷新：{} 台设备", n),
            Err(e) => warn!("DPI 数据刷新失败（沿用旧缓存/回退原逻辑）：{}", e),
        }
        tokio::time::sleep(Duration::from_secs(REFRESH_SEC)).await;
    }
}

async fn refresh(
    pool: &sqlx::MySqlPool,
    video_apps: &HashSet<String>,
    game_apps: &HashSet<String>,
    gap: i64,
) -> anyhow::Result<usize> {
    let now = Local::now();
    let day_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap()
        .timestamp();
    let now_ts = now.timestamp();

    // 一次把今天所有娱乐类 App 的连接捞出来，按 host 分组算并集
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT host, app, ts_start, ts_end FROM access_log WHERE ts_start >= ",
    );
    qb.push_bind(day_start);
    qb.push(" AND ts_start <= ");
    qb.push_bind(now_ts);
    qb.push(" AND app IN (");
    {
        let mut sep = qb.separated(", ");
        for a in video_apps.iter().chain(game_apps.iter()) {
            sep.push_bind(a.clone());
        }
    }
    qb.push(")");
    let rows = qb.build().fetch_all(pool).await?;

    let mut per_host: HashMap<String, (Vec<(i64, i64)>, Vec<(i64, i64)>, Vec<(i64, i64)>)> =
        HashMap::new();
    let mut spans: HashMap<String, Vec<(i64, i64, String)>> = HashMap::new();

    for row in rows {
        let host: String = row.try_get("host")?;
        let app: String = row.try_get("app")?;
        let s: i64 = row.try_get("ts_start")?;
        let e: i64 = row.try_get("ts_end")?;

        let is_video = video_apps.contains(&app);
        let is_game = game_apps.contains(&app);
        if !is_video && !is_game {
            continue;
        }
        // 与 DPI 面板口径一致：单条连接至少按 60 秒计
        let e = e.max(s + 60);
        let mac_key = host.to_uppercase();
        // 原始区间（不合并），供活动判定按「覆盖」判断
        spans.entry(mac_key.clone()).or_default().push((s, e, app.clone()));
        let entry = per_host
            .entry(mac_key)
            .or_insert_with(|| (Vec::new(), Vec::new(), Vec::new()));
        entry.0.push((s, e)); // 全部娱乐
        if is_video {
            entry.1.push((s, e));
        }
        if is_game {
            entry.2.push((s, e));
        }
    }

    let mut data = HashMap::new();
    for (mac, (all, video, game)) in per_host {
        // 先按 gap 合并断档，再算总时长
        let active = secs_to_min(union_seconds(merge_with_gap(all, gap)));
        let v = secs_to_min(union_seconds(merge_with_gap(video, gap)));
        let g = secs_to_min(union_seconds(merge_with_gap(game, gap)));
        if active > 0 {
            data.insert(mac, (active, v, g));
        }
    }
    let n = data.len();

    let mut c = CACHE.write().map_err(|_| anyhow::anyhow!("缓存锁中毒"))?;
    c.data = data;
    c.spans = spans;
    c.ts = Some(Instant::now());
    Ok(n)
}

/// 供测试：直接塞一份数据进缓存
#[cfg(test)]
pub fn inject_for_test(mac: &str, active: i64, video: i64, game: i64) {
    if let Ok(mut c) = CACHE.write() {
        c.data.insert(mac.to_uppercase(), (active, video, game));
        c.ts = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_merges_overlap() {
        let v = vec![(0, 600), (300, 900), (1000, 1060)];
        assert_eq!(union_seconds(v), 900 + 60);
    }

    #[test]
    fn union_empty() {
        assert_eq!(union_seconds(vec![]), 0);
    }

    #[test]
    fn minutes_rounding() {
        assert_eq!(secs_to_min(3599), 60);
        assert_eq!(secs_to_min(30), 1);
        assert_eq!(secs_to_min(29), 0);
    }

    #[test]
    fn gap_merges_short_pauses() {
        // 玩 1 分钟、断 1 分钟、再玩 1 分钟：gap=120 应并成 3 分钟
        let iv = vec![(0i64, 60i64), (120, 180)];
        let merged = merge_with_gap(iv, 120);
        assert_eq!(merged.len(), 1);
        assert_eq!(union_seconds(merged), 180);
    }

    #[test]
    fn gap_keeps_real_breaks() {
        // 断开 10 分钟，不能被并进来
        let iv = vec![(0i64, 60i64), (660, 720)];
        let merged = merge_with_gap(iv, 120);
        assert_eq!(merged.len(), 2);
        assert_eq!(union_seconds(merged), 120);
    }

    #[test]
    fn gap_handles_overlap_and_nesting() {
        let iv = vec![(0i64, 300i64), (60, 120), (280, 400)];
        let merged = merge_with_gap(iv, 0);
        assert_eq!(merged, vec![(0, 400)]);
    }
}
