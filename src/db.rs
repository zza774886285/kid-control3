use std::sync::Mutex;
use rusqlite::{Connection, params};
use anyhow::Result;
use tracing::info;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        let db = Self { conn: Mutex::new(conn) };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS video_windows (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts TEXT NOT NULL,
                date TEXT NOT NULL,
                mac TEXT NOT NULL,
                ip TEXT NOT NULL,
                download_bytes INTEGER DEFAULT 0,
                dns_total_queries INTEGER DEFAULT 0,
                video_domains TEXT DEFAULT '[]',
                video_platforms TEXT DEFAULT '[]',
                video_score INTEGER DEFAULT 0,
                video_status TEXT DEFAULT 'IDLE',
                activity_type TEXT DEFAULT 'none'
            );
            CREATE INDEX IF NOT EXISTS idx_vw_mac_date ON video_windows(mac, date);
            CREATE INDEX IF NOT EXISTS idx_vw_ts ON video_windows(ts);

            CREATE TABLE IF NOT EXISTS video_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                mac TEXT NOT NULL,
                date TEXT NOT NULL,
                session_start TEXT,
                session_end TEXT,
                platform TEXT DEFAULT '',
                total_download INTEGER DEFAULT 0,
                avg_score INTEGER DEFAULT 0,
                domains TEXT DEFAULT '[]',
                status TEXT DEFAULT 'active',
                updated_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_vs_mac_date ON video_sessions(mac, date);

            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT UNIQUE NOT NULL,
                display_name TEXT NOT NULL,
                role TEXT NOT NULL,
                password_hash TEXT,
                tablet_key TEXT
            );

            CREATE TABLE IF NOT EXISTS point_requests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                request_type TEXT NOT NULL,
                points INTEGER NOT NULL,
                status TEXT DEFAULT 'pending',
                admin_note TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                processed_at TEXT,
                FOREIGN KEY (user_id) REFERENCES users(id)
            );

            CREATE TABLE IF NOT EXISTS point_transactions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                tx_type TEXT NOT NULL,
                points INTEGER NOT NULL,
                balance_after INTEGER NOT NULL,
                description TEXT,
                request_id INTEGER,
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (user_id) REFERENCES users(id)
            );

            CREATE TABLE IF NOT EXISTS point_exchanges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                points_spent INTEGER NOT NULL,
                minutes_granted INTEGER NOT NULL,
                tablet_mac TEXT,
                status TEXT DEFAULT 'pending',
                error_msg TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (user_id) REFERENCES users(id)
            );

            CREATE TABLE IF NOT EXISTS weekly_quota (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                week_start TEXT NOT NULL,
                tutoring_used INTEGER DEFAULT 0,
                homework_used INTEGER DEFAULT 0,
                other_used INTEGER DEFAULT 0,
                UNIQUE(user_id, week_start),
                FOREIGN KEY (user_id) REFERENCES users(id)
            );
        ")?;
        info!("数据库表初始化完成");
        Ok(())
    }

    // ============ Config 操作 ============

    pub fn get_config(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        ).ok()
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_all_config(&self) -> std::collections::HashMap<String, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM config").unwrap();
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }).unwrap();
        let mut map = std::collections::HashMap::new();
        for row in rows.flatten() {
            map.insert(row.0, row.1);
        }
        map
    }

    // ============ Video Windows 操作 ============

    pub fn insert_video_window(&self, ts: &str, date: &str, mac: &str, ip: &str,
        download_bytes: i64, dns_total_queries: i64,
        video_domains: &str, video_platforms: &str,
        video_score: i64, video_status: &str, activity_type: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO video_windows (ts, date, mac, ip, download_bytes, dns_total_queries,
             video_domains, video_platforms, video_score, video_status, activity_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![ts, date, mac, ip, download_bytes, dns_total_queries,
                    video_domains, video_platforms, video_score, video_status, activity_type],
        )?;
        Ok(())
    }

    pub fn get_video_windows(&self, mac: &str, date: &str) -> Vec<crate::models::VideoWindow> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT ts, date, mac, ip, download_bytes, dns_total_queries,
                    video_domains, video_platforms, video_score, video_status, activity_type
             FROM video_windows WHERE mac=?1 AND date=?2 ORDER BY ts"
        ).unwrap();
        let rows = stmt.query_map(params![mac, date], |row| {
            Ok(crate::models::VideoWindow {
                ts: row.get(0)?,
                date: row.get(1)?,
                mac: row.get(2)?,
                ip: row.get(3)?,
                download_bytes: row.get(4)?,
                dns_total_queries: row.get(5)?,
                video_domains: row.get(6)?,
                video_platforms: row.get(7)?,
                video_score: row.get(8)?,
                video_status: row.get(9)?,
                activity_type: row.get(10)?,
            })
        }).unwrap();
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn get_daily_active_minutes(&self, mac: &str, date: &str) -> (i64, i64, i64) {
        let windows = self.get_video_windows(mac, date);
        if windows.is_empty() {
            return (0, 0, 0);
        }
        // 简单计算：连续 ACTIVE 窗口算 1 分钟
        let mut active_min = 0i64;
        let mut video_min = 0i64;
        let mut game_min = 0i64;
        for w in &windows {
            if w.video_status == "ACTIVE" {
                active_min += 1;
                match w.activity_type.as_str() {
                    t if t.starts_with("game") => game_min += 1,
                    t if t != "none" && t != "other" => video_min += 1,
                    _ => {}
                }
            }
        }
        (active_min, video_min, game_min)
    }

    // ============ Session 操作 ============

    pub fn upsert_session(&self, mac: &str, date: &str, ts: &str, status: &str,
        platform: &str, domains: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        if status == "ACTIVE" {
            let existing: Option<i64> = conn.query_row(
                "SELECT id FROM video_sessions WHERE mac=?1 AND date=?2 AND status='active'",
                params![mac, date],
                |row| row.get(0),
            ).ok();
            if let Some(id) = existing {
                conn.execute(
                    "UPDATE video_sessions SET session_end=?1, platform=?2, domains=?3, updated_at=?1 WHERE id=?4",
                    params![ts, platform, domains, id],
                )?;
            } else {
                conn.execute(
                    "INSERT INTO video_sessions (mac, date, session_start, session_end, platform, domains, status, updated_at)
                     VALUES (?1, ?2, ?3, ?3, ?4, ?5, 'active', ?3)",
                    params![mac, date, ts, platform, domains],
                )?;
            }
        } else {
            conn.execute(
                "UPDATE video_sessions SET status='closed', updated_at=?1
                 WHERE mac=?2 AND date=?3 AND status='active'",
                params![ts, mac, date],
            )?;
        }
        Ok(())
    }

    // ============ 积分系统 ============

    pub fn get_or_create_user(&self, username: &str, display_name: &str, role: &str, tablet_key: Option<&str>) -> crate::models::User {
        let conn = self.conn.lock().unwrap();
        let existing: Option<crate::models::User> = conn.query_row(
            "SELECT id, username, display_name, role, tablet_key FROM users WHERE username=?1",
            params![username],
            |row| Ok(crate::models::User {
                id: row.get(0)?,
                username: row.get(1)?,
                display_name: row.get(2)?,
                role: row.get(3)?,
                tablet_key: row.get(4)?,
            }),
        ).ok();
        if let Some(user) = existing {
            return user;
        }
        conn.execute(
            "INSERT INTO users (username, display_name, role, tablet_key) VALUES (?1, ?2, ?3, ?4)",
            params![username, display_name, role, tablet_key],
        ).unwrap();
        conn.query_row(
            "SELECT id, username, display_name, role, tablet_key FROM users WHERE username=?1",
            params![username],
            |row| Ok(crate::models::User {
                id: row.get(0)?,
                username: row.get(1)?,
                display_name: row.get(2)?,
                role: row.get(3)?,
                tablet_key: row.get(4)?,
            }),
        ).unwrap()
    }

    pub fn get_user_points_balance(&self, user_id: i64) -> i64 {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COALESCE(SUM(CASE WHEN tx_type='earn' THEN points ELSE -points END), 0)
             FROM point_transactions WHERE user_id=?1",
            params![user_id],
            |row| row.get(0),
        ).unwrap_or(0)
    }

        pub fn get_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    pub fn get_pending_requests(&self) -> Vec<crate::models::PointRequest> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, request_type, points, status, admin_note, created_at
             FROM point_requests WHERE status='pending' ORDER BY created_at DESC"
        ).unwrap();
        let rows = stmt.query_map([], |row| {
            Ok(crate::models::PointRequest {
                id: row.get(0)?,
                user_id: row.get(1)?,
                request_type: row.get(2)?,
                points: row.get(3)?,
                status: row.get(4)?,
                admin_note: row.get(5)?,
                created_at: row.get(6)?,
            })
        }).unwrap();
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn approve_request(&self, request_id: i64, action: &str, note: Option<&str>) {
        let conn = self.conn.lock().unwrap();
        let status = if action == "approve" { "approved" } else { "rejected" };
        conn.execute(
            "UPDATE point_requests SET status=?1, admin_note=?2, processed_at=datetime('now') WHERE id=?3",
            rusqlite::params![status, note, request_id],
        ).unwrap();
        if action == "approve" {
            let req: Option<(i64, i64, String)> = conn.query_row(
                "SELECT user_id, points, request_type FROM point_requests WHERE id=?1",
                rusqlite::params![request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).ok();
            if let Some((user_id, points, req_type)) = req {
                // 直接 SQL 查余额，避免死锁（已持有 conn 锁）
                let balance: i64 = conn.query_row(
                    "SELECT COALESCE(SUM(CASE WHEN tx_type='earn' THEN points ELSE -points END), 0)
                     FROM point_transactions WHERE user_id=?1",
                    rusqlite::params![user_id],
                    |row| row.get(0),
                ).unwrap_or(0);
                let new_balance = balance + points;
                conn.execute(
                    "INSERT INTO point_transactions (user_id, tx_type, points, balance_after, description, request_id)
                     VALUES (?1, 'earn', ?2, ?3, ?4, ?5)",
                    rusqlite::params![user_id, points, new_balance, format!("{}积分", req_type), request_id],
                ).unwrap();
            }
        }
    }

    pub fn record_exchange(&self, user_id: i64, points: i64, minutes: i64, mac: &str) {
        let conn = self.conn.lock().unwrap();
        // 直接 SQL 查余额，避免死锁（已持有 conn 锁）
        let balance: i64 = conn.query_row(
            "SELECT COALESCE(SUM(CASE WHEN tx_type='earn' THEN points ELSE -points END), 0)
             FROM point_transactions WHERE user_id=?1",
            rusqlite::params![user_id],
            |row| row.get(0),
        ).unwrap_or(0);
        let new_balance = balance - points;
        conn.execute(
            "INSERT INTO point_transactions (user_id, tx_type, points, balance_after, description)
             VALUES (?1, 'exchange', ?2, ?3, ?4)",
            rusqlite::params![user_id, points, new_balance, format!("兑换{}分钟平板时间", minutes)],
        ).unwrap();
        conn.execute(
            "INSERT INTO point_exchanges (user_id, points_spent, minutes_granted, tablet_mac, status)
             VALUES (?1, ?2, ?3, ?4, 'success')",
            rusqlite::params![user_id, points, minutes, mac],
        ).unwrap();
    }

    pub fn get_point_history(&self, user_id: i64) -> Vec<crate::models::PointTransaction> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, tx_type, points, balance_after, description, created_at
             FROM point_transactions WHERE user_id=?1 ORDER BY created_at DESC LIMIT 20"
        ).unwrap();
        let rows = stmt.query_map(rusqlite::params![user_id], |row| {
            Ok(crate::models::PointTransaction {
                id: row.get(0)?,
                user_id: row.get(1)?,
                tx_type: row.get(2)?,
                points: row.get(3)?,
                balance_after: row.get(4)?,
                description: row.get(5)?,
                created_at: row.get(6)?,
            })
        }).unwrap();
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn get_all_users(&self) -> Vec<crate::models::User> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, display_name, role, tablet_key FROM users"
        ).unwrap();
        let rows = stmt.query_map([], |row| {
            Ok(crate::models::User {
                id: row.get(0)?,
                username: row.get(1)?,
                display_name: row.get(2)?,
                role: row.get(3)?,
                tablet_key: row.get(4)?,
            })
        }).unwrap();
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn get_user_by_id(&self, user_id: i64) -> Option<crate::models::User> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, username, display_name, role, tablet_key FROM users WHERE id=?1",
            params![user_id],
            |row| Ok(crate::models::User {
                id: row.get(0)?,
                username: row.get(1)?,
                display_name: row.get(2)?,
                role: row.get(3)?,
                tablet_key: row.get(4)?,
            }),
        ).ok()
    }
}
