use serde_json::Value;
use crate::ros::client::RosClient;
use tracing::info;

pub struct Ipv6Rule {
    pub id: String,
    pub disabled: bool,
}

pub async fn get_ipv6_filter_rules(client: &RosClient, comment_filter: &str) -> Vec<Ipv6Rule> {
    let data = match client.get("/ipv6/firewall/filter").await {
        Some(val) => val,
        None => return vec![],
    };
    let arr = match data.as_array() {
        Some(a) => a,
        None => return vec![],
    };
    arr.iter()
        .filter_map(|rule| {
            let comment = rule.get("comment")?.as_str()?;
            if comment == comment_filter {
                Some(Ipv6Rule {
                    id: rule.get(".id")?.as_str()?.to_string(),
                    disabled: rule.get("disabled")?.as_str() == Some("true"),
                })
            } else {
                None
            }
        })
        .collect()
}

pub async fn set_ipv6_filter_disabled(client: &RosClient, rule_id: &str, disabled: bool) {
    let body = serde_json::json!({ "disabled": disabled.to_string() });
    client.patch(&format!("/ipv6/firewall/filter/{}", rule_id), &body).await;
    info!("IPv6 规则 {} disabled={}", rule_id, disabled);
}
