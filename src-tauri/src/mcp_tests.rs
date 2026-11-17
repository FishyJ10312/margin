use super::*;

#[test]
fn schemas_are_bounded_and_do_not_expose_generic_rpc_or_accounts() {
    let definitions = tools::definitions();
    assert_eq!(definitions.len(), 14);
    let names: std::collections::HashSet<_> = definitions
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), definitions.len());
    assert!(tools::validate("get_chatgpt_state", &json!({})).is_err());
    assert!(tools::validate("margin_import_photos", &json!({"paths":[]})).is_err());
    assert!(tools::validate(
        "margin_update_settings",
        &json!({"patch":{"borderPercent":31}})
    )
    .is_err());
    assert!(tools::validate("margin_update_settings", &json!({"patch":{"__proto__":{}}})).is_err());
    assert!(tools::validate("margin_update_settings", &json!({"patch":{"watermark":{"sizePercent":2,"text":"中文署名"},"ratio":{"width":4,"height":5}}})).is_ok());
}

#[tokio::test]
async fn legacy_and_modern_discovery_do_not_need_a_gui() {
    let root = tempfile::tempdir().unwrap();
    let service = AgentService::new(root.path().into(), PathBuf::from("/test/Margin"));
    let (_, response) = service.rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}), &HashMap::new()).await;
    assert_eq!(response.unwrap()["result"]["protocolVersion"], "2025-11-25");
    let headers = HashMap::from([
        ("mcp-protocol-version".into(), MODERN.into()),
        ("mcp-method".into(), "server/discover".into()),
    ]);
    let request = json!({"jsonrpc":"2.0","id":"discover","method":"server/discover","params":{"_meta":{META_VERSION:MODERN,META_CLIENT:{"name":"QA","version":"1"},META_CAPS:{}}}});
    let (status, response) = service.rpc(request.clone(), &headers).await;
    assert_eq!(status, 200);
    assert_eq!(response.unwrap()["result"]["supportedVersions"][0], MODERN);
    let (status, response) = service.rpc(request, &HashMap::new()).await;
    assert_eq!(status, 400);
    assert_eq!(response.unwrap()["error"]["code"], -32020);
}

#[tokio::test]
async fn opt_in_and_read_only_are_enforced_before_gui_dispatch() {
    let root = tempfile::tempdir().unwrap();
    let service = AgentService::new(root.path().into(), PathBuf::from("/test/Margin"));
    assert!(!service.status().await.enabled);
    assert!(service
        .call("margin_get_workspace", json!({}))
        .await
        .is_err());
    service.set_access(true, false).await.unwrap();
    assert!(service
        .call("margin_remove_photos", json!({"ids":["photo"]}))
        .await
        .unwrap_err()
        .contains("只读"));
    service.set_access(false, false).await.unwrap();
    assert!(!service.directory.join("connection.json").exists());
}

fn fake_gui(service: &Arc<AgentService>) {
    let weak = Arc::downgrade(service);
    service.set_dispatcher(Arc::new(move |command| {
        let service = weak.upgrade().unwrap();
        let result = if command.name == "margin_get_preview" {
            json!({"dataUrl":"data:image/png;base64,c3ludGhldGlj","width":12,"height":9})
        } else {
            json!({"photoCount":2,"tool":command.name})
        };
        service.complete(command.request_id, Some(result), None)
    }));
    service.mark_ready(true);
}

#[tokio::test]
async fn real_http_requires_private_auth_exact_host_and_no_browser_origin() {
    let root = tempfile::tempdir().unwrap();
    let service = AgentService::new(root.path().into(), PathBuf::from("/test/Margin"));
    fake_gui(&service);
    service.set_access(true, true).await.unwrap();
    let connection: Connection = read_private(&service.directory.join("connection.json")).unwrap();
    let url = format!("http://127.0.0.1:{}/mcp", connection.port);
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"margin_get_workspace","arguments":{}}});
    assert_eq!(
        client
            .post(&url)
            .header("Accept", "application/json, text/event-stream")
            .json(&request)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let post = || {
        client
            .post(&url)
            .bearer_auth(&connection.token)
            .header("Accept", "application/json, text/event-stream")
            .json(&request)
    };
    assert_eq!(
        post()
            .header("Origin", "https://untrusted.invalid")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        post()
            .header("Host", "untrusted.invalid")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .get(&url)
            .bearer_auth(&connection.token)
            .send()
            .await
            .unwrap()
            .status(),
        405
    );
    let result: Value = post().send().await.unwrap().json().await.unwrap();
    assert_eq!(result["result"]["structuredContent"]["photoCount"], 2);
    let legacy = Arc::new(Mutex::new(VERSIONS[1].into()));
    let result = stdio::forward(
        &client,
        &service.directory.join("connection.json"),
        &request,
        &legacy,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result["result"]["structuredContent"]["photoCount"], 2);
    service.set_access(false, true).await.unwrap();
    assert!(!service.directory.join("connection.json").exists());
}

