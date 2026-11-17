use super::{rpc_error, AgentService, Connection};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{oneshot, Semaphore},
};

pub(super) const MAX_BODY: usize = 512 * 1024;
const MAX_HEADERS: usize = 16 * 1024;

pub(super) async fn serve(
    listener: TcpListener,
    service: Weak<AgentService>,
    connection: Connection,
    mut stopped: oneshot::Receiver<()>,
) {
    let clients = Arc::new(Semaphore::new(8));
    loop {
        tokio::select! {
            _ = &mut stopped => break,
            accepted = listener.accept() => {
                let Ok((stream, peer)) = accepted else { break; };
                if !peer.ip().is_loopback() { continue; }
                let Ok(permit) = clients.clone().try_acquire_owned() else { continue; };
                let Some(service) = service.upgrade() else { break; };
                let connection = connection.clone();
                tauri::async_runtime::spawn(async move {
                    let _permit = permit;
                    let _ = handle(stream, service, connection).await;
                });
            }
        }
    }
}

async fn reply(stream: &mut TcpStream, status: u16, value: Option<Value>) -> Result<(), String> {
    let body = value.map(|v| v.to_string()).unwrap_or_default();
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        _ => "Internal Server Error",
    };
    let head = format!("HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nAllow: POST\r\nX-Content-Type-Options: nosniff\r\n\r\n", body.len());
    tokio::time::timeout(Duration::from_secs(10), async {
        stream.write_all(head.as_bytes()).await?;
        stream.write_all(body.as_bytes()).await?;
        stream.shutdown().await
    })
    .await
    .map_err(|_| "MCP response timed out")?
    .map_err(|_| "MCP response could not be sent".to_owned())
}

fn equal_secret(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

async fn handle(
    mut stream: TcpStream,
    service: Arc<AgentService>,
    connection: Connection,
) -> Result<(), String> {
    let mut bytes = Vec::with_capacity(4096);
    let header_end = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                return Ok(index + 4);
            }
            if bytes.len() >= MAX_HEADERS {
                return Err("MCP headers too large");
            }
            let mut buffer = [0_u8; 2048];
            let count = stream
                .read(&mut buffer)
                .await
                .map_err(|_| "Cannot read MCP request")?;
            if count == 0 {
                return Err("MCP connection closed");
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
    })
    .await
    .map_err(|_| "MCP header timeout")??;
    if header_end > MAX_HEADERS {
        return reply(&mut stream, 413, None).await;
    }
    let header = match std::str::from_utf8(&bytes[..header_end]) {
        Ok(header) => header,
        Err(_) => return reply(&mut stream, 400, None).await,
    };
    let mut lines = header.split("\r\n");
    let request: Vec<&str> = lines.next().unwrap_or("").split(' ').collect();
    if request.len() != 3 || request[2] != "HTTP/1.1" {
        return reply(&mut stream, 400, None).await;
    }
    let method = request[0].to_owned();
    let path = request[1].to_owned();
    let mut headers = HashMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let Some((key, value)) = line.split_once(':') else {
            return reply(&mut stream, 400, None).await;
        };
        let key = key.to_ascii_lowercase();
        if key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || headers.insert(key, value.trim().to_owned()).is_some()
        {
            return reply(&mut stream, 400, None).await;
        }
    }
    if path != "/mcp" {
        return reply(&mut stream, 404, None).await;
    }
    // This endpoint is for native MCP clients. Browser-origin requests are never
    // accepted, including localhost pages; the Tauri UI uses its own native IPC.
    if headers.contains_key("origin")
        || headers.get("host") != Some(&format!("127.0.0.1:{}", connection.port))
    {
        return reply(&mut stream, 403, None).await;
    }
    let valid = {
        let runtime = service.runtime.lock().await;
        runtime.running.as_ref().is_some_and(|r| {
            r.connection.port == connection.port
                && equal_secret(&r.connection.token, &connection.token)
        })
    };
    let supplied = headers
        .get("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("");
    if !valid || !equal_secret(supplied, &connection.token) {
        return reply(&mut stream, 401, None).await;
    }
    if method != "POST" {
        return reply(&mut stream, 405, None).await;
    }
    if !headers.get("content-type").is_some_and(|s| {
        s.split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("application/json")
    }) {
        return reply(&mut stream, 415, None).await;
    }
    let accept = headers.get("accept").map(String::as_str).unwrap_or("");
    if !accept.contains("application/json") || !accept.contains("text/event-stream") {
        return reply(&mut stream, 406, None).await;
    }
    if headers.contains_key("transfer-encoding") {
        return reply(&mut stream, 400, None).await;
    }
    let Some(length) = headers
        .get("content-length")
        .and_then(|s| s.parse::<usize>().ok())
    else {
        return reply(&mut stream, 400, None).await;
    };
    if length > MAX_BODY {
        return reply(&mut stream, 413, None).await;
    }
    let current = bytes.len() - header_end;
    if current < length {
        bytes.resize(header_end + length, 0);
        if tokio::time::timeout(
            Duration::from_secs(15),
            stream.read_exact(&mut bytes[header_end + current..]),
        )
        .await
        .map_or(true, |r| r.is_err())
        {
            return reply(&mut stream, 400, None).await;
        }
    }
    let body = &bytes[header_end..header_end + length];
    let request = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => {
            return reply(
                &mut stream,
                400,
                Some(rpc_error(None, -32700, "Invalid JSON", None)),
            )
            .await
        }
    };
    // Tool names are header-safe, but a conforming client may use the standard
    // base64 sentinel. Compare the decoded name to the JSON body.
    if let Some(value) = headers.get_mut("mcp-name") {
        if let Some(encoded) = value
            .strip_prefix("=?base64?")
            .and_then(|v| v.strip_suffix("?="))
        {
            use base64::Engine;
            match base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .ok()
                .and_then(|v| String::from_utf8(v).ok())
            {
                Some(decoded) => *value = decoded,
                None => {
                    return reply(
                        &mut stream,
                        400,
                        Some(rpc_error(None, -32020, "Invalid MCP name header", None)),
                    )
                    .await
                }
            }
        }
    }
    let (status, body) = service.rpc(request, &headers).await;
    reply(&mut stream, status, body).await
}
