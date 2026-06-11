use crate::error::*;
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

#[derive(Clone)]
pub struct CustomerGlobalIdApiClient {
    client: reqwest::Client,
    endpoint: String,
    admin_api_key: String,
    retry_count: usize,
    retry_backoff: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerGlobalIdUploadOutcome {
    pub attempts: usize,
    pub http_status: Option<u16>,
    pub headers: HashMap<String, String>,
    pub body: serde_json::Value,
    pub transport_error: Option<String>,
}

impl CustomerGlobalIdApiClient {
    pub fn new(
        endpoint: String,
        admin_api_key: String,
        timeout: Duration,
        retry_count: usize,
        retry_backoff: Duration,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()?;

        Ok(Self {
            client,
            endpoint,
            admin_api_key,
            retry_count,
            retry_backoff,
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub async fn upload_batch_file(
        &self,
        batch_file: &Path,
        file_name: &str,
    ) -> CustomerGlobalIdUploadOutcome {
        let bytes = match tokio::fs::read(batch_file).await {
            Ok(bytes) => bytes,
            Err(error) => {
                return CustomerGlobalIdUploadOutcome {
                    attempts: 0,
                    http_status: None,
                    headers: HashMap::new(),
                    body: serde_json::Value::Null,
                    transport_error: Some(error.to_string()),
                };
            }
        };

        let max_attempts = self.retry_count + 1;
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            let form = multipart::Form::new().part(
                "file",
                multipart::Part::bytes(bytes.clone())
                    .file_name(file_name.to_string())
                    .mime_str("text/csv")
                    .unwrap_or_else(|_| multipart::Part::bytes(bytes.clone())),
            );

            let response = self
                .client
                .post(&self.endpoint)
                .header(
                    "Authorization",
                    format!("admin-api-key={}", self.admin_api_key),
                )
                .multipart(form)
                .send()
                .await;

            match response {
                Ok(response) => {
                    let status = response.status();
                    let mut headers = HashMap::new();
                    for (key, value) in response.headers() {
                        if let Ok(value_str) = value.to_str() {
                            headers.insert(key.to_string(), value_str.to_string());
                        }
                    }

                    let response_text = response.text().await.unwrap_or_default();
                    let body = serde_json::from_str(&response_text)
                        .unwrap_or_else(|_| serde_json::Value::String(response_text));

                    if status.is_server_error() && attempt < max_attempts {
                        tokio::time::sleep(self.retry_backoff).await;
                        continue;
                    }

                    return CustomerGlobalIdUploadOutcome {
                        attempts: attempt,
                        http_status: Some(status.as_u16()),
                        headers,
                        body,
                        transport_error: None,
                    };
                }
                Err(error) => {
                    last_error = Some(error.to_string());
                    if attempt < max_attempts {
                        tokio::time::sleep(self.retry_backoff).await;
                    }
                }
            }
        }

        CustomerGlobalIdUploadOutcome {
            attempts: max_attempts,
            http_status: None,
            headers: HashMap::new(),
            body: serde_json::Value::Null,
            transport_error: last_error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn uploads_batch_with_admin_auth_and_file_part() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("Skipping local HTTP mock test: {}", error);
                return;
            }
            Err(error) => panic!("failed to bind local test server: {}", error),
        };
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0; 8192];
            let mut read = socket.read(&mut buffer).await.unwrap();

            let headers_end = loop {
                if let Some(pos) = buffer[..read].windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
                let next = socket.read(&mut buffer[read..]).await.unwrap();
                if next == 0 {
                    break read;
                }
                read += next;
            };

            let request_head = String::from_utf8_lossy(&buffer[..headers_end]).to_string();
            let content_length = request_head
                .lines()
                .find_map(|line| line.strip_prefix("Content-Length: "))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);

            let mut body = buffer[headers_end..read].to_vec();
            while body.len() < content_length {
                let mut chunk = vec![0; content_length - body.len()];
                let next = socket.read(&mut chunk).await.unwrap();
                if next == 0 {
                    break;
                }
                body.extend_from_slice(&chunk[..next]);
            }

            let response_body = concat!(
                "{\"updated_count\":1,\"skipped_count\":0,\"failed_count\":0,",
                "\"results\":[{\"status\":\"updated_null_id\"}]}"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).await.unwrap();

            (request_head, String::from_utf8_lossy(&body).to_string())
        });

        let dir = std::env::temp_dir().join(format!(
            "migratus-customer-upload-test-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&dir).unwrap();
        let batch_path = dir.join("batch_0001.csv");
        fs::write(&batch_path, "merchant_id,customer_id\nm1,c1\n").unwrap();

        let client = CustomerGlobalIdApiClient::new(
            format!("http://{}/v2/customers/migrate/global-id", addr),
            "secret".to_string(),
            Duration::from_secs(5),
            0,
            Duration::from_millis(1),
        )
        .unwrap();

        let outcome = client
            .upload_batch_file(&batch_path, "batch_0001.csv")
            .await;
        let (request_head, body) = server.await.unwrap();

        assert_eq!(outcome.http_status, Some(200));
        assert!(request_head.starts_with("POST /v2/customers/migrate/global-id HTTP/1.1"));
        assert!(request_head.contains("Authorization: admin-api-key=secret"));
        assert!(body.contains("name=\"file\""));
        assert!(body.contains("filename=\"batch_0001.csv\""));
        assert!(body.contains("merchant_id,customer_id"));

        fs::remove_dir_all(dir).unwrap();
    }
}
