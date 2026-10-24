//! The model APIs the chat speaks natively: OpenAI-compatible chat completions (OpenAI, OpenRouter,
//! Groq, Mistral, DeepSeek, xAI, Ollama, LM Studio, …), Anthropic Messages and Google Gemini.
//!
//! The conversation is kept in the chat completions shape. Each request converts it to the
//! provider's, and each answer comes back to it. What a provider needs back verbatim on the next
//! request (Claude's thinking blocks, Gemini's thought signatures) rides in the assistant message
//! under [`NATIVE`], and a failed tool result is marked with [`FAILED`]. Fields that start with `_`
//! never reach a request.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::backend::HttpRequest;

/// The `anthropic-version` the Messages requests send.
pub const ANTHROPIC_VERSION: &str = "2026-10-21";
/// An assistant message's own content as its provider sent it: `{wire, content}`.
pub const NATIVE: &str = "_native";
/// On a tool message: the call failed.
pub const FAILED: &str = "_failed";
/// Most tokens one Claude answer may use, thinking included (kept under the non-streaming limit).
const ANTHROPIC_MAX_TOKENS: u32 = 16_000;
/// Tool call ids made up for Gemini calls that carry none.
const GEMINI_ID: &str = "gemini-call-";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wire {
    /// `POST {base}/chat/completions`.
    #[default]
    OpenAi,
    /// `POST {base}/messages`.
    Anthropic,
    /// `POST {base}/models/{model}:generateContent`.
    Gemini,
}

impl Wire {
    pub const ALL: [Wire; 3] = [Wire::OpenAi, Wire::Anthropic, Wire::Gemini];

    pub fn id(self) -> &'static str {
        match self {
            Wire::OpenAi => "openai",
            Wire::Anthropic => "anthropic",
            Wire::Gemini => "gemini",
        }
    }

    pub fn label(self) -> &'static str {
        crate::i18n::tr(match self {
            Wire::OpenAi => "ui.ai_wire_openai",
            Wire::Anthropic => "ui.ai_wire_anthropic",
            Wire::Gemini => "ui.ai_wire_gemini",
        })
    }

    fn headers(self, key: Option<&str>) -> Vec<(String, String)> {
        let mut h = vec![("Content-Type".to_string(), "application/json".to_string())];
        match (self, key) {
            (Wire::OpenAi, Some(k)) => h.push(("Authorization".into(), format!("Bearer {k}"))),
            (Wire::Anthropic, k) => {
                if let Some(k) = k {
                    h.push(("x-api-key".into(), k.to_string()));
                }
                h.push(("anthropic-version".into(), ANTHROPIC_VERSION.to_string()));
            }
            (Wire::Gemini, Some(k)) => h.push(("x-goog-api-key".into(), k.to_string())),
            (_, None) => {}
        }
        h
    }

    /// One model request: the conversation so far and the tools (chat completions function tools).
    pub fn request(self, base_url: &str, model: &str, key: Option<&str>, history: &[Value], tools: &[Value], timeout_s: u64) -> HttpRequest {
        let (url, body) = match self {
            Wire::OpenAi => {
                let mut body = json!({"model": model, "messages": history.iter().map(public).collect::<Vec<_>>()});
                if !tools.is_empty() {
                    body["tools"] = Value::Array(tools.to_vec());
                }
                (format!("{base_url}/chat/completions"), body)
            }
            Wire::Anthropic => (format!("{base_url}/messages"), anthropic_body(model, history, tools)),
            Wire::Gemini => (format!("{base_url}/models/{}:generateContent", path_safe(model)), gemini_body(history, tools)),
        };
        HttpRequest { method: "POST", url, headers: self.headers(key), body: Some(body.to_string().into_bytes()), timeout_s }
    }

    /// The answer as a chat completions assistant message (`content`, `tool_calls`, and the
    /// provider's own content under [`NATIVE`]), or why there is none.
    pub fn reply(self, status: u16, body: &Value) -> Result<Value, String> {
        if status != 200 {
            return Err(http_error(status, body));
        }
        match self {
            Wire::OpenAi => {
                body.pointer("/choices/0/message").filter(|m| m.is_object()).cloned().ok_or_else(|| crate::i18n::tr("ui.ai_no_answer").to_string())
            }
            Wire::Anthropic => anthropic_reply(body),
            Wire::Gemini => gemini_reply(body),
        }
    }

    /// `GET` the provider's model list.
    pub fn models_request(self, base_url: &str, key: Option<&str>) -> HttpRequest {
        let url = match self {
            Wire::Gemini => format!("{base_url}/models?pageSize=1000"),
            Wire::Anthropic => format!("{base_url}/models?limit=1000"),
            _ => format!("{base_url}/models"),
        };
        let mut headers = self.headers(key);
        headers.retain(|(k, _)| k != "Content-Type");
        HttpRequest { method: "GET", url, headers, body: None, timeout_s: 15 }
    }

    /// The model ids in a model list, in its order: only the ones that can chat, where it says.
    pub fn model_ids(self, body: &Value) -> Vec<String> {
        let list = |key: &str| body.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
        match self {
            Wire::OpenAi | Wire::Anthropic => {
                list("data").iter().take(500).filter_map(|m| m.get("id").and_then(Value::as_str).map(String::from)).collect()
            }
            Wire::Gemini => list("models")
                .iter()
                .take(1000)
                .filter(|m| {
                    m.get("supportedGenerationMethods")
                        .and_then(Value::as_array)
                        .is_none_or(|a| a.iter().any(|x| x.as_str() == Some("generateContent")))
                })
                .filter_map(|m| m.get("name").and_then(Value::as_str).map(|n| n.trim_start_matches("models/").to_string()))
                .collect(),
        }
    }
}

