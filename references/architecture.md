# Architecture and synchronization model

## Stages

`inspect → author → validate → audio → record → merge → review`

Rust owns orchestration and timing. External processes:

| Tool | Role |
|---|---|
| `agent-browser` | named browser session, gates, actions, continuous video |
| `edge-tts` | narration mp3 generation (via `uv tool`) |
| `ffprobe` | narration and video durations |
| `ffmpeg` | cut, pad, audio delay, loudnorm, concat, final encode |

## Continuous take

The browser is opened, authenticated, warmed up and settled **before**
recording. Then a single `agent-browser record start output/raw/<id>.webm
--cursor` begins the take. `startEpoch` is captured immediately after that
command returns.

For each segment the recorder emits:

```json
{ "segment_id": "...", "start_ms": 0, "narration_start_ms": 250, "end_ms": 1800 }
```

- `start_ms`: when this segment's visible actions begin, relative to record start.
- `narration_start_ms`: after pre-roll, actions and settle gates complete. The
  narration mp3 is delayed by this amount during merge.
- `end_ms`: `narration_start_ms + audio_duration + tail`, so the screen holds
  long enough to hear the sentence finish.

Timings are monotonic; `tests/fixtures/timings.json` asserts the invariant.

## Why audio-first

Narration duration is measured before the take. The hold is derived from it,
so the UI waits for the voice — never the voice for the UI. This is the v1
invariant that removed desync; keep it.

## Merge math

`leadMs = min(narrationStartMs - startMs, MAX_LEAD_MS)` (2500 ms cap),
tail padding 1200 ms with `tpad=stop_mode=clone`, `adelay=leadMs`,
`loudnorm` on the audio, and re-encode (never `-c:v copy`) so cuts stay
frame-accurate.

## agent-browser specifics

- Every command uses `--session <unique-name>`; a `Drop`/cleanup step closes
  the session even on failure.
- `record start` captures the active page as-is; warm-up navigation must
  happen first so hydration is not re-run on camera.
- CSS selectors go straight to `wait`/`click`/`fill`; semantic `target` values
  use `find` (role, text, label, placeholder, testid) or a direct `@eN` ref.
- Each command is a separate process, so cut points are accurate to roughly
  one CLI call (~50–150 ms). Freeze-padding and the lead cap absorb that.
  Do not add time-stretching to compensate.

## Rollout

0.1.0: CLI scaffold, config/script/timing types, fixtures, CI and docs.
Then: `inspect` and `validate` (browser adapter, auth setup, live gates), then
`audio` (cached edge-tts clips + ffprobe manifest).
Then: `record` (warm-up, continuous take, monotonic timing markers).
Next PRs: `merge`, `all`, each with tests and the same PR → CI → merge loop.
