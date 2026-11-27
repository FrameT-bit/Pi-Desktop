# Changelog

## 1.8.0 — Stage Fright

### Added

- Git Changes/Staged groups, per-file and group staging, branch switching and
  creation, Fetch, fast-forward-only Pull, and Publish for branches without an upstream.
- MeGit-inspired Git workbench with compact Unstaged/Staged lists, immediate
  previews, Ctrl/Shift multi-selection, batch staging, filters and keyboard actions.
- Read-only Git history: actual parent graph, refs, author/date, full message,
  changed files and per-file diffs; 200 commits initially, expandable to 2,000.
- Open a local Git repository without changing the chat workspace; show its
  resolved root and return to the workspace with one action. Invalid/cancelled
  selections preserve the active repository, and commit drafts survive switching
  repositories within the app run.
- Focused Git UX refinement without rebranding: compact repository header/local
  tabs, New branch and Refresh under More, plain-language sync status, distinct
  empty states, and commit readiness hints.
- Filtered bulk staging/unstaging affects shown files only; hidden changes remain
  untouched. Short operation confirmations retain Git details in a tooltip.
- Explicit merge/rebase conflicts, Open file, confirmed Abort, and Continue after
  conflicts are staged.
- Discard saves a named stash and reports its recovery reference. Permanent
  destruction is a separate, confirmed action.
- Persistent plan approvals in the bottom bar; simultaneous plans carry session labels. All plan tasks
  and their progress count appear in the rail, including Chat mode.
- Plan approvals keep the bottom bar's dimensions unchanged. A single plan shows
  its choices inline beside a softly highlighted Plan label, without an inner
  frame. Only multiple simultaneous plans compact into session-labelled popovers.
  Header controls no longer have decorative borders.
- Tool-specific glyphs and running dots, with an offline icon generator.
- Header action icons and provider logos share a 24px slot; secondary indicators
  use 16px. Help uses the same pixel-art icon family.
- Saved window size, maximized state, rail width and rail visibility. Window
  position remains compositor-managed on Wayland.

### Fixed

- Read real Pi `assistantMessageEvent` and `contentIndex` fields while preserving
  the legacy aliases. Text streams on an 80 ms timer, without tool-output clipping,
  and becomes Markdown in place when its block ends.
- The sidebar no longer reports a hardcoded Pi 1.5.0: sidebar, Credits, Updates,
  update checks and `--version` share the package-derived application version.
- **Prefer waiting for responses** in Settings persists as `[ui] wait_for_response`.
- Git status handles unborn branches, detached HEAD, quoted/NUL-delimited paths,
  renames containing arrows, partial files and conflict states.
- Unstaging works before the first commit; diffs disable ANSI colors and share
  the chat renderer with horizontal scrolling and a 2,000-row limit per file.
- Git polls silently every three seconds while visible, every fifteen seconds
  otherwise, and refreshes its sidebar badge when a turn settles.
- Reopening a closed session restores its tab; Ctrl+Shift+T reopens the last closed tab.
- Unicode title prefixes cannot panic; semicolons in model-family overrides survive saving.
- Completed sessions retain DONE without restoring stale running state on selection.
- The native UI harness uses portable temporary directories and versioned output,
  with streaming, thinking, plan-bar, status and Git fixtures.

## 1.7.1

### Added

- Install with one command, `curl -fsSL https://framet-bit.github.io/Pi-Desktop/install.sh | bash`.
  The installer builds the latest release from source into the user's home,
  compiling at low priority with half the cores and a reusable cache. A run
  that is interrupted continues where it stopped. Nothing is written outside
  the user's home.
- Maintenance flags through the same command, as `--update`, `--status`,
  `--dry-run`, `--uninstall` and `--purge`.
- `--container` builds inside a rootless podman container, for systems whose
  GTK4 is older than the 4.16 the app requires.
- A preflight check reads the GTK4 and libadwaita versions before building, so
  a system that cannot build is told in two seconds instead of failing halfway
  through the compiler.

### Changed

- Systems with GTK4 older than 4.16 are asked whether to upgrade the
  distribution, which is the recommended path, or to build in a container.
- The application itself is unchanged.

## 1.7.0

### Added

- Attach multiple images and files to a message. Images get thumbnails; other files get attachment cards. Pi receives supported images over RPC, and PDFs can be included as text when `pdftotext` is available.
- Assistant images appear inline rather than inside tool cards. Consecutive images form a carousel. Click an image to enlarge it.
- Compact Memory, Personality, and the custom System Prompt from Settings. Choose a model, see how much shorter the result is, and review the draft before saving.
- Optional Clarify questions with preset answers, a free-text field, and a notification.
- Best, Balanced, and Cheap sub-agent presets, with model overrides per family.
- A dedicated Updates tab checks the official GitHub repository, labels published previews, and links to releases and compatible AppImages.

### Changed

- Long conversations initially render the latest 20 messages. Use **Load earlier messages** to browse the rest.
- Tool and diff cards start collapsed by default. Enable **Keep tool cards expanded** in Settings to change the default.

### Fixed

- Removed reference cycles that kept old chat and sidebar widgets in memory, and reduced redundant sidebar rebuilds.
- Autoscroll catches up when the window returns to the foreground if you were following the end of the conversation.
- The activity rail compacts when space is limited without losing your preferred width.
- The custom title bar can be dragged in windowed mode.
- The model picker’s search field now accepts focus and filters by name, provider, or full model ID.
- Update checks work when the repository has only pre-releases and do not offer a downgrade.
