use serde_json::json;
use crate::ros::client::RosClient;
use tracing::info;

/// 获取 IPv6 firewall filter 规则（family 链）
async fn get_family_rules(client: &RosClient) -> Vec<serde_json::Value> {
    let data = match client.get("/ipv6/firewall/filter?chain=family").await {
        Some(val) => val,
        None => return vec![],
    };
    match data.as_array() {
        Some(a) => a.clone(),
        None => vec![],
    }
}

/// 按 comment 禁用/启用规则
async fn set_rule_by_comment(client: &RosClient, comment: &str, disabled: bool) {
    let rules = get_family_rules(client).await;
    for rule in rules {
        let rule_comment = rule.get("comment").and_then(|v| v.as_str()).unwrap_or("");
        if rule_comment == comment {
            if let Some(rule_id) = rule.get(".id").and_then(|v| v.as_str()) {
                let body = json!({ "disabled": disabled.to_string() });
                client.patch(
                    &format!("/ipv6/firewall/filter/{}", rule_id),
                    &body,
                ).await;
                info!("IPv6 rule {} (comment={}) → disabled={}", rule_id, comment, disabled);
                return;  // 只改第一条匹配的
            }
        }
    }
    info!("IPv6: 未找到 comment={} 的规则，跳过", comment);
}

/// 封禁 IPv6（disabled=true = 规则生效 = 放行/不阻断）
/// 注意：ROS 中 disabled=true 表示规则被禁用（不生效），disabled=false 表示规则生效（阻断）
/// 对于 drop 规则：disabled=true = 不 drop = 放行，disabled=false = drop = 封禁
pub async fn block_ipv6(client: &RosClient, comment: &str) {
    // drop 规则：disabled=false → 规则生效 → 流量被 drop → 封禁
    set_rule_by_comment(client, comment, false).await;
}

/// 解封 IPv6（disabled=true = 规则被禁用 = 不 drop = 放行）
pub async fn unblock_ipv6(client: &RosClient, comment: &str) {
    // drop 规则：disabled=true → 规则不生效 → 流量放行
    set_rule_by_comment(client, comment, true).await;
}
