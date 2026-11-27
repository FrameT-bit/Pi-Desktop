# Git workbench — MeGit / EGit interaction model

The [repository selector and shared version declaration](git-repository-qa.md)
are a subsequent local iteration. The counts and hashes below describe the
original workbench validation, not the newer binaries.
The [focused UX refinement](git-ux.md) further compacts the header and makes
filtered bulk actions visible-only.

Reference inspected: https://github.com/eclipsesource/megit and its actual
`doc/megit-screenshot.png`. Adapt the workbench interaction, not Eclipse/Java.

```
repository · branch                         Fetch  Pull  Push  Refresh
Changes | History                         upstream / ahead / behind
─────────────────────────────────────────────────────────────────────
Changes                                  History
┌ Filter files ───────┬ Selected file ┐    ┌ Filter / graph / commits ┐
│ Unstaged  Stage all│               │    │ hash · refs · subject   │
│ selectable rows    │ readable diff │    ├ changed files ┬ diff ──┤
│ Staged  Unstage all│               │    │              │        │
│ selectable rows    │               │    └──────────────┴────────┘
│ summary / Commit   │               │
└────────────────────┴───────────────┘
```

- One click previews a file; no repeated Diff button and no card per file.
- Ctrl/Shift selection and Stage/Unstage operate on exactly the selected files.
- Space or Enter in a file list stages/unstages the selection; Ctrl+Enter commits.
- Ctrl+F filters the current Git view. Clear filters explicitly.
- Preserve selection and scroll on polling; focus the destination group after
  a staging action. Partial files remain represented in both groups.
- Per-file context menu holds Open file, recoverable Discard and confirmed
  Destroy. No hidden destructive action on a double-click or Delete.
- History is read-only: actual parent graph, author, timestamp, references,
  complete message and files/diff for the selected commit. Browsing must not
  checkout a commit, alter the index or replace the working tree.
- Latest 200 commits initially; explicit bounded Load more, up to 2,000.
- Resizable panes, existing color/spacing/type tokens and pixel-art assets.
- All tests run in the existing private headless compositor, in background.
- This is an iteration of the local 1.8.0 work, not a new published release.

## Interaction details

Stage/Unstage labels include the visible selection count. Their **all** variants
operate on the whole group when no filter is active; while filtering they become
**Stage shown / Unstage shown** and leave hidden files untouched.
Files with both index and working-tree changes appear in both groups. The
commit action only uses the index, never implicitly stages the working tree.

Filtering a file or commit out of the current selection previews the first
matching result; an empty result clears the preview. History search covers the
loaded commits, not an unbounded repository scan. The parent graph is hidden
while filtering because compressed rows would suggest false parent connections.
Merge previews compare against the first parent and say so in the details.

Discard remains recoverable through the stash. Destroy still requires an
explicit confirmation. Neither double-click nor Delete destroys content.
This iteration does not copy MeGit code, introduce Java, or claim full client
feature parity (hunk staging and interactive rebases are outside this change).

## Verification

Surface inventory for this iteration: `README.md`, `CHANGELOG.md`, this report,
the historical `docs/stage-fright-qa.md` cross-reference, the command-palette
action in `src/ui/mod.rs`, and the shortcut sheet in `src/ui/hotkeys.rs`.
No release/version rename or published site deployment is part of this change.

- Rust: **66 passed**, one explicitly ignored network test. New coverage includes
  selected-only batch operations, literal pathspecs, unborn/renamed files, complete
  commit messages, binary/deleted/renamed paths, detached HEAD, merge graphs,
  first-parent diffs, pagination and read-only browsing.
- Git-specific headless checks cover both themes, partial files, conflicts,
  unborn branches, large diffs, stage/commit, merge recovery, multi-selection,
  keyboard staging, Ctrl+F routing, history and a narrow component.
- **28 unique offline scenarios passed**, plus **three checks of the optimized
  release binary at 125% font scale**: 31 UI executions in this validation.
  The initial full run stopped on history selection not returning after an empty
  filter. That bug was fixed; its regression case and every remaining scenario
  passed, followed by the three release checks. Existing passing cases were kept.
- Measured text contrast: **14.25:1 dark / 15.90:1 light** on the workbench;
  **7.38:1 dark / 12.25:1 light** on the selected-row background.
- Mapped buttons and file rows have a **44 px minimum height**, and mapped
  controls stay within the measured window.
- The 390 px screenshot exercises the Git component in its own isolated window,
  not the desktop shell at a fictional mobile width. Both panes stack vertically.
  The 1280 px screenshots show the real desktop shell.
- Builds retain version **1.8.0**. Previous installed binary and AppImage are
  preserved as `dist/pi-desktop-1.8.0-before-git-workbench.previous` and
  `dist/Pi-Desktop-1.8.0-before-git-workbench.AppImage`.
- PNGs, reports and run logs: `dist/qa/git-workbench-final`; release/font checks:
  `dist/qa/git-workbench-release-125`. `summary.json` records the final hashes,
  installation and bottom-bar measurements. No existing app window was restarted.

[Changes, 1280 px](../dist/qa/git-workbench-final/git-dark-1280.png) ·
[History, 1280 px](../dist/qa/git-workbench-final/git-history.png) ·
[Git component, 390 px](../dist/qa/git-workbench-final/git-390.png)

Reproduce without opening desktop windows:

```sh
cargo test --offline --quiet
python3 tools/verify-ui.py --output dist/qa/git-workbench-final
PI_DESKTOP_QA_BIN="$PWD/target/release/pi-desktop" python3 tools/verify-ui.py \
  git-workbench git-history git-390 --font-scale 125 \
  --output dist/qa/git-workbench-release-125
```
