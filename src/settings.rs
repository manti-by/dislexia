//! Settings schema, defaults and persistence (`wiki/pages/2026-10-26-05-settings.md`).

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The default system prompt: the text-fixer persona, embedded at compile
/// time (`plan 05 §1.1`). Product copy — change deliberately.
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
    /// Where the window was last left (`plan 03 §6`); `None` on first run.
    pub window_frame: Option<WindowFrame>,
}

/// The main window's last position and size, in screen points, plus the
/// display it was on — the uuid survives restarts, so a window parked on a
/// second monitor reopens there instead of on the primary display.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WindowFrame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// `CGDisplayCreateUUIDFromDisplayID`, as a string; `None` on first save.
    pub display: Option<Uuid>,
}

/// Model parameters; `None` fields are not sent to Ollama (`plan 05 §2`).
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
            // editing, not creative writing (`plan 05 §1`).
            options: ModelOptions {
                temperature: Some(0.2),
                ..ModelOptions::default()
            },
            launch_at_login: false,
            window_frame: None,
        }
    }
}

/// The app data directory: `~/Library/Application Support/dislexia/`
/// (`plan 02 §6`). `HOME` is read directly — the app is macOS-only and
/// never sandboxed.
pub fn data_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("HOME is set on macOS");
    home.join("Library")
        .join("Application Support")
        .join("dislexia")
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

/// Loads settings; a missing file means defaults, a corrupt one means
/// defaults + a warning (`plan 05 §4`) — never a crash on launch.
pub fn load() -> Settings {
    load_from(&settings_path())
}

pub fn load_from(path: &std::path::Path) -> Settings {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|error| {
            log::warn!("settings.json is unreadable ({}), using defaults", error);
            Settings::default()
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Settings::default(),
        Err(error) => {
            log::warn!("cannot read settings.json ({}), using defaults", error);
            Settings::default()
        }
    }
}

/// Persists settings atomically: write `settings.json.tmp`, then rename
/// over the target (`plan 05 §4`) — never a partially-written file.
pub fn save(settings: &Settings) -> Result<()> {
    save_to(&settings_path(), settings)
}

pub fn save_to(path: &std::path::Path, settings: &Settings) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(settings)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Normalizes a server URL on save (`plan 05 §3`): trim, strip trailing
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
        // Nothing to restore on a first run (`plan 03 §6`).
        assert_eq!(settings.window_frame, None);
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
    fn window_frame_round_trips_and_is_optional() {
        let frame = WindowFrame {
            x: 40.,
            y: 60.,
            width: 900.,
            height: 700.,
            display: Some(Uuid::nil()),
        };
        let settings = Settings {
            window_frame: Some(frame),
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.window_frame, Some(frame));

        // A settings.json written before the frame was persisted loads clean.
        let legacy: Settings = serde_json::from_str(r#"{"model": "llama3"}"#).unwrap();
        assert_eq!(legacy.window_frame, None);
        assert_eq!(legacy.model.as_deref(), Some("llama3"));
    }

    #[test]
    fn save_and_load_round_trip_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let settings = Settings {
            model: Some("llama3:8b".to_owned()),
            options: crate::settings::ModelOptions {
                temperature: Some(0.7),
                top_k: Some(40),
                ..crate::settings::ModelOptions::default()
            },
            ..Settings::default()
        };
        save_to(&path, &settings).unwrap();
        assert_eq!(load_from(&path), settings);
        // No temp file left behind.
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn missing_file_means_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load_from(&dir.path().join("settings.json")),
            Settings::default()
        );
    }

    #[test]
    fn corrupt_file_means_defaults_not_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(load_from(&path), Settings::default());
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
