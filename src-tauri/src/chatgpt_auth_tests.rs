use super::*;
use jsonwebtoken::{encode, EncodingKey, Header};
use std::{
    io::{Read, Write},
    net::TcpListener as StdListener,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Default)]
struct MemoryVault(Mutex<HashMap<String, String>>);
impl Vault for MemoryVault {
    fn get(&self, id: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(id).cloned())
    }
    fn set(&self, id: &str, v: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(id.into(), v.into());
        Ok(())
    }
    fn remove(&self, id: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(id);
        Ok(())
    }
}
const KEY: &[u8] = include_bytes!("../test-fixtures/oauth-test-only.pem");

#[derive(Default)]
struct WaitingVault {
    inner: MemoryVault,
    reads: AtomicUsize,
    wait: Mutex<bool>,
    released: std::sync::Condvar,
}
impl WaitingVault {
    fn release(&self) {
        *self.wait.lock().unwrap() = false;
        self.released.notify_all();
    }
}
impl Vault for WaitingVault {
    fn get(&self, id: &str) -> Result<Option<String>, String> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        let mut wait = self.wait.lock().unwrap();
        while *wait {
            wait = self.released.wait(wait).unwrap();
        }
        self.inner.get(id)
    }
    fn set(&self, id: &str, value: &str) -> Result<(), String> {
        self.inner.set(id, value)
    }
    fn remove(&self, id: &str) -> Result<(), String> {
        self.inner.remove(id)
    }
}
const JWKS: &str = include_str!("../test-fixtures/oauth-test-jwks.json");
fn jwt(claims: &Value) -> String {
    let mut h = Header::new(Algorithm::RS256);
    h.kid = Some("liubai-test".into());
    encode(&h, claims, &EncodingKey::from_rsa_pem(KEY).unwrap()).unwrap()
}
#[derive(Clone)]
struct Request {
    path: String,
    authorization: String,
    body: String,
}
struct ServerState {
    requests: Mutex<Vec<Request>>,
    refreshes: AtomicUsize,
    revoke_fails: AtomicBool,
    refresh_invalid: AtomicBool,
    models_unavailable: AtomicBool,
    stream_mode: AtomicUsize,
    refresh_delay_ms: AtomicUsize,
    output: Mutex<String>,
}
struct Server {
    base: String,
    data: Arc<ServerState>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new() -> Self {
        let listener = StdListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let data = Arc::new(ServerState {
            requests: Mutex::new(vec![]),
            refreshes: AtomicUsize::new(0),
            revoke_fails: AtomicBool::new(false),
            refresh_invalid: AtomicBool::new(false),
            models_unavailable: AtomicBool::new(false),
            stream_mode: AtomicUsize::new(0),
            refresh_delay_ms: AtomicUsize::new(0),
            output: Mutex::new("{\"design\":\"测试叶片\"}".into()),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let running = stop.clone();
        let state = data.clone();
        let origin = base.clone();
        let thread = std::thread::spawn(move || {
            while !running.load(Ordering::Relaxed) {
                let (mut socket, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(3));
                        continue;
                    }
                    Err(_) => break,
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut expected = None;
                loop {
                    let mut b = [0u8; 4096];
                    let n = match socket.read(&mut b) {
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&b[..n]);
                    if bytes.len() > 1024 * 1024 {
                        break;
                    }
                    if expected.is_none() {
                        if let Some(h) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            let header = String::from_utf8_lossy(&bytes[..h]);
                            let len = header
                                .lines()
                                .find_map(|l| {
                                    l.split_once(':')
                                        .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                                        .map(|(_, v)| v.trim().parse::<usize>().unwrap())
                                })
                                .unwrap_or(0);
                            expected = Some(h + 4 + len);
                        }
                    }
                    if expected.is_some_and(|n| bytes.len() >= n) {
                        break;
                    }
                }
                let Some(h) = bytes.windows(4).position(|w| w == b"\r\n\r\n") else {
                    continue;
                };
                let header = String::from_utf8_lossy(&bytes[..h]);
                let path = header
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .to_owned();
                let authorization = header
                    .lines()
                    .find_map(|l| {
                        l.split_once(':')
                            .filter(|(k, _)| k.eq_ignore_ascii_case("authorization"))
                            .map(|(_, v)| v.trim().to_owned())
                    })
                    .unwrap_or_default();
                let body = String::from_utf8_lossy(&bytes[h + 4..]).into_owned();
                state.requests.lock().unwrap().push(Request {
                    path: path.clone(),
                    authorization: authorization.clone(),
                    body: body.clone(),
                });
                let (status,kind,result)=match path.as_str(){
                    "/discovery"=>(200,"application/json",json!({"issuer":origin,"jwks_uri":format!("{origin}/jwks"),"revocation_endpoint":format!("{origin}/revoke")}).to_string()),
                    "/jwks"=>(200,"application/json",JWKS.into()),
                    "/models" if state.models_unavailable.load(Ordering::Relaxed)=>(503,"application/json",json!({"detail":"temporary outage"}).to_string()),
                    "/models"=>{let tag=if authorization.contains("access-b"){"b"}else{"a"};(200,"application/json",json!({"models":[{"slug":format!("test-vision-{tag}"),"display_name":format!("Test Vision {tag}"),"visibility":"list"},{"slug":"hidden","visibility":"hide"}]}).to_string())},
                    "/token"=>{
                        let form=reqwest::Url::parse(&format!("http://localhost/?{body}")).unwrap().query_pairs().into_owned().collect::<HashMap<_,_>>();
                        if form["grant_type"]=="refresh_token"{
                            state.refreshes.fetch_add(1,Ordering::Relaxed);
                            std::thread::sleep(Duration::from_millis(state.refresh_delay_ms.load(Ordering::Relaxed) as u64));
                            if state.refresh_invalid.load(Ordering::Relaxed){(400,"application/json",json!({"error":"invalid_grant"}).to_string())}else{
                                let tag=form["refresh_token"].split('-').nth(1).unwrap();(200,"application/json",json!({"access_token":format!("access-{tag}-2"),"refresh_token":format!("refresh-{tag}-2"),"token_type":"Bearer","expires_in":3600}).to_string())
                            }
                        }else{
                            let pieces:Vec<_>=form["code"].split('|').collect();let tag=pieces[0];
                            let token=jwt(&json!({"iss":origin,"sub":format!("subject-{tag}"),"aud":form["client_id"],"exp":now()+3600,"nonce":pieces[1],"email":"same@example.invalid"}));
                            let scopes=if pieces[2]=="1"{SCOPES}else{"openid profile email"};
                            (200,"application/json",json!({"access_token":format!("access-{tag}-1"),"refresh_token":format!("refresh-{tag}-1"),"id_token":token,"token_type":"Bearer","expires_in":3600,"scope":scopes}).to_string())
                        }
                    },
                    "/responses" if state.stream_mode.load(Ordering::Relaxed)==3=>(200,"application/json; charset=utf-8",json!({"object":"response","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":state.output.lock().unwrap().as_str()}]}]}).to_string()),
                    "/responses" if state.stream_mode.load(Ordering::Relaxed)==4=>(200,"application/json",json!({"error":{"code":"subscription_sharing_usage_limit_exceeded","message":"sensitive provider message"}}).to_string()),
                    "/responses" if state.stream_mode.load(Ordering::Relaxed)==5=>(200,"text/html","<!doctype html><title>private gateway content</title>".into()),
                    "/responses" if state.stream_mode.load(Ordering::Relaxed)==6=>(200,"application/json",json!({"object":"response","status":"incomplete","output":[{"type":"message","content":[{"type":"output_text","text":"partial design"}]}]}).to_string()),
                    "/responses"=>{
                        let delta=json!({"type":"response.output_text.delta","delta":state.output.lock().unwrap().as_str()});
                        let final_event=match state.stream_mode.load(Ordering::Relaxed){1=>json!({"type":"response.failed","response":{"error":{"code":"subscription_sharing_usage_limit_exceeded","message":"sensitive message must not be shown"}}}),2=>json!({"type":"response.incomplete"}),_=>json!({"type":"response.completed","response":{"status":"completed","output":[]}})};
                        (200,"text/event-stream",format!("data: {delta}\r\n\r\ndata: {final_event}\r\n\r\n"))
                    },
                    "/revoke" if state.revoke_fails.load(Ordering::Relaxed)=>(503,"application/json","{}".into()),
                    "/revoke"=>(200,"application/json",String::new()),
                    _=>(404,"application/json","{}".into()),
                };
                let response=format!("HTTP/1.1 {status} Test\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{result}",result.len());
                let _ = socket.write_all(response.as_bytes());
            }
        });
        Self {
            base,
            data,
            stop,
            thread: Some(thread),
        }
    }
    fn auth(&self, directory: PathBuf, vault: Arc<MemoryVault>) -> ChatGpt {
        ChatGpt::with_vault(
            directory,
            vault,
            Endpoints {
                issuer: self.base.clone(),
                resource: RESOURCE.into(),
                authorize: format!("{}/authorize", self.base),
                token: format!("{}/token", self.base),
                discovery: format!("{}/discovery", self.base),
                models: format!("{}/models", self.base),
                responses: format!("{}/responses", self.base),
            },
        )
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}
async fn login(auth: &ChatGpt, tag: &str, plan: bool) -> ConnectionState {
    let tag = tag.to_owned();
    auth.sign_in(None, false, move |raw| {
        let url = reqwest::Url::parse(&raw).unwrap();
        let q = url.query_pairs().into_owned().collect::<HashMap<_, _>>();
        assert_eq!(q["client_id"], "dynamic_agent_client");
        assert_eq!(q["agent_name_hint"], "Liubai");
        assert_eq!(q["code_challenge_method"], "S256");
        assert!(q["ext_agent_host_id"].starts_with("urn:uuid:"));
        let mut callback = reqwest::Url::parse(&q["redirect_uri"]).unwrap();
        callback
            .query_pairs_mut()
            .append_pair("state", &q["state"])
            .append_pair("client_id", &format!("oaiapp_{tag}"))
            .append_pair(
                "code",
                &format!("{tag}|{}|{}", q["nonce"], if plan { 1 } else { 0 }),
            );
        tokio::spawn(async move {
            let _ = reqwest::get(callback).await;
        });
        Ok(())
    })
    .await
    .unwrap()
}

