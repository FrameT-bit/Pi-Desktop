# Git UX — focused refinement, unchanged branding

Constraint: keep Pi Desktop's palette, pixel-art assets, square controls, Mono
typeface and current shell. No new library, palette, icon style or animation.

References: the supplied `eliah-ui-preferences` / `eliah-design-system` archive,
the project's canonical UI contract, and Don Norman's signifiers, feedback and
constraints. Apply those interaction principles, not another product's styling.

```
Workspace / Repository · path                  Open repository… [Use workspace]
branch                                  Fetch  Pull  Push/Publish  More
upstream / useful sync state                 (More: New branch, Refresh)
Changes  History
────────────────────────────────────────────────────────────────────────────
Filter files                      Selected file · staged or unstaged version
Unstaged · count   Stage / all     Diff
rows
Staged · count     Unstage / all
rows
Describe the staged changes                                       Commit
```

- Keep the frequently used actions visible; move only New branch and manual
  Refresh to More. Preserve the native repository chooser and branch dropdown.
- Stop stretching two local tabs across the whole workbench.
- Remove duplicated branch/count/zero-arrow summaries. Say whether the branch
  is unpublished, up to date, ahead/behind, detached or in an operation.
- Show one count per group; show "visible of total" only while filtering.
- While filtering, bulk actions say "Stage shown" / "Unstage shown" and act only
  on visible files. Hidden files must not be silently included.
- Distinguish no changes, no staged files, no filter matches and no repository.
  Offer Clear filter or View history where those are the next useful actions.
- For partially staged files, distinguish the staged and unstaged versions in
  their rows/tooltips instead of calling both versions "partial".
- Enable Commit only with staged files, a nonblank message and no conflicts.
  Explain disabled actions with tooltips; don't manufacture successful commits.
- Preserve drafts, selection, recoverable Discard, confirmed Destroy and the
  fixed bottom bar. Test offline in the private compositor at 390 / 1280 px.
- Confirm successful operations in one line; retain Git's successful output in
  the tooltip. Errors and stash recovery references remain visible, not hidden.

Surface inventory: `README.md`, `CHANGELOG.md`, this report, the historical
workbench/repository reports, and the actual toolbar, groups and commit controls.

## Regression coverage

- Rust: **69 passed**, one explicitly ignored network test. New pure tests cover
  useful sync summaries, detached/unborn states, conflicts and operation status.
- `git-ux` tests Commit readiness with empty/whitespace/valid messages, both
  filtered bulk actions, exact preservation of a hidden file's index/worktree
  content, clearing a no-match filter, and opening the actual More popover.
- `git-clean` verifies the clean-state message, disabled Commit and the working
  View history action. Existing repository, recovery, diff, selection and
  keyboard tests remain in the suite.
- The History pane initially reserves three commit rows instead of five, leaving
  more room for the actual patch. Its regression test checks that an added/deleted
  line fits inside the diff viewport, not merely that hidden diff widgets exist.
- Existing palette tokens, fonts, 24/16 px artwork, square control geometry and
  the app shell are reused. The focused CSS changes do not restyle other tabs.
- Previous binary/AppImage are saved as
  `dist/pi-desktop-1.8.0-before-ux-pass.previous` and
  `dist/Pi-Desktop-1.8.0-before-ux-pass.AppImage`.
- `dist/qa/git-ux-build.json` records the optimized native/AppImage hashes and
  source fingerprints. Both binaries' `--version` probes match `Cargo.toml`
  without opening windows or installing fonts.

## Measured layout comparison

In matching 1280×840 Git fixtures, the file/diff pane grows from **308 to
345 px high** (+37 px) without shrinking touch targets. In the isolated 390 px
component, it grows from **458 to 522 px** (+64 px). These are actual GTK
allocations compared with `dist/qa/git-repository-final`, not estimates from
screenshot pixels. The main desktop shell and bottom bar are unchanged.

Reproduce the offline checks without opening desktop windows:

```sh
cargo test --offline --quiet
python3 tools/verify-ui.py --output dist/qa/git-ux-final
PI_DESKTOP_QA_BIN="$PWD/target/release/pi-desktop" python3 tools/verify-ui.py \
  git-ux git-clean git-repository git-390 git-history --font-scale 125 \
  --output dist/qa/git-ux-release-125
```

The full 31-scenario offline run passed. A final History-divider adjustment was
then checked separately at 125% font scale, followed by all five focused
optimized-release scenarios (History, narrow component, repository selection,
filtered bulk actions and clean state) at 125%. Logs keep those stages separate.

Final native binary is installed in `~/.local/bin/pi-desktop`, byte-identical to
the checked optimized build. The AppImage was rebuilt as well. No existing app
window was closed or relaunched; no commit, push, tag or GitHub release was made.

Evidence: `dist/qa/git-ux-final/summary.json`, `metrics.json` and the copied run
logs; final optimized-release screenshots/reports are in
`dist/qa/git-ux-release-125`. Text contrast remains **14.25:1 dark / 15.90:1
light**, selected text **7.38:1 / 12.25:1**; measured controls remain at least
**44 px** high, with no overflow in the tested layouts. The bottom bar remains
`[24, 780, 1232, 48]` before/after approval.

One AppImage `--version` probe exceeded 30 seconds during concurrent checks;
no root cause is asserted. The isolated recheck with AppImageLauncher disabled
completed in **0.331 seconds**, returned 1.8.0 and installed no fonts.

[Changes, 1280 px](../dist/qa/git-ux-final/git-ux.png) ·
[Git component, 390 px](../dist/qa/git-ux-final/git-390.png) ·
[Final History, release at 125%](../dist/qa/git-ux-release-125/git-history.png)
