use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

const CALLBACK_TIMEOUT_SECS: u64 = 120;

const SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>Authorization Complete</title></head>
<body style="font-family: sans-serif; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: #1a1a2e; color: #e0e0e0;">
<div style="text-align: center; padding: 2em; border-radius: 12px; background: #16213e; box-shadow: 0 4px 6px rgba(0,0,0,0.3);">
<h1>Authorization Complete</h1>
<p>You can close this tab and return to Ergon.</p>
</div>
</body>
</html>"#;

const ERROR_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>Authorization Error</title></head>
<body style="font-family: sans-serif; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: #1a1a2e; color: #e0e0e0;">
<div style="text-align: center; padding: 2em; border-radius: 12px; background: #16213e; box-shadow: 0 4px 6px rgba(0,0,0,0.3);">
<h1>Authorization Error</h1>
<p>Something went wrong. Please try again.</p>
</div>
</body>
</html>"#;

/// OAuth2 callback result containing the authorization code and CSRF state token.
#[derive(Debug)]
pub struct OAuthCallbackResult {
    pub code: String,
    pub state: String,
}

/// Start a temporary localhost HTTP server to receive the OAuth2 authorization callback.
///
/// Listens on `127.0.0.1:{port}/callback`, waits for a single GET request with
/// `code` and `state` query parameters, responds with a success page, and returns
/// the extracted values.
///
/// Times out after 120 seconds if no callback is received.
pub async fn wait_for_oauth_callback(port: u16) -> Result<OAuthCallbackResult> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .with_context(|| format!("Failed to bind OAuth callback server on port {}", port))?;

    log::info!(
        "OAuth callback server listening on http://127.0.0.1:{}/callback",
        port
    );

    let result = tokio::time::timeout(
        Duration::from_secs(CALLBACK_TIMEOUT_SECS),
        handle_single_request(&listener),
    )
    .await
    .map_err(|_| anyhow!("OAuth callback timed out after {} seconds", CALLBACK_TIMEOUT_SECS))?;

    result
}

async fn handle_single_request(listener: &TcpListener) -> Result<OAuthCallbackResult> {
    loop {
        let (mut stream, addr) = listener
            .accept()
            .await
            .context("Failed to accept connection")?;

        log::debug!("OAuth callback: connection from {}", addr);

        let mut buf = vec![0u8; 4096];
        let n = stream
            .read(&mut buf)
            .await
            .context("Failed to read from connection")?;

        if n == 0 {
            continue;
        }

        let request = String::from_utf8_lossy(&buf[..n]);

        // Parse the HTTP request line to extract the path + query
        let request_line = match request.lines().next() {
            Some(line) => line,
            None => {
                send_response(&mut stream, 400, ERROR_HTML).await?;
                continue;
            }
        };

        // Expected format: "GET /callback?code=xxx&state=yyy HTTP/1.1"
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 || parts[0] != "GET" {
            send_response(&mut stream, 400, ERROR_HTML).await?;
            continue;
        }

        let path_and_query = parts[1];

        // Parse as a URL to extract query parameters
        let full_url = format!("http://127.0.0.1{}", path_and_query);
        let parsed = match Url::parse(&full_url) {
            Ok(u) => u,
            Err(_) => {
                send_response(&mut stream, 400, ERROR_HTML).await?;
                continue;
            }
        };

        // Only handle the /callback path
        if parsed.path() != "/callback" {
            // Could be a favicon request or similar, ignore
            send_response(&mut stream, 404, "Not Found").await?;
            continue;
        }

        let query_pairs: std::collections::HashMap<_, _> =
            parsed.query_pairs().into_owned().collect();

        let code = match query_pairs.get("code") {
            Some(c) => c.clone(),
            None => {
                // Check for error response from the auth server
                if let Some(error) = query_pairs.get("error") {
                    let desc = query_pairs
                        .get("error_description")
                        .cloned()
                        .unwrap_or_default();
                    send_response(&mut stream, 200, ERROR_HTML).await?;
                    return Err(anyhow!(
                        "OAuth authorization denied: {} ({})",
                        error,
                        desc
                    ));
                }
                send_response(&mut stream, 400, ERROR_HTML).await?;
                continue;
            }
        };

        let state = match query_pairs.get("state") {
            Some(s) => s.clone(),
            None => {
                send_response(&mut stream, 400, ERROR_HTML).await?;
                continue;
            }
        };

        // Send success response
        send_response(&mut stream, 200, SUCCESS_HTML).await?;

        log::info!("OAuth callback received successfully");

        return Ok(OAuthCallbackResult { code, state });
    }
}

async fn send_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    body: &str,
) -> Result<()> {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Error",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        status_text,
        body.len(),
        body
    );

    stream
        .write_all(response.as_bytes())
        .await
        .context("Failed to write response")?;
    stream
        .flush()
        .await
        .context("Failed to flush response")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_callback_parses_code_and_state() {
        // Bind to a random port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move { handle_single_request(&listener).await });

        // Simulate a browser callback
        let client = tokio::spawn(async move {
            // Give the server a moment to start
            tokio::time::sleep(Duration::from_millis(50)).await;
            let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
                .await
                .unwrap();
            let request = "GET /callback?code=test_code_123&state=csrf_state_456 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
            stream.write_all(request.as_bytes()).await.unwrap();
            stream.flush().await.unwrap();

            // Read response
            let mut buf = vec![0u8; 4096];
            let _ = stream.read(&mut buf).await.unwrap();
        });

        let (result, _) = tokio::join!(server, client);
        let callback = result.unwrap().unwrap();
        assert_eq!(callback.code, "test_code_123");
        assert_eq!(callback.state, "csrf_state_456");
    }

    #[tokio::test]
    async fn test_callback_rejects_missing_code() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        // Spawn the server with a timeout since it won't complete on missing code
        let server = tokio::spawn(async move {
            tokio::time::timeout(Duration::from_millis(500), handle_single_request(&listener)).await
        });

        let client = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
                .await
                .unwrap();
            // Missing code parameter
            let request =
                "GET /callback?state=csrf_state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
            stream.write_all(request.as_bytes()).await.unwrap();
            stream.flush().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let _ = stream.read(&mut buf).await.unwrap();
        });

        let (result, _) = tokio::join!(server, client);
        // Should timeout since the server keeps waiting for a valid request
        assert!(result.unwrap().is_err());
    }
}
