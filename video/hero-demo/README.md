# Hero demo video

Tools to shoot the README hero video: an agent on a remote server hands back a link, one click opens
the doc, and it is read and fixed in the browser. Script and shot list:
`vlog/docs/video-ideas/a1-mdview-hero-demo.md` (outside this repo). Look: dark stage, one ember accent.

| Path | What it is | In Git |
|---|---|---|
| `sandbox.sh` | The demo machine `build-01`: a second mdview daemon with its own HOME on port 7710, the demo project, and a project `.mcp.json` that points agents at that daemon | yes |
| `fixture/acme-payments/` | Invented project: a payment retry plan (table, Mermaid, YAML with a deliberate `max_retries: 10`), an ADR, `CLAUDE.md` telling the agent to hand back mdview links | yes |
| `record.mjs` | Records the browser scenes 4–6 with Playwright | yes |
| `tapes/` | VHS tapes (`scene12-path-hunt`, `scene3-link`, `warmup`, the older `scene1-path`), the shared `settings.tape`, and `render.sh`, which records the whole video | yes |
| `fresh.json`, `wezterm.lua`, `wezterm.sh` | Editor look for scene 2, and the WezTerm setup for recording it by hand | yes |
| `edit/` | Remotion project that cuts the recordings into the 45 s video and the 30 s README cut: `src/data.ts` is the cut (shots, captions, clicks) | yes (not `node_modules`, `public/fonts`, the music) |
| `out/` | Recordings, click logs (`*.clicks.json`) and the finished videos | no |

Your own mdview (port 7700, `~/.mdview`) keeps running untouched.

## Setup, once per session

```bash
video/hero-demo/sandbox.sh up
```

It prints the demo URL (`http://build-01:7710`) and the login token (`hero-demo`). For a real browser, add
`127.0.0.1 build-01` to `/etc/hosts`, and to `C:\Windows\System32\drivers\etc\hosts` if the browser runs on
Windows next to WSL. Sign in once in that browser before recording, so the login page never shows on camera.

## Record the whole video

```bash
video/hero-demo/tapes/render.sh
```

About 6 minutes. Runs, in order, and writes to `out/` (1920x1080 each):

| Step | Output | What it is |
|---|---|---|
| `warmup` | none | Trusts the demo folder in claude, off camera |
| `scene12` | `scene12-path-hunt.mp4` (~38 s) | Scenes 1+2 in one VHS take: the agent writes the plan and answers with a bare path; then quit, `fresh`, find the file in the tree (Ctrl+B), scroll raw markdown. One take, so the cut from 1 to 2 can't jump |
| `scene3` | `scene3-link.mp4` (~21 s) | Same prompt with mdview wired in: the reply ends with `http://build-01:7710/s/...`. Leaves the agent's plan in place |
| `browser` | `scene4-read.mp4`, `scene5-edit.mp4`, `scene6-link.mp4` | Playwright on the plan the agent just wrote: read and zoom the diagram; Edit the `max_retries` value and save; click the ADR link. Scene 5 restores the file |

One step at a time: `video/hero-demo/tapes/render.sh scene3 browser`.

How the takes stay clean (each one cost a failed take to learn):

- The agent really runs (`claude`, logged in as you). The demo project's `.claude/settings.local.json`
  accepts edits, denies shell commands (a shell approval prompt would stall the take) and turns prompt
  suggestions off (a greyed "open it in mdview" would give scene 3 away).
- The project's `CLAUDE.md` is a house plan template (goals, a table, a Mermaid diagram, a YAML block
  with `max_retries`, steps), so whatever the agent writes has what scenes 4–6 zoom, edit and click.
  The mdview part (`CLAUDE.mdview.md`) is added only outside `plain`.
- Tapes hide the agent's wait and sleep a fixed 120 s: VHS's `Wait+Screen` reads the top of the buffer,
  not what is on screen once output scrolls. Startup waits for the `Claude Code v` banner.
- `fresh.json` uses the `terminal` theme (the tape's colours) and does not restore the last session.
- Fonts: Geist Mono and Noto Sans Symbols 2 (for Claude Code's `⏺`) must be installed for fontconfig.
- Clicks are added in the edit: during the hold on the reply in scene 1, and on the link in scene 3.

## Cut the video

```bash
cd video/hero-demo/edit && npm ci && npm run render && npm run render:readme
```

Writes `out/hero-45s.mp4` (about 43 s) and `out/hero-30s.mp4` (about 32 s). Fonts: copy Geist
(Regular, Medium, SemiBold) and Geist Mono into `edit/public/fonts/`. `npm run studio` opens the timeline.
Music: download John Bartmann, "Nova Serenade" (CC0) from
https://commons.wikimedia.org/wiki/File:John_Bartmann_-_Nova_Serenade.ogg into
`edit/public/audio/nova-serenade.ogg`. The click and whoosh effects are in the repo.

The README takes the 30 s cut without sound (GitHub plays it muted anyway):

```bash
ffmpeg -i out/hero-30s.mp4 -an -c:v copy out/hero-30s-silent.mp4
```

The cut lives in `edit/src/data.ts`: each shot names a recording, the part to use, its speed, a hold on
its last frame, the captions and the clicks. Browser clicks come from `out/<scene>.clicks.json`, written by
`record.mjs`. Terminal clicks (the path in scene 1, the link in scene 3) and the cut points in the terminal
takes are measured on the recordings and must be measured again after a new take: list the screen changes
with `ffmpeg -i out/scene12-path-hunt.mp4 -vf "select='gt(scene,0.02)',showinfo" -f null -`, and read
positions off a frame with a grid (`-vf drawgrid=w=100:h=50`).

## Scene 2 by hand (fallback)

If you want a take with a real mouse: `sandbox.sh plain`, then `video/hero-demo/wezterm.sh` opens WezTerm
on Windows with the same look (1920x1080 inner window, `wezterm.lua`), in the demo shell. Record it with
OBS (Window Capture, Client Area). Steps: `vlog/docs/video-ideas/a1-quay-canh-2.md`.

## Tear down

```bash
video/hero-demo/sandbox.sh down
```

The sandbox lives in `${TMPDIR:-/tmp}/mdview-hero-demo`; delete the folder to remove everything.