#[tokio::test]
async fn tool_result_images_and_modern_headers_are_processed_without_generic_rpc() {
    let root = tempfile::tempdir().unwrap();
    let service = AgentService::new(root.path().into(), PathBuf::from("/test/Margin"));
    fake_gui(&service);
    service.set_access(true, true).await.unwrap();
    let headers = HashMap::from([
        ("mcp-protocol-version".into(), MODERN.into()),
        ("mcp-method".into(), "tools/call".into()),
        ("mcp-name".into(), "margin_get_preview".into()),
    ]);
    let request = json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"margin_get_preview","arguments":{},"_meta":{META_VERSION:MODERN,META_CLIENT:{"name":"QA","version":"1"},META_CAPS:{}}}});
    let (_, result) = service.rpc(request.clone(), &headers).await;
    let result = result.unwrap();
    assert_eq!(result["result"]["content"][1]["type"], "image");
    assert!(result["result"]["structuredContent"]
        .get("dataUrl")
        .is_none());
    assert_eq!(result["result"]["resultType"], "complete");
    let mut wrong = headers.clone();
    wrong.insert("mcp-name".into(), "margin_remove_photos".into());
    assert_eq!(service.rpc(request, &wrong).await.0, 400);
    let (_, result)=service.rpc(json!({"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"get_chatgpt_state","arguments":{}}}),&HashMap::new()).await;
    assert_eq!(result.unwrap()["error"]["code"], -32602);
}

#[tokio::test]
async fn mutation_gate_rejects_overlap_and_does_not_retain_completed_requests() {
    let root = tempfile::tempdir().unwrap();
    let service = AgentService::new(root.path().into(), PathBuf::from("/test/Margin"));
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    service.set_dispatcher(Arc::new(move |command| {
        sender.send(command).map_err(|_| "closed".into())
    }));
    service.mark_ready(true);
    service.set_access(true, true).await.unwrap();
    let first_service = service.clone();
    let first = tokio::spawn(async move {
        first_service
            .call(
                "margin_update_settings",
                json!({"patch":{"borderPercent":10}}),
            )
            .await
    });
    let command = receiver.recv().await.unwrap();
    assert!(service
        .call(
            "margin_update_settings",
            json!({"patch":{"borderPercent":20}})
        )
        .await
        .unwrap_err()
        .contains("另一个"));
    service
        .complete(
            command.request_id.clone(),
            Some(json!({"saved":true})),
            None,
        )
        .unwrap();
    assert_eq!(first.await.unwrap().unwrap()["saved"], true);
    assert!(service.pending.lock().unwrap().is_empty());
    assert!(service
        .complete(command.request_id, Some(json!({})), None)
        .is_err());
}

#[tokio::test]
async fn enabled_preference_restarts_with_fresh_token_and_secret_free_configuration() {
    let root = tempfile::tempdir().unwrap();
    let service = AgentService::new(root.path().into(), PathBuf::from("/test/Margin app"));
    service.set_access(true, true).await.unwrap();
    let old: Connection = read_private(&service.directory.join("connection.json")).unwrap();
    let status = service.status().await;
    assert!(!status.client_json.contains(&old.token));
    assert!(!status.codex_toml.contains(&old.token));
    let parsed: Value = serde_json::from_str(&status.client_json).unwrap();
    assert_eq!(parsed["mcpServers"]["margin"]["args"][0], "--mcp");
    drop(service);
    let restarted = AgentService::new(root.path().into(), PathBuf::from("/test/Margin app"));
    restarted.start_saved().await.unwrap();
    let new: Connection = read_private(&restarted.directory.join("connection.json")).unwrap();
    assert_ne!(old.token, new.token);
    restarted.set_access(false, true).await.unwrap();
    let restarted = AgentService::new(root.path().into(), PathBuf::from("/test/Margin app"));
    restarted.start_saved().await.unwrap();
    assert!(!restarted.status().await.enabled);
}

#[cfg(unix)]
#[test]
fn private_files_reject_symlinks_and_public_permissions() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("agent");
    write_private(
        &directory,
        "connection.json",
        &Connection {
            port: 12345,
            token: "a".repeat(64),
        },
    )
    .unwrap();
    let path = directory.join("connection.json");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(read_private::<Connection>(&path).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let linked = directory.join("linked.json");
    symlink(&path, &linked).unwrap();
    assert!(read_private::<Connection>(&linked).is_err());
    assert!(write_private(&directory, "linked.json", &json!({})).is_err());
}
