//! HTTP retry boundary with real local catalog/export storage, synthetic photos
//! and an in-process stand-in for the window's native-command adapter.
use super::*;
use crate::{jobs::Backend, model::RenderSettings};

struct Fixture {
    _root: tempfile::TempDir,
    service: Arc<AgentService>,
    connection: Connection,
    client: reqwest::Client,
    output: PathBuf,
    hold_next: Arc<AtomicBool>,
    held: Arc<Mutex<Option<(String, Result<Value, String>)>>>,
}

impl Fixture {
    async fn new(photo_count: usize) -> Self {
        Self::with_timeout(photo_count, Duration::from_secs(110)).await
    }

    async fn with_timeout(photo_count: usize, timeout: Duration) -> Self {
        let root = tempfile::tempdir().unwrap();
        let backend = Backend::new(root.path().join("synthetic-profile")).unwrap();
        let paths = (0..photo_count)
            .map(|index| {
                let path = root.path().join(format!("synthetic-{index}.png"));
                image::RgbImage::from_pixel(64, 48, image::Rgb([90, 140, 180]))
                    .save(&path)
                    .unwrap();
                path.to_string_lossy().into_owned()
            })
            .collect();
        assert_eq!(
            backend.import_photos(paths).unwrap().photos.len(),
            photo_count
        );
        let mut service = AgentService::new(
            root.path().join("mcp-profile"),
            PathBuf::from("/test/Margin"),
        );
        Arc::get_mut(&mut service).unwrap().response_timeout = timeout;
        let weak = Arc::downgrade(&service);
        let hold_next = Arc::new(AtomicBool::new(false));
        let held = Arc::new(Mutex::new(None));
        let (hold_dispatch, held_dispatch) = (hold_next.clone(), held.clone());
        service.set_dispatcher(Arc::new(move |command| {
            let result = (|| -> Result<Value, String> {
                let snapshot = backend.snapshot()?;
                match command.name.as_str() {
                    "margin_get_workspace" => Ok(json!({"photos": snapshot.photos.iter().map(|p|json!({"id":p.id,"name":p.name})).collect::<Vec<_>>(), "selection":snapshot.photo_selection,"history":snapshot.catalog_history})),
                    "margin_get_export_status" => Ok(json!({"batch": snapshot.batch})),
                    "margin_export_photos" => {
                        let ids = snapshot.photos.iter().map(|p| p.id.clone()).collect();
                        let batch = backend.start_batch(command.arguments["outputDirectory"].as_str().unwrap().into(), RenderSettings::default(), ids, Arc::new(|_| {}))?;
                        Ok(json!({"batch":batch}))
                    }
                    "margin_remove_photos" => {
                        let receipt = backend.remove_photos(snapshot.photo_selection.ids)?;
                        Ok(json!({"receipt":receipt}))
                    }
                    _ => Err("Unsupported fixture command".into()),
                }
            })();
            if command.name == "margin_export_photos" && hold_dispatch.swap(false, Ordering::AcqRel) {
                *held_dispatch.lock().unwrap() = Some((command.request_id, result));
                return Ok(());
            }
            let service = weak.upgrade().ok_or("Fixture closed")?;
            match result {
                Ok(value) => service.complete(command.request_id, Some(value), None),
                Err(error) => service.complete(command.request_id, None, Some(error)),
            }
        }));
        service.mark_ready(true);
        service.set_access(true, true).await.unwrap();
        let connection = read_private(&service.directory.join("connection.json")).unwrap();
        Self {
            output: root.path().join("exports"),
            _root: root,
            service,
            connection,
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            hold_next,
            held,
        }
    }

    async fn post(&self, request: Value) -> reqwest::Response {
        self.client
            .post(format!("http://127.0.0.1:{}/mcp", self.connection.port))
            .bearer_auth(&self.connection.token)
            .header("Accept", "application/json, text/event-stream")
            .json(&request)
            .send()
            .await
            .unwrap()
    }