#[test]
fn oauth_pkce_and_model_catalog_contract() {
    assert_eq!(
        challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    let v = json!({"models":[{"slug":"gpt-6.1-sol","display_name":"GPT 6.1","visibility":"list"},{"slug":"internal","visibility":"hide"},{"slug":"gpt-6.1-sol","visibility":"list"}]});
    let models = parse_models(&v).unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].slug, "gpt-6.1-sol");
    assert!(!plan_enabled(&parse_scopes("openid profile").unwrap()));
    assert!(plan_enabled(&parse_scopes(SCOPES).unwrap()));
}

#[test]
fn verifies_signature_issuer_audience_expiry_nonce_and_authorized_party() {
    let keys: JwkSet = serde_json::from_str(JWKS).unwrap();
    let claims =
        json!({"iss":ISSUER,"sub":"subject-a","aud":"oaiapp_a","exp":now()+3600,"nonce":"n"});
    assert!(verify_identity(&jwt(&claims), "oaiapp_a", Some("n"), ISSUER, &keys).is_ok());
    for (name, value) in [
        ("iss", json!("https://evil.invalid")),
        ("aud", json!("oaiapp_b")),
        ("exp", json!(now() - 3600)),
        ("nonce", json!("wrong")),
        ("azp", json!("oaiapp_b")),
    ] {
        let mut changed = claims.clone();
        changed[name] = value;
        assert!(
            verify_identity(&jwt(&changed), "oaiapp_a", Some("n"), ISSUER, &keys).is_err(),
            "{name}"
        );
    }
    let mut multi = claims.clone();
    multi["aud"] = json!(["oaiapp_a", "oaiapp_b"]);
    assert!(verify_identity(&jwt(&multi), "oaiapp_a", Some("n"), ISSUER, &keys).is_err());
    let hmac = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(b"not-an-openai-key"),
    )
    .unwrap();
    assert!(verify_identity(&hmac, "oaiapp_a", Some("n"), ISSUER, &keys).is_err());
    let mut damaged = jwt(&claims).into_bytes();
    let index = damaged.len() - 8;
    damaged[index] = if damaged[index] == b'A' { b'B' } else { b'A' };
    assert!(verify_identity(
        std::str::from_utf8(&damaged).unwrap(),
        "oaiapp_a",
        Some("n"),
        ISSUER,
        &keys
    )
    .is_err());
}

