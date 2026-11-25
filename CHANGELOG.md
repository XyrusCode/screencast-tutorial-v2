# Changelog

## [0.1.0] - 2026-11-25

- Initial scaffold: Rust CLI (`init`, help/version, stage placeholders),
  config + script/timing types with `${ENV:...}` interpolation, fixture and
  smoke tests, build-check and release workflows.
- Skill docs and references describing the agent-browser pipeline; v1 remains
  untouched.

## [Unreleased]

- `inspect`: named-session agent-browser route walk with auth setup, optional
  nav discovery and v1-shaped selector dump.
- `validate`: live gate replay using CSS selectors or semantic `target`
  locators before any recording.
- `audio`: cached edge-tts narration with ffprobe-measured durations and a
  per-script manifest consumed by `record`.
- Planned: `record`, `merge`, `all`.
