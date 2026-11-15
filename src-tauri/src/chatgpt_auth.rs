//! Official Sign in with ChatGPT public-client flow. No Codex-token extraction.
use crate::{
    chatgpt_stream::{self, ResponseStream},
    chatgpt_vault::{SystemVault, Vault},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{watch, Mutex as AsyncMutex},
};

const ISSUER: &str = "https://auth.openai.com";
const RESOURCE: &str = "https://api.openai.com/v1";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
const LOCK_ERROR: &str = "无法读取 ChatGPT 账户状态";
const RELOGIN: &str = "ChatGPT 授权已失效，请重新登录";
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random() -> Result<String, String> {
    let mut b = [0u8; 32];
    SystemRandom::new()
        .fill(&mut b)
        .map_err(|_| "无法创建安全登录请求")?;
    Ok(URL_SAFE_NO_PAD.encode(b))
}
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, verifier.as_bytes()).as_ref())
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 200
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./:".contains(&b))
}
fn token_valid(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32768 && !s.chars().any(char::is_control)
}

#[derive(Clone)]
struct Endpoints {
    issuer: String,
    resource: String,
    authorize: String,
    token: String,
    discovery: String,
    models: String,
    responses: String,
}
impl Default for Endpoints {
    fn default() -> Self {
        Self {
            issuer: ISSUER.into(),
            resource: RESOURCE.into(),
            authorize: format!("{ISSUER}/api/accounts/authorize"),
            token: format!("{ISSUER}/api/accounts/oauth/token"),
            discovery: format!("{ISSUER}/.well-known/openid-configuration"),
            models: format!("{RESOURCE}/models"),
            responses: format!("{RESOURCE}/responses"),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Registration {
    id: String,
    client_id: String,
    subject: Option<String>,
    label: String,
    connected: bool,
    plan_enabled: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Database {
    host_id: String,
    active_id: Option<String>,
    accounts: Vec<Registration>,
}
impl Default for Database {
    fn default() -> Self {
        Self {
            host_id: format!("urn:uuid:{}", uuid::Uuid::new_v4()),
            active_id: None,
            accounts: vec![],
        }
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub label: String,
    pub connected: bool,
    pub plan_enabled: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionState {
    pub accounts: Vec<Account>,
    pub active_id: Option<String>,
    pub diagnostic: Option<Diagnostic>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub status: u16,
    pub code: Option<String>,
    pub param: Option<String>,
    pub request_id: Option<String>,
    pub body_shape: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Credential {
    client_id: String,
    subject: String,
    access_token: String,
    refresh_token: String,
    id_token: String,
    scopes: Vec<String>,
    expires_at: u64,
    earliest_refresh_at: u64,
}
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    token_type: String,
    expires_in: u64,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    earliest_refresh_at: Option<Value>,
}
#[derive(Deserialize)]
struct Identity {
    sub: String,
    aud: Value,
    #[serde(default)]
    azp: Option<String>,
    #[serde(default)]
    nonce: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    name: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub slug: String,
    pub display_name: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Disconnect {
    pub state: ConnectionState,
    pub remote_revoked: bool,
}
struct AttemptGuard<'a>(&'a Mutex<Option<watch::Sender<bool>>>);
// The OS may keep a credential dialog/IPC pending after the caller times out.
// Keep a permit in the blocking operation itself, so retries cannot pile up.
struct VaultReadGuard(Arc<AtomicBool>);
impl Drop for VaultReadGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl Drop for AttemptGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = None
        }
    }
}

#[derive(Clone)]
pub struct ChatGpt {
    vault_read_timeout: Duration,
    vault_reading: Arc<AtomicBool>,
    directory: PathBuf,
    db: Arc<Mutex<Database>>,
    load_error: Option<String>,
    vault: Arc<dyn Vault>,
    operations: Arc<AsyncMutex<()>>,
    login_cancel: Arc<Mutex<Option<watch::Sender<bool>>>>,
    credentials: Arc<Mutex<HashMap<String, Credential>>>,
    catalogs: Arc<Mutex<HashMap<String, (Instant, Vec<Model>)>>>,
    diagnostic: Arc<Mutex<Option<Diagnostic>>>,
    endpoints: Endpoints,
}
impl ChatGpt {
    pub fn new(directory: PathBuf) -> Self {
        Self::with_vault(directory, Arc::new(SystemVault), Endpoints::default())
    }
    fn with_vault(directory: PathBuf, vault: Arc<dyn Vault>, endpoints: Endpoints) -> Self {
        let file = directory.join("accounts.json");
        let loaded = if file.exists() {
            fs::read(&file).map_err(|_| LOCK_ERROR).and_then(|b| {
                if b.len() > 128 * 1024 {
                    return Err(LOCK_ERROR);
                }
                let d: Database = serde_json::from_slice(&b).map_err(|_| LOCK_ERROR)?;
                if d.accounts.len() > 8
                    || uuid::Uuid::parse_str(d.host_id.trim_start_matches("urn:uuid:")).is_err()
                    || d.accounts.iter().any(|a| {
                        uuid::Uuid::parse_str(&a.id).is_err()
                            || !identifier(&a.client_id)
                            || a.client_id == "dynamic_agent_client"
                            || a.label.chars().count() > 400
                            || a.subject
                                .as_ref()
                                .is_some_and(|s| s.is_empty() || s.len() > 400)
                    })
                    || d.active_id
                        .as_ref()
                        .is_some_and(|id| !d.accounts.iter().any(|a| &a.id == id))
                {
                    return Err(LOCK_ERROR);
                }
                Ok(d)
            })
        } else {
            Ok(Database::default())
        };
        let (db, load_error) = match loaded {
            Ok(d) => (d, None),
            Err(e) => (Database::default(), Some(e.to_string())),
        };
        Self {
            vault_read_timeout: Duration::from_secs(8),
            vault_reading: Arc::new(AtomicBool::new(false)),
            directory,
            db: Arc::new(Mutex::new(db)),
            load_error,
            vault,
            operations: Arc::new(AsyncMutex::new(())),
            login_cancel: Arc::new(Mutex::new(None)),
            credentials: Arc::new(Mutex::new(HashMap::new())),
            catalogs: Arc::new(Mutex::new(HashMap::new())),
            diagnostic: Arc::new(Mutex::new(None)),
            endpoints,
        }
    }
    fn database(&self) -> Result<Database, String> {
        if let Some(e) = &self.load_error {
            return Err(e.clone());
        }
        Ok(self.db.lock().map_err(|_| LOCK_ERROR)?.clone())
    }
    fn update(
        &self,
        change: impl FnOnce(&mut Database) -> Result<(), String>,
    ) -> Result<(), String> {
        if let Some(e) = &self.load_error {
            return Err(e.clone());
        }
        let mut guard = self.db.lock().map_err(|_| LOCK_ERROR)?;
        let mut d = guard.clone();
        change(&mut d)?;
        fs::create_dir_all(&self.directory).map_err(|_| "无法保存 ChatGPT 账户记录")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700))
                .map_err(|_| "无法保护 ChatGPT 账户记录")?;
        }
        let mut temp = tempfile::NamedTempFile::new_in(&self.directory)
            .map_err(|_| "无法保存 ChatGPT 账户记录")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temp.as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| "无法保护 ChatGPT 账户记录")?;
        }
        temp.write_all(&serde_json::to_vec(&d).map_err(|_| LOCK_ERROR)?)
            .map_err(|_| "无法保存 ChatGPT 账户记录")?;
        temp.as_file()
            .sync_all()
            .map_err(|_| "无法保存 ChatGPT 账户记录")?;
        temp.persist(self.directory.join("accounts.json"))
            .map_err(|_| "无法保存 ChatGPT 账户记录")?;
        *guard = d;
        Ok(())
    }
    pub fn state(&self) -> Result<ConnectionState, String> {
        let d = self.database()?;
        Ok(ConnectionState {
            active_id: d.active_id,
            diagnostic: self.diagnostic.lock().map_err(|_| LOCK_ERROR)?.clone(),
            accounts: d
                .accounts
                .iter()
                .map(|a| Account {
                    id: a.id.clone(),
                    label: if a.label.is_empty() {
                        "待完成授权".into()
                    } else {
                        a.label.clone()
                    },
                    connected: a.connected,
                    plan_enabled: a.plan_enabled,
                })
                .collect(),
        })
    }
    pub fn select(&self, id: &str) -> Result<ConnectionState, String> {
        self.update(|d| {
            if !d.accounts.iter().any(|a| a.id == id) {
                return Err("请选择已保存的 ChatGPT 账户".into());
            }
            d.active_id = Some(id.into());
            Ok(())
        })?;
        self.state()
    }
    pub fn cancel_login(&self) {
        if let Ok(slot) = self.login_cancel.lock() {
            if let Some(tx) = slot.as_ref() {
                tx.send_replace(true);
            }
        }
    }
    fn registration(&self, id: &str) -> Result<Registration, String> {
        self.database()?
            .accounts
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| "请选择已保存的 ChatGPT 账户".into())
    }
    async fn vault_get(&self, id: &str) -> Result<Credential, String> {
        if let Some(c) = self
            .credentials
            .lock()
            .map_err(|_| LOCK_ERROR)?
            .get(id)
            .cloned()
        {
            return Ok(c);
        }
        let vault = self.vault.clone();
        let key = id.to_owned();
        self.vault_reading
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "系统凭据库仍在等待响应，请先处理系统授权窗口后重试")?;
        let permit = VaultReadGuard(self.vault_reading.clone());
        let operation = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            vault.get(&key)
        });
        let secret = tokio::time::timeout(self.vault_read_timeout, operation)
            .await
            .map_err(|_| "读取系统凭据库超时，请先处理系统授权窗口，再到模型配置中重试或重新登录")?
            .map_err(|_| LOCK_ERROR)??
            .ok_or(RELOGIN)?;
        let c: Credential = serde_json::from_str(&secret).map_err(|_| RELOGIN)?;
        let a = self.registration(id)?;
        if c.client_id != a.client_id
            || a.subject.as_deref() != Some(&c.subject)
            || !token_valid(&c.access_token)
            || !token_valid(&c.id_token)
            || c.scopes.len() > 32
        {
            return Err(RELOGIN.into());
        }
        self.credentials
            .lock()
            .map_err(|_| LOCK_ERROR)?
            .insert(id.into(), c.clone());
        Ok(c)
    }
    async fn vault_set(&self, id: &str, c: &Credential) -> Result<(), String> {
        let vault = self.vault.clone();
        let key = id.to_owned();
        let secret = serde_json::to_string(c).map_err(|_| LOCK_ERROR)?;
        tokio::task::spawn_blocking(move || vault.set(&key, &secret))
            .await
            .map_err(|_| LOCK_ERROR)??;
        self.credentials
            .lock()
            .map_err(|_| LOCK_ERROR)?
            .insert(id.into(), c.clone());
        Ok(())
    }
    async fn clear_credentials(&self, id: &str) -> Result<(), String> {
        self.credentials.lock().map_err(|_| LOCK_ERROR)?.remove(id);
        self.catalogs.lock().map_err(|_| LOCK_ERROR)?.remove(id);
        self.update(|d| {
            if let Some(a) = d.accounts.iter_mut().find(|a| a.id == id) {
                a.connected = false;
                a.plan_enabled = false;
            }
            Ok(())
        })?;
        let vault = self.vault.clone();
        let key = id.to_owned();
        tokio::task::spawn_blocking(move || vault.remove(&key))
            .await
            .map_err(|_| LOCK_ERROR)??;
        Ok(())
    }
    fn client(&self, seconds: u64) -> Result<reqwest::Client, String> {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(seconds))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "无法建立 ChatGPT 连接".into())
    }
    async fn json(&self, mut r: reqwest::Response, max: usize) -> Result<Value, String> {
        let status = r.status();
        let request_id = r
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .filter(|s| {
                s.len() <= 100
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            })
            .map(str::to_owned);
        if r.content_length().is_some_and(|n| n > max as u64) {
            return Err("ChatGPT 返回内容过大".into());
        }
        let mut bytes = Vec::new();
        while let Some(b) = r.chunk().await.map_err(|_| "ChatGPT 响应未接收完整")? {
            if b.len() > max.saturating_sub(bytes.len()) {
                return Err("ChatGPT 返回内容过大".into());
            }
            bytes.extend_from_slice(&b);
        }
        let parsed = serde_json::from_slice::<Value>(&bytes);
        let v = parsed.as_ref().cloned().unwrap_or(Value::Null);
        if !status.is_success() || parsed.is_err() {
            let code = v
                .pointer("/error/code")
                .or_else(|| v.get("error"))
                .and_then(Value::as_str);
            let safe = |s: &str| {
                s.len() <= 120
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_.[]".contains(&b))
            };
            *self.diagnostic.lock().map_err(|_| LOCK_ERROR)? = Some(Diagnostic {
                status: status.as_u16(),
                code: code.filter(|s| safe(s)).map(str::to_owned),
                param: v
                    .pointer("/error/param")
                    .and_then(Value::as_str)
                    .filter(|s| safe(s))
                    .map(str::to_owned),
                request_id,
                body_shape: if parsed.is_err() {
                    if bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) == Some(b'<') {
                        "html"
                    } else {
                        "non-json"
                    }
                } else if v.get("detail").is_some() {
                    "detail"
                } else if v.get("error").is_some() {
                    "error"
                } else {
                    "other-json"
                }
                .into(),
            });
            if status.is_success() {
                return Err("ChatGPT 连接返回了网页或非模型内容，请在模型配置中检查连接；也可选择已登录的 Pi / OpenAI 订阅入口".into());
            }
            if matches!(
                code,
                Some(
                    "invalid_grant"
                        | "invalid_refresh_token"
                        | "token_expired"
                        | "refresh_token_expired"
                        | "refresh_token_invalidated"
                        | "refresh_token_reused"
                )
            ) {
                return Err(RELOGIN.into());
            }
            if code.is_some_and(|s| {
                s.starts_with("subscription_sharing_") || s.starts_with("chatpass_v2_")
            }) {
                return Err(chatgpt_stream::error_message(code));
            }
            return Err(match status.as_u16() {
                401 => "ChatGPT 授权未被接受，请重新登录或检查套餐授权",
                403 => "此 ChatGPT 账户、工作区或地区暂不允许此操作",
                429 => "ChatGPT 授权用量已达上限，请在 ChatGPT 用量设置中查看",
                503 => "ChatGPT 套餐用量暂时不可用，请稍后手动重试",
                _ => "ChatGPT 请求失败，请检查连接后手动重试",
            }
            .into());
        }
        if parsed.is_err() {
            return Err("ChatGPT 返回格式无效".into());
        }
        Ok(v)
    }
    fn trusted_auth_url(&self, raw: &str) -> Result<reqwest::Url, String> {
        let u = reqwest::Url::parse(raw).map_err(|_| "ChatGPT 授权配置无效")?;
        let root = reqwest::Url::parse(&self.endpoints.issuer).map_err(|_| LOCK_ERROR)?;
        if u.origin() != root.origin()
            || !u.username().is_empty()
            || u.password().is_some()
            || u.fragment().is_some()
        {
            return Err("ChatGPT 授权配置无效".into());
        }
        Ok(u)
    }
    async fn discovery(&self) -> Result<Value, String> {
        let r = self
            .client(15)?
            .get(&self.endpoints.discovery)
            .send()
            .await
            .map_err(|_| "无法读取 ChatGPT 授权配置，请检查网络")?;
        let v = self.json(r, 128 * 1024).await?;
        if v["issuer"] != self.endpoints.issuer {
            return Err("ChatGPT 授权配置无效".into());
        }
        Ok(v)
    }
    async fn identity(
        &self,
        token: &str,
        client_id: &str,
        nonce: Option<&str>,
    ) -> Result<Identity, String> {
        let discovery = self.discovery().await?;
        let uri = self.trusted_auth_url(
            discovery["jwks_uri"]
                .as_str()
                .ok_or("ChatGPT 授权配置无效")?,
        )?;
        let r = self
            .client(15)?
            .get(uri)
            .send()
            .await
            .map_err(|_| "无法验证 ChatGPT 身份，请检查网络")?;
        let jwks: JwkSet = serde_json::from_value(self.json(r, 128 * 1024).await?)
            .map_err(|_| "ChatGPT 身份公钥无效")?;
        verify_identity(token, client_id, nonce, &self.endpoints.issuer, &jwks)
    }
    pub async fn sign_in(
        &self,
        selected: Option<String>,
        consent: bool,
        open: impl FnOnce(String) -> Result<(), String>,
    ) -> Result<ConnectionState, String> {
        let _operation = self
            .operations
            .try_lock()
            .map_err(|_| "正在处理 ChatGPT 账户，请稍候")?;
        let previous = selected
            .as_deref()
            .map(|id| self.registration(id))
            .transpose()?;
        if previous.is_none() && self.database()?.accounts.len() >= 8 {
            return Err("最多保存 8 个 ChatGPT 账户，请先使用已有连接".into());
        }
        self.update(|_| Ok(()))?;
        let (tx, mut cancel) = watch::channel(false);
        *self.login_cancel.lock().map_err(|_| LOCK_ERROR)? = Some(tx);
        let _attempt = AttemptGuard(&self.login_cancel);
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| "无法建立本机授权回调，请检查防火墙")?;
        let port = listener.local_addr().map_err(|_| LOCK_ERROR)?.port();
        let redirect = format!("http://127.0.0.1:{port}/auth/callback");
        let state = random()?;
        let nonce = random()?;
        let verifier = random()?;
        let mut url = reqwest::Url::parse(&self.endpoints.authorize).map_err(|_| LOCK_ERROR)?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair(
                "client_id",
                previous
                    .as_ref()
                    .map(|a| a.client_id.as_str())
                    .unwrap_or("dynamic_agent_client"),
            )
            .append_pair("ext_agent_host_id", &self.database()?.host_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect)
            .append_pair("scope", SCOPES)
            .append_pair("resource", &self.endpoints.resource)
            .append_pair("state", &state)
            .append_pair("nonce", &nonce)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &challenge(&verifier));
            if previous.is_none() {
                q.append_pair("agent_name_hint", "Liubai");
            }
            if consent {
                q.append_pair("prompt", "consent");
            }
        }
        // Never log this URL; returning-login hints could contain an ID token.
        open(url.into())?;
        let callback = tokio::select! {r=receive_callback(listener,&state,previous.as_ref().map(|a|a.client_id.as_str()),port)=>r?,_=cancel.changed()=>return Err("已取消 ChatGPT 登录".into()),_=tokio::time::sleep(Duration::from_secs(300))=>return Err("ChatGPT 登录等待超时，请重新连接".into())};
        let (code, client_id) = callback;
        let id = if let Some(a) = &previous {
            a.id.clone()
        } else if let Some(a) = self
            .database()?
            .accounts
            .into_iter()
            .find(|a| a.client_id == client_id)
        {
            a.id
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            self.update(|d| {
                d.accounts.push(Registration {
                    id: id.clone(),
                    client_id: client_id.clone(),
                    subject: None,
                    label: String::new(),
                    connected: false,
                    plan_enabled: false,
                });
                Ok(())
            })?;
            id
        };
        let response = tokio::select! {r=self.client(20)?.post(&self.endpoints.token).form(&[("grant_type","authorization_code"),("client_id",client_id.as_str()),("code",code.as_str()),("code_verifier",verifier.as_str()),("redirect_uri",redirect.as_str()),("resource",self.endpoints.resource.as_str())]).send()=>r.map_err(|_|"ChatGPT 授权交换失败，请重新连接")?,_=cancel.changed()=>return Err("已取消 ChatGPT 登录".into())};
        let t: TokenResponse = serde_json::from_value(self.json(response, 128 * 1024).await?)
            .map_err(|_| "ChatGPT 授权响应无效")?;
        let id_token = t
            .id_token
            .as_deref()
            .filter(|s| token_valid(s))
            .ok_or("ChatGPT 未返回可验证的身份")?;
        let identity = self.identity(id_token, &client_id, Some(&nonce)).await?;
        if self
            .registration(&id)?
            .subject
            .as_ref()
            .is_some_and(|s| s != &identity.sub)
        {
            return Err("登录账户与所选连接不一致，请新增账户后重试".into());
        }
        if *cancel.borrow() {
            return Err("已取消 ChatGPT 登录".into());
        }
        let scopes = parse_scopes(t.scope.as_deref().unwrap_or(""))?;
        let c = make_credential(t, &client_id, &identity.sub, scopes, None)?;
        let plan = plan_enabled(&c.scopes);
        // Establish the verified identity before touching its OS-vault entry.
        let label = identity
            .email
            .or(identity.name)
            .unwrap_or_else(|| format!("ChatGPT · {}", &id[..6]));
        self.update(|d| {
            let a = d
                .accounts
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or(LOCK_ERROR)?;
            a.subject = Some(identity.sub);
            a.label = label.chars().take(320).collect();
            Ok(())
        })?;
        self.vault_set(&id, &c).await?;
        self.update(|d| {
            let a = d
                .accounts
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or(LOCK_ERROR)?;
            a.connected = true;
            a.plan_enabled = plan;
            d.active_id = Some(id);
            Ok(())
        })?;
        self.state()
    }
    async fn access(&self, id: &str) -> Result<String, String> {
        let a = self.registration(id)?;
        if !a.connected {
            return Err(RELOGIN.into());
        }
        let c = self.vault_get(id).await?;
        if !plan_enabled(&c.scopes) {
            return Err("已登录 ChatGPT，但尚未授权使用套餐额度".into());
        }
        if c.expires_at > now() + 90 || c.earliest_refresh_at > now() && c.expires_at > now() + 5 {
            return Ok(c.access_token);
        }
        if c.refresh_token.is_empty() {
            return Err(RELOGIN.into());
        }
        let r = self
            .client(20)?
            .post(&self.endpoints.token)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", c.client_id.as_str()),
                ("refresh_token", c.refresh_token.as_str()),
                ("resource", self.endpoints.resource.as_str()),
            ])
            .send()
            .await
            .map_err(|_| "ChatGPT 授权刷新失败，请检查网络")?;
        let v = match self.json(r, 128 * 1024).await {
            Ok(v) => v,
            Err(e) => {
                if e == RELOGIN {
                    self.clear_credentials(id).await?
                }
                return Err(e);
            }
        };
        let t: TokenResponse = serde_json::from_value(v).map_err(|_| "ChatGPT 授权响应无效")?;
        if let Some(jwt) = &t.id_token {
            let identity = self.identity(jwt, &c.client_id, None).await?;
            if identity.sub != c.subject {
                return Err("ChatGPT 刷新身份与所选账户不一致".into());
            }
        }
        let scopes = if let Some(s) = &t.scope {
            parse_scopes(s)?
        } else {
            c.scopes.clone()
        };
        let next = make_credential(t, &c.client_id, &c.subject, scopes, Some(&c))?;
        self.vault_set(id, &next).await?;
        let enabled = plan_enabled(&next.scopes);
        self.update(|d| {
            if let Some(a) = d.accounts.iter_mut().find(|a| a.id == id) {
                a.plan_enabled = enabled;
            }
            Ok(())
        })?;
        if !enabled {
            return Err("已登录 ChatGPT，但尚未授权使用套餐额度".into());
        }
        Ok(next.access_token)
    }
    async fn authorized_access(&self, id: &str) -> Result<String, String> {
        // A generation may be cancelled while its rotating refresh token is
        // being committed. Finish this small credential transaction under its
        // own lock even when the caller stops waiting; never repeat a refresh.
        let service = self.clone();
        let id = id.to_owned();
        tokio::spawn(async move {
            let _operation = service.operations.lock().await;
            service.access(&id).await
        })
        .await
        .map_err(|_| LOCK_ERROR)?
    }
    pub async fn models(&self, id: &str) -> Result<Vec<Model>, String> {
        let access = self.authorized_access(id).await?;
        let r = self
            .client(20)?
            .get(&self.endpoints.models)
            .bearer_auth(access)
            .send()
            .await
            .map_err(|_| "无法读取 ChatGPT 模型列表，请检查网络")?;
        let v = self.json(r, 2 * 1024 * 1024).await?;
        let models = parse_models(&v)?;
        self.catalogs
            .lock()
            .map_err(|_| LOCK_ERROR)?
            .insert(id.into(), (Instant::now(), models.clone()));
        Ok(models)
    }
    pub async fn complete(
        &self,
        id: &str,
        model: &str,
        prompt: &str,
        images: &[&str],
    ) -> Result<String, String> {
        if !identifier(model)
            || prompt.len() > 128 * 1024
            || images.is_empty()
            || images.len() > 2
            || images
                .iter()
                .any(|s| s.len() > 2 * 1024 * 1024 || !s.starts_with("data:image/jpeg;base64,"))
        {
            return Err("ChatGPT 分析输入无效".into());
        }
        *self.diagnostic.lock().map_err(|_| LOCK_ERROR)? = None;
        let cached = self
            .catalogs
            .lock()
            .map_err(|_| LOCK_ERROR)?
            .get(id)
            .filter(|(t, _)| t.elapsed() < Duration::from_secs(900))
            .map(|(_, v)| v.clone());
        let models = if let Some(v) = cached {
            v
        } else {
            self.models(id).await?
        };
        if !models.iter().any(|m| m.slug == model) {
            return Err("请选择当前 ChatGPT 账户列表中的模型".into());
        }
        let access = self.authorized_access(id).await?;
        let mut content = vec![json!({"type":"input_text","text":prompt})];
        for image in images {
            content.push(json!({"type":"input_image","image_url":image}));
        }
        let body = json!({"model":model,"store":false,"stream":true,"input":[{"role":"user","content":content}]});
        let mut r = self
            .client(160)?
            .post(&self.endpoints.responses)
            .bearer_auth(access)
            .header("Accept", "text/event-stream")
            .json(&body)
            .send()
            .await
            .map_err(|_| "无法连接 ChatGPT，请检查网络或超时")?;
        if !r.status().is_success() {
            self.json(r, 128 * 1024).await?;
            return Err("ChatGPT 请求失败，请检查连接后手动重试".into());
        }
        let request_id = r
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .filter(|s| {
                s.len() <= 100
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            })
            .map(str::to_owned);
        if !r
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(';')
                    .next()
                    .is_some_and(|s| s.trim().eq_ignore_ascii_case("text/event-stream"))
            })
        {
            let value = self.json(r, 8 * 1024 * 1024).await?;
            let mut response = ResponseStream::new();
            let result = response.json(&value);
            if result.is_err() {
                *self.diagnostic.lock().map_err(|_| LOCK_ERROR)? = Some(Diagnostic {
                    status: 200,
                    code: response.error_code,
                    param: response.error_param,
                    request_id,
                    body_shape: if value.get("error").is_some() {
                        "error"
                    } else if value["object"] == "response" {
                        "response-json"
                    } else {
                        "other-json"
                    }
                    .into(),
                });
            }
            return result;
        }
        let mut stream = ResponseStream::new();
        while let Some(chunk) = r.chunk().await.map_err(|_| "ChatGPT 响应未接收完整")? {
            match stream.push(&chunk) {
                Ok(Some(result)) => return Ok(result),
                Ok(None) => {}
                Err(e) => {
                    *self.diagnostic.lock().map_err(|_| LOCK_ERROR)? = Some(Diagnostic {
                        status: 200,
                        code: stream.error_code.clone(),
                        param: stream.error_param.clone(),
                        request_id,
                        body_shape: "response-event".into(),
                    });
                    return Err(e);
                }
            }
        }
        Err("ChatGPT 响应尚未完成，请手动重试".into())
    }
    pub async fn disconnect(&self, id: &str) -> Result<Disconnect, String> {
        let _operation = self.operations.lock().await;
        let a = self.registration(id)?;
        let c = self.vault_get(id).await;
        let mut revoked = false;
        if let Ok(c) = c {
            if !c.refresh_token.is_empty() {
                if let Ok(d) = self.discovery().await {
                    if let Some(raw) = d["revocation_endpoint"].as_str() {
                        if let Ok(url) = self.trusted_auth_url(raw) {
                            for _ in 0..2 {
                                match self
                                    .client(8)?
                                    .post(url.clone())
                                    .form(&[
                                        ("token", c.refresh_token.as_str()),
                                        ("token_type_hint", "refresh_token"),
                                        ("client_id", a.client_id.as_str()),
                                    ])
                                    .send()
                                    .await
                                {
                                    Ok(r) if r.status() == reqwest::StatusCode::OK => {
                                        revoked = true;
                                        break;
                                    }
                                    Ok(r) if !r.status().is_server_error() => break,
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        } else if !a.connected {
            revoked = true
        }
        self.clear_credentials(id).await?;
        Ok(Disconnect {
            state: self.state()?,
            remote_revoked: revoked,
        })
    }
}
fn parse_scopes(s: &str) -> Result<Vec<String>, String> {
    if s.len() > 4096 {
        return Err("ChatGPT 授权范围无效".into());
    }
    let scopes: Vec<_> = s.split_whitespace().map(str::to_owned).collect();
    if scopes.len() > 32 {
        return Err("ChatGPT 授权范围无效".into());
    }
    Ok(scopes)
}
fn plan_enabled(scopes: &[String]) -> bool {
    ["resource.invoke", "chatgpt.tokens.use.direct"]
        .iter()
        .all(|s| scopes.iter().any(|v| v == s))
}
fn make_credential(
    t: TokenResponse,
    client_id: &str,
    sub: &str,
    scopes: Vec<String>,
    old: Option<&Credential>,
) -> Result<Credential, String> {
    if !token_valid(&t.access_token)
        || !t.token_type.eq_ignore_ascii_case("bearer")
        || t.expires_in == 0
        || t.expires_in > 86400
    {
        return Err("ChatGPT 授权响应无效".into());
    }
    let refresh = t
        .refresh_token
        .or_else(|| old.map(|c| c.refresh_token.clone()))
        .unwrap_or_default();
    if !refresh.is_empty() && !token_valid(&refresh) {
        return Err("ChatGPT 授权响应无效".into());
    }
    let id_token = t
        .id_token
        .or_else(|| old.map(|c| c.id_token.clone()))
        .ok_or("ChatGPT 未返回可验证的身份")?;
    if !token_valid(&id_token) {
        return Err("ChatGPT 授权响应无效".into());
    }
    Ok(Credential {
        client_id: client_id.into(),
        subject: sub.into(),
        access_token: t.access_token,
        refresh_token: refresh,
        id_token,
        scopes,
        expires_at: now() + t.expires_in,
        earliest_refresh_at: t.earliest_refresh_at.and_then(|v| v.as_u64()).unwrap_or(0),
    })
}
fn verify_identity(
    token: &str,
    client_id: &str,
    nonce: Option<&str>,
    issuer: &str,
    jwks: &JwkSet,
) -> Result<Identity, String> {
    if !token_valid(token) {
        return Err("ChatGPT 身份验证失败".into());
    }
    let h = decode_header(token).map_err(|_| "ChatGPT 身份验证失败")?;
    if h.alg != Algorithm::RS256 {
        return Err("ChatGPT 身份签名算法不受支持".into());
    }
    let jwk = jwks
        .find(h.kid.as_deref().ok_or("ChatGPT 身份缺少签名标识")?)
        .ok_or("未找到 ChatGPT 身份签名公钥")?;
    let key = DecodingKey::from_jwk(jwk).map_err(|_| "ChatGPT 身份公钥无效")?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[client_id]);
    validation.set_issuer(&[issuer]);
    validation.leeway = 30;
    let identity = decode::<Identity>(token, &key, &validation)
        .map_err(|_| "ChatGPT 身份验证失败")?
        .claims;
    if identity.azp.as_ref().is_some_and(|p| p != client_id)
        || identity.aud.as_array().is_some_and(|a| a.len() > 1)
            && identity.azp.as_deref() != Some(client_id)
    {
        return Err("ChatGPT 身份与应用注册不匹配".into());
    }
    if identity.sub.is_empty()
        || identity.sub.len() > 400
        || nonce.is_some_and(|n| identity.nonce.as_deref() != Some(n))
    {
        return Err("ChatGPT 身份或登录校验码不匹配".into());
    }
    Ok(identity)
}
fn parse_models(v: &Value) -> Result<Vec<Model>, String> {
    let items = v["models"]
        .as_array()
        .filter(|a| a.len() <= 4096)
        .ok_or("ChatGPT 未返回有效模型列表")?;
    let mut seen = HashSet::new();
    let mut models = vec![];
    for item in items {
        if item["visibility"] != "list" {
            continue;
        }
        let Some(slug) = item["slug"].as_str().filter(|s| identifier(s)) else {
            continue;
        };
        if !seen.insert(slug) {
            continue;
        }
        let name = item["display_name"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(slug);
        models.push(Model {
            slug: slug.into(),
            display_name: name.chars().take(120).collect(),
        })
    }
    if models.is_empty() {
        return Err("此 ChatGPT 账户暂未提供可用模型".into());
    }
    Ok(models)
}
async fn html(socket: &mut TcpStream, status: &str, message: &str) {
    let body=format!("<!doctype html><html lang=\"zh-CN\"><meta charset=\"utf-8\"><meta name=\"referrer\" content=\"no-referrer\"><title>Liubai</title><body><h1>Liubai · 留白</h1><p>{message}</p><p lang=\"en\">Return to Liubai to see the connection result.</p></body></html>");
    let response=format!("HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",body.len());
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.shutdown().await;
}
async fn receive_callback(
    listener: TcpListener,
    state: &str,
    expected_client: Option<&str>,
    port: u16,
) -> Result<(String, String), String> {
    loop {
        let (mut socket, peer) = listener
            .accept()
            .await
            .map_err(|_| "本机授权回调连接失败")?;
        if !peer.ip().is_loopback() {
            continue;
        }
        let mut buf = Vec::new();
        let read = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let mut part = [0u8; 1024];
                let n = socket.read(&mut part).await?;
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&part[..n]);
                if buf.len() > 16384 || buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Ok::<(), std::io::Error>(())
        })
        .await;
        if !matches!(read, Ok(Ok(()))) || buf.len() > 16384 {
            continue;
        }
        let Ok(request) = std::str::from_utf8(&buf) else {
            continue;
        };
        let first = request.lines().next().unwrap_or("");
        let parts: Vec<_> = first.split_whitespace().collect();
        let host = request.lines().find_map(|line| {
            let (k, v) = line.split_once(':')?;
            k.eq_ignore_ascii_case("Host").then_some(v.trim())
        });
        if parts.len() != 3
            || parts[0] != "GET"
            || !parts[1].starts_with("/auth/callback?")
            || host != Some(format!("127.0.0.1:{port}").as_str())
        {
            html(
                &mut socket,
                "404 Not Found",
                "此地址仅用于留白的本机登录回调。",
            )
            .await;
            continue;
        }
        let Ok(url) = reqwest::Url::parse(&format!("http://127.0.0.1:{port}{}", parts[1])) else {
            continue;
        };
        let q: Vec<_> = url.query_pairs().collect();
        let field = |name: &str| {
            let mut found = q.iter().filter(|(k, _)| k == name);
            let value = found.next().map(|(_, v)| v.to_string());
            if found.next().is_some() {
                None
            } else {
                value
            }
        };
        if field("state").as_deref() != Some(state) {
            html(
                &mut socket,
                "400 Bad Request",
                "登录校验未通过，请返回留白重新连接。",
            )
            .await;
            continue;
        }
        if field("error").is_some() {
            html(&mut socket, "200 OK", "你已取消授权。请返回留白。").await;
            return Err("ChatGPT 授权未完成，未发送任何照片".into());
        }
        let code = field("code")
            .filter(|s| token_valid(s))
            .ok_or("ChatGPT 回调缺少授权码")?;
        let supplied = field("client_id");
        if let (Some(expected), Some(actual)) = (expected_client, supplied.as_deref()) {
            if expected != actual {
                return Err("ChatGPT 回调与所选账户注册不一致".into());
            }
        }
        let client = supplied
            .or_else(|| expected_client.map(str::to_owned))
            .filter(|s| identifier(s) && s != "dynamic_agent_client")
            .ok_or("ChatGPT 注册尚未完成，请重新连接")?;
        html(&mut socket,"200 OK","已收到授权回调。请返回留白查看身份验证与连接结果。 / Return to Liubai to finish connecting.").await;
        return Ok((code, client));
    }
}

#[cfg(test)]
#[path = "chatgpt_auth_tests.rs"]
mod tests;
