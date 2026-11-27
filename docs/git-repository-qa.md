# Independent repository selection and application version

This follows the original Stage Fright and MeGit-inspired workbench validation.
The app stays on the local, unpublished 1.8.0 iteration.
The [later UX refinement](git-ux.md) supersedes this report's layout and binary
hashes while retaining independent repository selection and the shared version.

## Behavior

- **Open repository…** opens GTK's native local-folder chooser. The chosen folder
  is resolved with Git to its worktree root, including folders inside a repo.
  A bare Git directory, missing folder or folder outside a repository is rejected.
  The selection never creates a repository or recursively scans sibling folders.
- The active path and its source (**Workspace** / **Repository**) remain visible.
  An explicit selection stays independent when navigating between app tabs or
  changing the chat workspace. **Use workspace** restores automatic following.
- Choosing a repository does not change the chat workspace, RPC transport,
  branch, index or working-tree files. Subsequent Git actions target the selected
  repository, not the chat workspace.
- Cancel and validation failures keep the current repository. Repository changes
  are blocked during a Git mutation. Commit-message drafts are kept per repository
  in memory when switching; the repository choice lasts for the current app run.
- Without a repository, the workbench actions are hidden and the folder-opening
  action remains available. No stale branch or file list is actionable.

## One application version

`APP_VERSION` in `src/main.rs` reads `env!("CARGO_PKG_VERSION")`: `Cargo.toml`
owns the version. The sidebar, Credits, Updates, update checks and `--version`
consume that declaration. The sidebar says **Pi Desktop 1.8.0**, not a separately
maintained `pi 1.5.0`. The external Pi RPC process is not assigned a fabricated
application version.

Surface inventory: `README.md`, `CHANGELOG.md`, this report, the Git sidebar
tooltip, `src/main.rs`, `src/ui/sidebar.rs`, both Settings pages and
`src/updater.rs`. Existing workbench descriptions remain valid.

## Verification scope

- Rust tests cover root/subfolder resolution, spaces in paths, missing folders,
  bare repositories and Git-internal directories.
- The isolated UI fixture starts with a non-Git workspace, opens a different
  repository through the chooser-result callback, checks cancellation and invalid
  selections, blocks switching during an operation, returns to the workspace,
  restores a commit draft and stages a file in the external repository.
- The fixture confirms that navigating away and back preserves the explicit
  repository, while the chat workspace stays unchanged and uninitialized.
- The native chooser itself is not driven by the fixture: only its result is
  injected. Normal app launches always invoke `GtkFileDialog.select_folder`.
- Every UI scenario compares the actual sidebar label with the version read from
  `Cargo.toml`, rather than asserting another hand-maintained version string.
- Filtering and selection use the rows' own `visible` property rather than
  effective ancestor visibility, so temporarily hiding the workbench does not
  erase its logical selection.

All graphical checks use the private headless compositor. No desktop window,
existing app session, GitHub publication or remote repository is touched.

## Build evidence

- Rust: **67 passed**, one explicitly ignored network test.
- Native optimized build and AppImage succeeded; both `--version` probes match
  the package version without opening GTK windows or installing fonts.
- `dist/qa/git-repository-build.json` records source and artifact hashes.
- Previous installed binary and AppImage:
  `dist/pi-desktop-1.8.0-before-repository-picker.previous` and
  `dist/Pi-Desktop-1.8.0-before-repository-picker.AppImage`.
- **29 offline UI scenarios passed in one complete run**, followed by **three
  optimized-release checks at 125% font scale**. All 32 executions were headless.
- Actual Git text contrast: **14.25:1 dark / 15.90:1 light**; selected-row text:
  **7.38:1 dark / 12.25:1 light**. Mapped controls have at least **44 px** height,
  with no measured horizontal overflow. The bottom bar remains
  `[24, 780, 1232, 48]` before and after plan approval.
- Current PNGs, JSON reports, logs, measured metrics and installation summary:
  `dist/qa/git-repository-final`; release/font checks:
  `dist/qa/git-repository-release-125`.

[Repository and version, 1280 px](../dist/qa/git-repository-final/git-repository.png) ·
[Git component, 390 px](../dist/qa/git-repository-final/git-390.png)
