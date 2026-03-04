use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::net::TcpListener;
use tokio::sync::oneshot;

/// Start a local HTTP server to receive the OAuth callback.
/// Returns (port, receiver for the authorization code).
pub async fn start_callback_server(
    expected_state: String,
) -> Result<(u16, oneshot::Receiver<String>)> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Failed to bind callback server")?;
    let port = listener.local_addr()?.port();
    let (tx, rx) = oneshot::channel();

    tokio::task::spawn_blocking(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]);

            // Parse the GET request for code and state
            if let Some(query) = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|path| path.split('?').nth(1))
            {
                let params: Vec<(&str, &str)> =
                    query.split('&').filter_map(|p| p.split_once('=')).collect();

                let code = params.iter().find(|(k, _)| *k == "code").map(|(_, v)| *v);
                let state = params.iter().find(|(k, _)| *k == "state").map(|(_, v)| *v);

                if let (Some(code), Some(state)) = (code, state)
                    && state == expected_state
                {
                    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Authentication successful!</h1><p>You can close this window.</p></body></html>";
                    let _ = stream.write_all(response.as_bytes());
                    let _ = tx.send(code.to_string());
                    return;
                }
            }

            let response = "HTTP/1.1 400 Bad Request\r\n\r\nAuthentication failed.";
            let _ = stream.write_all(response.as_bytes());
        }
    });

    Ok((port, rx))
}
