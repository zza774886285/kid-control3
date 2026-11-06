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
async fn set_rule_by_comment(client: &RosClient, comment: &str, disabled: bool) -> bool {
    let rules = get_family_rules(client).await;
    for rule in rules {
        let rule_comment = rule.get("comment").and_then(|v| v.as_str()).unwrap_or("");
        if rule_comment == comment {
            if let Some(rule_id) = rule.get(".id").and_then(|v| v.as_str()) {
                let body = json!({ "disabled": disabled.to_string() });
                let ok = client.patch(
                    &format!("/ipv6/firewall/filter/{}", rule_id),
                    &body,
                ).await.is_some();
                if ok {
                    info!("IPv6 rule {} (comment={}) → disabled={}", rule_id, comment, disabled);
                }
                return ok;
            }
        }
    }
    info!("IPv6: 未找到 comment={} 的规则，跳过", comment);
    false
}

/// 封禁 IPv6：让 drop 规则生效。
pub async fn block_ipv6(client: &RosClient, comment: &str) -> bool {
    set_rule_by_comment(client, comment, false).await
}

/// 解封 IPv6：禁用 drop 规则。
pub async fn unblock_ipv6(client: &RosClient, comment: &str) -> bool {
    set_rule_by_comment(client, comment, true).await
}
