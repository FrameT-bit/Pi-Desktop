#!/usr/bin/env python3
"""Run the GTK regression fixtures without touching real Pi sessions or APIs."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "dist/qa/2026-11-12"


def contrast(a, b):
    def luminance(color):
        channels = [int(color[i:i + 2], 16) / 255 for i in (1, 3, 5)]
        channels = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in channels]
        return sum(v * weight for v, weight in zip(channels, (0.2126, 0.7152, 0.0722)))
    light, dark = sorted((luminance(a), luminance(b)), reverse=True)
    return (light + 0.05) / (dark + 0.05)


def run(name, scenario, theme):
    for suffix in (".json", ".json.live", ".rpc.jsonl", ".png", ".log"):
        (OUTPUT / f"{name}{suffix}").unlink(missing_ok=True)
    with tempfile.TemporaryDirectory(prefix="pi-ui-", dir="/tmp/opencode") as directory:
        home = Path(directory)
        fonts = home / ".local/share/fonts/pi-desktop"
        fonts.parent.mkdir(parents=True)
        fonts.symlink_to(ROOT / "resources/fonts", target_is_directory=True)
        config = home / "config.ini"
        fixture = ROOT / "tests/fixtures/pi-rpc.py"
        fixture.chmod(0o755)
        config.write_text(f"""[pi]
target=local
bin={fixture}
workspace={home}
session_id=ui-fixture
offline=true
no_extensions=true
no_mcp=true
[ui]
theme=dark
light_theme=oat-milk
dark_theme=midnight-fudge
view=code
session_tabs=false
""")
        sessions = home / ".pi/agent/sessions/--fixture--"
        sessions.mkdir(parents=True)
        entries = [
            {"type": "session", "id": "history-fixture"},
            {"type": "model_change", "provider": "openai", "modelId": "gpt-6.1-sol"},
            {"type": "message", "message": {"role": "user", "content": "Check session metadata"}},
            {"type": "message", "message": {"role": "assistant", "provider": "openai", "model": "gpt-6.1-sol", "timestamp": 1794481200000}},
        ]
        (sessions / "history.jsonl").write_text("\n".join(json.dumps(entry) for entry in entries))
        env = dict(os.environ, HOME=str(home), PI_DESKTOP_CONFIG=str(config), PI_BIN=str(fixture),
                   PI_CODING_AGENT_DIR=str(home / ".pi/agent"), PI_DESKTOP_THEME=theme,
                   PI_DESKTOP_DEMO="qa", PI_DESKTOP_QA_SCENARIO=scenario,
                   PI_DESKTOP_QA_REPORT=str(OUTPUT / f"{name}.json"),
                   PI_DESKTOP_SNAPSHOT=str(OUTPUT / f"{name}.png"), PI_DESKTOP_SNAPSHOT_DELAY="7",
                   PI_DESKTOP_FIXTURE_LOG=str(OUTPUT / f"{name}.rpc.jsonl"))
        if scenario == "reasoning":
            env["PI_DESKTOP_SNAPSHOT_TARGET"] = "popover"
        if scenario == "settings":
            env["PI_DESKTOP_SNAPSHOT_TARGET"] = "dialog"
        binary = os.environ.get("PI_DESKTOP_QA_BIN", str(ROOT / "target/debug/pi-desktop"))
        result = subprocess.run([binary], cwd=ROOT, env=env,
                                capture_output=True, text=True, timeout=40)
        (OUTPUT / f"{name}.log").write_text(result.stderr)
        assert result.returncode == 0, result.stderr
        assert "Theme parser error" not in result.stderr, result.stderr
        assert "CRITICAL" not in result.stderr, result.stderr
        report = json.loads((OUTPUT / f"{name}.json").read_text())
        assert not report["raw_snapshot_visible"]
        assert not report["connected_visible"]
        assert report["tabs_visible"] == (scenario == "tabs")
        assert report["settings_visible"]
        assert not report["horizontal_overflow"], report
        themes = report["theme_round_trip"]
        assert themes["before"] == themes["restored"] != themes["opposite"]
        sizes = report["rail_resizes"]
        assert sizes[1]["actual"] > sizes[0]["actual"] + 150, sizes
        assert all(abs(item["actual"] - item["wanted"]) <= 5 for item in sizes), sizes
        assert report["drag_handler"]["after"] >= report["drag_handler"]["before"] + 100, report["drag_handler"]
        ratio = contrast(report["code_foreground"], report["card_background"])
        assert ratio >= 4.5, ratio
        heights = [item["bounds"][3] for item in report["controls"] if "model-chip" in item["classes"] or "mode-switch" in item["classes"]]
        assert len(heights) == 2 and max(heights) - min(heights) <= 1, heights
        for item in report["controls"]:
            if "mode-switch" in item["classes"]:
                continue
            assert item["bounds"][2] >= 44 and item["bounds"][3] >= 44, item
        if scenario == "reasoning":
            assert any(item["mapped"] for item in report["popovers"]), report["popovers"]
            for item in report["popovers"]:
                if item["mapped"]:
                    assert item["bounds"][1] >= 0
                    assert item["bounds"][1] + item["bounds"][3] <= report["window"][1], item
            requests = [json.loads(line) for line in (OUTPUT / f"{name}.rpc.jsonl").read_text().splitlines()]
            assert any(request.get("type") == "set_thinking_level" and request.get("level") == "low" for request in requests)
        print(f"{name}: PASS · card contrast {ratio:.2f}:1 · rail {sizes[0]['actual']}→{sizes[1]['actual']} px · header {heights}")


if __name__ == "__main__":
    OUTPUT.mkdir(parents=True, exist_ok=True)
    subprocess.run(["cargo", "build", "--quiet"], cwd=ROOT, check=True)
    for name, scenario, theme in [
        ("code-dark-1280", "code", "midnight-fudge"),
        ("code-light-1280", "code", "oat-milk"),
        ("chat-1280", "chat", "midnight-fudge"),
        ("optional-tabs-1280", "tabs", "midnight-fudge"),
        ("reasoning-390", "reasoning", "midnight-fudge"),
        ("personalization", "settings", "oat-milk"),
    ]:
        run(name, scenario, theme)
    print(f"6 native UI scenarios passed. PNGs and measured reports: {OUTPUT}")
