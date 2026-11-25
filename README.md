# screencast-tutorial-v2

Narrated screen-walkthrough tutorials of any web app, driven by
[Vercel agent-browser](https://github.com/vercel-labs/agent-browser) and a
small Rust CLI. This is **v2** of the `screencast-tutorial` skill; v1
(TypeScript + Playwright) stays installed and untouched.

> **Status: 0.1.0** — CLI scaffold, config/script parsing, docs and CI are in
> place. `inspect` and `validate` are implemented; `audio`, `record`, `merge`
> and `all` land in follow-up PRs.

## Install the skill

Copy the repo into your agent's global skills directory (the root `SKILL.md`
makes it auto-discoverable), then install the release binary:

```bash
# OpenCode example; use your agent's skills dir for Claude/Codex/Cursor.
git clone --depth 1 https://github.com/XyrusCode/screencast-tutorial-v2.git \
  ~/.config/opencode/skills/screencast-tutorial-v2

# Pick the asset for your OS/arch from releases/latest:
#   screencast-v2_linux-x86_64
#   screencast-v2_macos-x86_64
#   screencast-v2_macos-aarch64
#   screencast-v2_windows-x86_64.exe
curl -fL https://github.com/XyrusCode/screencast-tutorial-v2/releases/latest/download/screencast-v2_linux-x86_64 \
  -o ~/.local/bin/screencast-v2 && chmod +x ~/.local/bin/screencast-v2
```

Prerequisites: `ffmpeg`/`ffprobe` on PATH, plus
`npm install -g agent-browser && agent-browser install` and
`uv tool install edge-tts`. Verify with `screencast-v2 --version` and
`agent-browser doctor`.

The ai-sync README carries a copy-paste prompt that performs these steps on
any machine.

## CLI

```
screencast-v2 init                         # write starter config + script
screencast-v2 inspect --routes "/,/users"  # dump routes and observed selectors
screencast-v2 validate --script <file>     # assert gates on the live app
screencast-v2 audio    --script <file>     # narration mp3s + durations
screencast-v2 record   --script <file>     # one continuous WebM + timings JSON
screencast-v2 merge    --script <file>     # cut, sync and mux final MP4s
screencast-v2 all      --script <file>     # audio -> record -> merge
```

## Development

**Never build or test locally.** CI is the only authority: branch → push → PR →
`gh pr checks --watch` → rinse-and-repeat on failure → regular merge. See
[AGENTS.md](AGENTS.md) for the full rules.

## License

MIT — see [LICENSE](LICENSE).