#[tokio::test]
async fn official_oauth_contract_round_trip_keeps_credentials_out_of_metadata() {
    let server = Server::new();
    let directory = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let auth = server.auth(directory.path().into(), vault.clone());
    let state = login(&auth, "a", true).await;
    let id = state.active_id.unwrap();
    assert!(state.accounts[0].plan_enabled);
    let models = auth.models(&id).await.unwrap();
    assert_eq!(models.len(), 1);
    let output = auth
        .complete(
            &id,
            &models[0].slug,
            "Make a minimal border",
            &["data:image/jpeg;base64,eA=="],
        )
        .await
        .unwrap();
    assert!(output.contains("测试叶片"));
    let meta = fs::read_to_string(directory.path().join("accounts.json")).unwrap();
    for secret in [
        "access-a-1",
        "refresh-a-1",
        "accessToken",
        "refreshToken",
        "idToken",
    ] {
        assert!(!meta.contains(secret))
    }
    let requests = server.data.requests.lock().unwrap();
    let token = requests.iter().find(|r| r.path == "/token").unwrap();
    assert!(token.body.contains("client_id=oaiapp_a"));
    assert!(token.body.contains("code_verifier="));
    assert!(!token.body.contains("dynamic_agent_client"));
    let body: Value = serde_json::from_str(
        &requests
            .iter()
            .find(|r| r.path == "/responses")
            .unwrap()
            .body,
    )
    .unwrap();
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    assert_eq!(body["input"][0]["content"][1]["type"], "input_image");
    for field in [
        "max_output_tokens",
        "tools",
        "temperature",
        "previous_response_id",
        "metadata",
    ] {
        assert!(body.get(field).is_none())
    }
    let public = serde_json::to_string(&auth.state().unwrap()).unwrap();
    assert!(!public.contains("token"));
    assert!(!public.contains("client_id"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(directory.path().join("accounts.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[tokio::test]
async fn identity_only_login_never_sends_an_inference_request() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let auth = server.auth(dir.path().into(), Arc::new(MemoryVault::default()));
    let s = login(&auth, "a", false).await;
    let id = s.active_id.unwrap();
    assert!(s.accounts[0].connected);
    assert!(!s.accounts[0].plan_enabled);
    assert!(auth.models(&id).await.err().unwrap().contains("尚未授权"));
    assert!(!server
        .data
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.path == "/responses" || r.path == "/models"));
}

#[tokio::test]
async fn completed_json_response_is_accepted_without_resending() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let auth = server.auth(dir.path().into(), Arc::new(MemoryVault::default()));
    let id = login(&auth, "a", true).await.active_id.unwrap();
    server.data.stream_mode.store(3, Ordering::Relaxed);
    let result = auth
        .complete(
            &id,
            "test-vision-a",
            "test",
            &["data:image/jpeg;base64,eA=="],
        )
        .await;
    assert_eq!(result.unwrap(), *server.data.output.lock().unwrap());
    assert_eq!(
        server
            .data
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.path == "/responses")
            .count(),
        1
    );
}

#[tokio::test]
async fn stalled_system_vault_returns_a_bounded_error_and_does_not_stack_reads() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(WaitingVault::default());
    let mut auth = ChatGpt::with_vault(
        dir.path().into(),
        vault.clone(),
        Endpoints {
            issuer: server.base.clone(),
            resource: RESOURCE.into(),
            authorize: format!("{}/authorize", server.base),
            token: format!("{}/token", server.base),
            discovery: format!("{}/discovery", server.base),
            models: format!("{}/models", server.base),
            responses: format!("{}/responses", server.base),
        },
    );
    let id = login(&auth, "a", true).await.active_id.unwrap();
    auth.credentials.lock().unwrap().clear();
    auth.vault_read_timeout = Duration::from_millis(20);
    *vault.wait.lock().unwrap() = true;
    let result = tokio::time::timeout(Duration::from_millis(300), auth.vault_get(&id)).await;
    if result.is_err() {
        vault.release();
        panic!("A blocked OS vault must return a bounded error");
    }
    assert!(result.unwrap().err().unwrap().contains("超时"));
    let repeated = tokio::time::timeout(Duration::from_millis(100), auth.vault_get(&id)).await;
    vault.release();
    assert!(repeated.unwrap().err().unwrap().contains("仍在等待"));
    assert_eq!(
        vault.reads.load(Ordering::Relaxed),
        1,
        "Timed-out reads must not accumulate threads"
    );
}

