#!/usr/bin/env python3
"""Run native checks on a private compositor, never on the operator's desktop."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent


def main():
    for program in ("mutter", "dbus-run-session"):
        if not shutil.which(program):
            raise SystemExit(f"Headless checks require {program}; no visible-desktop fallback is allowed.")
    logs = ROOT / ".tools"
    logs.mkdir(exist_ok=True)
    base = "/tmp/opencode" if Path("/tmp/opencode").is_dir() else None
    with tempfile.TemporaryDirectory(prefix="pi-headless-", dir=base) as runtime:
        # No activation directories: portal/GVfs daemons otherwise create FUSE
        # mounts in the throwaway runtime and can outlive the test client.
        bus_config = Path(runtime) / "session.conf"
        bus_config.write_text(f"""<busconfig>
<type>session</type>
<listen>unix:tmpdir={runtime}</listen>
<auth>EXTERNAL</auth>
<policy context="default">
<allow own="*"/>
<allow send_destination="*"/>
<allow receive_sender="*"/>
</policy>
</busconfig>
""")
        env = dict(os.environ, XDG_RUNTIME_DIR=runtime, PI_DESKTOP_QA_ISOLATED="1",
                   GDK_BACKEND="wayland", LIBGL_ALWAYS_SOFTWARE="1",
                   GALLIUM_DRIVER="llvmpipe", GIO_USE_VFS="local",
                   GSETTINGS_BACKEND="memory", GTK_USE_PORTAL="0")
        for key in ("DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS", "WAYLAND_SOCKET"):
            env.pop(key, None)
        command = [
            "dbus-run-session", "--config-file", str(bus_config), "--", "mutter", "--wayland", "--headless",
            "--no-x11", "--virtual-monitor=1920x1080", "--wayland-display=pi-qa",
            "--", sys.executable, str(ROOT / "tools/verify-ui.py"), *sys.argv[1:],
        ]
        with (logs / "qa-headless.log").open("w") as log:
            result = subprocess.run(command, cwd=ROOT, env=env, stderr=log)
        if result.returncode:
            print(f"Headless checks failed; diagnostics: {logs / 'qa-headless.log'}", file=sys.stderr)
        return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
