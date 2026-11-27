# Stage Fright — execution and acceptance

Base: `main`, `c4b8d8d` (1.7.1). The 1.7.1 installer fixes are preserved.
Implementation stays in `PUBLIC-pi-desktop`; the old private clone was not changed.

This report records the original Stage Fright acceptance run. The subsequent
[MeGit-inspired Git workbench](git-workbench.md) supersedes its Git layout and
build hashes; its validation and artifacts are recorded separately. The original
installed binary/AppImage were preserved before rebuilding the workbench.

## Implemented

- Both RPC event spellings and content-index aliases; 80ms streaming without
  tool-output clipping, in-place Markdown at block end, persistent wait preference.
- All eleven section-B fixes, including Unicode titles, two-family settings,
  unborn/detached Git, literal filenames, empty-index Enter, color-free diffs,
  tab reopening, English labels, casing and action notices.
- Git Changes/Staged groups, shared bounded diff renderer, quiet polling/badge,
  branch creation/switch, Fetch, Pull `--ff-only`, Publish, conflict recovery,
  recoverable Discard and separately confirmed Destroy.
- Session-scoped persistent plan approvals and tasks, including Chat mode.
- Completed-session state and closed-tab reopening regression coverage.
- Window size, maximize and rail preferences saved on an actual close.

## Additional UI requirements

- Only the header's outer frame has a border; its internal controls have none.
  Keyboard-focus feedback remains enabled.
- Action icons and provider logos occupy 24px; secondary indicators use 16px.
  Help, globe, play and circle come from Pixelarticons.
- Brain and file-plus are passed through the existing `tools/pixelify.py`
  with a hard 24px pixel grid, with exact nearest-neighbour 2× assets.
- A plan replaces the normal status in the same 48px bottom line. Approval
  choices open in a popover, not in layout. QA compares the visible status and
  approval rectangles, not just a surrounding container.

## Checks

UI checks default to a private headless Mutter compositor, private runtime and
private D-Bus session. They cannot fall back to the operator's desktop. Only an
explicit `--interactive` opts into visible windows. Local VFS and in-memory
GSettings avoid desktop preference writes. The private bus has no service
activation directories, so portal/GVfs daemons cannot create runtime FUSE mounts.

```sh
cargo test --offline --quiet
python3 tools/verify-ui.py --temp-root /tmp --output dist/qa/1.8.0
python3 tools/verify-ui.py code git plan-bar --font-scale 125 --output dist/qa/1.8.0-125
python3 tools/verify-ui.py live-stream live-wait --live-stream --output dist/qa/1.8.0-live
bash tools/gen-icons.sh
bash packaging/build-appimage.sh
git diff --check
```

- Rust: 58 passed, one explicitly ignored GitHub-network test.
- Final isolated UI run: 25 offline scenarios, five repetitions at 125% font
  scale, and two real-provider scenarios: **32 successful UI scenarios**.
  A deliberately invalid scenario also confirmed nonzero failure propagation.
- Git tests use disposable real repositories: unborn, detached, Unicode/newline/
  arrow/wildcard names, partial staging, rename unstaging, color.ui=always,
  recoverable per-file stash, merge/rebase abort and continue, local-remote publish,
  fetch and divergent fast-forward-only pull.
- Native checks include a real DropDown branch switch after confirmed merge
  abort, commit/empty-index Enter, a 5,000-line diff capped at 2,000 rendered rows,
  two plan requests, live thinking/text, task counts and three geometry launches.
- Both real-provider modes have passed with OpenAI `gpt-6.1-sol`, ephemeral
  sessions and all tools disabled.
- Measured code/card contrast: 8.75:1 dark and 9.59:1 light. Header controls:
  at least 44px. UI JSON reports include icon allocations, outer/internal border
  widths, overflow and exact bottom-bar rectangles.
- The visible status and approval rectangles both measured
  `[24, 780, 1232, 48]` at 1280×840, including their position.
- All six new glyphs were checked for binary alpha and exact nearest-neighbour
  2× pixels. All 420 PNG variants regenerate byte-for-byte deterministically.
- Release build and AppImage succeeded. Both executables report `1.8.0`;
  the packaged version probe completed in 1.485s without installing fonts.
  All 420 PNGs are embedded intact in both native and packaged executables.
- Installed binary: `~/.local/bin/pi-desktop`, byte-identical to the checked
  release binary. Previous 1.7.1 binary: `dist/pi-desktop-1.7.1.previous`.
  No live application window was closed or relaunched.
- Artifact hashes: `dist/qa/1.8.0/artifacts.json`. Final PNGs/reports:
  `dist/qa/1.8.0`, `dist/qa/1.8.0-125`, `dist/qa/1.8.0-live`.

## Limits / not claimed

- GTK4/Wayland does not expose application-controlled global window position.
  Position is left to the compositor; size/maximize/rail are persisted.
- Git cannot stash before the initial commit. Discard reports the Git error
  and leaves the file unchanged; this refusal is tested.
- The harness ran with temporary files outside `/tmp/opencode`, but was not
  executed on a second clean machine. It still requires GTK4, Rust and a display.
- No GitHub release/tag, push or Discord message is published by this work.
- The installed Rust toolchain has no `rustfmt`; no formatter result is claimed.
- Existing dead-code compiler warnings remain; builds are checked for success.
