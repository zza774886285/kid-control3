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
    /// mac(大写) -> [(分钟时间戳, app_id)]，最近若干分钟的目标 App 分钟行。
    /// 判定用 traffic_minute 而不是 access_log：后者要等连接结束才落库，
    /// 长连接进行中查不到，会整段判成 IDLE；分钟表只落后约 1 分钟
    minutes: HashMap<String, Vec<(i64, String)>>,
    ts: Option<Instant>,
}

static CACHE: Lazy<RwLock<Cache>> = Lazy::new(|| {
    RwLock::new(Cache {
        data: HashMap::new(),
        minutes: HashMap::new(),
        ts: None,
    })
});

/// 同步读缓存。None 表示 DPI 缓存不可用；缓存新鲜但该设备无匹配连接时，今日用量为 0。
pub fn cached(mac: &str) -> Option<(i64, i64, i64)> {
    let c = CACHE.read().ok()?;
    let ts = c.ts?;
    if ts.elapsed() > Duration::from_secs(STALE_SEC) {
        return None;
    }
    Some(cached_usage(&c.data, mac))
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

/// 诊断用：该设备缓存里有多少条分钟行
pub fn span_count(mac: &str) -> usize {
    CACHE
        .read()
        .ok()
        .and_then(|c| c.minutes.get(&mac.to_uppercase()).map(|v| v.len()))
        .unwrap_or(0)
}

/// 时刻 `at` 前多少秒内有过目标 App 的分钟行，就返回那个 app_id。
/// 窗口取 180 秒：traffic_minute 落后约 1 分钟，再加上采集与刷新错位。
pub fn active_at(mac: &str, at: i64) -> Option<String> {
    let c = CACHE.read().ok()?;
    let fresh = c.ts.map(|t| t.elapsed() <= Duration::from_secs(STALE_SEC))?;
    if !fresh {
        return None;
    }
    c.minutes
        .get(&mac.to_uppercase())?
        .iter()
        .filter(|(m, _)| *m <= at + 60 && at - *m <= 180)
        .max_by_key(|(m, _)| *m)
        .map(|(_, app)| app.clone())
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

/// 只过滤单条、短于 60 秒且没有相邻目标 App 连接的孤立记录。
/// 由多条连接组成的会话不滤，且不看流量字节。
fn filter_isolated_short_intervals(
    iv: Vec<(i64, i64, String)>,
    gap: i64,
) -> Vec<(i64, i64, String)> {
    if iv.is_empty() {
        return Vec::new();
    }

    let mut sorted = iv;
    sorted.sort_unstable();
    let mut kept = Vec::with_capacity(sorted.len());
    let mut session: Vec<(i64, i64, String)> = Vec::new();
    let mut session_end = i64::MIN;

    for interval @ (s, e, _) in sorted {
        if !session.is_empty() && s > session_end + gap {
            if session.len() > 1 || session[0].1 - session[0].0 >= 60 {
                kept.append(&mut session);
            } else {
                session.clear();
            }
            session_end = e;
        } else {
            session_end = session_end.max(e);
        }
        session.push(interval);
    }

    if session.len() > 1 || session[0].1 - session[0].0 >= 60 {
        kept.extend(session);
    }
    kept
}

fn cached_usage(data: &HashMap<String, (i64, i64, i64)>, mac: &str) -> (i64, i64, i64) {
    data.get(&mac.to_uppercase()).copied().unwrap_or((0, 0, 0))
}

/// 保留原有单条短连接至少计 60 秒的口径，但先过滤孤立零星会话。
fn credit_minimum_connection_minute(iv: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    iv.into_iter().map(|(s, e)| (s, e.max(s + 60))).collect()
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

    let mut per_host: HashMap<String, Vec<(i64, i64, String)>> = HashMap::new();

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
        // 先保留真实连接时长；通过短会话过滤后再沿用至少计 60 秒的旧口径。
        per_host
            .entry(host.to_uppercase())
            .or_default()
            .push((s, e.max(s), app));
    }

    // ---- 活动判定数据：traffic_minute 最近 15 分钟的目标 App 分钟行 ----
    // 不用 access_log：它要等连接结束才落库，长连接进行中查不到（会整段判 IDLE）
    let mut minutes: HashMap<String, Vec<(i64, String)>> = HashMap::new();
    let mut qb2 = sqlx::QueryBuilder::new("SELECT host, app, ts FROM traffic_minute WHERE ts >= ");
    qb2.push_bind(now_ts - 900);
    qb2.push(" AND app IN (");
    {
        let mut sep = qb2.separated(", ");
        for a in video_apps.iter().chain(game_apps.iter()) {
            sep.push_bind(a.clone());
        }
    }
    qb2.push(")");
    if let Ok(mrows) = qb2.build().fetch_all(pool).await {
        for row in mrows {
            let h: String = row.try_get("host").unwrap_or_default();
            let a: String = row.try_get("app").unwrap_or_default();
            let t: i64 = row.try_get("ts").unwrap_or(0);
            if !h.is_empty() && !a.is_empty() && t > 0 {
                minutes.entry(h.to_uppercase()).or_default().push((t, a));
            }
        }
    }

    let mut data = HashMap::new();
    for (mac, all) in per_host {
        // 设备不同目标 App 之间也保持连续性；小流量但持续的连接仍按时长计，不看字节数。
        let qualified = filter_isolated_short_intervals(all, gap);
        let active_intervals =
            credit_minimum_connection_minute(qualified.iter().map(|(s, e, _)| (*s, *e)).collect());
        let video_intervals = credit_minimum_connection_minute(
            qualified
                .iter()
                .filter(|(_, _, app)| video_apps.contains(app))
                .map(|(s, e, _)| (*s, *e))
                .collect(),
        );
        let game_intervals = credit_minimum_connection_minute(
            qualified
                .iter()
                .filter(|(_, _, app)| game_apps.contains(app))
                .map(|(s, e, _)| (*s, *e))
                .collect(),
        );
        // 通过门槛后，原有断档补算与区间并集逻辑保持不变。
        let active = secs_to_min(union_seconds(merge_with_gap(active_intervals, gap)));
        let v = secs_to_min(union_seconds(merge_with_gap(video_intervals, gap)));
        let g = secs_to_min(union_seconds(merge_with_gap(game_intervals, gap)));
        if active > 0 {
            data.insert(mac, (active, v, g));
        }
    }
    let n = data.len();

    let mut c = CACHE.write().map_err(|_| anyhow::anyhow!("缓存锁中毒"))?;
    c.data = data;
    c.minutes = minutes;
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
    fn short_isolated_session_is_removed() {
        let filtered = filter_isolated_short_intervals(vec![(0, 10, "xiaohongshu".into())], 300);
        assert!(filtered.is_empty());
    }

    #[test]
    fn continuous_session_uses_real_time_to_qualify_then_keeps_gap_credit() {
        let intervals = vec![
            (0, 30, "xiaohongshu".into()),
            (120, 150, "xiaohongshu".into()),
        ];
        let filtered = filter_isolated_short_intervals(intervals.clone(), 300);
        assert_eq!(filtered, intervals);
        let credited =
            credit_minimum_connection_minute(filtered.iter().map(|(s, e, _)| (*s, *e)).collect());
        assert_eq!(union_seconds(merge_with_gap(credited, 300)), 180);
    }

    #[test]
    fn multiple_short_connections_across_apps_are_kept() {
        let intervals = vec![(0, 10, "xiaohongshu".into()), (200, 230, "game_egg".into())];
        assert_eq!(
            filter_isolated_short_intervals(intervals.clone(), 300),
            intervals
        );
    }

    #[test]
    fn low_volume_is_not_part_of_session_filter() {
        // 过滤器只收到时间区间，不读取流量字节；低流量的持续使用仍通过。
        let intervals = vec![
            (0, 35, "xiaohongshu".into()),
            (40, 65, "xiaohongshu".into()),
        ];
        assert_eq!(
            filter_isolated_short_intervals(intervals.clone(), 300),
            intervals
        );
    }

    #[test]
    fn fresh_cache_without_matching_device_is_zero_not_fallback() {
        assert_eq!(
            cached_usage(&HashMap::new(), "AA:BB:CC:DD:EE:FF"),
            (0, 0, 0)
        );
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
