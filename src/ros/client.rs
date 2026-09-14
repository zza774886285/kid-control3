use reqwest::Client;
use serde_json::Value;

pub struct RosClient {
    http: Client,
    base_url: String,
    user: String,
    pass: String,
}

impl RosClient {
    pub fn new(host: &str, port: u16, user: &str, pass: &str) -> Self {
        let base_url = format!("http://{}:{}/rest", host, port);
        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("failed to build reqwest client"),
            base_url,
            user: user.to_string(),
            pass: pass.to_string(),
        }
    }

    pub async fn get(&self, path: &str) -> Option<Value> {
        let url = self.build_url(path);
        let resp = self.http.get(&url)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .send().await.ok()?;
        let raw = resp.text().await.ok()?;
        if raw.trim().is_empty() { return Some(Value::Array(vec![])); }
        serde_json::from_str(&raw).ok()
    }

    pub async fn put(&self, path: &str, body: &Value) -> Option<Value> {
        let url = self.build_url(path);
        let resp = self.http.put(&url)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(body)
            .send().await.ok()?;
        let raw = resp.text().await.ok()?;
        if raw.trim().is_empty() { return Some(Value::Array(vec![])); }
        serde_json::from_str(&raw).ok()
    }

    pub async fn patch(&self, path: &str, body: &Value) -> Option<Value> {
        let url = self.build_url(path);
        let resp = self.http.patch(&url)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(body)
            .send().await.ok()?;
        let raw = resp.text().await.ok()?;
        if raw.trim().is_empty() { return Some(Value::Array(vec![])); }
        serde_json::from_str(&raw).ok()
    }

    pub async fn delete(&self, path: &str) -> Option<Value> {
        let url = self.build_url(path);
        let resp = self.http.delete(&url)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .send().await.ok()?;
        let raw = resp.text().await.ok()?;
        if raw.trim().is_empty() { return Some(Value::Array(vec![])); }
        serde_json::from_str(&raw).ok()
    }

    fn build_url(&self, path: &str) -> String {
        if path.starts_with('/') {
            format!("{}{}", self.base_url, path)
        } else {
            format!("{}/{}", self.base_url, path)
        }
    }
}
