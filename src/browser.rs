//! Thin wrapper around the Vercel `agent-browser` CLI.
//!
//! Every command runs in one named session and uses array arguments only —
//! no shell interpolation. The session is closed on drop, even after errors.

use anyhow::{anyhow, bail, Result};
use base64::Engine;
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{Action, Target};

pub struct Browser {
    bin: String,
    session: String,
    headed: bool,
}

impl Browser {
    pub fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        Self {
            bin: std::env::var("AGENT_BROWSER_BIN").unwrap_or_else(|_| "agent-browser".into()),
            session: format!("screencast-{label}-{nonce}"),
            headed: std::env::var("AGENT_BROWSER_HEADED")
                .map(|value| value == "true" || value == "1")
                .unwrap_or(false),
        }
    }

    pub fn session(&self) -> &str {
        &self.session
    }

    fn prefix(&self) -> Vec<String> {
        let mut args = vec!["--session".into(), self.session.clone()];
        if self.headed {
            args.push("--headed".into());
        }
        args
    }

    pub fn run(&self, args: &[&str]) -> Result<String> {
        let mut command = Command::new(&self.bin);
        command.args(self.prefix()).args(args);
        let output = command
            .output()
            .map_err(|error| anyhow!("cannot run {}: {error}", self.bin))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            bail!("agent-browser {} failed: {}", args.join(" "), detail);
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn run_owned(&self, args: Vec<String>) -> Result<String> {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run(&refs)
    }

    pub fn run_json(&self, args: &[&str]) -> Result<Value> {
        let mut full = vec!["--json".to_string()];
        full.extend(args.iter().map(|value| value.to_string()));
        let raw = self.run_owned(full)?;
        serde_json::from_str(&raw).map_err(|error| {
            anyhow!(
                "invalid JSON from agent-browser {}: {error} (got {})",
                args.join(" "),
                &raw[..raw.len().min(200)]
            )
        })
    }

    pub fn open(&self, url: Option<&str>, init_script: Option<&Path>) -> Result<()> {
        let mut args = Vec::new();
        if let Some(script) = init_script {
            args.push("--init-script".into());
            args.push(script.to_string_lossy().into_owned());
        }
        args.push("open".into());
        if let Some(value) = url {
            args.push(value.to_string());
        }
        self.run_owned(args)?;
        Ok(())
    }

    pub fn goto(&self, url: &str) -> Result<()> {
        self.run_owned(vec!["goto".into(), url.to_string()])?;
        Ok(())
    }

    pub fn get_url(&self) -> Result<String> {
        self.run(&["get", "url"])
    }

    pub fn reload(&self) -> Result<()> {
        self.run(&["reload"])?;
        Ok(())
    }

    pub fn set_viewport(&self, width: u32, height: u32) -> Result<()> {
        self.run(&["set", "viewport", &width.to_string(), &height.to_string()])?;
        Ok(())
    }

    pub fn wait_ms(&self, ms: u64) -> Result<()> {
        self.run(&["wait", &ms.to_string()])?;
        Ok(())
    }

    pub fn wait_selector(&self, selector: &str, state: &str, timeout_ms: u64) -> Result<()> {
        self.run(&[
            "wait",
            selector,
            "--state",
            state,
            "--timeout",
            &timeout_ms.to_string(),
        ])?;
        Ok(())
    }

    pub fn wait_text(&self, text: &str, timeout_ms: u64) -> Result<()> {
        self.run(&[
            "wait",
            "--text",
            text,
            "--timeout",
            &timeout_ms.to_string(),
        ])?;
        Ok(())
    }

    pub fn wait_fn(&self, expression: &str, timeout_ms: u64) -> Result<()> {
        self.run(&[
            "wait",
            "--fn",
            expression,
            "--timeout",
            &timeout_ms.to_string(),
        ])?;
        Ok(())
    }

    pub fn storage_set(&self, key: &str, value: &str) -> Result<()> {
        self.run(&["storage", "local", "set", key, value])?;
        Ok(())
    }

    pub fn eval(&self, script: &str) -> Result<Value> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(script.as_bytes());
        let response = self.run_json(&["eval", "-b", &encoded])?;
        Ok(extract_result(&response))
    }

    pub fn run_action(&self, action: &Action, base_url: &str) -> Result<()> {
        let args = action_args(action, base_url)?;
        if args.is_empty() {
            return Ok(());
        }
        self.run_owned(args)?;
        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        self.run(&["close"])?;
        Ok(())
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.run(&["close"]);
    }
}

/// Unwrap the `--json` envelope: `{"success": true, "data": {"result": ...}}`.
pub fn extract_result(response: &Value) -> Value {
    response
        .get("data")
        .and_then(|data| data.get("result"))
        .cloned()
        .or_else(|| response.get("result").cloned())
        .unwrap_or_else(|| response.clone())
}

pub fn resolve_url(base_url: &str, url: &str) -> String {
    if url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("data:")
        || url.starts_with("about:")
    {
        return url.to_string();
    }
    format!("{}/{}", base_url.trim_end_matches('/'), url.trim_start_matches('/'))
}