#[tokio::test]
async fn buffered_errors_html_and_incomplete_results_are_never_designs() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let auth = server.auth(dir.path().into(), Arc::new(MemoryVault::default()));
    let id = login(&auth, "a", true).await.active_id.unwrap();
    for (mode, expected, shape) in [
        (4, "用量已达上限", "error"),
        (5, "网页或非模型内容", "html"),
        (6, "尚未完成", "response-json"),
    ] {
        server.data.stream_mode.store(mode, Ordering::Relaxed);
        let error = auth
            .complete(
                &id,
                "test-vision-a",
                "test",
                &["data:image/jpeg;base64,eA=="],
            )
            .await
            .unwrap_err();
        assert!(error.contains(expected), "{error}");
        let diagnostic = auth.state().unwrap().diagnostic.unwrap();
        assert_eq!(diagnostic.body_shape, shape);
        let public = serde_json::to_string(&diagnostic).unwrap();
        assert!(
            !public.contains("sensitive")
                && !public.contains("private")
                && !public.contains("partial design")
        );
    }
    assert_eq!(
        server
            .data
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.path == "/responses")
            .count(),
        3
    );
}

#[tokio::test]
async fn accounts_with_same_email_keep_separate_models_and_bearer_credentials() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let auth = server.auth(dir.path().into(), Arc::new(MemoryVault::default()));
    let a = login(&auth, "a", true).await.active_id.unwrap();
    let b = login(&auth, "b", true).await.active_id.unwrap();
    assert_ne!(a, b);
    auth.models(&a).await.unwrap();
    let models = auth.models(&b).await.unwrap();
    assert_eq!(models[0].slug, "test-vision-b");
    assert!(auth
        .complete(
            &b,
            "test-vision-a",
            "test",
            &["data:image/jpeg;base64,eA=="]
        )
        .await
        .is_err());
    auth.complete(
        &b,
        "test-vision-b",
        "test",
        &["data:image/jpeg;base64,eA=="],
    )
    .await
    .unwrap();
    let r = server.data.requests.lock().unwrap();
    assert_eq!(r.iter().filter(|r| r.path == "/responses").count(), 1);
    assert_eq!(
        r.iter()
            .find(|r| r.path == "/responses")
            .unwrap()
            .authorization,
        "Bearer access-b-1"
    );
}

