//! Settings schema and defaults (`plan/05-settings.md`). Load/save with
//! atomic writes lands in M3; the schema is here already because the request
//! builder and the app wiring consume it.

use serde::{Deserialize, Serialize};

/// The default system prompt: the text-fixer persona, embedded at compile
/// time (`plan/05 §1.1`). Product copy — change deliberately.
pub const DEFAULT_SYSTEM_PROMPT: &str = include_str!("../prompts/default-system-prompt.md");

pub const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:11434";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub server_url: String,
    pub model: Option<String>,
    pub system_prompt: String,
    pub options: ModelOptions,
    pub launch_at_login: bool,
}

/// Model parameters; `None` fields are not sent to Ollama (`plan/05 §2`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModelOptions {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub num_predict: Option<i32>,
    pub num_ctx: Option<u32>,
    pub repeat_penalty: Option<f64>,
    pub seed: Option<i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server_url: DEFAULT_SERVER_URL.to_owned(),
            model: None,
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_owned(),
            // Low on purpose: the default persona is deterministic text
            // editing, not creative writing (`plan/05 §1`).
            options: ModelOptions {
                temperature: Some(0.2),
                ..ModelOptions::default()
            },
            launch_at_login: false,
        }
    }
}

/// Normalizes a server URL on save (`plan/05 §3`): trim, strip trailing
/// slashes, prefix `http://` when the scheme is missing. Only http(s) hosts
/// are accepted; anything else returns `Err` with the field message.
pub fn normalize_server_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Server URL is empty".to_owned());
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("http://{trimmed}")
    };
    let url = with_scheme.trim_end_matches('/').to_owned();

    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| format!("Not a valid URL: {url}"))?;
    if scheme != "http" && scheme != "https" {
        return Err(format!("Unsupported scheme: {scheme}"));
    }
    let host = rest.split(['/']).next().unwrap_or_default();
    let looks_like_host = host.contains('.') || host.contains(':') || host == "localhost";
    if host.is_empty() || host.contains(char::is_whitespace) || !looks_like_host {
        return Err(format!("Not a valid URL: {url}"));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_plan() {
        let settings = Settings::default();
        assert_eq!(settings.server_url, "http://127.0.0.1:11434");
        assert_eq!(settings.model, None);
        assert_eq!(settings.system_prompt, DEFAULT_SYSTEM_PROMPT);
        assert_eq!(settings.options.temperature, Some(0.2));
        assert!(!settings.launch_at_login);
    }

    #[test]
    fn default_prompt_is_the_embedded_text_fixer() {
        assert!(DEFAULT_SYSTEM_PROMPT.contains("writing editor"));
        assert!(DEFAULT_SYSTEM_PROMPT.contains("Output only the edited text"));
    }

    #[test]
    fn settings_serde_round_trip() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, settings);
    }

    #[test]
    fn unknown_fields_are_ignored_on_load() {
        let json = r#"{"server_url": "http://x", "future_field": 1}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.server_url, "http://x");
        // Everything else falls back to defaults.
        assert_eq!(
            settings,
            Settings {
                server_url: "http://x".to_owned(),
                ..Settings::default()
            }
        );
    }

    #[test]
    fn server_url_normalization() {
        assert_eq!(
            normalize_server_url("  127.0.0.1:11434/ ").unwrap(),
            "http://127.0.0.1:11434"
        );
        assert_eq!(
            normalize_server_url("http://localhost:11434///").unwrap(),
            "http://localhost:11434"
        );
        assert!(normalize_server_url("").is_err());
        assert!(normalize_server_url("ftp://x").is_err());
        assert!(normalize_server_url("ht!tp://x").is_err());
    }
}
