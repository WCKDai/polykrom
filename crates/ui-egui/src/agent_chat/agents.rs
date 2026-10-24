//! The agents the chat can use: the installed ones it finds by itself (Claude Code, Codex,
//! Antigravity, OpenCode, …) and the ones the user adds (another ACP command, or a model API:
//! Anthropic, Gemini, or any OpenAI-compatible endpoint such as OpenAI, OpenRouter or Ollama).
//! Also the permission modes.
//! Preferences only: no design data, no keys (those are in the system keychain).

use serde::{Deserialize, Serialize};

pub use super::wire::Wire;

/// How much the agent may do without asking.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Every change to the design, file and destructive action asks first.
    Ask,
    /// Modelling commands run; files and destructive actions still ask (the default).
    #[default]
    Edits,
    /// Nothing asks (crash reports and the folder limits still hold).
    All,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Ask, Mode::Edits, Mode::All];
    pub fn id(self) -> &'static str {
        match self {
            Mode::Ask => "ask",
            Mode::Edits => "edits",
            Mode::All => "all",
        }
    }
    pub fn parse(s: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.id() == s)
    }
    pub fn label(self) -> &'static str {
        crate::i18n::tr(match self {
            Mode::Ask => "ui.ai_mode_ask",
            Mode::Edits => "ui.ai_mode_edits",
            Mode::All => "ui.ai_mode_all",
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A program that speaks the Agent Client Protocol.
    #[default]
    Acp,
    /// A model API ([`Wire`]).
    Api,
}

/// An agent the user added.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    /// ACP: the command line.
    pub command: String,
    /// API: how to speak to it, the endpoint (`…/v1`) and model.
    pub wire: Wire,
    pub base_url: String,
    pub model: String,
    /// API: a key is stored for it in the system keychain.
    pub key: bool,
}

/// An agent as the picker and the switcher show it.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    /// ACP: what to start.
    pub argv: Vec<String>,
    pub version: Option<String>,
    /// `signed_in`, `key`, `signed_out`, `unknown` (found agents), or empty.
    pub login: &'static str,
    /// Found installed (not added by the user).
    pub found: bool,
    pub wire: Wire,
    pub base_url: String,
    pub model: String,
    pub key: bool,
}

impl Entry {
    fn from_config(c: &AgentConfig) -> Entry {
        Entry {
            id: c.id.clone(),
            name: if c.name.trim().is_empty() { c.id.clone() } else { c.name.clone() },
            kind: c.kind,
            argv: super::acp::split_command(&c.command),
            version: None,
            login: "",
            found: false,
            wire: c.wire,
            base_url: c.base_url.clone(),
            model: c.model.clone(),
            key: c.key,
        }
    }
    /// A short line under the name: the version, the model, or the command.
    pub fn detail(&self) -> String {
        match self.kind {
            Kind::Api => format!("{} · {}", self.model, self.base_url),
            Kind::Acp => match &self.version {
                Some(v) => v.clone(),
                None => super::panel::short_command(&self.argv.join(" ")),
            },
        }
    }
}

/// Every agent the chat can use: the installed ones found, then the ones the user added.
pub fn entries(app: &crate::SolveApp) -> Vec<Entry> {
    let mut out = Vec::new();
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(d) = &app.agent_chat.detection {
        out.extend(d.found.iter().map(|f| Entry {
            id: f.id.to_string(),
            name: f.name.to_string(),
            kind: Kind::Acp,
            argv: f.argv.clone(),
            version: f.version.clone(),
            login: f.login.id(),
            found: true,
            wire: Wire::OpenAi,
            base_url: String::new(),
            model: String::new(),
            key: false,
        }));
    }
    out.extend(app.preferences.agents.iter().map(Entry::from_config));
    out
}

/// The agent a conversation uses: the one picked in the panel, else the default, else the first.
pub fn current(app: &crate::SolveApp) -> Option<Entry> {
    let all = entries(app);
    let pick = |id: &str| all.iter().find(|e| e.id == id).cloned();
    app.agent_chat.agent_id.as_deref().and_then(pick).or_else(|| pick(&app.preferences.agent_default)).or_else(|| all.first().cloned())
}

/// The permission mode for an agent: its own, else the global one.
pub fn mode_for(p: &crate::prefs::Prefs, id: &str) -> Mode {
    p.agent_permissions.get(id).copied().unwrap_or(p.ai_permissions)
}

/// Whether `tool` is always allowed for `agent`.
pub fn always_allowed(p: &crate::prefs::Prefs, agent: &str, tool: &str) -> bool {
    p.agent_always_allow.get(agent).is_some_and(|t| t.iter().any(|x| x == tool))
}

/// Remember an "always allow" answer.
pub fn remember(p: &mut crate::prefs::Prefs, agent: &str, tool: &str) {
    let list = p.agent_always_allow.entry(agent.to_string()).or_default();
    if !list.iter().any(|x| x == tool) && list.len() < 500 {
        list.push(tool.chars().take(200).collect());
    }
}

/// A free id for a new agent: `prefix-1`, `prefix-2`, …
pub fn new_id(p: &crate::prefs::Prefs, prefix: &str) -> String {
    (1..).map(|n| format!("{prefix}-{n}")).find(|id| !p.agents.iter().any(|a| &a.id == id)).unwrap_or_else(|| prefix.to_string())
}

