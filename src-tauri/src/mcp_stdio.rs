//! The installed application doubles as a small stdio connector. No Node/Python/Pi.
use super::{read_private, rpc_error, Connection, META_VERSION, VERSIONS};
use serde_json::Value;
use std::{
    io::{BufRead, Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

pub(super) fn entry_if_requested() -> bool {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.first().map(String::as_str) != Some("--mcp") {
        return false;
    }
    if arguments.len() != 3 || arguments[1] != "--connection-file" {
        eprintln!("Usage: Margin --mcp --connection-file <app-generated connection.json>");
        std::process::exit(2);
    }
    let connection = PathBuf::from(&arguments[2]);
    if !connection.is_absolute() {
        eprintln!("Margin MCP requires an absolute app-generated connection path");
        std::process::exit(2);
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    if let Err(error) = runtime.block_on(run(connection)) {
        eprintln!("{error}");
        std::process::exit(1);
    }
    true
}

pub(super) async fn forward(
    client: &reqwest::Client,
    path: &std::path::Path,
    request: &Value,
    legacy: &Arc<Mutex<String>>,
) -> Result<Option<Value>, String> {
    let connection: Connection = read_private(path)?;
    if connection.port == 0
        || connection.token.len() != 64
        || !connection.token.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(
            "Margin MCP connection is invalid; re-enable Agent connection in the app".into(),
        );
    }
    let modern = request["params"]["_meta"][META_VERSION].as_str();
    let version = modern
        .map(str::to_owned)
        .unwrap_or_else(|| legacy.lock().unwrap_or_else(|p| p.into_inner()).clone());
    let mut outgoing = client
        .post(format!("http://127.0.0.1:{}/mcp", connection.port))
        .header("Authorization", format!("Bearer {}", connection.token))
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", version)
        .header("Mcp-Method", request["method"].as_str().unwrap_or(""));
    if request["method"] == "tools/call" {
        if let Some(name) = request["params"]["name"].as_str() {
            use base64::Engine;
            let header = if name.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
                name.to_owned()
            } else {
                format!(
                    "=?base64?{}?=",
                    base64::engine::general_purpose::STANDARD.encode(name)
                )
            };
            outgoing = outgoing.header("Mcp-Name", header);
        }
    }
    let response = outgoing.json(request).send().await.map_err(|_| "Cannot connect to Margin. Open the app and enable Agent connection; existing exports may still be running.")?;
    if response.status() == reqwest::StatusCode::ACCEPTED {
        return Ok(None);
    }
    if response
        .content_length()
        .is_some_and(|n| n > super::MAX_RESULT as u64 + 128 * 1024)
    {
        return Err("Margin MCP response is too large".into());
    }
    let status = response.status();
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Cannot read Margin MCP response")?
    {
        if bytes.len() + chunk.len() > super::MAX_RESULT + 128 * 1024 {
            return Err("Margin MCP response is too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
        if status.is_success() {
            "Invalid Margin MCP response"
        } else {
            "Margin MCP access was refused; re-enable Agent connection in the app"
        }
    })?;
    if request["method"] == "initialize" {
        if let Some(version) = value["result"]["protocolVersion"]
            .as_str()
            .filter(|v| VERSIONS.contains(v))
        {
            *legacy.lock().unwrap_or_else(|p| p.into_inner()) = version.to_owned();
        }
    }
    Ok(Some(value))
}

async fn run(path: PathBuf) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| "Cannot initialize Margin MCP connector")?;
    let (sender, mut requests) = tokio::sync::mpsc::channel(8);
    std::thread::spawn(move || {
        let input = std::io::stdin();
        let mut input = input.lock();
        loop {
            let mut bytes = Vec::new();
            let result = input
                .by_ref()
                .take(super::transport::MAX_BODY as u64 + 1)
                .read_until(b'\n', &mut bytes);
            match result {
                Ok(0) | Err(_) => break,
                Ok(_) if bytes.len() > super::transport::MAX_BODY => {
                    eprintln!("Margin MCP request exceeds size limit");
                    break;
                }
                Ok(_) => {
                    if sender.blocking_send(bytes).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let legacy = Arc::new(Mutex::new(VERSIONS[1].to_owned()));
    let output = Arc::new(Mutex::new(std::io::stdout()));
    let mut tasks = tokio::task::JoinSet::new();
    while let Some(bytes) = requests.recv().await {
        while tasks.len() >= 8 {
            tasks.join_next().await;
        }
        let (client, path, legacy, output) =
            (client.clone(), path.clone(), legacy.clone(), output.clone());
        tasks.spawn(async move {
            let request: Value = match serde_json::from_slice(&bytes) {
                Ok(value) => value,
                Err(_) => {
                    send(&output, rpc_error(None, -32700, "Invalid JSON", None));
                    return;
                }
            };
            match forward(&client, &path, &request, &legacy).await {
                Ok(Some(result)) => send(&output, result),
                Ok(None) => {}
                Err(error) => {
                    if request.get("id").is_some() {
                        send(
                            &output,
                            rpc_error(request.get("id").cloned(), -31000, &error, None),
                        );
                    }
                }
            }
        });
    }
    tasks.abort_all(); // EOF means the client has gone; never keep a connector daemon.
    Ok(())
}

fn send(output: &Mutex<std::io::Stdout>, value: Value) {
    let mut output = output.lock().unwrap_or_else(|p| p.into_inner());
    let _ = writeln!(output, "{value}");
    let _ = output.flush();
}