#[tokio::test]
async fn concurrent_expiry_refreshes_once_and_saves_rotated_refresh_token() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let auth = server.auth(dir.path().into(), vault.clone());
    let id = login(&auth, "a", true).await.active_id.unwrap();
    auth.credentials
        .lock()
        .unwrap()
        .get_mut(&id)
        .unwrap()
        .expires_at = now() - 1;
    let (a, b) = tokio::join!(auth.models(&id), auth.models(&id));
    assert!(a.is_ok() && b.is_ok());
    assert_eq!(server.data.refreshes.load(Ordering::Relaxed), 1);
    let saved = vault.0.lock().unwrap().get(&id).unwrap().clone();
    assert!(saved.contains("refresh-a-2"));
    assert!(!saved.contains("refresh-a-1"));
}

#[tokio::test]
async fn cancelling_a_caller_does_not_interrupt_rotating_token_commit() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let auth = server.auth(dir.path().into(), vault.clone());
    let id = login(&auth, "a", true).await.active_id.unwrap();
    auth.credentials
        .lock()
        .unwrap()
        .get_mut(&id)
        .unwrap()
        .expires_at = now() - 1;
    server.data.refresh_delay_ms.store(120, Ordering::Relaxed);
    let running = auth.clone();
    let selected = id.clone();
    let task = tokio::spawn(async move { running.models(&selected).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while server.data.refreshes.load(Ordering::Relaxed) == 0 {
            tokio::time::sleep(Duration::from_millis(3)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    let _ = task.await;
    let models = auth.models(&id).await.unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(server.data.refreshes.load(Ordering::Relaxed), 1);
    assert!(vault
        .0
        .lock()
        .unwrap()
        .get(&id)
        .unwrap()
        .contains("refresh-a-2"));
}

#[tokio::test]
async fn temporary_outage_preserves_login_but_revoked_refresh_disables_it() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let auth = server.auth(dir.path().into(), vault.clone());
    let id = login(&auth, "a", true).await.active_id.unwrap();
    server
        .data
        .models_unavailable
        .store(true, Ordering::Relaxed);
    assert!(auth.models(&id).await.is_err());
    assert!(auth.state().unwrap().accounts[0].connected);
    assert!(vault.0.lock().unwrap().contains_key(&id));
    server.data.refresh_invalid.store(true, Ordering::Relaxed);
    auth.credentials
        .lock()
        .unwrap()
        .get_mut(&id)
        .unwrap()
        .expires_at = now() - 1;
    assert!(auth.models(&id).await.is_err());
    assert!(!auth.state().unwrap().accounts[0].connected);
    assert!(!vault.0.lock().unwrap().contains_key(&id));
}

#[tokio::test]
async fn disconnect_reports_remote_failure_and_clears_only_selected_account() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let vault = Arc::new(MemoryVault::default());
    let auth = server.auth(dir.path().into(), vault.clone());
    let a = login(&auth, "a", true).await.active_id.unwrap();
    let b = login(&auth, "b", true).await.active_id.unwrap();
    server.data.revoke_fails.store(true, Ordering::Relaxed);
    let result = auth.disconnect(&a).await.unwrap();
    assert!(!result.remote_revoked);
    assert!(!vault.0.lock().unwrap().contains_key(&a));
    assert!(vault.0.lock().unwrap().contains_key(&b));
    assert!(
        result
            .state
            .accounts
            .iter()
            .find(|v| v.id == b)
            .unwrap()
            .connected
    );
    assert!(auth
        .database()
        .unwrap()
        .accounts
        .iter()
        .any(|v| v.id == a && v.client_id == "oaiapp_a"));
}

#[tokio::test]
async fn cancelled_login_and_rejected_callback_never_exchange_code() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let auth = Arc::new(server.auth(dir.path().into(), Arc::new(MemoryVault::default())));
    let (tx, rx) = tokio::sync::oneshot::channel();
    let running = auth.clone();
    let task = tokio::spawn(async move {
        running
            .sign_in(None, false, move |url| {
                let _ = tx.send(url);
                Ok(())
            })
            .await
    });
    rx.await.unwrap();
    auth.cancel_login();
    assert!(task.await.unwrap().err().unwrap().contains("取消"));
    assert!(auth.state().unwrap().accounts.is_empty());
    let result = auth
        .sign_in(None, false, |raw| {
            let url = reqwest::Url::parse(&raw).unwrap();
            let q = url.query_pairs().into_owned().collect::<HashMap<_, _>>();
            let mut callback = reqwest::Url::parse(&q["redirect_uri"]).unwrap();
            callback
                .query_pairs_mut()
                .append_pair("state", &q["state"])
                .append_pair("error", "access_denied");
            tokio::spawn(async move {
                let _ = reqwest::get(callback).await;
            });
            Ok(())
        })
        .await;
    assert!(result.err().unwrap().contains("未完成"));
    assert!(!server
        .data
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.path == "/token"));
}

