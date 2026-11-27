# Plan approval — single inline, multiple compact

User correction: keep **Plan** at the left and its three choices next to it.
Compact only when multiple plans are pending. The outer status-bar frame is
enough; Plan itself gets only a lighter background from the existing selection
token, not an outlined button or a rounded box.

## Implementation

- Exactly one pending selection request: plain Plan label, then Execute,
  Stay in plan and Refine in the same horizontal line. There is no menu,
  dropdown arrow or popup for the single plan.
- More than one request: session-labelled menus with the existing pixel-art
  chevron. Menu buttons and options have no decorative inner border or shadow.
- Answering one of two requests immediately expands the remaining plan inline.
- Known long option captions are shortened only for display. The exact original
  option string is retained in the tooltip and sent unchanged over RPC.
  Confirm requests retain their real two boolean choices, not a fabricated third.
- The approval remains in the same 48px bar, in Code and Chat modes.
  Horizontal scrolling is available for unusually long extension captions;
  shrinking the window never turns a single plan into a dropdown.
- Snapshot export now uses the allocated CSS border box rather than the content
  dimensions, so a 390×48 component is exported as 390×48, not 356×46.

## Checks

Native GTK fixtures verify one → two → one pending requests, mapped inline
choices, no inner borders, exact bar positions/dimensions, real RPC values for
Stay/Refine and the boolean Confirm reply. A separate 390px window verifies that
all three inline choices fit, including at the 125% font setting.

Regression coverage also includes the normal Code page, both color schemes,
the reasoning popup and the image viewer, because snapshot export is shared.
Tests run only in the private headless compositor and disposable configurations.

Surface inventory: `src/ui/plan_bar.rs`, scoped/shared control CSS, the
opt-in snapshot exporter, native/Python UI fixtures, README and changelog.
The initial Stage Fright acceptance report remains historical.

The local 1.8.0 version and current branding are unchanged. Previous binary and
AppImage are retained as `dist/pi-desktop-1.8.0-before-plan-inline.previous` and
`dist/Pi-Desktop-1.8.0-before-plan-inline.AppImage`.

## Final evidence

- **70 Rust tests passed**, one network test explicitly ignored.
- **Eight isolated UI executions at 125% font scale**, plus **four optimized
  release executions at the default scale**, passed. Both light/dark themes,
  inline choices, multi-plan menus and all reply checks passed.
- Status, single plan, two plans and the remaining single plan all measured
  `[24, 780, 1232, 48]`. Plan tag/action borders measured zero; only the outer
  bar retains its one-pixel frame. Actions remain at least 44px high.
- All three single-plan actions fit in the measured 390×48 component, without
  a dropdown. Tag contrast passed 4.5:1 in both themes.
- Native binary installed atomically in `~/.local/bin/pi-desktop`; AppImage rebuilt.
  Installed/release hashes match. No existing window was closed or relaunched.
- Source/artifact fingerprints: `dist/qa/plan-inline-build.json`. Final release
  reports, screenshots, run logs and installation summary:
  `dist/qa/plan-inline-release`. The 125% regression evidence is in
  `dist/qa/plan-inline-final`.

[Full window, 1280 px](../dist/qa/plan-inline-release/plan-single.png) ·
[Plan bar, 390 px](../dist/qa/plan-inline-release/plan-single-390.png)
