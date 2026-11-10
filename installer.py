#!/usr/bin/python3
"""Pi Desktop graphical setup wizard.

The wizard only draws the interface. installer-backend.sh performs every
filesystem change, and it runs as the current user: no root and no password.
"""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path

try:
    import gi

    gi.require_version("Gtk", "4.0")
    gi.require_version("Adw", "1")
    from gi.repository import Adw, GLib, Gtk
except (ImportError, ValueError) as exc:
    print(
        f"GTK4 and libadwaita for the system Python are required ({exc}).",
        file=sys.stderr,
    )
    raise SystemExit(1)

APP_ID = "io.github.FrameT.PiDesktop.Setup"
WINDOW_TITLE = "Pi Desktop Setup"
HERE = Path(__file__).resolve().parent
BACKEND = HERE / "installer-backend.sh"
BRAND = HERE / "resources/brand/logo-128.png"
EXEC_PATH = Path.home() / ".local/bin/pi-desktop"
DATA_HOME = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share"))
DESKTOP_FILE = DATA_HOME / "applications/io.github.FrameT.PiDesktop.desktop"
FONT_DIR = DATA_HOME / "fonts/pi-desktop"
CONFIG_FILE = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "pi-desktop/config.ini"

PAGES = ("welcome", "checks", "install", "done")


@dataclass
class Check:
    name: str
    ok: bool
    detail: str
    blocking: bool = True


