//! Optional Pi AI SDK process. Tokens stay with Pi; the webview only sees status.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, watch, Mutex as AsyncMutex},
};

const SCRIPT: &str = include_str!("../resources/pi-bridge.mjs");
pub const PROVIDERS: [&str; 4] = ["openai", "anthropic", "github-copilot", "openrouter"];
const BUSY: &str = "Pi 正在处理连接或分析，请先等待或取消";
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}
#[derive(Clone)]
struct Runtime {
    info: RuntimeInfo,
    root: PathBuf,
    node: PathBuf,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub name: String,
    pub input: Vec<String>,
    pub context_window: u64,
    pub reasoning: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub provider: String,
    pub connected: bool,
    pub auth_type: Option<String>,
    pub models: Vec<Model>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub id: String,
    pub kind: String,
    pub message: String,
    pub placeholder: String,
    pub options: Vec<Value>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthEvent {
    pub session_id: String,
    pub provider: String,
    pub phase: String,
    pub host: Option<String>,
    pub device_code: Option<String>,
    pub prompt: Option<Prompt>,
}
pub type EventSink = Arc<dyn Fn(AuthEvent) + Send + Sync>;
pub type Browser = Arc<dyn Fn(String) -> Result<(), String> + Send + Sync>;
struct Answer {
    id: String,
    value: String,
}
struct Control {
    id: String,
    cancel: watch::Sender<bool>,
    answers: mpsc::Sender<Answer>,
    url: Option<String>,
    prompt_id: Option<String>,
}
#[derive(Clone)]
pub struct PiBridge {
    directory: PathBuf,
    gate: Arc<AsyncMutex<()>>,
    control: Arc<Mutex<Option<Control>>>,
}
struct CancelOnDrop(Option<watch::Sender<bool>>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(tx) = &self.0 {
            tx.send_replace(true);
        }
    }
}
impl PiBridge {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            gate: Arc::new(AsyncMutex::new(())),
            control: Arc::new(Mutex::new(None)),
        }
    }
    pub fn info(&self, custom: Option<&str>) -> RuntimeInfo {
        discover(custom).map(|r| r.info).unwrap_or(RuntimeInfo {
            available: false,
            path: None,
            version: None,
        })
    }
    pub fn cancel(&self, session_id: Option<&str>) {
        if let Ok(slot) = self.control.lock() {
            if let Some(c) = slot.as_ref() {
                if session_id.is_none_or(|id| id == c.id) {
                    c.cancel.send_replace(true);
                }
            }
        }
    }
    pub async fn answer(&self, session_id: &str, id: &str, value: String) -> Result<(), String> {
        if value.len() > 32768 {
            return Err("授权输入过长".into());
        }
        let tx = {
            let slot = self.control.lock().map_err(|_| BUSY)?;
            let c = slot
                .as_ref()
                .filter(|c| c.id == session_id && c.prompt_id.as_deref() == Some(id))
                .ok_or("授权步骤已结束，请重新连接")?;
            c.answers.clone()
        };
        tx.send(Answer {
            id: id.into(),
            value,
        })
        .await
        .map_err(|_| "授权步骤已结束，请重新连接".into())
    }
    pub fn reopen(&self, session_id: &str, browser: Browser) -> Result<(), String> {
        let url = self
            .control
            .lock()
            .map_err(|_| BUSY)?
            .as_ref()
            .filter(|c| c.id == session_id)
            .and_then(|c| c.url.clone())
            .ok_or("尚未收到可打开的授权页面")?;
        browser(url)
    }
    fn host_id(&self) -> Result<String, String> {
        let file = self.directory.join("host-id");
        if let Ok(s) = fs::read_to_string(&file) {
            if uuid::Uuid::parse_str(s.trim()).is_ok() {
                return Ok(s.trim().into());
            }
            return Err("Pi 设备记录无效，请检查应用数据目录".into());
        }
        fs::create_dir_all(&self.directory).map_err(|_| "无法保存 Pi 设备记录")?;
        let id = uuid::Uuid::new_v4().to_string();
        let mut temp =
            tempfile::NamedTempFile::new_in(&self.directory).map_err(|_| "无法保存 Pi 设备记录")?;
        use std::io::Write;
        temp.write_all(id.as_bytes())
            .map_err(|_| "无法保存 Pi 设备记录")?;
        temp.persist(&file).map_err(|_| "无法保存 Pi 设备记录")?;
        Ok(id)
    }
    pub async fn catalog(
        &self,
        path: Option<&str>,
        provider: &str,
        refresh: bool,
    ) -> Result<Catalog, String> {
        let value = self
            .operation(
                path,
                provider,
                "catalog",
                None,
                json!({"refresh":refresh}),
                None,
                None,
            )
            .await?;
        parse_catalog(value, provider)
    }
    pub async fn login(
        &self,
        path: Option<&str>,
        provider: &str,
        session_id: &str,
        sink: EventSink,
        browser: Browser,
    ) -> Result<Catalog, String> {
        let value = self
            .operation(
                path,
                provider,
                "login",
                Some(session_id),
                json!({}),
                Some(sink),
                Some(browser),
            )
            .await?;
        parse_catalog(value, provider)
    }
    pub async fn logout(&self, path: Option<&str>, provider: &str) -> Result<Catalog, String> {
        let value = self
            .operation(path, provider, "logout", None, json!({}), None, None)
            .await?;
        parse_catalog(value, provider)
    }
    pub async fn infer(
        &self,
        path: Option<&str>,
        provider: &str,
        model: &str,
        prompt: &str,
        images: &[&str],
    ) -> Result<String, String> {
        if model.is_empty()
            || model.len() > 250
            || model.chars().any(char::is_control)
            || prompt.len() > 128 * 1024
            || images.is_empty()
            || images.len() > 2
            || images
                .iter()
                .any(|s| s.len() > 2 * 1024 * 1024 || !s.starts_with("data:image/jpeg;base64,"))
        {
            return Err("Pi 分析输入无效".into());
        }
        let value = self
            .operation(
                path,
                provider,
                "infer",
                None,
                json!({"model":model,"prompt":prompt,"images":images}),
                None,
                None,
            )
            .await?;
        value["text"]
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.len() <= 256 * 1024)
            .map(str::to_owned)
            .ok_or_else(|| "Pi 未返回完整的配框结果".into())
    }
    async fn operation(
        &self,
        path: Option<&str>,
        provider: &str,
        operation: &str,
        session_id: Option<&str>,
        extra: Value,
        sink: Option<EventSink>,
        browser: Option<Browser>,
    ) -> Result<Value, String> {
        if !PROVIDERS.contains(&provider) {
            return Err("请选择支持的 Pi 订阅服务".into());
        }
        let runtime =
            discover(path).ok_or("未找到可用的 Pi 与 Node，请在模型配置中选择已安装的 Pi 程序")?;
        let gate = self.gate.clone().try_lock_owned().map_err(|_| BUSY)?;
        let id = session_id
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if uuid::Uuid::parse_str(&id).is_err() {
            return Err("Pi 会话标识无效".into());
        }
        let host_id = self.host_id()?;
        let (tx, rx) = watch::channel(false);
        let (answers, answers_rx) = mpsc::channel(8);
        *self.control.lock().map_err(|_| BUSY)? = Some(Control {
            id: id.clone(),
            cancel: tx.clone(),
            answers,
            url: None,
            prompt_id: None,
        });
        let mut init = extra;
        init["type"] = json!("init");
        init["operation"] = json!(operation);
        init["provider"] = json!(provider);
        init["piRoot"] = json!(runtime.root);
        init["hostId"] = json!(host_id);
        let service = self.clone();
        let provider = provider.to_owned();
        let timeout = if operation == "login" {
            300
        } else if operation == "infer" {
            170
        } else {
            25
        };
        let task = tokio::spawn(async move {
            let _gate = gate;
            let result = service
                .run(
                    runtime, &id, &provider, init, rx, answers_rx, sink, browser, timeout,
                )
                .await;
            if let Ok(mut slot) = service.control.lock() {
                if slot.as_ref().is_some_and(|c| c.id == id) {
                    *slot = None;
                }
            }
            result
        });
        let mut cancel = CancelOnDrop(Some(tx));
        let result = task.await.map_err(|_| "Pi 连接意外中断，请重试")?;
        cancel.0 = None;
        result
    }
    async fn run(
        &self,
        runtime: Runtime,
        id: &str,
        provider: &str,
        init: Value,
        mut cancel: watch::Receiver<bool>,
        mut answers: mpsc::Receiver<Answer>,
        sink: Option<EventSink>,
        browser: Option<Browser>,
        seconds: u64,
    ) -> Result<Value, String> {
        let temp = tempfile::tempdir().map_err(|_| "无法准备 Pi 临时环境")?;
        let mut child = tokio::process::Command::new(&runtime.node)
            .args(["--input-type=module", "--eval", SCRIPT])
            .current_dir(temp.path())
            .env("PI_OFFLINE", "1")
            .env("PI_TELEMETRY", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| "无法启动 Pi 模型连接，请检查程序路径")?;
        let mut stdin = child.stdin.take().ok_or("无法建立 Pi 连接")?;
        let mut stdout = child.stdout.take().ok_or("无法建立 Pi 连接")?;
        write(&mut stdin, &init).await?;
        let mut result = None;
        let mut failure = None;
        let mut bytes = Vec::new();
        let mut total = 0usize;
        let mut buffer = [0u8; 8192];
        let mut deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
        let mut stopping = false;
        loop {
            tokio::select! {
             read=stdout.read(&mut buffer)=>{
              let n=read.map_err(|_|"Pi 响应未接收完整")?;if n==0{break}total+=n;if total>4*1024*1024{return Err("Pi 返回内容过大".into())}
              for b in &buffer[..n]{if *b==b'\n'{let line=std::mem::take(&mut bytes);if line.is_empty(){continue}let event:Value=serde_json::from_slice(&line).map_err(|_|"Pi 连接协议不兼容，请检查 Pi 版本")?;
               match event["type"].as_str().unwrap_or(""){
                "result"=>{if result.is_some(){return Err("Pi 返回了重复结果".into())}result=Some(event["value"].clone());},
                "error"=>failure=Some(error_message(event["code"].as_str().unwrap_or("runtime"))),
                "auth_url"|"device_code"=>{
                 let raw=event["url"].as_str().filter(|s|s.len()<=20000).ok_or("Pi 授权地址无效")?;let url=auth_url(provider,raw)?;let host=url.host_str().map(str::to_owned);
                 if let Ok(mut slot)=self.control.lock(){if let Some(c)=slot.as_mut().filter(|c|c.id==id){c.url=Some(url.to_string());}}
                 let device=event["userCode"].as_str().filter(|s|s.len()<=80&&!s.chars().any(char::is_control)).map(str::to_owned);
                 if let Some(sink)=&sink{sink(AuthEvent{session_id:id.into(),provider:provider.into(),phase:if device.is_some(){"device_code"}else{"browser"}.into(),host,device_code:device.clone(),prompt:None});}
                 if device.is_none(){if let Some(browser)=&browser{let _=browser(url.into());}}
                },
                "prompt"=>{
                 let prompt_id=event["id"].as_str().filter(|s|s.len()<=100).ok_or("Pi 授权步骤无效")?.to_owned();let kind=event["kind"].as_str().filter(|s|matches!(*s,"text"|"secret"|"select"|"manual_code")).ok_or("Pi 授权步骤无效")?.to_owned();
                 if let Ok(mut slot)=self.control.lock(){if let Some(c)=slot.as_mut().filter(|c|c.id==id){c.prompt_id=Some(prompt_id.clone());}}
                 if let Some(sink)=&sink{sink(AuthEvent{session_id:id.into(),provider:provider.into(),phase:"prompt".into(),host:None,device_code:None,prompt:Some(Prompt{id:prompt_id,kind,message:event["message"].as_str().unwrap_or("").chars().take(1500).collect(),placeholder:event["placeholder"].as_str().unwrap_or("").chars().take(250).collect(),options:event["options"].as_array().map(|v|v.iter().take(20).cloned().collect()).unwrap_or_default()})});}
                },
                "prompt_cancel"=>{if let Ok(mut slot)=self.control.lock(){if let Some(c)=slot.as_mut().filter(|c|c.id==id){c.prompt_id=None;}}if let Some(sink)=&sink{sink(AuthEvent{session_id:id.into(),provider:provider.into(),phase:"waiting".into(),host:None,device_code:None,prompt:None});}},
                "progress"=>{},
                _=>return Err("Pi 连接协议不兼容，请检查 Pi 版本".into()),
               }
              }else{if bytes.len()>1024*1024{return Err("Pi 返回内容过大".into())}bytes.push(*b);}}
             },
             _=cancel.changed(),if !stopping=>{stopping=true;deadline=tokio::time::Instant::now()+Duration::from_secs(20);let _=write(&mut stdin,&json!({"type":"cancel"})).await;},
             answer=answers.recv(),if !stopping=>{if let Some(answer)=answer{write(&mut stdin,&json!({"type":"answer","id":answer.id,"value":answer.value})).await?;}},
             _=tokio::time::sleep_until(deadline)=>{if stopping{return Err("已取消 Pi 请求；已发出的请求可能仍计入服务商用量".into())}let _=write(&mut stdin,&json!({"type":"cancel"})).await;let _=tokio::time::timeout(Duration::from_secs(20),child.wait()).await;return Err("Pi 连接或分析超时，请手动重试".into());}
            }
        }
        let status = tokio::time::timeout(Duration::from_secs(20), child.wait())
            .await
            .map_err(|_| "Pi 结束等待超时")?
            .map_err(|_| "Pi 进程意外中断")?;
        if stopping {
            return Err("已取消 Pi 请求；已发出的请求可能仍计入服务商用量".into());
        }
        if let Some(e) = failure {
            return Err(e);
        }
        if !status.success() || !bytes.is_empty() {
            return Err("Pi 进程意外中断".into());
        }
        result.ok_or_else(|| "Pi 未返回完整结果".into())
    }
}
async fn write(stdin: &mut tokio::process::ChildStdin, value: &Value) -> Result<(), String> {
    let mut b = serde_json::to_vec(value).map_err(|_| "无法建立 Pi 连接")?;
    b.push(b'\n');
    stdin
        .write_all(&b)
        .await
        .map_err(|_| "Pi 连接已结束".into())
}
fn parse_catalog(v: Value, provider: &str) -> Result<Catalog, String> {
    let c: Catalog = serde_json::from_value(v).map_err(|_| "Pi 模型目录格式无效")?;
    if c.provider != provider
        || c.models.len() > 600
        || c.auth_type
            .as_deref()
            .is_some_and(|s| !matches!(s, "oauth" | "api_key"))
        || c.connected
            && c.auth_type.as_deref() != Some("oauth")
            && !(provider == "openrouter" && c.auth_type.as_deref() == Some("api_key"))
        || c.models.iter().any(|m| {
            m.id.is_empty()
                || m.id.len() > 250
                || m.name.chars().count() > 250
                || !m.input.iter().any(|s| s == "image")
        })
    {
        return Err("Pi 模型目录格式无效".into());
    }
    Ok(c)
}
fn error_message(code: &str) -> String {
    match code {
        "cancelled" => "已取消 Pi 请求；已发出的请求可能仍计入服务商用量",
        "auth" => "Pi 订阅授权未就绪，请重新登录；不会自动改用 API Key",
        "model" => "请选择支持图片输入的 Pi 模型",
        "quota" => "此模型的订阅或账户用量已达限制，请到服务商查看",
        "output" => "Pi 返回内容过大或格式无效",
        "incomplete" => "Pi 模型没有完整完成响应，请手动重试",
        _ => "Pi 模型连接失败，请检查网络、Pi 版本和订阅权限",
    }
    .into()
}
fn auth_url(provider: &str, raw: &str) -> Result<reqwest::Url, String> {
    let u = reqwest::Url::parse(raw).map_err(|_| "Pi 授权地址无效")?;
    let host = u.host_str().unwrap_or("");
    let allowed = match provider {
        "openai" => matches!(host, "auth.openai.com" | "chatgpt.com"),
        "anthropic" => matches!(
            host,
            "claude.ai" | "console.anthropic.com" | "platform.claude.com"
        ),
        "github-copilot" => host == "github.com",
        "openrouter" => host == "openrouter.ai",
        _ => false,
    };
    if !allowed || u.scheme() != "https" || !u.username().is_empty() || u.password().is_some() {
        return Err("Pi 返回了非官方授权地址，已停止打开".into());
    }
    Ok(u)
}
fn discover(custom: Option<&str>) -> Option<Runtime> {
    let mut paths = vec![];
    if let Some(p) = custom.filter(|p| !p.trim().is_empty()) {
        paths.push(PathBuf::from(p));
    } else {
        if let Some(raw) = std::env::var_os("PATH") {
            for d in std::env::split_paths(&raw) {
                paths.push(d.join(if cfg!(windows) { "pi.cmd" } else { "pi" }));
            }
        }
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            if let Ok(entries) = fs::read_dir(home.join(".nvm/versions/node")) {
                let mut dirs: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
                dirs.sort();
                for d in dirs.into_iter().rev() {
                    paths.push(d.join("bin/pi"));
                }
            }
        }
        if let Some(appdata) = std::env::var_os("APPDATA") {
            paths.push(PathBuf::from(appdata).join("npm/pi.cmd"));
        }
        paths.extend([
            PathBuf::from("/opt/homebrew/bin/pi"),
            PathBuf::from("/usr/local/bin/pi"),
        ]);
    }
    for path in paths {
        if !path.is_file() {
            continue;
        }
        let Ok(resolved) = path.canonicalize() else {
            continue;
        };
        let mut roots: Vec<_> = resolved.ancestors().map(Path::to_path_buf).collect();
        if let Some(parent) = path.parent() {
            roots.push(parent.join("node_modules/@earendil-works/pi-coding-agent"));
        }
        for root in roots {
            let Ok(bytes) = fs::read(root.join("package.json")) else {
                continue;
            };
            if bytes.len() > 128 * 1024 {
                continue;
            }
            let Ok(v) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            if v["name"] != "@earendil-works/pi-coding-agent" {
                continue;
            }
            let mut nodes = vec![];
            for d in path.ancestors().chain(root.ancestors()) {
                nodes.push(d.join(if cfg!(windows) { "node.exe" } else { "node" }));
                nodes.push(d.join(if cfg!(windows) {
                    "node.exe"
                } else {
                    "bin/node"
                }));
            }
            if let Some(raw) = std::env::var_os("PATH") {
                for d in std::env::split_paths(&raw) {
                    nodes.push(d.join(if cfg!(windows) { "node.exe" } else { "node" }));
                }
            }
            if let Some(node) = nodes.into_iter().find(|p| p.is_file()) {
                return Some(Runtime {
                    info: RuntimeInfo {
                        available: true,
                        path: Some(path.to_string_lossy().into_owned()),
                        version: v["version"].as_str().map(str::to_owned),
                    },
                    root,
                    node,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authorization_urls_are_provider_scoped() {
        assert!(auth_url(
            "openai",
            "https://auth.openai.com/api/accounts/authorize?state=synthetic"
        )
        .is_ok());
        for u in [
            "http://auth.openai.com/x",
            "https://evil.invalid/x",
            "https://user:pass@auth.openai.com/x",
        ] {
            assert!(auth_url("openai", u).is_err())
        }
        assert!(auth_url("openrouter", "https://github.com/login/device").is_err());
    }
    #[test]
    fn catalog_checks_provider_and_image_capability() {
        let v = json!({"provider":"openai","connected":true,"authType":"oauth","models":[{"id":"test","name":"Test","input":["text","image"],"contextWindow":1000,"reasoning":false}]});
        assert!(parse_catalog(v.clone(), "openai").is_ok());
        assert!(parse_catalog(v.clone(), "anthropic").is_err());
        let mut bad = v;
        bad["models"][0]["input"] = json!(["text"]);
        assert!(parse_catalog(bad, "openai").is_err());
    }
    #[cfg(unix)]
    fn fixture() -> (tempfile::TempDir, String) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("fake-pi");
        fs::create_dir_all(root.join("dist")).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"name":"@earendil-works/pi-coding-agent","version":"fixture-only"}"#,
        )
        .unwrap();
        fs::write(root.join("dist/cli.js"), "// synthetic entry").unwrap();
        let code = r###"#!/usr/bin/env python3
import sys,json
m=json.loads(sys.stdin.readline())
def send(value): print(json.dumps(value),flush=True)
catalog={'provider':m['provider'],'connected':m['operation']!='logout','authType':'oauth','models':[{'id':'fake-vision','name':'Fixture Vision','input':['text','image'],'contextWindow':1000,'reasoning':False}],'accessToken':'MUST_NOT_REACH_WEBVIEW'}
if m['operation']=='login':
    send({'type':'auth_url','url':'https://auth.openai.com/api/accounts/authorize?state=fixture'})
    send({'type':'prompt','id':'p1','kind':'manual_code','message':'Synthetic authorization code','placeholder':'fixture-only','options':[]})
    for line in sys.stdin:
        reply=json.loads(line)
        if reply['type']=='cancel': send({'type':'error','code':'cancelled'});break
        if reply['type']=='answer': send({'type':'result','value':catalog});break
elif m['operation']=='infer':
    if m['model']=='auth-error': send({'type':'error','code':'auth','message':'SYNTHETIC_SECRET_NEVER_SHOW'})
    elif m['model']=='oversize': print('x'*(2*1024*1024),flush=True)
    else:
        suggestion={'style':'art-museum','title':'Pi fixture','rationale':'Local fake process','caption':'','frameColor':'#F3ECDD','colors':['#2B2A27','#B9B1A2','#D9D3C6'],'borderPercent':14,'intensity':1,'scale':1}
        send({'type':'result','value':{'text':json.dumps({'suggestions':[suggestion]*3})}})
else: send({'type':'result','value':catalog})
"###;
        fs::write(root.join("node"), code).unwrap();
        fs::set_permissions(root.join("node"), fs::Permissions::from_mode(0o755)).unwrap();
        let path = root.join("dist/cli.js").to_string_lossy().into_owned();
        (dir, path)
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn subprocess_catalog_and_frame_pipeline_never_return_credential_fields() {
        let (dir, path) = fixture();
        let bridge = PiBridge::new(dir.path().join("state"));
        let runtime = bridge.info(Some(&path));
        assert!(runtime.available);
        let c = bridge.catalog(Some(&path), "openai", false).await.unwrap();
        assert!(c.connected);
        assert!(!serde_json::to_string(&c)
            .unwrap()
            .contains("MUST_NOT_REACH"));
        let ai = crate::art_ai::ArtAi::new(dir.path().join("app"));
        let p = ai
            .prepare("data:image/jpeg;base64,eA==".into(), 900, 600)
            .unwrap();
        let config = crate::art_ai::AiConfig {
            provider: "pi".into(),
            pi_provider: "openai".into(),
            pi_path: Some(path),
            model: "fake-vision".into(),
            ..Default::default()
        };
        assert_eq!(config.image.provider, "codex");
        let result = ai
            .generate(&p.token, config, "UNUSED_API_KEY".into())
            .await
            .unwrap();
        assert_eq!(result.len(), 3);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn oauth_prompt_roundtrip_rejects_stale_replies_and_uses_scoped_browser() {
        let (dir, path) = fixture();
        let bridge = PiBridge::new(dir.path().join("state"));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sink: EventSink = Arc::new(move |e| {
            let _ = tx.send(e);
        });
        let opened = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = opened.clone();
        let browser: Browser = Arc::new(move |url| {
            assert!(url.starts_with("https://auth.openai.com/"));
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        });
        let id = uuid::Uuid::new_v4().to_string();
        let task_bridge = bridge.clone();
        let task_id = id.clone();
        let task = tokio::spawn(async move {
            task_bridge
                .login(Some(&path), "openai", &task_id, sink, browser)
                .await
        });
        let event = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let e = rx.recv().await.unwrap();
                if e.prompt.is_some() {
                    break e;
                }
            }
        })
        .await
        .unwrap();
        assert!(bridge.answer(&id, "stale", "test".into()).await.is_err());
        bridge
            .answer(&id, &event.prompt.unwrap().id, "fixture-only-code".into())
            .await
            .unwrap();
        assert!(task.await.unwrap().unwrap().connected);
        assert!(opened.load(std::sync::atomic::Ordering::Relaxed));
        assert!(bridge.answer(&id, "p1", "late".into()).await.is_err());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn dropping_a_login_cancels_the_child_and_releases_the_next_request() {
        let (dir, path) = fixture();
        let bridge = PiBridge::new(dir.path().join("state"));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sink: EventSink = Arc::new(move |e| {
            let _ = tx.send(e);
        });
        let browser: Browser = Arc::new(|_| Ok(()));
        let id = uuid::Uuid::new_v4().to_string();
        let child = bridge.clone();
        let path2 = path.clone();
        let task = tokio::spawn(async move {
            child
                .login(Some(&path2), "openai", &id, sink, browser)
                .await
        });
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        task.abort();
        let _ = task.await;
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match bridge.catalog(Some(&path), "openai", false).await {
                    Ok(v) => break v,
                    Err(e) if e == BUSY => tokio::time::sleep(Duration::from_millis(10)).await,
                    Err(e) => panic!("{e}"),
                }
            }
        })
        .await
        .unwrap();
        assert!(result.connected);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn subprocess_errors_are_redacted_and_oversized_output_is_bounded() {
        let (dir, path) = fixture();
        let bridge = PiBridge::new(dir.path().join("state"));
        let e = bridge
            .infer(
                Some(&path),
                "openai",
                "auth-error",
                "test",
                &["data:image/jpeg;base64,eA=="],
            )
            .await
            .unwrap_err();
        assert!(!e.contains("SYNTHETIC_SECRET"));
        assert!(e.contains("授权未就绪"));
        let e = bridge
            .infer(
                Some(&path),
                "openai",
                "oversize",
                "test",
                &["data:image/jpeg;base64,eA=="],
            )
            .await
            .unwrap_err();
        assert!(e.contains("过大"));
    }
}
