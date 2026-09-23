use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::net::TcpListener;
use tokio::sync::oneshot;

/// Ports registered as redirect URIs on the `k1-c/tui` application.
///
/// Linear matches the redirect URI exactly, port included, so the callback
/// server cannot take whatever ephemeral port the OS hands out — it has to land
/// on one of these, and they are tried in order.
pub const CALLBACK_PORTS: [u16; 3] = [53681, 53682, 53683];

const SUCCESS_BODY: &str = "<!doctype html><meta charset=\"utf-8\"><title>linear-tui</title>\
<body style=\"font:16px system-ui;padding:3rem\"><h1>Authorized</h1>\
<p>linear-tui is connected. You can close this tab and return to your terminal.</p>";

/// Start a local HTTP server to receive the OAuth callback.
/// Returns the bound port and a receiver for the authorization code.
pub async fn start_callback_server(
    expected_state: String,
) -> Result<(u16, oneshot::Receiver<String>)> {
    let (listener, port) = bind_callback_port()?;
    let (tx, rx) = oneshot::channel();

    tokio::task::spawn_blocking(move || serve(listener, &expected_state, tx));

    Ok((port, rx))
}

fn bind_callback_port() -> Result<(TcpListener, u16)> {
    let mut last_error = None;
    for port in CALLBACK_PORTS {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => return Ok((listener, port)),
            Err(e) => {
                tracing::debug!(port, error = %e, "callback port unavailable");
                last_error = Some(e);
            }
        }
    }

    let ports = CALLBACK_PORTS
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    Err(last_error.expect("CALLBACK_PORTS is never empty")).with_context(|| {
        format!(
            "Every callback port is taken ({ports}). \
             Linear only accepts these, so free one and run the login again."
        )
    })
}

/// Serve until the authorization callback arrives.
///
/// Browsers ask for more than the callback — `/favicon.ico` above all — so
/// anything that is not the callback is answered and ignored rather than
/// consuming the one connection the flow depends on.
fn serve(listener: TcpListener, expected_state: &str, tx: oneshot::Sender<String>) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };

        let mut buf = [0u8; 8192];
        let read = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..read]);

        let Some(target) = request_target(&request) else {
            respond(&mut stream, "400 Bad Request", "text/plain", "Bad request.");
            continue;
        };

        let (path, query) = split_target(target);
        if path != "/callback" {
            respond(&mut stream, "404 Not Found", "text/plain", "Not found.");
            continue;
        }

        let params = parse_query(query);
        let param = |key: &str| {
            params
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.to_string())
        };

        if let Some(error) = param("error") {
            let description = param("error_description").unwrap_or_default();
            tracing::warn!(%error, %description, "authorization denied");
            respond(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                &format!("<h1>Authorization cancelled</h1><p>{error}</p>"),
            );
            return;
        }

        match (param("code"), param("state")) {
            (Some(code), Some(state)) if state == expected_state => {
                respond(
                    &mut stream,
                    "200 OK",
                    "text/html; charset=utf-8",
                    SUCCESS_BODY,
                );
                let _ = tx.send(code);
                return;
            }
            (Some(_), Some(_)) => {
                tracing::error!("callback state did not match the request");
                respond(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain",
                    "State mismatch — the login was not started by this terminal.",
                );
                return;
            }
            _ => {
                respond(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain",
                    "Callback was missing its code.",
                );
                return;
            }
        }
    }
}

fn respond(stream: &mut impl Write, status: &str, content_type: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len(),
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// Pull the target out of a request line like `GET /callback?code=x HTTP/1.1`.
fn request_target(request: &str) -> Option<&str> {
    request.lines().next()?.split_whitespace().nth(1)
}

fn split_target(target: &str) -> (&str, &str) {
    match target.split_once('?') {
        Some((path, query)) => (path, query),
        None => (target, ""),
    }
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) => (percent_decode(key), percent_decode(value)),
            None => (percent_decode(pair), String::new()),
        })
        .collect()
}

/// Decode `%XX` escapes and `+` so a code containing them survives the trip.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    None => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }

    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_target_from_a_request_line() {
        let request = "GET /callback?code=abc&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(
            request_target(request),
            Some("/callback?code=abc&state=xyz")
        );
    }

    #[test]
    fn splits_a_target_without_a_query() {
        assert_eq!(split_target("/favicon.ico"), ("/favicon.ico", ""));
    }

    #[test]
    fn parses_query_parameters() {
        let params = parse_query("code=abc&state=xyz");
        assert_eq!(params[0], ("code".to_string(), "abc".to_string()));
        assert_eq!(params[1], ("state".to_string(), "xyz".to_string()));
    }

    #[test]
    fn decodes_percent_escapes_in_values() {
        let params = parse_query("code=a%2Fb%2Bc&state=x%20y");
        assert_eq!(params[0].1, "a/b+c");
        assert_eq!(params[1].1, "x y");
    }

    #[test]
    fn leaves_a_malformed_escape_intact() {
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn treats_plus_as_a_space() {
        assert_eq!(percent_decode("a+b"), "a b");
    }
}
