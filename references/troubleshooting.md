# Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Old binary reports a missing stage | v0.1.0 release is installed | re-download the matching `v0.2.0` asset from `/releases/latest/download/` |
| `agent-browser: command not found` | package not on PATH or not installed | `npm install -g agent-browser && agent-browser install`, verify `agent-browser doctor` |
| Recording fails before any frame | missing ffmpeg encoders | install ffmpeg with `libvpx` and `libx264`; re-run `agent-browser doctor` |
| Narration finishes before the UI moves | hold sized from a guessed duration | re-run `audio` so ffprobe measures the real mp3; never time-stretch |
| Narration starts too early | gate passed before hydration | add `ready_selector`/`ready_target`, or raise `shell.min_text_length` |
| Selector stops matching | text/CSS changed | use the observed `data-testid` or a semantic `target` (role/text/label) |
| `:has-text()` or `>>` fails | Playwright-only syntax | convert to `target` (see script-schema.md) |
| Black or frozen tail | clip ends before audio | increase `tail` padding / `hold_ms`; keep `tpad=stop_mode=clone` |
| Session left open after failure | aborted command | the CLI wrapper closes its session on exit; check for stale `screencast-*` sessions |
| `${ENV:NAME}` error | missing environment variable | export the variable or remove the placeholder from config |

## Debugging a failing take

Because every browser command uses one named session, you can attach a second
terminal to that same session while the pipeline is paused and run
`agent-browser snapshot`, `console` or `errors` directly. This is the main
ergonomic advantage over v1's in-process Playwright.
