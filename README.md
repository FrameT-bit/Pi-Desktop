![Pi Desktop](design/banner-github.png)

# Pi Desktop

A native Rust (GTK4 + libadwaita) desktop client for talking to Pi over RPC.
Local-first, two columns, three rows, no cloud dependency. SSH is optional for
running Pi on another machine.

Pi Desktop is an independent project by FrameT. It is not affiliated with,
endorsed by, or sponsored by Earendil Inc. The name "Pi" is used only to
describe compatibility with the Pi coding agent.

## Features

- Chat and code modes in one app, switched from the header.
- Live turn feedback: a 1px pulse under the sent message and a receipt with
  phase (sent, thinking, tools, writing, done), elapsed time and tokens.
- Sessions run in separate Pi RPC processes: New Session while a turn is
  running keeps the old turn alive in the background, and switching back
  attaches to that running process. Running-session indicators live in the
  sidebar and session tabs; background permission prompts wait until opened.
- Sessions, search, tree, artifacts, jobs and Pi extensions.
- Configure installed extensions when they expose settings: search, memory,
  personas and MCP open their settings page; guard, subagents and openwiki
  open their config file (JSON or .env).
- Workspace cleanup: right-click a project group to move all its sessions to
  the trash. Five or more sessions require three confirmations. The trash
  button in the sidebar footer restores them for 14 days. The project folder
  itself is never deleted; workspace cleanup is local-session only for now.
- 50 food themes, light and dark, searchable in Settings → Personalization.
- Personalization tab for theme, typeface (Pixelify/JetBrains, Adwaita or system)
  and font size.
- Memory, personas, plan mode, permissions, restore points and subagents.
- Weekly GitHub release check on app start, never on a background timer.
- Credits tab with version, links, license and the preinstalled extension bundle.

## Requirements

- Stable Rust
- GTK4 (>= 4.16) and libadwaita (>= 1.5)
- The `pi` binary available on `PATH` or configured in `config.ini`
- The fonts in `resources/fonts/` (installed by the script below)

## One-click setup

```sh
./setup.sh
```

The setup wizard installs into the current user's home: `~/.local/bin/pi-desktop`,
the desktop entry, icons and bundled fonts. It does not need root.

Non-interactive and maintenance modes:

```sh
./setup.sh --no-gui       # terminal install, no wizard
./setup.sh --dry-run      # print the plan; write nothing
./setup.sh --status       # show installed files
./setup.sh --uninstall    # remove the app; keep user data and config
```

If GTK4/PyGObject are unavailable for the system Python, the wizard falls back
to the terminal installer. Building still needs Rust and GTK4 development files.

## Build and install from source

```sh
cargo build --release
tools/install-fonts.sh
tools/install-icons.sh
```

The executable is installed at `~/.local/bin/pi-desktop`. The installer also
creates the app icon and the desktop menu entry.

## Configuration

The app reads `~/.config/pi-desktop/config.ini`. The `config.ini` in this
repository is a safe example with no machine-specific data:

```ini
[pi]
target=local
bin=~/.local/bin/pi
workspace=~/projects
session_id=desktop
```

To use another machine, set `remote_ssh` and change `target=remote`. No default
server is embedded.

## Preinstalled extensions

- `pi-memory` — durable memory and session search
- `pi-personas` — agent personalities
- `pi-plan` — read-only planning and task progress
- `pi-guard` — permission modes and safety rules
- `pi-checkpoints` — file restore points and rollback
- `pi-web-access` — web search and page extraction
- `pi-subagents` — delegated subagents in the activity rail

## Provider logos

The header model chip and the model list use a 24–28 px pixel mark per
provider, falling back to the provider initial when no mark exists. Logos are
generated from `resources/logos/src/` by `tools/gen-logos.sh`, which drives
`tools/pixelify.py` (block 2: grid 12 renders at 24 px, 14 at 28 px, 10 at
20 px). Line logos use nearest sampling; filled marks use averaged sampling.

| provider(s) | slug | grid | sampling |
|---|---|---|---|
| Anthropic | `anthropic` | 12 | average |
| OpenAI | `openai` | 14 | nearest |
| DeepSeek | `deepseek` | 10 | average |
| Google / Gemini | `googlegemini` | 10 | average |
| Groq | `groq` | 14 | average |
| Meta / Llama | `meta` | 10 | average |
| Mistral | `mistralai` | 12 | average |
| NVIDIA NIM | `nvidia` | 10 | average |
| Ollama | `ollama` | 12 | average |
| OpenRouter | `openrouter` | 14 | nearest |
| xAI | `xai` | 12 | average |
| Cerebras | `cerebras` | 14 | average |
| Fireworks | `fireworks` | 12 | average |
| Together AI | `together` | 12 | average |
| Baseten | `baseten` | 12 | average |
| Moonshot / Kimi | `moonshotai` | 14 | nearest |
| Z.AI | `zai` | 12 | average |
| MiniMax | `minimax` | 14 | nearest |
| Qwen | `qwen` | 14 | nearest |
| OpenCode | `opencode` | 12 | average |
| Vercel AI Gateway | `vercel` | 10 | average |
| Hugging Face | `huggingface` | 14 | nearest |
| GitHub Copilot | `githubcopilot` | 14 | average |
| Ant Ling | `antling` | 12 | average |
| Xiaomi MiMo | `xiaomi` | 14 | nearest |
| Radius | `radius` | 12 | average |

Sources and licenses are listed in `resources/logos/src/README.md`. The OpenRouter
and Ant Ling marks are simplified custom glyphs because no usable monochrome
asset was available; all other marks come from Simple Icons or the provider's
official favicon/press mark.

## Layout

```text
build.rs                  generates tokens and CSS from resources/tokens-site.json
resources/style.rules.css layout rules, no literal color
resources/style.css       generated at build time; do not edit by hand
resources/fonts/          Pixelify Sans and JetBrains Mono (OFL); Adwaita/system fonts are optional
resources/icons/          pixelarticons and interface icons
resources/brand/          Pi Desktop brand assets
resources/logos/          model provider marks
src/rpc.rs                local or SSH RPC client
src/ui/                   header, sidebar, chat, composer and settings
tools/install-fonts.sh    installs the bundled fonts for the current user
tools/install-icons.sh    installs the executable, icon and desktop entry
```

## License

Copyright (C) 2026 FrameT.

Pi Desktop is free software under the **GNU General Public License v3.0 or
later**. See `LICENSE`.

The license allows use, study, modification and redistribution, including paid
distribution, provided that distributed derivative works remain GPL and offer
the corresponding source code. The GPL does not prohibit selling copies.

The Pi Desktop name, icon and banner live in `resources/brand/` and `design/`.
No official pi.dev logo is included. See `TRADEMARKS.md`.


## AppImage

Build a portable x86_64 AppImage with:

```sh
packaging/build-appimage.sh
```

The artifact is written to `dist/Pi-Desktop-<version>-x86_64.AppImage`.
