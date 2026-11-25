use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Replace every `${ENV:NAME}` reference in `input` using the process environment.
pub fn expand_env(input: &str) -> Result<String> {
    expand_env_with(input, |name| {
        std::env::var(name).map_err(|_| anyhow!("missing environment variable {name}"))
    })
}

/// Testable core of [`expand_env`]: the caller supplies the lookup function.
pub fn expand_env_with<F>(input: &str, lookup: F) -> Result<String>
where
    F: Fn(&str) -> Result<String>,
{
    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(start) = rest.find("${ENV:") {
        out.push_str(&rest[..start]);
        let tail = &rest[start + "${ENV:".len()..];
        let end = tail
            .find('}')
            .ok_or_else(|| anyhow!("unterminated ${{ENV:...}} reference in {input:?}"))?;
        let name = &tail[..end];
        if name.is_empty() {
            return Err(anyhow!("empty ${{ENV:...}} reference in {input:?}"));
        }
        out.push_str(&lookup(name)?);
        rest = &tail[end + 1..];
    }

    out.push_str(rest);
    Ok(out)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub name: String,
    pub base_url: String,
    pub login_path: String,
    pub voice: String,
    pub output_dir: String,
    pub viewport: Viewport,
    pub shell: ShellConfig,
    pub auth: AuthConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            name: "app".into(),
            base_url: "http://localhost:3000".into(),
            login_path: "/login".into(),
            voice: "en-US-AriaNeural".into(),
            output_dir: "output".into(),
            viewport: Viewport::default(),
            shell: ShellConfig::default(),
            auth: AuthConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1440,
            height: 900,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    pub root_selector: String,
    pub spinner_selectors: Vec<String>,
    pub ready_selector: Option<String>,
    pub min_text_length: usize,
    pub nav_expand_selectors: Vec<String>,
    pub nav_link_selector: Option<String>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            root_selector: "[data-app-root]".into(),
            spinner_selectors: vec!["[data-testid='spinner']".into(), ".loading".into()],
            ready_selector: None,
            min_text_length: 120,
            nav_expand_selectors: Vec::new(),
            nav_link_selector: Some("nav a[href], aside a[href]".into()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub mode: String,
    pub token_env: Option<String>,
    pub storage_key: Option<String>,
    pub token_url: Option<String>,
    pub ui_login: Option<UiLogin>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            mode: "none".into(),
            token_env: None,
            storage_key: None,
            token_url: None,
            ui_login: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UiLogin {
    pub login_path: String,
    pub identifier_selector: String,
    pub password_selector: String,
    pub submit_selector: String,
}

impl Default for UiLogin {
    fn default() -> Self {
        Self {
            login_path: "/login".into(),
            identifier_selector: "input[name='email'], #email".into(),
            password_selector: "input[type='password']".into(),
            submit_selector: "button[type='submit']".into(),
        }
    }
}

pub fn load_config(path: &Path) -> Result<AppConfig> {
    let raw = fs::read_to_string(path)
        .map_err(|e| anyhow!("cannot read config {}: {e}", path.display()))?;
    let expanded = expand_env(&raw)?;
    serde_json::from_str(&expanded)
        .map_err(|e| anyhow!("invalid config {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_env_references() {
        let out = expand_env_with("Bearer ${ENV:TOKEN}!", |name| {
            assert_eq!(name, "TOKEN");
            Ok("abc123".into())
        })
        .unwrap();
        assert_eq!(out, "Bearer abc123!");
    }

    #[test]
    fn rejects_unterminated_reference() {
        assert!(expand_env_with("${ENV:BROKEN", |_| Ok("x".into())).is_err());
    }

    #[test]
    fn parses_fixture_config() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/config.json");
        let config = load_config(&path).unwrap();
        assert_eq!(config.name, "Demo App");
        assert_eq!(config.viewport.width, 1280);
        assert_eq!(config.auth.mode, "localStorage-token");
    }
}
