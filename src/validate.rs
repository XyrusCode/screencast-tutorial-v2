//! Replay gates and actions on the live app without recording.

use anyhow::{bail, Result};
use std::path::Path;

use crate::auth;
use crate::browser::Browser;
use crate::config::AppConfig;
use crate::types::{load_script, Segment, Target};

pub fn run(config: &AppConfig, script_path: &Path) -> Result<()> {
    let script = load_script(script_path)?;
    let browser = Browser::new("validate");
    browser.set_viewport(config.viewport.width, config.viewport.height)?;

    let init_script = auth::prepare(&browser, config, Path::new(&config.output_dir))?;
    browser.open(Some(&config.base_url), init_script.as_deref())?;

    let mut failures = Vec::new();
    if let Err(error) = settle(&browser, config, None) {
        failures.push(format!("shell: {error}"));
    }

    for segment in &script.segments {
        for action in segment.pre_actions.iter().chain(segment.actions.iter()) {
            if let Err(error) = browser.run_action(action, &config.base_url) {
                failures.push(format!("{} action {action:?}: {error}", segment.id));
            }
        }

        if let Err(error) = settle(&browser, config, Some(segment)) {
            failures.push(format!("{} gate: {error}", segment.id));
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

/// Assert the shell is up, spinners are gone and the requested ready gate is
/// visible. Used by both `validate` and `record` so a take cannot start on an
/// unsettled screen.
pub fn settle(browser: &Browser, config: &AppConfig, segment: Option<&Segment>) -> Result<()> {
    let mut failures = Vec::new();

    if let Err(error) = browser.wait_selector(&config.shell.root_selector, "attached", 15_000) {
        failures.push(format!("shell root `{}`: {error}", config.shell.root_selector));
    }
    for spinner in &config.shell.spinner_selectors {
        if let Err(error) = browser.wait_selector(spinner, "hidden", 15_000) {
            failures.push(format!("spinner `{spinner}`: {error}"));
        }
    }

    let segment_ready = segment.and_then(|value| value.ready_selector.as_deref());
    let config_ready = config.shell.ready_selector.as_deref();
    if let Some(selector) = segment_ready.or(config_ready) {
        if let Err(error) = browser.wait_selector(selector, "visible", 10_000) {
            failures.push(format!("ready selector `{selector}`: {error}"));
        }
    }

    if let Some(target) = segment.and_then(|value| value.ready_target.as_ref()) {
        match target {
            Target::Text(text) | Target::TextExact(text) => {
                if let Err(error) = browser.wait_text(text, 10_000) {
                    failures.push(format!("ready text `{text}`: {error}"));
                }
            }
            target => {
                let selector = target_label(target);
                if let Err(error) = browser.wait_selector(&selector, "visible", 10_000) {
                    failures.push(format!("ready target: {error}"));
                }
            }
        }
    }

    // Fallback used by v1 when a segment has no explicit ready gate: wait for
    // enough stable shell text so hydration has actually finished.
    if segment_ready.is_none()
        && segment.and_then(|value| value.ready_target.as_ref()).is_none()
        && config_ready.is_none()
    {
        let min = config.shell.min_text_length;
        let expression = format!("document.body && document.body.innerText.trim().length >= {min}");
        if let Err(error) = browser.wait_fn(&expression, 10_000) {
            failures.push(format!("shell text >= {min} chars: {error}"));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        bail!("{}", failures.join("; "))
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