#[tokio::test]
async fn wrong_state_is_rejected_before_valid_callback_and_issued_id_is_bound() {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let running = tokio::spawn(receive_callback(
        listener,
        "correct-state",
        Some("oaiapp_a"),
        port,
    ));
    let bad = reqwest::get(format!(
        "http://127.0.0.1:{port}/auth/callback?state=wrong&code=x&client_id=oaiapp_a"
    ))
    .await
    .unwrap();
    assert_eq!(bad.status().as_u16(), 400);
    let _ = reqwest::get(format!(
        "http://127.0.0.1:{port}/auth/callback?state=correct-state&code=x&client_id=oaiapp_b"
    ))
    .await;
    assert!(running.await.unwrap().unwrap_err().contains("不一致"));
}

#[tokio::test]
async fn late_usage_failure_is_not_returned_as_a_partial_design() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let auth = server.auth(dir.path().into(), Arc::new(MemoryVault::default()));
    let id = login(&auth, "a", true).await.active_id.unwrap();
    server.data.stream_mode.store(1, Ordering::Relaxed);
    let result = auth
        .complete(
            &id,
            "test-vision-a",
            "test",
            &["data:image/jpeg;base64,eA=="],
        )
        .await;
    let error = result.unwrap_err();
    assert!(error.contains("用量已达上限"));
    assert!(!error.contains("sensitive"));
    assert_eq!(
        server
            .data
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.path == "/responses")
            .count(),
        1
    );
}

