# Script schema

A script is JSON with `id`, optional `title` and a `segments` array. See
`assets/script.example.json` for a complete example.

## Segment

| Field | Meaning |
|---|---|
| `id` | used for audio, raw video and final MP4 filenames |
| `narration` | exact text spoken by edge-tts |
| `pre_actions` | actions run before the visible segment begins (warm navigation, scroll) |
| `actions` | on-camera interactions |
| `ready_selector` / `ready_target` | gate asserted before narration starts |
| `hold_ms` | optional extra hold added after narration |

## Actions

`navigate` (`url`), `click`, `fill` (`value`), `select` (`value`), `wait`
(`ms`), `scroll` (`y`, optional `selector`), `hover`, `press` (`key`),
`wait_for_selector` (`timeout_ms`) and `note` (`text`).

## Hybrid targeting

Every interactive action keeps a CSS `selector` (the v1 convention) and may
add a `target`. When both exist, `target` wins. Supported forms:

```json
{ "type": "click", "selector": "#save", "target": { "role": { "role": "button", "name": "Save" } } }
{ "type": "click", "selector": ".cta",  "target": { "text": "Sign in" } }
{ "type": "click", "selector": ".cta",  "target": { "text_exact": "Sign in" } }
{ "type": "fill",  "selector": "#email","target": { "label": "Email" }, "value": "a@b.c" }
{ "type": "click", "selector": ".save", "target": { "testid": "save-project" } }
{ "type": "click", "selector": ".go",  "target": { "placeholder": "Search" } }
{ "type": "click", "selector": "button", "target": { "ref": "@e12" } }
```

`ready_target` on a segment uses the same forms.

## Converting Playwright-only selectors

`:has-text()` and `>>` chains are Playwright syntax and have no agent-browser
equivalent. Replace them with a semantic target:

- `button:has-text("Save")` → `{"role":{"role":"button","name":"Save"}}`
- `div.card >> text=Details` → `{"text":"Details"}`

Prefer observed `data-testid` values over text when available. Never invent a
selector that was not seen in `output/ui-inspection.json`.

## Configuration

`config/app.config.json` fields: `name`, `base_url`, `login_path`, `voice`,
`output_dir`, `viewport` (`width`, `height`), `shell` (`root_selector`,
`spinner_selectors`, `ready_selector`, `min_text_length`) and `auth`
(`mode`: `none` / `localStorage-token` / `ui-login`, plus `token_env`,
`storage_key`, `token_url` as needed). String values support `${ENV:NAME}`.