    async fn call(&self, id: u64, name: &str, arguments: Value, operation: Option<&str>) -> Value {
        let mut request = json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}});
        if let Some(operation) = operation {
            request["params"]["_meta"]["com.fishyj.margin/operationId"] = json!(operation);
        }
        let response: Value = self.post(request).await.json().await.unwrap();
        assert!(response.get("error").is_none(), "{response}");
        assert_ne!(response["result"]["isError"], true, "{response}");
        response["result"]["structuredContent"].clone()
    }

    async fn completed_batch(&self) -> Value {
        for _ in 0..100 {
            let value = self
                .call(90, "margin_get_export_status", json!({}), None)
                .await;
            if value["batch"]["status"] == "completed" {
                return value["batch"].clone();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("Synthetic export did not complete");
    }
}

#[tokio::test]
async fn lost_removal_ack_replay_preserves_the_twenty_entry_history_and_selection() {
    let fixture = Fixture::new(24).await;
    let source_bytes = fs::read(fixture._root.path().join("synthetic-0.png")).unwrap();
    for id in 1..20 {
        fixture
            .call(id, "margin_remove_photos", json!({}), None)
            .await;
    }
    let args = json!({"operationId":"synthetic-removal-lost-ack"});
    drop(fixture.post(json!({"jsonrpc":"2.0","id":20,"method":"tools/call","params":{"name":"margin_remove_photos","arguments":args}})).await);
    let before = fixture
        .call(21, "margin_get_workspace", json!({}), None)
        .await;
    assert_eq!(before["photos"].as_array().unwrap().len(), 4);
    assert_eq!(before["history"]["undo"].as_array().unwrap().len(), 20);
    fixture.call(22, "margin_remove_photos", args, None).await;
    let after = fixture
        .call(23, "margin_get_workspace", json!({}), None)
        .await;
    assert_eq!(
        after, before,
        "Replay must not remove the next selection or evict the oldest undo record"
    );
    for index in 0..24 {
        assert_eq!(
            fs::read(fixture._root.path().join(format!("synthetic-{index}.png"))).unwrap(),
            source_bytes
        );
    }
    fixture.service.set_access(false, true).await.unwrap();
}

#[tokio::test]
async fn timed_out_export_keeps_its_gate_until_late_completion_and_can_be_queried() {
    let fixture = Fixture::with_timeout(1, Duration::from_millis(30)).await;
    fixture.hold_next.store(true, Ordering::Release);
    let args = json!({"outputDirectory":fixture.output,"scope":"all","operationId":"synthetic-delayed-export"});
    let response: Value = fixture.post(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"margin_export_photos","arguments":args}})).await.json().await.unwrap();
    assert_eq!(response["result"]["isError"], true);
    let batch = fixture.completed_batch().await;
    for operation in ["synthetic-delayed-export", "different-operation"] {
        let request = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"margin_export_photos","arguments":{"outputDirectory":fixture.output,"scope":"all","operationId":operation}}});
        let result: Value = fixture.post(request).await.json().await.unwrap();
        assert_eq!(
            result["result"]["isError"], true,
            "An uncertain write must not be repeated: {result}"
        );
    }
    let (request_id, result) = fixture.held.lock().unwrap().take().unwrap();
    fixture
        .service
        .complete(request_id, Some(result.unwrap()), None)
        .unwrap();
    let replay = fixture.call(3, "margin_export_photos", args, None).await;
    assert_eq!(replay["batch"]["id"], batch["id"]);
    assert_eq!(fs::read_dir(&fixture.output).unwrap().count(), 1);
    let conflicting_directory = fixture._root.path().join("different-export");
    let conflict: Value = fixture.post(json!({"jsonrpc":"2.0","id":43,"method":"tools/call","params":{"name":"margin_export_photos","arguments":{"operationId":"synthetic-delayed-export","outputDirectory":conflicting_directory,"scope":"all"}}})).await.json().await.unwrap();
    assert_eq!(conflict["result"]["isError"], true);
    assert!(
        !conflicting_directory.exists(),
        "Reusing a key for different arguments must fail before any side effect"
    );
    fixture.service.set_access(false, true).await.unwrap();
}

#[tokio::test]
async fn full_retry_history_rejects_new_writes_without_forgetting_an_old_receipt() {
    let fixture = Fixture::new(1).await;
    let first = fixture
        .call(1, "margin_remove_photos", json!({}), Some("bounded-0"))
        .await;
    for index in 1..128 {
        fixture
            .call(
                index + 1,
                "margin_remove_photos",
                json!({}),
                Some(&format!("bounded-{index}")),
            )
            .await;
    }
    let overflow: Value = fixture.post(json!({"jsonrpc":"2.0","id":129,"method":"tools/call","params":{"name":"margin_remove_photos","arguments":{"operationId":"one-too-many"}}})).await.json().await.unwrap();
    assert_eq!(overflow["result"]["isError"], true);
    assert!(overflow["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("history is full"));
    assert_eq!(
        fixture
            .call(130, "margin_remove_photos", json!({}), Some("bounded-0"))
            .await,
        first
    );
    let workspace = fixture
        .call(131, "margin_get_workspace", json!({}), None)
        .await;
    assert_eq!(workspace["history"]["undo"].as_array().unwrap().len(), 1);
    fixture.service.set_access(false, true).await.unwrap();
}

#[tokio::test]
async fn lost_export_ack_replay_does_not_start_a_second_batch() {
    let fixture = Fixture::new(1).await;
    let args = json!({"outputDirectory":fixture.output,"scope":"all"});
    // The client receives no tool result. A new client later repeats the same
    // operation with a different JSON-RPC request ID.
    let first = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"margin_export_photos","arguments":args,"_meta":{"com.fishyj.margin/operationId":"synthetic-export-lost-ack"}}});
    drop(fixture.post(first).await);
    let completed = fixture.completed_batch().await;
    let replay = fixture
        .call(
            42,
            "margin_export_photos",
            args,
            Some("synthetic-export-lost-ack"),
        )
        .await;
    assert_eq!(
        replay["batch"]["id"], completed["id"],
        "A lost reply must not create a second export batch"
    );
    fixture.completed_batch().await;
    assert_eq!(fs::read_dir(&fixture.output).unwrap().count(), 1);
    fixture.service.set_access(false, true).await.unwrap();
}
