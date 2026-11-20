# Changelog

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