/// A model provider the Add agent dialog offers.
pub struct Provider {
    pub name: &'static str,
    pub wire: Wire,
    pub base_url: &'static str,
    /// It needs a key.
    pub key: bool,
    /// The model it starts with (empty: pick one after Test).
    pub model: &'static str,
    /// Where to get a key.
    pub keys_at: &'static str,
}

const fn provider(name: &'static str, wire: Wire, base_url: &'static str, key: bool, model: &'static str, keys_at: &'static str) -> Provider {
    Provider { name, wire, base_url, key, model, keys_at }
}

/// The providers people use, hosted first, then the ones on this computer.
pub const PROVIDERS: &[Provider] = &[
    provider("Anthropic", Wire::Anthropic, "https://api.anthropic.com/v1", true, "claude-opus-5-5", "https://platform.claude.com/settings/keys"),
    provider("OpenAI", Wire::OpenAi, "https://api.openai.com/v1", true, "", "https://platform.openai.com/api-keys"),
    provider("Google Gemini", Wire::Gemini, "https://generativelanguage.googleapis.com/v1beta", true, "", "https://aistudio.google.com/apikey"),
    provider("OpenRouter", Wire::OpenAi, "https://openrouter.ai/api/v1", true, "", "https://openrouter.ai/settings/keys"),
    provider("Mistral", Wire::OpenAi, "https://api.mistral.ai/v1", true, "", "https://console.mistral.ai/api-keys"),
    provider("xAI", Wire::OpenAi, "https://api.x.ai/v1", true, "", "https://console.x.ai"),
    provider("DeepSeek", Wire::OpenAi, "https://api.deepseek.com/v1", true, "deepseek-chat", "https://platform.deepseek.com/api_keys"),
    provider("Groq", Wire::OpenAi, "https://api.groq.com/openai/v1", true, "", "https://console.groq.com/keys"),
    provider("Ollama", Wire::OpenAi, "http://localhost:11434/v1", false, "", ""),
    provider("LM Studio", Wire::OpenAi, "http://localhost:1234/v1", false, "", ""),
];

/// Whether a key may be sent to `url`: only over HTTPS, or to this computer.
pub fn key_safe_for(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    if u.starts_with("https://") {
        return true;
    }
    let Some(rest) = u.strip_prefix("http://") else { return false };
    let host = rest.split(['/', '?']).next().unwrap_or("");
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let name = if host.starts_with('[') {
        host.split(']').next().map(|h| format!("{h}]")).unwrap_or_default()
    } else {
        host.split(':').next().unwrap_or("").to_string()
    };
    matches!(name.as_str(), "localhost" | "127.0.0.1" | "[::1]")
}

/// A base URL the API backend accepts: `http(s)://host…`, without a trailing slash.
pub fn normalize_url(url: &str) -> Result<String, String> {
    let u = url.trim().trim_end_matches('/').to_string();
    let lower = u.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) || u.len() < 10 || u.contains(char::is_whitespace) {
        return Err(crate::i18n::tr("ui.ai_bad_url").to_string());
    }
    Ok(u)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_go_only_over_https_or_to_this_computer() {
        assert!(key_safe_for("https://api.openai.com/v1"));
        assert!(key_safe_for("http://localhost:11434/v1"));
        assert!(key_safe_for("http://127.0.0.1:1234/v1"));
        assert!(key_safe_for("http://[::1]:8080/v1"));
        assert!(!key_safe_for("http://192.168.1.20:11434/v1"));
        assert!(!key_safe_for("http://localhost.evil.example/v1"));
        assert!(!key_safe_for("http://user@evil.example/v1"));
        assert!(!key_safe_for("ftp://x"));
    }

    #[test]
    fn modes_and_ids_round_trip() {
        for m in Mode::ALL {
            assert_eq!(Mode::parse(m.id()), Some(m));
        }
        let mut p = crate::prefs::Prefs::default();
        assert_eq!(p.ai_permissions, Mode::Edits, "model edits run, files ask: the default");
        p.agent_permissions.insert("codex".into(), Mode::Ask);
        assert_eq!(mode_for(&p, "codex"), Mode::Ask);
        assert_eq!(mode_for(&p, "claude"), Mode::Edits);
        remember(&mut p, "claude", "mcp__solidcraft__save");
        remember(&mut p, "claude", "mcp__solidcraft__save");
        assert!(always_allowed(&p, "claude", "mcp__solidcraft__save") && !always_allowed(&p, "codex", "mcp__solidcraft__save"));
        assert_eq!(p.agent_always_allow["claude"].len(), 1);
        p.agents.push(AgentConfig { id: "api-1".into(), ..Default::default() });
        assert_eq!(new_id(&p, "api"), "api-2");
        assert_eq!(normalize_url(" http://localhost:11434/v1/ ").as_deref(), Ok("http://localhost:11434/v1"));
        assert!(normalize_url("localhost:11434").is_err());
    }

    #[test]
    fn providers_send_keys_only_where_it_is_safe() {
        for p in PROVIDERS {
            assert!(normalize_url(p.base_url).is_ok(), "{}", p.name);
            assert!(!p.key || key_safe_for(p.base_url), "{} needs a key over HTTPS", p.name);
            assert!(p.keys_at.is_empty() || p.keys_at.starts_with("https://"), "{}", p.name);
        }
        // An agent saved before there were wires speaks chat completions.
        let old: AgentConfig =
            serde_json::from_str(r#"{"id": "api-1", "kind": "api", "base_url": "http://localhost:11434/v1", "model": "m"}"#).unwrap();
        assert_eq!(old.wire, Wire::OpenAi);
    }
}
