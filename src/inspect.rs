//! Dump observed selectors and page data into v1-compatible JSON.

use anyhow::Result;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

use crate::auth;
use crate::browser::{resolve_url, Browser};
use crate::config::AppConfig;

const DUMP_JS: &str = r#"
(() => {
  const pick = (selector, fn, limit) =>
    Array.from(document.querySelectorAll(selector)).slice(0, limit).map(fn);
  const text = el => (el.innerText || '').trim();
  const cls = el => String(el.className || '').slice(0, 60);
  const attr = (el, name) => el.getAttribute(name) || undefined;
  return {
    url: location.href,
    title: document.title,
    headings: pick('h1, h2, h3, h4', el => ({ tag: el.tagName, text: text(el).slice(0, 120) }), 100),
    buttons: pick('button, [role="button"], a[role="button"]', el => ({
      tag: el.tagName, text: text(el).slice(0, 80), id: el.id || undefined,
      testid: attr(el, 'data-testid'), ariaLabel: attr(el, 'aria-label'),
      type: attr(el, 'type'), cls: cls(el)
    }), 40),
    inputs: pick('input, select, textarea', el => ({
      tag: el.tagName, type: attr(el, 'type'), name: attr(el, 'name'),
      placeholder: attr(el, 'placeholder'), id: el.id || undefined,
      testid: attr(el, 'data-testid'), ariaLabel: attr(el, 'aria-label')
    }), 100),
    links: pick('a[href]', el => ({ text: text(el).slice(0, 60), href: attr(el, 'href') }), 60),
    tabs: pick('[role="tab"], [class*="tab" i][role], nav a', el => ({
      text: text(el).slice(0, 60), testid: attr(el, 'data-testid')
    }), 30),
    comboboxes: pick('[role="combobox"], select, [class*="select" i]', el => ({
      cls: cls(el), ariaLabel: attr(el, 'aria-label'), testid: attr(el, 'data-testid')
    }), 30),
    dialogs: pick('[role="dialog"], [class*="modal" i], [class*="drawer" i]', el => ({
      cls: cls(el), ariaLabel: attr(el, 'aria-label')
    }), 10),
    emptyStates: pick('[class*="empty" i]', el => ({ text: text(el).slice(0, 120) }), 10),
    textSample: (document.body ? document.body.innerText : '').replace(/\s+/g, ' ').slice(0, 500)
  };
})()
"#;

pub fn run(config: &AppConfig, routes: &str, out: &Path, discover: bool) -> Result<()> {
    let browser = Browser::new("inspect");
    browser.set_viewport(config.viewport.width, config.viewport.height)?;

    let init_script = auth::prepare(&browser, config, Path::new(&config.output_dir))?;
    browser.open(Some(&config.base_url), init_script.as_deref())?;

    let mut all = Map::new();

    if discover {
        for selector in &config.shell.nav_expand_selectors {
            let quoted = json!(selector);
            let count = browser.eval(&format!(
                "document.querySelectorAll({quoted}).length"
            ))?;
            let count = count.as_u64().unwrap_or(0).min(20);
            for index in 0..count {
                let _ = browser.eval(&format!(
                    "document.querySelectorAll({quoted})[{index}].click()"
                ));
                browser.wait_ms(400)?;
            }
        }

        let nav_selector = config
            .shell
            .nav_link_selector
            .as_deref()
            .unwrap_or("nav a[href], aside a[href]");
        let quoted = json!(nav_selector);
        let script = format!(
            "Array.from(document.querySelectorAll({quoted})).map(el => ({{ text: (el.innerText || '').trim(), href: el.getAttribute('href') }}))"
        );
        all.insert("__nav".into(), browser.eval(&script)?);
    }

    for route in routes.split(',').map(str::trim).filter(|value| !value.is_empty()) {
        let full_url = resolve_url(&config.base_url, route);
        let name = if route == "/" {
            "root".to_string()
        } else {
            route
                .trim_start_matches('/')
                .replace(|c: char| matches!(c, '/' | '?' | '&' | '='), "-")
        };

        println!("[inspect] {route}");
        browser.goto(&full_url)?;
        browser.wait_ms(2500)?;

        let value = match browser.eval(DUMP_JS) {
            Ok(value) => value,
            Err(error) => {
                let first = error.to_string().lines().next().unwrap_or("eval failed").to_string();
                json!({ "error": first })
            }
        };
        all.insert(name, value);
    }

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, serde_json::to_string_pretty(&Value::Object(all))?)?;
    println!("[inspect] wrote {}", out.display());
    Ok(())
}