def backend_check() -> list[Check]:
    try:
        result = subprocess.run(
            [str(BACKEND), "check"],
            capture_output=True,
            text=True,
            timeout=120,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return [Check("Installer backend", False, str(exc), True)]
    checks: list[Check] = []
    for line in result.stdout.splitlines():
        fields = line.split("|", 3)
        if len(fields) >= 4 and fields[0] == "CHECK":
            checks.append(Check(fields[2], fields[1] == "ok", fields[3], fields[1] != "ok"))
    if not checks:
        detail = (result.stdout + result.stderr).strip()[-300:] or "no output"
        checks.append(Check("Installer backend", False, detail, True))
    return checks


def install_state() -> list[tuple[str, str, str]]:
    rows: list[tuple[str, str, str]] = []
    for label, path in [
        ("Application", EXEC_PATH),
        ("Desktop entry", DESKTOP_FILE),
        ("Bundled fonts", FONT_DIR),
        ("Configuration", CONFIG_FILE),
    ]:
        rows.append((label, str(path), "present" if path.exists() else "missing"))
    return rows


class BackendJob:
    def __init__(self, argv: list[str], on_record, on_finished):
        self.argv = argv
        self.on_record = on_record
        self.on_finished = on_finished
        self.process: subprocess.Popen | None = None
        self.cancelled = False

    def start(self) -> None:
        self.process = subprocess.Popen(
            self.argv,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            start_new_session=True,
        )
        threading.Thread(target=self._pump, daemon=True).start()

    def _pump(self) -> None:
        process = self.process
        assert process is not None and process.stdout is not None
        for line in process.stdout:
            GLib.idle_add(self.on_record, line.rstrip("\n"))
        code = process.wait()
        GLib.idle_add(self.on_finished, code, self.cancelled)

    def cancel(self) -> None:
        self.cancelled = True
        if self.process is None or self.process.poll() is not None:
            return
        try:
            os.killpg(os.getpgid(self.process.pid), signal.SIGTERM)
        except OSError:
            self.process.terminate()


class SetupWindow(Adw.ApplicationWindow):
    def __init__(self, application, demo: bool = False, dry_run: bool = False,
                 no_build: bool = False, no_fonts: bool = False, no_shortcut: bool = False):
        super().__init__(application=application)
        self.demo = demo
        self.dry_run = dry_run or demo
        self.no_build = no_build
        self.no_fonts = no_fonts
        self.no_shortcut = no_shortcut
        self.page = "welcome"
        self.job: BackendJob | None = None
        self.saw_result = False
        self.result_ok = False
        self.result_text = ""
        self.checks = backend_check()
        self.checks_hint = Gtk.Label(xalign=0, wrap=True)
        self.checks_hint.add_css_class("dim-label")
        self.checks_hint.set_visible(False)
        self.summary_filled = False

        self.set_title(WINDOW_TITLE)
        self.set_default_size(820, 620)

        self.stack = Gtk.Stack()
        self.stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self.welcome = self.build_welcome()
        self.checks_page = self.build_checks()
        self.install_page = self.build_install()
        self.done_page = self.build_done()
        self.stack.add_named(self.welcome, "welcome")
        self.stack.add_named(self.checks_page, "checks")
        self.stack.add_named(self.install_page, "install")
        self.stack.add_named(self.done_page, "done")

        header = Adw.HeaderBar()
        header.set_title_widget(Adw.WindowTitle(title=WINDOW_TITLE))
        toolbar = Adw.ToolbarView()
        toolbar.add_top_bar(header)
        toolbar.set_content(self.stack)
        toolbar.add_bottom_bar(self.build_footer())
        self.set_content(toolbar)
        self.connect("close-request", self.on_close)

        self.sync()
        GLib.idle_add(self.first_page)

    def build_welcome(self) -> Gtk.Widget:
        page = Adw.StatusPage()
        page.set_title("Set up Pi Desktop")
        page.set_description(
            "This installs the native GTK4 client, the bundled fonts and, if you "
            "want, the application-menu shortcut. Everything is user-level; no "
            "root password is needed."
        )
        if BRAND.exists():
            try:
                page.set_paintable(Gtk.Image.new_from_file(str(BRAND)).get_paintable())
            except (AttributeError, TypeError):
                page.set_icon_name("system-software-install-symbolic")
        else:
            page.set_icon_name("system-software-install-symbolic")
        self.welcome_note = Gtk.Label(xalign=0, wrap=True)
        self.welcome_note.add_css_class("dim-label")
        self.welcome_note.set_margin_start(24)
        self.welcome_note.set_margin_end(24)
        self.welcome_note.set_text(
            f"Install target: {EXEC_PATH}\nConfiguration and user data stay outside the project."
        )
        page.set_child(self.welcome_note)
        return page

    def build_checks(self) -> Gtk.Widget:
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=18)
        box.set_margin_top(24)
        box.set_margin_bottom(24)
        box.set_margin_start(24)
        box.set_margin_end(24)
        group = Adw.PreferencesGroup(
            title="System checks",
            description="Read-only checks. Missing requirements block the install.",
        )
        for check in self.checks:
            row = Adw.ActionRow(title=check.name, subtitle=check.detail)
            row.set_activatable(False)
            row.set_subtitle_selectable(True)
            icon = Gtk.Image.new_from_icon_name(
                "object-select-symbolic" if check.ok else "dialog-error-symbolic"
            )
            icon.add_css_class("success" if check.ok else "error")
            row.add_prefix(icon)
            group.add(row)
        box.append(group)
        if any(not check.ok for check in self.checks):
            self.checks_hint.set_text("Install the missing dependencies, then reopen setup.")
            self.checks_hint.set_visible(True)
            box.append(self.checks_hint)

        options = Adw.PreferencesGroup(title="Options")
        self.build_switch = Adw.SwitchRow(
            title="Build the release binary",
            subtitle="Run cargo build --release before installing.",
            active=not self.no_build,
        )
        self.font_switch = Adw.SwitchRow(
            title="Install bundled fonts",
            subtitle="Pixelify Sans and JetBrains Mono, SIL OFL.",
            active=not self.no_fonts,
        )
        self.shortcut_switch = Adw.SwitchRow(
            title="Create the application-menu shortcut",
            subtitle="Desktop entry and hicolor icons for the current user.",
            active=not self.no_shortcut,
        )
        options.add(self.build_switch)
        options.add(self.font_switch)
        options.add(self.shortcut_switch)
        box.append(options)
        scroll = Gtk.ScrolledWindow()
        scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        clamp = Adw.Clamp()
        clamp.set_maximum_size(760)
        clamp.set_child(box)
        scroll.set_child(clamp)
        return scroll

    def build_install(self) -> Gtk.Widget:
        self.phase_label = Gtk.Label(label="Waiting", xalign=0, wrap=True)
        self.phase_label.add_css_class("heading")
        self.progress = Gtk.ProgressBar(show_text=True)
        self.log_buffer = Gtk.TextBuffer()
        self.log_view = Gtk.TextView(buffer=self.log_buffer)
        self.log_view.set_editable(False)
        self.log_view.set_cursor_visible(False)
        self.log_view.set_monospace(True)
        self.log_view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        self.log_scroller = Gtk.ScrolledWindow()
        self.log_scroller.set_policy(Gtk.PolicyType.AUTOMATIC, Gtk.PolicyType.AUTOMATIC)
        self.log_scroller.set_min_content_height(300)
        self.log_scroller.add_css_class("card")
        self.log_scroller.set_child(self.log_view)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14)
        box.set_margin_top(24)
        box.set_margin_bottom(24)
        box.set_margin_start(24)
        box.set_margin_end(24)
        box.append(self.phase_label)
        box.append(self.progress)
        caption = Gtk.Label(label="Installation log", xalign=0)
        caption.add_css_class("dim-label")
        box.append(caption)
        box.append(self.log_scroller)
        clamp = Adw.Clamp()
        clamp.set_maximum_size(760)
        clamp.set_child(box)
        return clamp

    def build_done(self) -> Gtk.Widget:
        page = Adw.StatusPage()
        page.set_title("Pi Desktop is ready")
        page.set_icon_name("object-select-symbolic")
        page.set_description(
            "Open Pi Desktop from the application menu, or run pi-desktop in a terminal."
        )
        self.summary_group = Adw.PreferencesGroup(title="Installed")
        page.set_child(self.summary_group)
        return page

    def build_footer(self) -> Gtk.Widget:
        self.quit_button = Gtk.Button(label="Quit")
        self.quit_button.connect("clicked", lambda *_: self.close())
        self.back_button = Gtk.Button(label="Back")
        self.back_button.connect("clicked", lambda *_: self.go(PAGES.index(self.page) - 1))
        self.primary_button = Gtk.Button(label="Continue")
        self.primary_button.add_css_class("suggested-action")
        self.primary_button.connect("clicked", self.on_primary)
        controls = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        controls.append(self.back_button)
        controls.append(self.primary_button)
        footer = Gtk.CenterBox()
        footer.set_margin_top(10)
        footer.set_margin_bottom(10)
        footer.set_margin_start(18)
        footer.set_margin_end(18)
        footer.set_start_widget(self.quit_button)
        footer.set_end_widget(controls)
        return footer

    def first_page(self):
        seconds = os.environ.get("INSTALLER_TEST_AUTOCLOSE", "")
        if seconds.isdigit() and int(seconds) > 0:
            GLib.timeout_add(int(seconds) * 1000, self.close)
        if not self.demo:
            return GLib.SOURCE_REMOVE
        wanted = os.environ.get("INSTALLER_DEMO_PAGE", "")
        if wanted in PAGES:
            self.go(PAGES.index(wanted))
        if os.environ.get("INSTALLER_DEMO_AUTORUN") == "1":
            self.go(2)
            self.start_install()
        return GLib.SOURCE_REMOVE

    def go(self, index: int) -> None:
        index = max(0, min(index, len(PAGES) - 1))
        self.page = PAGES[index]
        self.stack.set_visible_child_name(self.page)
        self.sync()

    def check_blocked(self) -> bool:
        return any(not check.ok for check in self.checks)

    def sync(self) -> None:
        running = self.job is not None
        self.back_button.set_visible(self.page == "checks" and not running)
        self.quit_button.set_sensitive(not running)
        self.primary_button.set_visible(self.page != "install" or not running)
        if self.page == "welcome":
            self.primary_button.set_label("Continue")
            self.primary_button.set_sensitive(True)
        elif self.page == "checks":
            self.primary_button.set_label("Preview plan" if self.dry_run else "Install")
            self.primary_button.set_sensitive(not self.check_blocked())
        elif self.page == "install":
            self.primary_button.set_label("Install")
            self.primary_button.set_sensitive(False)
        else:
            self.primary_button.set_label("Close")
            self.primary_button.set_sensitive(True)

    def on_primary(self, _button) -> None:
        if self.page == "welcome":
            self.go(1)
        elif self.page == "checks":
            self.go(2)
            self.start_install()
        else:
            self.close()

    def install_flags(self) -> list[str]:
        flags = ["--source", str(HERE)]
        if not self.build_switch.get_active():
            flags.append("--no-build")
        if not self.font_switch.get_active():
            flags.append("--no-fonts")
        if not self.shortcut_switch.get_active():
            flags.append("--no-shortcut")
        if self.dry_run:
            flags.append("--dry-run")
        return flags

    def start_install(self) -> None:
        self.log_buffer.set_text("")
        self.progress.set_fraction(0.0)
        self.saw_result = False
        self.result_ok = False
        self.result_text = ""
        if self.demo:
            self.phase_label.set_text("Demo run")
            self.append("--- demo: no files will be written")
            threading.Thread(target=self.demo_walk, daemon=True).start()
            return
        argv = [str(BACKEND), "install", *self.install_flags()]
        self.phase_label.set_text("Reading the install plan...")
        self.append("--- " + " ".join(argv))
        self.job = BackendJob(argv, self.on_record, self.on_finished)
        self.sync()
        self.job.start()

    def demo_walk(self) -> None:
        steps = [
            (5, "checking dependencies"),
            (25, "building the release binary"),
            (55, "installing the application"),
            (70, "installing bundled fonts"),
            (85, "creating the application-menu shortcut"),
            (100, "done"),
        ]
        for percent, detail in steps:
            GLib.idle_add(self.on_record, f"STEP|{percent}|install|{detail}")
            GLib.idle_add(self.on_record, f"LOG|demo: {detail}")
            time.sleep(0.35)
        GLib.idle_add(self.on_record, "RESULT|ok|demo: nothing was installed")
        GLib.idle_add(self.on_finished, 0, False)

    def on_record(self, line: str):
        kind, _, rest = line.partition("|")
        if kind == "STEP":
            fields = rest.split("|", 2)
            percent = int(fields[0]) if fields and fields[0].isdigit() else 0
            detail = fields[2] if len(fields) > 2 else rest
            self.progress.set_fraction(percent / 100.0)
            self.phase_label.set_text(detail)
            self.append(f"[{percent:>3}%] {detail}")
        elif kind == "LOG":
            self.append(rest)
        elif kind == "CHECK":
            self.append(rest)
        elif kind == "RESULT":
            fields = rest.split("|", 1)
            self.saw_result = True
            self.result_ok = fields[0] == "ok"
            self.result_text = fields[1] if len(fields) > 1 else ""
            self.append(("OK: " if self.result_ok else "FAILED: ") + self.result_text)
        elif line.strip():
            self.append(line)
        return GLib.SOURCE_REMOVE

    def append(self, text: str) -> None:
        self.log_buffer.insert(self.log_buffer.get_end_iter(), text + "\n", -1)
        self.log_view.scroll_to_iter(self.log_buffer.get_end_iter(), 0.0, False, 0.0, 1.0)

    def on_finished(self, code: int, cancelled: bool):
        self.job = None
        if cancelled:
            self.phase_label.set_text("Cancelled")
            self.sync()
        elif code == 0 and self.saw_result and self.result_ok:
            self.progress.set_fraction(1.0)
            self.phase_label.set_text("Done")
            self.fill_summary()
            self.go(3)
        else:
            self.phase_label.set_text(self.result_text or f"Installer stopped with status {code}")
            self.sync()
        return GLib.SOURCE_REMOVE

    def fill_summary(self) -> None:
        if self.summary_filled:
            return
        self.summary_filled = True
        for label, path, state in install_state():
            row = Adw.ActionRow(title=label, subtitle=path)
            row.set_activatable(False)
            row.add_suffix(Gtk.Label(label=state))
            self.summary_group.add(row)

    def on_close(self, *_args) -> bool:
        if self.job is not None:
            self.job.cancel()
        return False


class SetupApplication(Adw.Application):
    def __init__(self, demo: bool = False, dry_run: bool = False,
                 no_build: bool = False, no_fonts: bool = False, no_shortcut: bool = False):
        super().__init__(application_id=APP_ID)
        self.demo = demo
        self.dry_run = dry_run
        self.no_build = no_build
        self.no_fonts = no_fonts
        self.no_shortcut = no_shortcut

    def do_activate(self) -> None:
        window = SetupWindow(self, self.demo, self.dry_run, self.no_build, self.no_fonts, self.no_shortcut)
        window.present()


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("--demo", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument("--no-fonts", action="store_true")
    parser.add_argument("--no-shortcut", action="store_true")
    args = parser.parse_args()
    app = SetupApplication(args.demo, args.dry_run, args.no_build, args.no_fonts, args.no_shortcut)
    return app.run([sys.argv[0]])


if __name__ == "__main__":
    raise SystemExit(main())
