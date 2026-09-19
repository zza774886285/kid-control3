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
mod telegram;

use config::ConfigManager;
use db::Database;
use ros::RosClient;
use detector::activity::ActivityDetector;
use detector::dns_collector::DnsCollector;
use detector::ip_registry::GameIpRegistry;

pub struct AppState {
    pub db: Arc<Database>,
    pub config: ConfigManager,
    pub ros: RosClient,
    pub http_client: reqwest::Client,
    pub detector: ActivityDetector,
    pub dns_collector: DnsCollector,
    pub ip_registry: Arc<RwLock<GameIpRegistry>>,
    pub registry_path: std::path::PathBuf,
    pub cache: RwLock<Option<serde_json::Value>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kid_control3=info,tower_http=info".into())
        )
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::new(
            "%Y-%m-%d %H:%M:%S".to_string(),
        ))
        .init();

    info!("kid-control3 启动中...");

    dotenvy::dotenv().ok();

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "/data/kid-control.db".to_string());
    let ros_host = std::env::var("ROS_HOST").expect("请设置 ROS_HOST");
    let ros_port: u16 = std::env::var("ROS_PORT").unwrap_or_else(|_| "80".to_string()).parse().unwrap_or(80);
    let ros_user = std::env::var("ROS_USER").expect("请设置 ROS_USER");
    let ros_pass = std::env::var("ROS_PASS").expect("请设置 ROS_PASS");
    let mosdns_url = {
        let raw = std::env::var("MOSDNS_URL").unwrap_or_else(|_| {
            let host = std::env::var("MOSDNS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
            let port = std::env::var("MOSDNS_PORT").unwrap_or_else(|_| "9099".to_string());
            format!("http://{}:{}", host, port)
        });
        if raw.starts_with("http") { raw } else { format!("http://{}", raw) }
    };
    let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:18089".to_string());
    let kid_listen_addr = std::env::var("KID_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:18090".to_string());

    let db = Arc::new(Database::new(&db_path).expect("数据库初始化失败"));
    let config = ConfigManager::new(db.clone());
    let ros = RosClient::new(&ros_host, ros_port, &ros_user, &ros_pass);
    let detector = ActivityDetector::new();
    let dns_collector = DnsCollector::new(&mosdns_url);

    // 游戏 IP 注册表
    let registry_path = std::path::PathBuf::from(
        std::env::var("GAME_IP_REGISTRY").unwrap_or_else(|_| "/data/game-ips.json".to_string())
    );
    let ip_registry = Arc::new(RwLock::new(GameIpRegistry::load(&registry_path)));
    info!("游戏IP注册表已加载: {}条IP, {}个域名模式",
        ip_registry.read().await.ips.len(),
        ip_registry.read().await.domains.len());

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
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .no_proxy()
            .build()
            .unwrap_or_default(),
        detector,
        dns_collector,
        ip_registry: ip_registry.clone(),
        registry_path: registry_path.clone(),
        cache: RwLock::new(None),
    });

    // Admin API 路由（全部路由）
    let admin_api_routes = Router::new()
        .route("/api/data", get(api::data::get_data))
        .route("/api/video-windows", get(api::data::get_video_windows))
        .route("/api/weekly-stats", get(api::data::get_weekly_stats))
        .route("/api/recent-activity", get(api::data::get_recent_activity))
        .route("/api/control-status", get(api::control::get_control_status))
        .route("/api/settings", get(api::config_api::get_settings).post(api::config_api::post_settings))
        .route("/api/vacation", get(api::config_api::get_vacation).post(api::config_api::set_vacation))
        .route("/api/kid-adjust", post(api::control::kid_adjust))
        .route("/api/switch", post(api::control::switch_device))
        .route("/api/pause", post(api::control::pause_device))
        .route("/api/points/balance", get(api::points::get_points_balance))
        .route("/api/points/apply", post(api::points::apply_points))
        .route("/api/points/pending", get(api::points::get_pending_requests))
        .route("/api/points/approve", post(api::points::approve_request))
        .route("/api/points/exchange", post(api::points::exchange_points))
        .route("/api/points/my", get(api::points::get_my_points))
        .route("/api/points/config", get(api::points::get_points_config).post(api::points::set_points_config))
        .route("/api/points/recent", get(api::points::get_recent_transactions))
        .route("/api/points/set", post(api::points::set_points))
        .route("/api/game-ips", get(api::game_ips::get_game_ips))
        .route("/api/game-ips/ip", post(api::game_ips::add_game_ip).delete(api::game_ips::remove_game_ip))
        .route("/api/game-ips/domain", post(api::game_ips::add_game_domain).delete(api::game_ips::remove_game_domain));

    // Admin 静态文件（web/dist/，fallback 到 index.html）
    let admin_static_service = ServeDir::new("web/dist")
        .fallback(tower_http::services::ServeFile::new("web/dist/index.html"));

    let admin_app = Router::new()
        .merge(admin_api_routes)
        .fallback_service(admin_static_service)
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state.clone());

    // Kid API 路由（仅积分相关）
    let kid_api_routes = Router::new()
        .route("/api/points/balance", get(api::points::get_points_balance))
        .route("/api/points/apply", post(api::points::apply_points))
        .route("/api/points/pending", get(api::points::get_pending_requests))
        .route("/api/points/exchange", post(api::points::exchange_points))
        .route("/api/points/my", get(api::points::get_my_points))
        .route("/api/points/config", get(api::points::get_points_config))
        .route("/api/points/recent", get(api::points::get_recent_transactions));

    // Kid 静态文件（web/dist/kid/，不设 SPA fallback）
    let kid_static_service = ServeDir::new("web/dist/kid");

    let kid_app = Router::new()
        .merge(kid_api_routes)
        .fallback_service(kid_static_service)
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state.clone());

    // 启动定时任务
    let state_clone = state.clone();
    tokio::spawn(async move {
        loop {
            scheduler::collect::run_data_collection(
                &state_clone.ros, &state_clone.config, &state_clone.db,
                &state_clone.http_client, &state_clone.detector, &state_clone.dns_collector,
                &state_clone.ip_registry, &state_clone.registry_path,
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

    tokio::spawn(scheduler::cache::refresh_cache_loop(state.clone()));

    // Telegram 积分审批轮询
    let state_clone = state.clone();
    tokio::spawn(async move {
        telegram::poll_telegram_updates(state_clone).await;
    });

    // 启动双端口
    info!("admin 监听 {}", listen_addr);
    info!("kid   监听 {}", kid_listen_addr);

    let admin_listener = tokio::net::TcpListener::bind(&listen_addr).await.unwrap();
    let kid_listener = tokio::net::TcpListener::bind(&kid_listen_addr).await.unwrap();

    tokio::join!(
        axum::serve(admin_listener, admin_app),
        axum::serve(kid_listener, kid_app)
    );
}