/// An endpoint's refusal as people read it.
pub fn http_error(status: u16, body: &Value) -> String {
    let detail =
        body.pointer("/error/message").or_else(|| body.get("error")).and_then(Value::as_str).unwrap_or("").chars().take(300).collect::<String>();
    let what = match status {
        401 | 403 => crate::i18n::tr("ui.ai_http_key"),
        404 => crate::i18n::tr("ui.ai_http_not_found"),
        429 => crate::i18n::tr("ui.ai_http_busy"),
        _ => "",
    };
    format!(
        "HTTP {status}{}{}",
        if what.is_empty() { String::new() } else { format!(": {what}") },
        if detail.is_empty() { String::new() } else { format!(" ({detail})") }
    )
}

/// A message without the fields only the chat reads.
fn public(m: &Value) -> Value {
    match m {
        Value::Object(o) => Value::Object(o.iter().filter(|(k, _)| !k.starts_with('_')).map(|(k, v)| (k.clone(), v.clone())).collect()),
        other => other.clone(),
    }
}

/// A model id as one URL path segment.
fn path_safe(model: &str) -> String {
    model.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '@')).collect()
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().filter_map(|p| p.get("text").and_then(Value::as_str)).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// A user message's parts: `(Some(text), None)` or `(None, Some((mime, base64)))`.
fn user_parts(content: &Value) -> Vec<(Option<String>, Option<(String, String)>)> {
    match content {
        Value::String(s) => vec![(Some(s.clone()), None)],
        Value::Array(a) => a
            .iter()
            .filter_map(|p| match p.get("type").and_then(Value::as_str) {
                Some("text") => Some((p.get("text").and_then(Value::as_str).map(String::from), None)),
                Some("image_url") => {
                    let url = p.pointer("/image_url/url").and_then(Value::as_str)?;
                    let (head, data) = url.strip_prefix("data:")?.split_once(";base64,")?;
                    Some((None, Some((head.to_string(), data.to_string()))))
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn calls(m: &Value) -> Vec<(String, String, Value)> {
    m.get("tool_calls")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|c| {
                    let id = c.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                    let name = c.pointer("/function/name").and_then(Value::as_str).unwrap_or("").to_string();
                    let args = match c.pointer("/function/arguments") {
                        Some(Value::String(s)) => serde_json::from_str::<Value>(s).ok().filter(Value::is_object).unwrap_or(json!({})),
                        Some(v @ Value::Object(_)) => v.clone(),
                        _ => json!({}),
                    };
                    (id, name, args)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn native(m: &Value, wire: Wire) -> Option<&Value> {
    m.get(NATIVE).filter(|n| n.get("wire").and_then(Value::as_str) == Some(wire.id())).and_then(|n| n.get("content"))
}

/// Append blocks as a message of `role`, joining them to the last message when it has that role
/// (Messages and Gemini alternate roles: tool results go back together, in one user turn).
fn push(messages: &mut Vec<Value>, role: &str, parts_key: &str, blocks: Vec<Value>) {
    if blocks.is_empty() {
        return;
    }
    if let Some(last) = messages.last_mut()
        && last.get("role").and_then(Value::as_str) == Some(role)
        && let Some(a) = last.get_mut(parts_key).and_then(Value::as_array_mut)
    {
        a.extend(blocks);
        return;
    }
    messages.push(json!({"role": role, parts_key: blocks}));
}

fn anthropic_body(model: &str, history: &[Value], tools: &[Value]) -> Value {
    let mut system: Vec<Value> = Vec::new();
    let mut messages: Vec<Value> = Vec::new();
    for m in history {
        match m.get("role").and_then(Value::as_str) {
            Some("system") => system.push(json!({"type": "text", "text": text_of(&m["content"])})),
            Some("user") => {
                let blocks = user_parts(&m["content"])
                    .into_iter()
                    .filter_map(|p| match p {
                        (Some(t), _) if !t.is_empty() => Some(json!({"type": "text", "text": t})),
                        (_, Some((mime, data))) => Some(json!({"type": "image", "source": {"type": "base64", "media_type": mime, "data": data}})),
                        _ => None,
                    })
                    .collect();
                push(&mut messages, "user", "content", blocks);
            }
            Some("assistant") => {
                let blocks = match native(m, Wire::Anthropic).and_then(Value::as_array) {
                    Some(a) => a.clone(),
                    None => {
                        let mut b = Vec::new();
                        let text = text_of(&m["content"]);
                        if !text.is_empty() {
                            b.push(json!({"type": "text", "text": text}));
                        }
                        b.extend(calls(m).into_iter().map(|(id, name, input)| json!({"type": "tool_use", "id": id, "name": name, "input": input})));
                        b
                    }
                };
                push(&mut messages, "assistant", "content", blocks);
            }
            Some("tool") => {
                let mut r = json!({"type": "tool_result", "tool_use_id": m["tool_call_id"], "content": text_of(&m["content"])});
                if m.get(FAILED).and_then(Value::as_bool) == Some(true) {
                    r["is_error"] = json!(true);
                }
                push(&mut messages, "user", "content", vec![r]);
            }
            _ => {}
        }
    }
    // Two cache breakpoints: the tools and system prompt, and the conversation so far (each tool
    // round in a turn then reads everything before it from the cache).
    if let Some(last) = system.last_mut() {
        last["cache_control"] = json!({"type": "ephemeral"});
    }
    if let Some(block) = messages.last_mut().and_then(|m| m.get_mut("content")).and_then(Value::as_array_mut).and_then(|a| a.last_mut())
        && matches!(block.get("type").and_then(Value::as_str), Some("text" | "image" | "tool_result"))
    {
        block["cache_control"] = json!({"type": "ephemeral"});
    }
    let mut body = json!({
        "model": model,
        "max_tokens": ANTHROPIC_MAX_TOKENS,
        "thinking": {"type": "adaptive"},
        "messages": messages,
    });
    if !system.is_empty() {
        body["system"] = Value::Array(system);
    }
    if !tools.is_empty() {
        body["tools"] = Value::Array(
            tools
                .iter()
                .filter_map(|t| {
                    let f = t.get("function")?;
                    Some(json!({"name": f.get("name")?, "description": f.get("description").cloned().unwrap_or(json!("")), "input_schema": f.get("parameters").cloned().unwrap_or(json!({"type": "object"}))}))
                })
                .collect(),
        );
    }
    body
}

fn tool_call(id: &str, name: &str, args: &Value) -> Value {
    json!({"id": id, "type": "function", "function": {"name": name, "arguments": args.to_string()}})
}

fn assistant(text: String, tool_calls: Vec<Value>, wire: Wire, content: Value) -> Value {
    let mut m = json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { Value::String(text) }, NATIVE: {"wire": wire.id(), "content": content}});
    if !tool_calls.is_empty() {
        m["tool_calls"] = Value::Array(tool_calls);
    }
    m
}

fn anthropic_reply(body: &Value) -> Result<Value, String> {
    let content = body.get("content").cloned().unwrap_or(json!([]));
    let blocks = content.as_array().cloned().unwrap_or_default();
    let stop = body.get("stop_reason").and_then(Value::as_str).unwrap_or("");
    if stop == "refusal" {
        return Err(crate::i18n::tr("ui.ai_refused").to_string());
    }
    let mut text: Vec<&str> = blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b.get("text").and_then(Value::as_str)).collect();
    let cut = crate::i18n::tr("ui.ai_answer_cut");
    let cut_line = format!("({cut})");
    if stop == "max_tokens" {
        text.push(&cut_line);
    }
    let tool_calls = blocks
        .iter()
        .filter(|b| b["type"] == "tool_use")
        .map(|b| tool_call(b.get("id").and_then(Value::as_str).unwrap_or(""), b.get("name").and_then(Value::as_str).unwrap_or(""), &b["input"]))
        .collect();
    Ok(assistant(text.join("\n\n"), tool_calls, Wire::Anthropic, content))
}

fn gemini_body(history: &[Value], tools: &[Value]) -> Value {
    let mut system: Vec<String> = Vec::new();
    let mut contents: Vec<Value> = Vec::new();
    // Gemini answers a call by its function's name: the names of the calls made so far, by id.
    let mut names: Map<String, Value> = Map::new();
    for m in history {
        match m.get("role").and_then(Value::as_str) {
            Some("system") => system.push(text_of(&m["content"])),
            Some("user") => {
                let parts = user_parts(&m["content"])
                    .into_iter()
                    .filter_map(|p| match p {
                        (Some(t), _) if !t.is_empty() => Some(json!({"text": t})),
                        (_, Some((mime, data))) => Some(json!({"inlineData": {"mimeType": mime, "data": data}})),
                        _ => None,
                    })
                    .collect();
                push(&mut contents, "user", "parts", parts);
            }
            Some("assistant") => {
                let made = calls(m);
                for (id, name, _) in &made {
                    names.insert(id.clone(), json!(name));
                }
                let parts = match native(m, Wire::Gemini).and_then(Value::as_array) {
                    Some(a) => a.clone(),
                    None => {
                        let mut p = Vec::new();
                        let text = text_of(&m["content"]);
                        if !text.is_empty() {
                            p.push(json!({"text": text}));
                        }
                        p.extend(made.into_iter().map(|(_, name, args)| json!({"functionCall": {"name": name, "args": args}})));
                        p
                    }
                };
                push(&mut contents, "model", "parts", parts);
            }
            Some("tool") => {
                let id = m.get("tool_call_id").and_then(Value::as_str).unwrap_or("");
                let text = text_of(&m["content"]);
                let failed = m.get(FAILED).and_then(Value::as_bool) == Some(true);
                let mut r = json!({
                    "name": names.get(id).cloned().unwrap_or(json!("")),
                    "response": if failed { json!({"error": text}) } else { json!({"result": text}) },
                });
                if !id.is_empty() && !id.starts_with(GEMINI_ID) {
                    r["id"] = json!(id);
                }
                push(&mut contents, "user", "parts", vec![json!({"functionResponse": r})]);
            }
            _ => {}
        }
    }
    let mut body = json!({"contents": contents});
    if !system.is_empty() {
        body["systemInstruction"] = json!({"parts": [{"text": system.join("\n\n")}]});
    }
    if !tools.is_empty() {
        let decls: Vec<Value> = tools
            .iter()
            .filter_map(|t| {
                let f = t.get("function")?;
                Some(json!({"name": f.get("name")?, "description": f.get("description").cloned().unwrap_or(json!("")), "parametersJsonSchema": f.get("parameters").cloned().unwrap_or(json!({"type": "object"}))}))
            })
            .collect();
        body["tools"] = json!([{"functionDeclarations": decls}]);
    }
    body
}

fn gemini_reply(body: &Value) -> Result<Value, String> {
    let Some(candidate) = body.pointer("/candidates/0") else {
        let why = body.pointer("/promptFeedback/blockReason").and_then(Value::as_str).unwrap_or("no answer");
        return Err(format!("{} ({why})", crate::i18n::tr("ui.ai_refused")));
    };
    let parts = candidate.pointer("/content/parts").and_then(Value::as_array).cloned().unwrap_or_default();
    let reason = candidate.get("finishReason").and_then(Value::as_str).unwrap_or("STOP");
    if parts.is_empty() && !matches!(reason, "STOP" | "MAX_TOKENS") {
        return Err(format!("{} ({reason})", crate::i18n::tr("ui.ai_refused")));
    }
    let mut text: Vec<String> = parts
        .iter()
        .filter(|p| p.get("thought").and_then(Value::as_bool) != Some(true))
        .filter_map(|p| p.get("text").and_then(Value::as_str).map(String::from))
        .collect();
    if reason == "MAX_TOKENS" {
        text.push(format!("({})", crate::i18n::tr("ui.ai_answer_cut")));
    }
    let tool_calls = parts
        .iter()
        .filter_map(|p| p.get("functionCall"))
        .enumerate()
        .map(|(i, c)| {
            let id = c.get("id").and_then(Value::as_str).map_or_else(|| format!("{GEMINI_ID}{i}"), String::from);
            tool_call(&id, c.get("name").and_then(Value::as_str).unwrap_or(""), c.get("args").unwrap_or(&json!({})))
        })
        .collect();
    Ok(assistant(text.join(""), tool_calls, Wire::Gemini, Value::Array(parts)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<Value> {
        vec![
            json!({"type": "function", "function": {"name": "measure", "description": "Measure", "parameters": {"type": "object", "properties": {"bodies": {"type": "array"}}, "additionalProperties": false}}}),
        ]
    }

    /// A conversation with a picture, a tool round (one call failed) and a reply.
    fn history() -> Vec<Value> {
        vec![
            json!({"role": "system", "content": "You are in Polykrom."}),
            json!({"role": "user", "content": [{"type": "text", "text": "Model this"}, {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBOR"}}]}),
            json!({"role": "assistant", "content": null, "tool_calls": [
                {"id": "t1", "type": "function", "function": {"name": "measure", "arguments": "{\"bodies\":[\"Body1\"]}"}},
                {"id": "t2", "type": "function", "function": {"name": "measure", "arguments": "{}"}}]}),
            json!({"role": "tool", "tool_call_id": "t1", "content": "volume 1000"}),
            json!({"role": "tool", "tool_call_id": "t2", "content": "no such body", FAILED: true}),
            json!({"role": "assistant", "content": "Done."}),
            json!({"role": "user", "content": "Thanks"}),
        ]
    }

    fn body(r: &HttpRequest) -> Value {
        serde_json::from_slice(r.body.as_deref().unwrap_or_default()).unwrap()
    }

    fn header<'a>(r: &'a HttpRequest, k: &str) -> Option<&'a str> {
        r.headers.iter().find(|(h, _)| h == k).map(|(_, v)| v.as_str())
    }

    #[test]
    fn openai_requests_leave_the_chats_own_fields_out() {
        let mut h = history();
        h[5][NATIVE] = json!({"wire": "anthropic", "content": []});
        let r = Wire::OpenAi.request("https://api.openai.com/v1", "m", Some("sk"), &h, &tools(), 300);
        assert_eq!(r.url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(header(&r, "Authorization"), Some("Bearer sk"));
        let b = body(&r);
        assert!(!b.to_string().contains("_native") && !b.to_string().contains("_failed"));
        assert_eq!(b["messages"].as_array().map(Vec::len), Some(7));
        assert_eq!(b["tools"][0]["function"]["name"], "measure");
    }

    #[test]
    fn anthropic_requests_alternate_cache_and_mark_failed_results() {
        let r = Wire::Anthropic.request("https://api.anthropic.com/v1", "claude-opus-5-5", Some("k"), &history(), &tools(), 300);
        assert_eq!(r.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(header(&r, "x-api-key"), Some("k"));
        assert_eq!(header(&r, "anthropic-version"), Some(ANTHROPIC_VERSION));
        assert_eq!(header(&r, "Authorization"), None, "the key goes only in x-api-key");
        let b = body(&r);
        assert_eq!(b["thinking"]["type"], "adaptive");
        assert!(b.get("temperature").is_none() && b.get("tool_choice").is_none());
        assert_eq!(b["system"][0]["text"], "You are in Polykrom.");
        assert_eq!(b["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(b["tools"][0]["input_schema"]["properties"]["bodies"]["type"], "array");
        let m = b["messages"].as_array().unwrap();
        let roles: Vec<&str> = m.iter().map(|x| x["role"].as_str().unwrap()).collect();
        assert_eq!(roles, ["user", "assistant", "user", "assistant", "user"], "both tool results go back in one user turn");
        assert_eq!(m[0]["content"][1]["source"], json!({"type": "base64", "media_type": "image/png", "data": "iVBOR"}));
        assert_eq!(m[1]["content"][0], json!({"type": "tool_use", "id": "t1", "name": "measure", "input": {"bodies": ["Body1"]}}));
        assert_eq!(m[2]["content"][0]["tool_use_id"], "t1");
        assert!(m[2]["content"][0].get("is_error").is_none());
        assert_eq!(m[2]["content"][1]["is_error"], true);
        assert_eq!(m[4]["content"][0]["cache_control"]["type"], "ephemeral", "the conversation so far is cached");
    }

    #[test]
    fn claude_answers_come_back_with_their_thinking_kept_for_the_next_request() {
        let answer = json!({"stop_reason": "tool_use", "content": [
            {"type": "thinking", "thinking": "plan", "signature": "sig"},
            {"type": "text", "text": "Measuring."},
            {"type": "tool_use", "id": "toolu_1", "name": "measure", "input": {"bodies": []}}]});
        let m = Wire::Anthropic.reply(200, &answer).unwrap();
        assert_eq!(m["content"], "Measuring.");
        assert_eq!(m["tool_calls"][0]["id"], "toolu_1");
        assert_eq!(m["tool_calls"][0]["function"]["arguments"], "{\"bodies\":[]}");
        let h = vec![json!({"role": "user", "content": "go"}), m, json!({"role": "tool", "tool_call_id": "toolu_1", "content": "ok"})];
        let b = body(&Wire::Anthropic.request("u", "m", None, &h, &[], 1));
        assert_eq!(b["messages"][1]["content"][0], json!({"type": "thinking", "thinking": "plan", "signature": "sig"}), "verbatim");
        assert!(Wire::Anthropic.reply(200, &json!({"stop_reason": "refusal", "content": []})).is_err());
        let cut = Wire::Anthropic.reply(200, &json!({"stop_reason": "max_tokens", "content": [{"type": "text", "text": "Half"}]})).unwrap();
        assert!(cut["content"].as_str().unwrap().starts_with("Half\n\n("));
        let err = Wire::Anthropic
            .reply(401, &json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}}))
            .unwrap_err();
        assert!(err.contains("401") && err.contains("invalid x-api-key"), "{err}");
    }

    #[test]
    fn gemini_requests_name_each_answered_function_and_keep_signatures() {
        let r = Wire::Gemini.request("https://generativelanguage.googleapis.com/v1beta", "gemini-x/../y", Some("g"), &history(), &tools(), 300);
        assert_eq!(r.url, "https://generativelanguage.googleapis.com/v1beta/models/gemini-x..y:generateContent", "the model stays one path segment");
        assert_eq!(header(&r, "x-goog-api-key"), Some("g"));
        let b = body(&r);
        assert_eq!(b["systemInstruction"]["parts"][0]["text"], "You are in Polykrom.");
        assert_eq!(b["tools"][0]["functionDeclarations"][0]["parametersJsonSchema"]["additionalProperties"], false);
        let c = b["contents"].as_array().unwrap();
        let roles: Vec<&str> = c.iter().map(|x| x["role"].as_str().unwrap()).collect();
        assert_eq!(roles, ["user", "model", "user", "model", "user"]);
        assert_eq!(c[0]["parts"][1]["inlineData"]["mimeType"], "image/png");
        assert_eq!(c[1]["parts"][0]["functionCall"], json!({"name": "measure", "args": {"bodies": ["Body1"]}}));
        assert_eq!(c[2]["parts"][0]["functionResponse"], json!({"name": "measure", "id": "t1", "response": {"result": "volume 1000"}}));
        assert_eq!(c[2]["parts"][1]["functionResponse"]["response"], json!({"error": "no such body"}));

        let answer = json!({"candidates": [{"finishReason": "STOP", "content": {"role": "model", "parts": [
            {"text": "thinking…", "thought": true},
            {"text": "Measuring."},
            {"functionCall": {"name": "measure", "args": {}}, "thoughtSignature": "abc"}]}}]});
        let m = Wire::Gemini.reply(200, &answer).unwrap();
        assert_eq!(m["content"], "Measuring.", "thoughts are not shown as the answer");
        let id = m["tool_calls"][0]["id"].as_str().unwrap().to_string();
        let h = vec![json!({"role": "user", "content": "go"}), m, json!({"role": "tool", "tool_call_id": id, "content": "ok"})];
        let b = body(&Wire::Gemini.request("u", "m", None, &h, &[], 1));
        assert_eq!(b["contents"][1]["parts"][2]["thoughtSignature"], "abc", "the call goes back with its signature");
        assert_eq!(
            b["contents"][2]["parts"][0]["functionResponse"],
            json!({"name": "measure", "response": {"result": "ok"}}),
            "a made-up id is not sent"
        );
        assert!(Wire::Gemini.reply(200, &json!({"promptFeedback": {"blockReason": "SAFETY"}})).unwrap_err().contains("SAFETY"));
    }

    #[test]
    fn model_lists_keep_the_models_that_chat() {
        assert_eq!(Wire::OpenAi.model_ids(&json!({"data": [{"id": "a"}, {"id": "b"}]})), ["a", "b"]);
        assert_eq!(Wire::Anthropic.model_ids(&json!({"data": [{"id": "claude-opus-5-5", "display_name": "Claude Opus 5.5"}]})), ["claude-opus-5-5"]);
        let g = json!({"models": [
            {"name": "models/gemini-a", "supportedGenerationMethods": ["generateContent", "countTokens"]},
            {"name": "models/embedding-b", "supportedGenerationMethods": ["embedContent"]}]});
        assert_eq!(Wire::Gemini.model_ids(&g), ["gemini-a"]);
        let r = Wire::Anthropic.models_request("https://api.anthropic.com/v1", Some("k"));
        assert_eq!((r.method, r.url.as_str()), ("GET", "https://api.anthropic.com/v1/models?limit=1000"), "a full page, not the first 20");
        assert!(Wire::OpenAi.reply(200, &json!({"html": "a proxy's page"})).is_err(), "no message is an error, not an empty turn");
        assert_eq!(header(&r, "anthropic-version"), Some(ANTHROPIC_VERSION));
        assert_eq!(Wire::Gemini.models_request("https://g/v1beta", Some("g")).url, "https://g/v1beta/models?pageSize=1000");
    }
}
