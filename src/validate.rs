//! Replay gates and actions on the live app without recording.

use anyhow::{bail, Result};
use std::path::Path;

use crate::auth;
use crate::browser::Browser;
use crate::config::AppConfig;
use crate::types::{load_script, Target};

pub fn run(config: &AppConfig, script_path: &Path) -> Result<()> {
    let script = load_script(script_path)?;
    let browser = Browser::new("validate");
    browser.set_viewport(config.viewport.width, config.viewport.height)?;

    let init_script = auth::prepare(&browser, config, Path::new(&config.output_dir))?;
    browser.open(Some(&config.base_url), init_script.as_deref())?;

    let mut failures = Vec::new();
    shell_gate(&browser, config, &mut failures);

    for segment in &script.segments {
        for action in segment.pre_actions.iter().chain(segment.actions.iter()) {
            if let Err(error) = browser.run_action(action, &config.base_url) {
                failures.push(format!("{} action {action:?}: {error}", segment.id));
            }
        }

        if let Some(selector) = segment
            .ready_selector
            .as_deref()
            .or(config.shell.ready_selector.as_deref())
        {
            if let Err(error) = browser.wait_selector(selector, "visible", 10_000) {
                failures.push(format!("{} ready selector `{selector}`: {error}", segment.id));
            }
        }

        match &segment.ready_target {
            Some(Target::Text(text)) | Some(Target::TextExact(text)) => {
                if let Err(error) = browser.wait_text(text, 10_000) {
                    failures.push(format!("{} ready text `{text}`: {error}", segment.id));
                }
            }
            Some(target) => {
                if let Err(error) = browser.wait_selector(&target_label(target), "visible", 10_000) {
                    failures.push(format!("{} ready target: {error}", segment.id));
                }
            }
            None => {}
        }
    }

    if !failures.is_empty() {
        bail!(
            "{} gate(s) failed:\n- {}",
            failures.len(),
            failures.join("\n- ")
        );
    }

    println!("[validate] all gates passed for {}", script.id);
    Ok(())
}

fn shell_gate(browser: &Browser, config: &AppConfig, failures: &mut Vec<String>) {
    if let Err(error) = browser.wait_selector(&config.shell.root_selector, "attached", 15_000) {
        failures.push(format!("shell root `{}`: {error}", config.shell.root_selector));
    }
    for spinner in &config.shell.spinner_selectors {
        if let Err(error) = browser.wait_selector(spinner, "hidden", 15_000) {
            failures.push(format!("spinner `{spinner}`: {error}"));
        }
    }
}

/// Semantic targets without a direct wait form fall back to the CSS selector
/// supplied alongside them; scripts should always provide one.
fn target_label(target: &Target) -> String {
    match target {
        Target::Css(selector) => selector.clone(),
        Target::Ref(reference) => reference.clone(),
        Target::Testid(testid) => format!("[data-testid=\"{testid}\"]"),
        Target::Label(label) => format!("[aria-label=\"{label}\"]"),
        Target::Placeholder(placeholder) => format!("[placeholder=\"{placeholder}\"]"),
        Target::Role { role, .. } => format!("[role=\"{role}\"]"),
        Target::Text(text) | Target::TextExact(text) => format!("text={text}"),
    }
}
