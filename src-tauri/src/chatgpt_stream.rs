//! Bounded SSE decoding. Partial output is never accepted as a completed design.
use serde_json::Value;

pub const MAX_OUTPUT: usize = 256 * 1024;
pub struct ResponseStream {
    line: Vec<u8>,
    data: Vec<u8>,
    output: String,
    received: usize,
    pub error_code: Option<String>,
    pub error_param: Option<String>,
}
impl ResponseStream {
    /// Some authorized gateways buffer a complete Responses object despite stream=true.
    /// Accept only a completed response, using the same checks as the SSE final event.
    pub fn json(&mut self, value: &Value) -> Result<String, String> {
        let event = if value.get("error").is_some() {
            serde_json::json!({"type":"error","error":value["error"]})
        } else if value["object"] == "response" {
            serde_json::json!({"type":"response.completed","response":value})
        } else {
            return Err("ChatGPT 返回的内容不是模型响应，请在模型配置中检查连接".into());
        };
        self.data = serde_json::to_vec(&event).map_err(|_| "ChatGPT 响应格式无效")?;
        self.event()?.ok_or_else(|| "ChatGPT 未返回配框方案".into())
    }
    pub fn new() -> Self {
        Self {
            line: vec![],
            data: vec![],
            output: String::new(),
            received: 0,
            error_code: None,
            error_param: None,
        }
    }
    pub fn push(&mut self, bytes: &[u8]) -> Result<Option<String>, String> {
        self.received = self.received.saturating_add(bytes.len());
        if self.received > 8 * 1024 * 1024 {
            return Err("ChatGPT 返回内容过大".into());
        }
        for b in bytes {
            if *b == b'\n' {
                if self.line.last() == Some(&b'\r') {
                    self.line.pop();
                }
                let line = std::mem::take(&mut self.line);
                if line.is_empty() {
                    if let Some(value) = self.event()? {
                        return Ok(Some(value));
                    }
                } else if line.starts_with(b"data:") {
                    let mut v = &line[5..];
                    if v.first() == Some(&b' ') {
                        v = &v[1..]
                    }
                    if !self.data.is_empty() {
                        self.data.push(b'\n')
                    }
                    self.data.extend_from_slice(v);
                }
            } else {
                if self.line.len() > 2 * MAX_OUTPUT {
                    return Err("ChatGPT 返回内容过大".into());
                }
                self.line.push(*b);
            }
        }
        Ok(None)
    }
    fn event(&mut self) -> Result<Option<String>, String> {
        let data = std::mem::take(&mut self.data);
        if data.is_empty() {
            return Ok(None);
        }
        if data == b"[DONE]" {
            return Err("ChatGPT 响应尚未完成，请手动重试".into());
        }
        let v: Value = serde_json::from_slice(&data).map_err(|_| "ChatGPT 流式响应格式无效")?;
        match v["type"].as_str().unwrap_or("") {
            "response.output_text.delta" => {
                let delta = v["delta"].as_str().ok_or("ChatGPT 流式响应格式无效")?;
                if self.output.len() + delta.len() > MAX_OUTPUT {
                    return Err("ChatGPT 返回内容过大".into());
                }
                self.output.push_str(delta);
                Ok(None)
            }
            "response.completed" => {
                if v.pointer("/response/status").and_then(Value::as_str) != Some("completed") {
                    return Err("ChatGPT 响应尚未完成，请手动重试".into());
                }
                let mut final_text = String::new();
                if let Some(items) = v.pointer("/response/output").and_then(Value::as_array) {
                    for item in items {
                        if let Some(parts) = item["content"].as_array() {
                            for p in parts {
                                if p["type"] == "refusal" {
                                    return Err("ChatGPT 未能提供配框方案，请调整设计方向".into());
                                }
                                if p["type"] == "output_text" {
                                    if let Some(t) = p["text"].as_str() {
                                        if final_text.len() + t.len() > MAX_OUTPUT {
                                            return Err("ChatGPT 返回内容过大".into());
                                        }
                                        final_text.push_str(t);
                                    }
                                }
                            }
                        }
                    }
                }
                if final_text.is_empty() {
                    final_text = std::mem::take(&mut self.output)
                }
                if final_text.trim().is_empty() {
                    return Err("ChatGPT 未返回配框方案".into());
                }
                Ok(Some(final_text))
            }
            "response.failed" | "error" => {
                let safe = |s: &str| {
                    s.len() <= 120
                        && s.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_.[]".contains(&b))
                };
                self.error_code = v
                    .pointer("/response/error/code")
                    .or_else(|| v.pointer("/error/code"))
                    .and_then(Value::as_str)
                    .filter(|s| safe(s))
                    .map(str::to_owned);
                self.error_param = v
                    .pointer("/response/error/param")
                    .or_else(|| v.pointer("/error/param"))
                    .and_then(Value::as_str)
                    .filter(|s| safe(s))
                    .map(str::to_owned);
                Err(error_message(self.error_code.as_deref()))
            }
            "response.incomplete" => Err("ChatGPT 响应尚未完成，请手动重试".into()),
            _ => Ok(None),
        }
    }
}
pub fn error_message(code: Option<&str>) -> String {
    match code.unwrap_or("") {
        "subscription_sharing_usage_limit_exceeded" => {
            "ChatGPT 授权用量已达上限，请在 ChatGPT 用量设置中查看"
        }
        "subscription_sharing_user_not_eligible" => "此 ChatGPT 账户或工作区暂不支持套餐授权",
        "subscription_sharing_usage_unavailable" | "subscription_sharing_user_unavailable" => {
            "ChatGPT 套餐用量暂时不可用，请稍后手动重试"
        }
        "subscription_sharing_unsupported_capability"
        | "subscription_sharing_route_not_supported" => {
            "ChatGPT 授权不支持此模型或请求功能，请换一个可用模型"
        }
        "chatpass_v2_scope_not_authorized" | "chatpass_v2_invalid_authorization_context" => {
            "ChatGPT 套餐使用权限不足，请重新授权"
        }
        _ => "ChatGPT 请求失败，请检查连接后手动重试",
    }
    .into()
}