/// Build `find`/direct-locator arguments for a semantic target.
pub fn find_command(target: &Target, operation: &str, value: Option<&str>) -> Vec<String> {
    let mut args = vec!["find".to_string()];

    match target {
        Target::Css(selector) => {
            args.push("first".into());
            args.push(selector.clone());
        }
        Target::Role { role, .. } => {
            args.push("role".into());
            args.push(role.clone());
        }
        Target::Text(text) | Target::TextExact(text) => {
            args.push("text".into());
            args.push(text.clone());
        }
        Target::Label(label) => {
            args.push("label".into());
            args.push(label.clone());
        }
        Target::Placeholder(placeholder) => {
            args.push("placeholder".into());
            args.push(placeholder.clone());
        }
        Target::Testid(testid) => {
            args.push("testid".into());
            args.push(testid.clone());
        }
        Target::Ref(reference) => {
            args.clear();
            args.push(operation.to_string());
            args.push(reference.clone());
            if let Some(value) = value {
                args.push("--".into());
                args.push(value.to_string());
            }
            return args;
        }
    }

    args.push(operation.to_string());
    if let Target::Role {
        name: Some(name), ..
    } = target
    {
        args.push("--name".into());
        args.push(name.clone());
    }
    if matches!(target, Target::TextExact(_)) {
        args.push("--exact".into());
    }
    if let Some(value) = value {
        args.push("--".into());
        args.push(value.to_string());
    }
    args
}

/// Translate one script action into an agent-browser command.
pub fn action_args(action: &Action, base_url: &str) -> Result<Vec<String>> {
    let with_target = |selector: &str,
                       target: &Option<Target>,
                       operation: &str,
                       value: Option<&str>|
     -> Vec<String> {
        if let Some(target) = target {
            find_command(target, operation, value)
        } else {
            let mut args = vec![operation.to_string(), selector.to_string()];
            if let Some(value) = value {
                args.push("--".into());
                args.push(value.to_string());
            }
            args
        }
    };

    Ok(match action {
        Action::Navigate { url } => vec!["goto".into(), resolve_url(base_url, url)],
        Action::Wait { ms } => vec!["wait".into(), ms.to_string()],
        Action::Press { key } => vec!["press".into(), key.clone()],
        Action::Note { .. } => Vec::new(),
        Action::Scroll { y, .. } => {
            let (direction, amount) = if *y >= 0 { ("down", *y) } else { ("up", -y) };
            vec!["scroll".into(), direction.into(), amount.to_string()]
        }
        Action::Click { selector, target } => with_target(selector, target, "click", None),
        Action::Fill {
            selector,
            value,
            target,
        } => with_target(selector, target, "fill", Some(value)),
        Action::Select {
            selector,
            value,
            target,
        } => with_target(selector, target, "select", Some(value)),
        Action::Hover { selector, target } => with_target(selector, target, "hover", None),
        Action::WaitForSelector {
            selector,
            target,
            timeout_ms,
        } => {
            let timeout = timeout_ms.unwrap_or(10_000);
            match target {
                Some(Target::Text(text)) => vec![
                    "wait".into(),
                    "--text".into(),
                    text.clone(),
                    "--timeout".into(),
                    timeout.to_string(),
                ],
                Some(Target::TextExact(text)) => vec![
                    "wait".into(),
                    "--text".into(),
                    text.clone(),
                    "--timeout".into(),
                    timeout.to_string(),
                ],
                _ => vec![
                    "wait".into(),
                    selector.clone(),
                    "--state".into(),
                    "visible".into(),
                    "--timeout".into(),
                    timeout.to_string(),
                ],
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_urls() {
        assert_eq!(resolve_url("http://localhost:3000/", "/dashboard"), "http://localhost:3000/dashboard");
        assert_eq!(resolve_url("http://localhost:3000", "x"), "http://localhost:3000/x");
        assert_eq!(resolve_url("http://x", "https://y/z"), "https://y/z");
    }

    #[test]
    fn maps_css_and_semantic_targets() {
        let click = Action::Click {
            selector: "#save".into(),
            target: None,
        };
        assert_eq!(action_args(&click, "http://x").unwrap(), vec!["click", "#save"]);

        let click = Action::Click {
            selector: "#save".into(),
            target: Some(Target::Role {
                role: "button".into(),
                name: Some("Save".into()),
            }),
        };
        assert_eq!(
            action_args(&click, "http://x").unwrap(),
            vec!["find", "role", "button", "click", "--name", "Save"]
        );
    }

    #[test]
    fn fill_uses_literal_value_marker() {
        let fill = Action::Fill {
            selector: "#email".into(),
            value: "--not-a-flag".into(),
            target: Some(Target::Ref("@e3".into())),
        };
        assert_eq!(
            action_args(&fill, "http://x").unwrap(),
            vec!["fill", "@e3", "--", "--not-a-flag"]
        );
    }

    #[test]
    fn unwraps_json_envelope() {
        let response: Value = serde_json::from_str(r#"{"success":true,"data":{"result":{"ok":true}}}"#).unwrap();
        assert_eq!(extract_result(&response), serde_json::json!({"ok": true}));
    }
}
