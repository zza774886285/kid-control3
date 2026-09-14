use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};
use crate::api::data::collect_api_data;
use crate::AppState;

const CACHE_TTL_SECS: u64 = 60;

pub async fn refresh_cache_loop(state: Arc<AppState>) {
    let mut failures: u32 = 0;
    loop {
        match tokio::time::timeout(Duration::from_secs(10), collect_api_data(&state)).await {
            Ok(Ok(data)) => {
                let mut cache = state.cache.write().await;
                *cache = Some(data);
                failures = 0;
            }
            Ok(Err(e)) => {
                failures += 1;
                warn!("缓存刷新失败 ({}): {}", failures, e);
                if failures >= 3 {
                    warn!("连续失败3次，暂停5分钟");
                    sleep(Duration::from_secs(300)).await;
                    failures = 0;
                    continue;
                }
            }
            Err(_) => {
                failures += 1;
                warn!("缓存刷新超时");
            }
        }
        sleep(Duration::from_secs(CACHE_TTL_SECS)).await;
    }
}
