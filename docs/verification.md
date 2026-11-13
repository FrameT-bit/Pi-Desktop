# Verifying Pi Desktop

```sh
cargo test
python3 tools/verify-ui.py
```

The native GTK scenarios use `tests/fixtures/pi-rpc.py` and isolated temporary
homes/configuration under `/tmp/opencode`. They do not contact a model, execute
tools, or change existing sessions. A graphical display is required; X11 can be
selected with `GDK_BACKEND=x11` when running under Wayland.
Set `PI_DESKTOP_QA_BIN` to run the same scenarios against an installed binary or
an AppImage (`APPIMAGE_EXTRACT_AND_RUN=1`) instead of the debug executable.

The UI checks cover:

- Saved light/dark themes surviving a round-trip through the header button.
- The rail's allocated widths and its drag-handler signal path.
- Equal header control heights and at least 44px control hit targets.
- Session tabs hidden by default and visible when enabled.
- Parsed sub-agent status without machine JSON leaking into the composer.
- Healthy connection state hidden, with settings still visible in Chat mode.
- Choosing a reasoning option producing the expected Pi RPC command.
- Card text contrast, horizontal bounds and reasoning popup containment.

PNGs and JSON measurements are written to `dist/qa/2026-11-12/` (gitignored).
The reasoning popup is also captured at 390px. Pointer-driven checks on a live
session remain a manual smoke test; the automated checks emit GTK signals and
inspect allocated geometry rather than clicking unrelated desktop windows.
