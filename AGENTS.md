# Screencast Tutorial v2 — Repository Instructions

## Build & verify via CI, not locally

- **We do not run builds or tests locally.** No `cargo build`, `cargo check`,
  `cargo test`, `cargo run`, or `cargo fmt` for verification. The local machine
  intentionally has no Rust toolchain; GitHub Actions is the source of truth.
- After making changes: commit to a feature branch, push, open a PR, then watch
  CI with `gh pr checks --watch`.
- **When CI fails:** use the **rinse-and-repeat** skill (`/rinse-and-repeat`):
  it reads the failed job log, fixes the root cause, pushes, and polls until green.
- CI (`.github/workflows/build-check.yml`) runs `cargo check --workspace`,
  `cargo test --workspace` and a CLI smoke job on `windows-latest`.
- Once CI is green, merge manually with a regular merge commit via the
  `gh pr merge` flow. No squash, no auto-merge. Never commit directly to `main`.

## Releases

- Releases are version-driven from the first `version = ` line in `Cargo.toml`
  and handled entirely by `.github/workflows/release.yml` on a push to `main`
  that touches `Cargo.toml`.
- A version number is single-use: bump it for every release; never reuse or
  lower it. Release notes come from the matching `## [<version>]` heading in
  `CHANGELOG.md`.
- Release asset names are deliberately **version-free**
  (`screencast-v2_linux-x86_64`, `screencast-v2_macos-aarch64`, …) so the
  `/releases/latest/download/...` URLs in the ai-sync README install prompt
  never need editing. Do not rename them.
- The binary is built on CI for Linux x86_64, Windows x86_64, macOS x86_64 and
  macOS arm64. There is no local packaging step.

## Architecture

- This is a single Rust crate: `src/main.rs` (CLI) plus `src/lib.rs` exposing
  `config` and `types`. Keep dependencies lean; no async runtime.
- All browser interaction goes through the **Vercel `agent-browser` CLI**
  (`agent-browser`, never the inference.sh `belt`-based tool). Every command
  runs in a named session that is always closed on exit.
- Narration uses `edge-tts` as a subprocess through `uv tool`; FFmpeg/FFprobe
  handle durations and final mux. Rust owns orchestration and timing only.
- v1 (`screencast-tutorial`, TypeScript/Playwright) is a separate installed
  skill and must remain byte-for-byte untouched.
- Preserve the v1 invariants: audio before video, settled screen before
  narration, no time-stretching, assert every gate, and only use selectors
  actually observed on the target app. See `references/architecture.md`.
