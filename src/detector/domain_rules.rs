use std::collections::HashMap;
use crate::models::DomainClassification;

lazy_static::lazy_static! {
    static ref RULES: HashMap<String, Vec<String>> = {
        let mut m = HashMap::new();
        m.insert("GAME_EGG".to_string(), vec![
            "egg.fp.ps.netease.com".to_string(),
            "gameyw.netease.com".to_string(),
        ]);
        m.insert("GAME_PRETERNATURAL".to_string(), vec![
            "preternatural.cn".to_string(),
            "anticheatexpert.com".to_string(),
        ]);
        m.insert("XIAOHONGSHU".to_string(), vec![
            "xiaohongshu.com".to_string(),
            "xhscdn.com".to_string(),
        ]);
        m.insert("TENCENT_VIDEO".to_string(), vec![
            "v.qq.com".to_string(),
            "tencent-video.com".to_string(),
            "mff.gtimg.com".to_string(),
        ]);
        m.insert("HONGGUO".to_string(), vec![
            "hongguo.com".to_string(),
            "hongguo.tv".to_string(),
            "ixigua.com".to_string(),
        ]);
        m
    };

    static ref CATEGORY_NAMES: HashMap<String, String> = {
        let mut m = HashMap::new();
        m.insert("GAME_EGG".to_string(), "蛋仔派对".to_string());
        m.insert("GAME_PRETERNATURAL".to_string(), "超自然".to_string());
        m.insert("XIAOHONGSHU".to_string(), "小红书".to_string());
        m.insert("TENCENT_VIDEO".to_string(), "腾讯视频".to_string());
        m.insert("HONGGUO".to_string(), "红果短剧".to_string());
        m
    };

    static ref EXCLUDE: Vec<String> = vec![
        "dbankcloud.cn".to_string(), "dbankcdn.cn".to_string(),
        "dbankcloud.com".to_string(), "dbankcdn.com".to_string(),
        "hicloud.com".to_string(), "connectivitycheck".to_string(),
        "beacon".to_string(), "track.".to_string(), "analytics".to_string(),
        "sdk.".to_string(), "pixel.".to_string(), "event.".to_string(),
        "stat.".to_string(), "metrics".to_string(), "apm".to_string(),
        "ipv6.arpa".to_string(), "apple.com".to_string(), "qq.com".to_string(),
        "icloud.com".to_string(), "baidu.com".to_string(),
        "push.".to_string(), "firebase".to_string(),
    ];

    static ref PRIORITY: Vec<String> = vec![
        "GAME_EGG".to_string(), "GAME_PRETERNATURAL".to_string(),
        "XIAOHONGSHU".to_string(), "HONGGUO".to_string(), "TENCENT_VIDEO".to_string(),
    ];
}

fn match_domain(domain: &str) -> Option<String> {
    let dl = domain.to_lowercase();
    for ex in EXCLUDE.iter() {
        if dl.contains(ex.as_str()) {
            return None;
        }
    }
    for (cat, domains) in RULES.iter() {
        for d in domains {
            if dl.contains(d.as_str()) {
                return Some(cat.clone());
            }
        }
    }
    None
}

pub fn classify_domains(domains: &[String]) -> DomainClassification {
    let mut matched_cats = std::collections::HashSet::new();
    let mut matched_domains = std::collections::HashSet::new();
    for d in domains {
        if let Some(cat) = match_domain(d) {
            matched_cats.insert(cat);
            matched_domains.insert(d.clone());
        }
    }
    if matched_cats.is_empty() {
        return DomainClassification {
            category: "NONE".to_string(),
            name: "其它".to_string(),
            domains: vec![],
        };
    }
    for cat in PRIORITY.iter() {
        if matched_cats.contains(cat) {
            let name = CATEGORY_NAMES.get(cat).cloned().unwrap_or_else(|| cat.clone());
            let mut doms: Vec<String> = matched_domains.into_iter().collect();
            doms.sort();
            return DomainClassification { category: cat.clone(), name, domains: doms };
        }
    }
    let mut doms: Vec<String> = matched_domains.into_iter().collect();
    doms.sort();
    DomainClassification {
        category: "OTHER".to_string(),
        name: "其它".to_string(),
        domains: doms,
    }
}

pub fn get_category_chinese(category: &str) -> String {
    CATEGORY_NAMES.get(category).cloned().unwrap_or_else(|| "其它活动".to_string())
}
