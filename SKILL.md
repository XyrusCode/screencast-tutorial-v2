---
name: screencast-tutorial-v2
description: "Record narrated screen-walkthrough tutorials of any web app using the v2 Rust CLI and Vercel agent-browser (not Playwright, not the inference.sh belt tool). Use when asked to make tutorial videos, product walkthroughs, onboarding demos, feature explainers, or 'a video showing how to use X' with the agent-browser-based workflow. Free neural narration via edge-tts, one continuous browser take via agent-browser, FFmpeg cut/mux, and v1-style audio-first synchronization. Also use to fix an existing v2 batch: narration ahead of the UI, slow-motion footage, desync, black segments, or stale selectors."
---

# Screencast Tutorial v2

**v2 is a separate skill.** It uses a Rust CLI driving the **Vercel
`agent-browser`** CLI. If the user asks for the original TypeScript/Playwright
workflow, use `screencast-tutorial` (v1) instead. Never mix the two.

> Release status: `0.1.0` ships the CLI scaffold, config/script parsing and
> docs. `inspect` and `validate` are available in this repo; `audio`, `record`,
> `merge` and `all` land in follow-up PRs. If a stage reports "not implemented",
> say so plainly rather than improvising a different toolchain.

## Prerequisites

- `ffmpeg` and `ffprobe` on PATH (with `libvpx` and `libx264` encoders).
- Vercel agent-browser: `npm install -g agent-browser && agent-browser install`.
  Verify with `agent-browser doctor`. Every browser command runs in a named
  session that is closed on exit.
- `edge-tts` via `uv tool install edge-tts` (free neural voices). Narration is
  generated **before** recording.
- `screencast-v2` release binary on PATH (no Rust toolchain needed on user
  machines).

## Setup

```bash
mkdir -p my-tutorial && cd my-tutorial
screencast-v2 init                 # writes config/app.config.json + config/script.example.json
agent-browser doctor
```

Edit `config/app.config.json`: `base_url`, `login_path`, `voice`, `output_dir`,
viewport, shell root/spinner/ready selectors and auth mode. Secrets can be
written as `${ENV:NAME}` — they are expanded from the environment at load
time, never stored in the repo.

## Workflow

1. **Inspect** — `screencast-v2 inspect --routes "/,/students" [--discover]`
   walks the configured routes with agent-browser and writes
   `output/ui-inspection.json` (override with `--out`). Author selectors only
   from what you actually observed; no guessed CSS.
2. **Author** — write `config/<script>.json` (see
   [script-schema.md](references/script-schema.md)). Every visible step gets
   narration; every interactive step gets an observable selector or semantic
   `target`. Prefer `target` for flaky text-only selectors.
3. **Validate** — `screencast-v2 validate --script config/<script>.json`
   replays gates on the live app. All gates must pass before recording.
4. **Audio** — `screencast-v2 audio --script ...` renders mp3s with edge-tts,
   caches them, and measures each duration with ffprobe.
5. **Record** — `screencast-v2 record --script ...` warms up the app, starts
   `agent-browser record start output/raw/<script>.webm --cursor`, then walks
   the script. It writes `output/raw/<script>-timings.json`.
6. **Merge** — `screencast-v2 merge --script ...` cuts the continuous take per
   timing entry, delays narration to its start, freeze-pads the video tail,
   loudnorms audio and writes `output/final/<segment>.mp4`.
7. **Review** — watch the output at 1x. Check narration triggers after the
   screen settles, clicks are visible, and no segment is black.

`all` runs stages 4–6 in one command.

## Hard rules

- Audio first: narration duration sizes each on-screen hold, never the reverse.
- Never time-stretch video or audio to force sync. Fix the script or gates.
- Assert success: shell root attached, spinners gone, ready gate visible (or
  stable text fallback) before narration starts.
- One continuous take; `record start` happens after warm-up, and all timings
  are measured from that moment.
- Browser interaction is via `agent-browser` commands in one named session,
  never Playwright and never the `belt` app.
- No local builds or tests of the Rust crate. CI is the only authority.

See [architecture.md](references/architecture.md) for the sync model and
[troubleshooting.md](references/troubleshooting.md) for common failures.
