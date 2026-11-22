# Changelog

## 1.7.1

### Added

- Install with one command:
  `curl -fsSL https://framet-bit.github.io/Pi-Desktop/install.sh | bash`.
  The installer builds the latest release from source into the user's home,
  compiling at low priority with half the cores and a reusable cache, and
  resumes where it stopped when interrupted. Nothing is written outside the
  user's home.
- Maintenance flags through the same command: `--update`, `--status`,
  `--dry-run`, `--uninstall` and `--purge`.
- `--container` builds inside a rootless podman container, for systems whose
  GTK4 is older than the 4.16 the app requires.
- A preflight check reads the GTK4 and libadwaita versions before building and
  stops with instructions instead of failing halfway through the compiler.

### Changed

- Systems with GTK4 older than 4.16 are asked whether to upgrade the
  distribution (recommended) or build in a container.
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