#[tokio::test]
async fn frame_service_uses_chatgpt_for_design_and_keeps_image_provider_independent() {
    let server = Server::new();
    let dir = tempfile::tempdir().unwrap();
    let mut ai = crate::art_ai::ArtAi::new(dir.path().into());
    ai.chatgpt = server.auth(dir.path().join("chatgpt"), Arc::new(MemoryVault::default()));
    let id = login(&ai.chatgpt, "a", true).await.active_id.unwrap();
    let suggestions:Vec<_>=(0..3).map(|i|json!({"title":format!("方案 {i}"),"rationale":"呼应照片的青绿线条","frameColor":"#F3ECDD","borderPercent":5,"decoration":{"kind":"none","color":"#456789","opacity":0.,"scale":1.},"artwork":{"elements":[{"kind":"curve","x":0.,"y":0.,"width":100.,"height":100.,"rotation":0.,"color":"#345678","opacity":0.8,"strokeWidth":2.,"filled":false,"points":[[10.,20.],[200.,40.],[700.,30.],[990.,20.]]}]}})).collect();
    *server.data.output.lock().unwrap() = json!({"suggestions":suggestions}).to_string();
    let prepared = ai
        .prepare("data:image/jpeg;base64,eA==".into(), 900, 600)
        .unwrap();
    let config = crate::art_ai::AiConfig {
        provider: "chatgpt".into(),
        chatgpt_account_id: Some(id),
        model: "test-vision-a".into(),
        ..Default::default()
    };
    assert_eq!(config.image.provider, "codex");
    let result = ai
        .generate(&prepared.token, config, "must-not-be-used-as-a-key".into())
        .await
        .unwrap();
    assert_eq!(result.len(), 3);
    let requests = server.data.requests.lock().unwrap();
    assert!(requests.iter().any(|r| r.path == "/responses"));
    assert!(!requests
        .iter()
        .any(|r| r.path.contains("images/") || r.path.contains("chat/completions")));
    assert!(!requests
        .iter()
        .any(|r| r.authorization.contains("must-not-be-used")));
}

#[test]
fn sse_handles_split_utf8_and_requires_completed_event() {
    let delta = json!({"type":"response.output_text.delta","delta":"中文署名附近的线条"});
    let complete =
        json!({"type":"response.completed","response":{"status":"completed","output":[]}});
    let bytes = format!("data: {delta}\r\n\r\ndata: {complete}\r\n\r\n");
    let mut stream = ResponseStream::new();
    let mut result = None;
    for b in bytes.as_bytes() {
        if let Some(v) = stream.push(&[*b]).unwrap() {
            result = Some(v)
        }
    }
    assert_eq!(result.as_deref(), Some("中文署名附近的线条"));
    let mut stream = ResponseStream::new();
    assert!(stream
        .push(format!("data: {delta}\n\n").as_bytes())
        .unwrap()
        .is_none());
    assert!(stream.push(b"data: [DONE]\n\n").is_err());
    let mut stream = ResponseStream::new();
    assert!(stream
        .push(b"data: {\"type\":\"response.incomplete\"}\n\n")
        .is_err());
}

#[test]
#[ignore = "Explicit native OS vault check; writes and removes only a random synthetic QA entry"]
fn native_os_vault_round_trip_large_synthetic_secret() {
    let id = format!("qa-{}", uuid::Uuid::new_v4());
    struct Cleanup(String);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = SystemVault.remove(&self.0);
        }
    }
    let _cleanup = Cleanup(id.clone());
    let secret = "Liubai fake OAuth credential · 测试 ".repeat(400);
    SystemVault.set(&id, &secret).unwrap();
    assert_eq!(SystemVault.get(&id).unwrap(), Some(secret));
    SystemVault.remove(&id).unwrap();
    assert!(SystemVault.get(&id).unwrap().is_none());
}
