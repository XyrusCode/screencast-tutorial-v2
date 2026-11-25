//! Auth setup for agent-browser sessions.

use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::browser::{resolve_url, Browser};
use crate::config::AppConfig;

/// Prepare auth before the first real navigation.
///
/// For `localStorage-token`, returns the path of an init script that must be
/// passed to [`Browser::open`] so the token exists before the SPA boots.
pub fn prepare(browser: &Browser, config: &AppConfig, output_dir: &Path) -> Result<Option<PathBuf>> {
    match config.auth.mode.as_str() {
        "none" => Ok(None),
        "localStorage-token" => {
            let env_name = config
                .auth
                .token_env
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("auth.token_env is required for localStorage-token"))?;
            let token = std::env::var(env_name)
                .map_err(|_| anyhow::anyhow!("missing token environment variable {env_name}"))?;
            let key = config
                .auth
                .storage_key
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("auth.storage_key is required for localStorage-token"))?;

            fs::create_dir_all(output_dir)?;
            let script_path = output_dir.join(".screencast-init.js");
            fs::write(&script_path, init_script(key, &token))?;
            Ok(Some(script_path))
        }
        "ui-login" => {
            login_ui(browser, config)?;
            Ok(None)
        }
        other => bail!("unknown auth.mode `{other}` (use none, localStorage-token or ui-login)"),
    }
}

fn login_ui(browser: &Browser, config: &AppConfig) -> Result<()> {
    let login = config
        .auth
        .ui_login
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("auth.ui_login is required for ui-login"))?;

    browser.goto(&resolve_url(&config.base_url, &login.login_path))?;
    browser.wait_selector(&login.identifier_selector, "visible", 60_000)?;

    let identifier = std::env::var("LOGIN_ID").unwrap_or_default();
    let password = std::env::var("LOGIN_PASSWORD").unwrap_or_default();

    for (selector, value) in [
        (&login.identifier_selector, identifier),
        (&login.password_selector, password),
    ] {
        browser.run_owned(vec![
            "fill".into(),
            selector.to_string(),
            "--".into(),
            value,
        ])?;
    }
    browser.run_owned(vec!["click".into(), login.submit_selector.clone()])?;

    let path = serde_json::to_string(&config.login_path)?;
    let expression = format!("!location.pathname.includes({path})");
    browser.wait_fn(&expression, 120_000)?;
    browser.wait_ms(2000)?;
    Ok(())
}

pub fn init_script(key: &str, token: &str) -> String {
    let key = serde_json::to_string(key).unwrap_or_else(|_| "\"\"".into());
    let token = serde_json::to_string(token).unwrap_or_else(|_| "\"\"".into());
    format!(
        "(function () {{ try {{ localStorage.setItem({key}, {token}); }} catch (e) {{}} }})();"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_script_escapes_values() {
        let script = init_script("demo.token", "a\"b\\c");
        assert!(script.contains("localStorage.setItem(\"demo.token\""));
        assert!(script.contains("a\\\"b\\\\c"));
    }
}
