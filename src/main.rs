use std::sync::Arc;
use axum::{routing::{get, post}, Router};
use tower_http::cors::{CorsLayer, Any};
use tower_http::services::ServeDir;
use tracing::info;
use tokio::sync::RwLock;

mod config;
mod db;
mod models;
mod ros;
mod detector;
mod scheduler;
mod api;

use config::ConfigManager;
use db::Database;
use ros::RosClient;
use detector::activity::ActivityDetector;
use detector::dns_collector::DnsCollector;

pub struct AppState {
    pub db: Arc<Database>,
    pub config: ConfigManager,
    pub ros: RosClient,
    pub detector: ActivityDetector,
    pub dns_collector: DnsCollector,
    pub cache: RwLock<Option<serde_json::Value>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kid_control3=info,tower_http=info".into())
        )
        .init();

    info!("kid-control3 启动中...");

    dotenvy::dotenv().ok();

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "/data/kid-control.db".to_string());
    let ros_host = std::env::var("ROS_HOST").expect("请设置 ROS_HOST");
    let ros_port: u16 = std::env::var("ROS_PORT").unwrap_or_else(|_| "80".to_string()).parse().unwrap_or(80);
    let ros_user = std::env::var("ROS_USER").expect("请设置 ROS_USER");
    let ros_pass = std::env::var("ROS_PASS").expect("请设置 ROS_PASS");
    let mosdns_url = std::env::var("MOSDNS_URL").unwrap_or_else(|_| {
        let host = std::env::var("MOSDNS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("MOSDNS_PORT").unwrap_or_else(|_| "9099".to_string());
        format!("http://{}:{}", host, port)
    });
    let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:18089".to_string());

    let db = Arc::new(Database::new(&db_path).expect("数据库初始化失败"));
    let config = ConfigManager::new(db.clone());
    let ros = RosClient::new(&ros_host, ros_port, &ros_user, &ros_pass);
    let detector = ActivityDetector::new();
    let dns_collector = DnsCollector::new(&mosdns_url);

    // 初始化默认用户
    {
        let conn = db.get_conn();
        conn.execute("INSERT OR IGNORE INTO users (username, display_name, role, tablet_key) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["admin", "管理员", "admin", None::<String>]).unwrap();
        conn.execute("INSERT OR IGNORE INTO users (username, display_name, role, tablet_key) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["lisa", "周楷依", "lisa", "lisa"]).unwrap();
        conn.execute("INSERT OR IGNORE INTO users (username, display_name, role, tablet_key) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["huawei", "周芓翕", "huawei", "huawei"]).unwrap();
    }

    let state = Arc::new(AppState {
        db: db.clone(),
        config,
        ros,
        detector,
        dns_collector,
        cache: RwLock::new(None),
    });

    // API 路由
    let api_routes = Router::new()
        .route("/api/data", get(api::data::get_data))
        .route("/api/video-windows", get(api::data::get_video_windows))
        .route("/api/control-status", get(api::control::get_control_status))
        .route("/api/settings", get(api::config_api::get_settings).post(api::config_api::post_settings))
        .route("/api/vacation", get(api::config_api::get_vacation).post(api::config_api::set_vacation))
        .route("/kid-adjust", post(api::control::kid_adjust))
        .route("/api/switch", post(api::control::switch_device))
        .route("/api/pause", post(api::control::pause_device))
        .route("/api/points/balance", get(api::points::get_points_balance))
        .route("/api/points/earn", post(api::points::earn_points))
        .route("/api/points/pending", get(api::points::get_pending_requests))
        .route("/api/points/approve", post(api::points::approve_request))
        .route("/api/points/exchange", post(api::points::exchange_points))
        .route("/api/points/my", get(api::points::get_my_points));

    // 静态文件（前端）
    let static_service = ServeDir::new("web/dist");

    let app = Router::new()
        .merge(api_routes)
        .fallback_service(static_service)
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state.clone());

    // 启动定时任务
    let state_clone = state.clone();
    tokio::spawn(async move {
        loop {
            scheduler::collect::run_data_collection(
                &state_clone.ros, &state_clone.config, &state_clone.db,
                &state_clone.detector, &state_clone.dns_collector,
            ).await;
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });

    let state_clone = state.clone();
    tokio::spawn(async move {
        loop {
            scheduler::collect::run_control_cycle(
                &state_clone.ros, &state_clone.config, &state_clone.db,
                &state_clone.detector, &state_clone.dns_collector,
            ).await;
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });

    tokio::spawn(scheduler::cache::refresh_cache_loop(state));

    info!("kid-control3 监听 {}", listen_addr);
    let listener = tokio::net::TcpListener::bind(&listen_addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
