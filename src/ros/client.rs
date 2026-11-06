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
            .send().await.ok()?.error_for_status().ok()?;
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
            .send().await.ok()?.error_for_status().ok()?;
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
            .send().await.ok()?.error_for_status().ok()?;
        let raw = resp.text().await.ok()?;
        if raw.trim().is_empty() { return Some(Value::Array(vec![])); }
        serde_json::from_str(&raw).ok()
    }

    pub async fn delete(&self, path: &str) -> Option<Value> {
        let url = self.build_url(path);
        let resp = self.http.delete(&url)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .send().await.ok()?.error_for_status().ok()?;
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

#[cfg(test)]
mod tests {
    use super::RosClient;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[tokio::test]
    async fn http_error_is_treated_as_failed_request() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("绑定测试端口");
        let port = listener.local_addr().expect("读取测试端口").port();
        let server = tokio::task::spawn_blocking(move || {
            let (mut stream, _) = listener.accept().expect("接受测试请求");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\n"
                        .as_slice(),
                )
                .expect("写入测试响应头");
            stream
                .write_all(b"Content-Length: 2\r\nConnection: close\r\n\r\n{}")
                .expect("写入测试响应体");
        });

        let client = RosClient::new("127.0.0.1", port, "test", "test");
        assert!(client.get("/test").await.is_none());
        server.await.expect("等待测试服务结束");
    }
}
