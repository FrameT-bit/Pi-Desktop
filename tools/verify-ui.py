#!/usr/bin/env python3
"""Run the GTK regression fixtures without touching real Pi sessions or APIs."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import argparse
import tomllib
import shutil
import sys
import struct

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "dist/qa" / tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
TEMP_ROOT = None
FONT_SCALE = 100
REAL_AGENT_DIR = Path.home() / ".pi/agent"


def git_fixture(repo, scenario):
    def git(*args, success=True):
        result = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)
        assert (result.returncode == 0) == success, result.stderr
    git("init", "-b", "main")
    git("config", "user.name", "UI Fixture")
    git("config", "user.email", "fixture@example.invalid")
    git("config", "commit.gpgsign", "false")
    if scenario == "git-unborn":
        (repo / "a -> b.txt").write_text("new file\n")
        git("add", "-A")
        return
    if scenario == "git-large":
        (repo / "large.txt").write_text("before\n")
        git("add", "-A")
        git("commit", "-m", "fixture")
        (repo / "large.txt").write_text("long line " * 200 + "\n" + "new line\n" * 4999)
        return
    for name in ("conflict.txt", "partial.txt", "old.txt"):
        (repo / name).write_text("base\n")
    git("add", "-A")
    git("commit", "-m", "fixture")
    if scenario == "git-clean":
        return
    git("switch", "-c", "other")
    (repo / "conflict.txt").write_text("other\n")
    git("commit", "-am", "other")
    git("switch", "main")
    (repo / "conflict.txt").write_text("main\n")
    git("commit", "-am", "main")
    if scenario not in ("git-actions", "git-workbench", "git-history", "git-390", "git-ux"):
        git("merge", "other", success=False)
    if scenario == "git-recovery":
        return
    git("mv", "old.txt", "renamed -> result.txt")
    (repo / "partial.txt").write_text("staged\n")
    git("add", "partial.txt")
    (repo / "partial.txt").write_text("unstaged\n")
    (repo / "a -> b.txt").write_text("new file\n")
    if scenario == "git-workbench":
        (repo / "leave-me.txt").write_text("do not stage me\n")


def contrast(a, b):
    def luminance(color):
        channels = [int(color[i:i + 2], 16) / 255 for i in (1, 3, 5)]
        channels = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in channels]
        return sum(v * weight for v, weight in zip(channels, (0.2126, 0.7152, 0.0722)))
    light, dark = sorted((luminance(a), luminance(b)), reverse=True)
    return (light + 0.05) / (dark + 0.05)

def check_inline_plan(state, labels=("Execute", "Stay in plan", "Refine")):
    assert state["menus"] == [] and len(state["tags"]) == 1, state
    assert state["border"] == [1, 1, 1, 1] and state["tags"][0]["border"] == [0, 0, 0, 0], state
    assert [item["label"] for item in state["buttons"]] == list(labels), state
    x, y, width, height = state["bar"]
    assert height == 48, state
    tag_x, tag_y, tag_width, tag_height = state["tags"][0]["bounds"]
    assert tag_x >= x and tag_y >= y and tag_y + tag_height <= y + height and tag_height < 44, state
    right = tag_x + tag_width
    for item in state["buttons"]:
        bx, by, bw, bh = item["bounds"]
        assert item["border"] == [0, 0, 0, 0] and bw >= 44 and bh >= 44, state
        assert bx >= right and bx + bw <= x + width and by >= y and by + bh <= y + height, state
        right = bx + bw
    assert contrast(state["tag_foreground"], state["tag_background"]) >= 4.5, state


def run(name, scenario, theme):
    for suffix in (".json", ".json.live", ".rpc.jsonl", ".png", ".log"):
        (OUTPUT / f"{name}{suffix}").unlink(missing_ok=True)
    with tempfile.TemporaryDirectory(prefix="pi-ui-", dir=TEMP_ROOT) as directory:
        home = Path(directory)
        fonts = home / ".local/share/fonts/pi-desktop"
        fonts.parent.mkdir(parents=True)
        fonts.symlink_to(ROOT / "resources/fonts", target_is_directory=True)
        config = home / "config.ini"
        workspace = home / "workspace"
        workspace.mkdir()
        if scenario == "git-repository":
            external = home / "external-repo"
            external.mkdir()
            git_fixture(external, "git-actions")
            (external / "folder with spaces").mkdir()
        elif scenario.startswith("git"):
            git_fixture(workspace, scenario)
        fixture = ROOT / "tests/fixtures/pi-rpc.py"
        fixture.chmod(0o755)
        config.write_text(f"""[pi]
target=local
bin={fixture}
workspace={workspace}
session_id=ui-fixture
offline=true
no_extensions=true
no_mcp=true
system_prompt=Keep the platform stable.\\nKeep the platform stable.
append_system_prompt=Use clear language.\\nUse clear language.
[ui]
theme=dark
light_theme=oat-milk
dark_theme=midnight-fudge
view=code
session_tabs=false
font_scale={FONT_SCALE}
""")
        live = scenario in ("live-stream", "live-wait")
        if live:
            binary_pi = shutil.which("pi")
            assert binary_pi, "pi is not on PATH"
            config.write_text(config.read_text().replace(f"bin={fixture}", f"bin={binary_pi}") +
                              "\n[pi]\nmodel=openai/gpt-6.1-sol\nno_tools=true\nno_session=true\nno_context_files=true\nno_skills=true\n")
        sessions = home / ".pi/agent/sessions/--fixture--"
        sessions.mkdir(parents=True)
        entries = [
            {"type": "session", "id": "history-fixture"},
            {"type": "model_change", "provider": "openai", "modelId": "gpt-6.1-sol"},
            {"type": "message", "message": {"role": "user", "content": "Check session metadata"}},
            {"type": "message", "message": {"role": "assistant", "provider": "openai", "model": "gpt-6.1-sol", "timestamp": 1794481200000}},
        ]
        (sessions / "history.jsonl").write_text("\n".join(json.dumps(entry) for entry in entries))
        memory = home / ".pi/agent/memory"
        memory.mkdir()
        original = "- Prefer Rust.\n- Prefer Rust."
        (memory / "USER.md").write_text(original)
        (memory / "MEMORY.md").write_text(original)
        env = dict(os.environ, HOME=str(home), PI_DESKTOP_CONFIG=str(config), PI_BIN=str(fixture),
                   PI_CODING_AGENT_DIR=str(home / ".pi/agent"), PI_DESKTOP_THEME=theme,
                   PI_DESKTOP_DEMO="qa", PI_DESKTOP_QA_SCENARIO=scenario,
                   PI_DESKTOP_QA_REPORT=str(OUTPUT / f"{name}.json"),
                    PI_DESKTOP_SNAPSHOT=str(OUTPUT / f"{name}.png"),
                    PI_DESKTOP_SNAPSHOT_DELAY="11" if scenario in ("stream", "thinking", "wait") else "8",
                    PI_DESKTOP_FIXTURE_LOG=str(OUTPUT / f"{name}.rpc.jsonl"))
        if live:
            env.update(PI_BIN=binary_pi, PI_CODING_AGENT_DIR=str(REAL_AGENT_DIR),
                       PI_DESKTOP_SNAPSHOT_DELAY="65", PI_DESKTOP_SNAPSHOT_ON_REPORT="1")
        if scenario in ("git-workbench", "git-history", "git-390", "git-ux", "git-clean") or scenario.startswith("plan-"):
            env.update(PI_DESKTOP_SNAPSHOT_DELAY="25", PI_DESKTOP_SNAPSHOT_ON_REPORT="1")
        if scenario == "git-repository":
            env.update(PI_DESKTOP_SNAPSHOT_DELAY="25", PI_DESKTOP_SNAPSHOT_ON_REPORT="1",
                       PI_DESKTOP_QA_PICK_REPOSITORY=str(external / "folder with spaces"))
        if scenario in ("reasoning", "models"):
            env["PI_DESKTOP_SNAPSHOT_TARGET"] = "popover"
        if scenario in ("settings", "viewer", "updates", "git-390", "plan-single-390") or scenario.startswith("compact-"):
            env["PI_DESKTOP_SNAPSHOT_TARGET"] = "dialog"
        if scenario.startswith("compact-"):
            env["PI_FIXTURE_COMPACT"] = "1"
        binary = os.environ.get("PI_DESKTOP_QA_BIN", str(ROOT / "target/debug/pi-desktop"))
        if scenario == "geometry":
            env.pop("PI_DESKTOP_SNAPSHOT", None)
            reports = []
            for step in ("save", "restore", "max"):
                env["PI_DESKTOP_QA_GEOMETRY_PASS"] = step
                result = subprocess.run([binary, "--workspace", str(workspace)], cwd=ROOT, env=env,
                                        capture_output=True, text=True, timeout=20)
                assert result.returncode == 0 and "CRITICAL" not in result.stderr, result.stderr
                reports.append(json.loads((OUTPUT / f"{name}.json").read_text()))
            assert reports[0] == reports[1] == {"size":[1100,720], "maximized":False, "rail_width":460, "rail_open":False}, reports
            assert reports[2]["maximized"] and reports[2]["rail_width"] == 460 and not reports[2]["rail_open"], reports
            (OUTPUT / f"{name}.json").write_text(json.dumps(reports, indent=2))
            print(f"{name}: PASS · three launches · size, maximize and rail persisted")
            return
        try:
            result = subprocess.run([binary], cwd=ROOT, env=env,
                                    capture_output=True, text=True, timeout=90 if live else 40)
        except subprocess.TimeoutExpired as error:
            stderr = error.stderr or b""
            (OUTPUT / f"{name}.log").write_text(stderr.decode(errors="replace") if isinstance(stderr, bytes) else stderr)
            raise
        (OUTPUT / f"{name}.log").write_text(result.stderr)
        assert result.returncode == 0, f"exit {result.returncode}: {result.stderr}"
        assert "Theme parser error" not in result.stderr, result.stderr
        assert "CRITICAL" not in result.stderr, result.stderr
        report = json.loads((OUTPUT / f"{name}.json").read_text())
        feature = report["feature"]
        if scenario in ("stream", "thinking", "wait", "live-stream", "live-wait"):
            assert feature["early"] == (scenario not in ("wait", "live-wait")), feature
            assert feature["running_early"] and not feature["running_final"], feature
            assert feature["final"] and not feature["cursor_final"], feature
            if scenario in ("stream", "live-stream"):
                assert feature["phase"] == "WRITING", feature
        elif scenario == "tools":
            assert feature["icons"] == 9 and feature["running"] == 1, feature
            assert all(name in feature["names"] for name in ("read", "write", "bash", "todo")), feature
        elif scenario == "status":
            assert feature == {"active_done":True, "background_done":True, "closed":True,
                               "reopened":True, "running_after_select":False}, feature
        elif scenario == "git-ux":
            assert feature["blank_blocked"] and feature["ready"] and feature["cleared_blocked"], feature
            assert feature["bulk_label"] == "Stage shown", feature
            assert feature["staged_shown"]["staged"] == 3 and feature["unstaged_shown"]["staged"] == 2, feature
            assert feature["partial_index"] == "staged\n" and feature["partial_worktree"] == "unstaged\n", feature
            assert feature["no_matches"] and len(feature["restored"]["visible_files"][0]) == 2, feature
            assert feature["menu_open"] and feature["restored"]["group_titles"] == ["Unstaged · 2", "Staged · 2"], feature
            assert feature["restored"]["summary"] == "Local branch · not published", feature
        elif scenario == "git-clean":
            assert feature["initial"]["count"] == 0 and not feature["initial"]["commit_enabled"], feature
            assert "Working tree clean" in feature["message"] and feature["history"]["commits"] == 1, feature
        elif scenario == "git-repository":
            assert feature["initial"]["branch"] is None and not feature["initial"]["workbench_visible"], feature
            assert feature["opened"]["branch"] == "main" and feature["opened"]["pinned"], feature
            assert feature["opened"]["repository"] == str(external), feature
            for key in ("independent", "cancelled", "invalid", "during_operation", "reopened", "after_navigation"):
                assert feature[key]["repository"] == str(external) and feature[key]["pinned"], (key, feature)
            assert "Could not open repository" in feature["invalid"]["status"], feature
            assert not feature["reset"]["pinned"] and feature["reset"]["repository"] == str(workspace), feature
            assert feature["reset"]["branch"] is None and not feature["reset"]["workbench_visible"], feature
            assert feature["draft"] == "Draft for external repository", feature
            assert feature["staged"]["staged"] == 3, feature
            assert feature["workspace_untouched"] and feature["chat_workspace_unchanged"], feature
        elif scenario.startswith("git"):
            assert feature["state"]["branch"] == "main", feature
            assert feature["state"]["count"] == (1 if scenario in ("git-unborn", "git-large", "git-recovery") else 3 if scenario in ("git-actions", "git-history", "git-390") else 4), feature
            if scenario == "git-unborn":
                assert feature["state"]["staged"] == 1, feature
            elif scenario == "git-large":
                assert feature["diff_rows"] == 2000 and feature["truncated"], feature
                assert feature["horizontal_scroll"] and feature["unellipsized"], feature
            elif scenario == "git-actions":
                assert feature["staged_all"]["staged"] == 3, feature
                assert feature["committed"]["count"] == 0 and feature["committed"]["selected"] is None, feature
                assert feature["empty_enter_status"] == feature["committed"]["status"], feature
            elif scenario == "git-recovery":
                assert feature["aborted"]["operation"] is None and feature["aborted"]["count"] == 0, feature
                assert feature["switched"]["branch"] == "other", feature
            elif scenario == "git-workbench":
                flow = feature["workbench"]
                assert flow["initial"]["selected"] is not None, flow
                assert sorted(flow["multi"]["selected_files"][0]) == ["a -> b.txt", "partial.txt"], flow
                assert flow["staged"]["staged"] == 3, flow
                assert flow["staged"]["visible_files"][0] == ["leave-me.txt"], flow
                assert flow["staged"]["selected"] == flow["polled"]["selected"], flow
                assert flow["unstaged"]["staged"] == 1, flow
                assert flow["filtered"]["visible_files"] == [["partial.txt"], []], flow
                assert flow["empty"]["visible_files"] == [[], []] and flow["empty"]["selected"] is None, flow
                assert len(flow["cleared"]["visible_files"][0]) == 3, flow
                assert flow["stage_key"] and flow["unstage_key"] and feature["filter_shortcut"], feature
            elif scenario == "git-history":
                history = feature["history"]
                assert history["initial"]["commits"] == 3 and history["initial"]["files"] == 1, history
                assert history["selected"]["files"] == 3 and "fixture" in history["selected"]["message"], history
                assert history["selected"]["visible_changes"] > 0, history
                assert history["initial"]["selected"] != history["selected"]["selected"], history
                assert history["empty"]["selected"] is None and history["empty"]["files"] == 0 and history["empty"]["visible"] == 0, history
                assert history["filtered"]["selected"] == history["selected"]["selected"] and history["filtered"]["visible"] == 1, history
                assert history["read_only"], history
            elif scenario == "git-390":
                narrow = feature["narrow"]
                assert narrow["width"] == 390 and narrow["state"]["vertical"], narrow
                assert narrow["state"]["selected"] is not None, narrow
                assert narrow["bounds"] == [0, 0, 390, narrow["height"]], narrow
                for x, y, width, height in narrow["controls"]:
                    assert width >= 44 and height >= 44 and x >= 0 and x + width <= 390 and y + height <= narrow["height"], narrow
            else:
                assert feature["partial_rows"] == 2 and "conflict" in feature["labels"], feature
                assert "a -> b.txt" in feature["labels"] and "renamed -> result.txt" in feature["labels"], feature
            assert feature["diff_rows"] > 0, feature
            geometry = feature["geometry"]
            assert contrast(geometry["foreground"], geometry["background"]) >= 4.5, geometry
            assert contrast(geometry["foreground"], geometry["selected_background"]) >= 4.5, geometry
            for x, y, width, height in geometry["controls"]:
                assert width >= 44 and height >= 44 and x >= 0 and x + width <= report["window"][0] and y + height <= report["window"][1], geometry
        elif scenario == "plan-bar":
            for key in ("both", "remaining_b", "pending", "rail_visible", "counter"):
                assert feature[key], feature
            assert feature["items"] == 12 and feature["current"] == 1, feature
            assert feature["bar_before"] == feature["bar_after"], feature
            check_inline_plan(feature["single"])
            check_inline_plan(feature["remaining"])
            assert feature["multiple"]["buttons"] == [] and len(feature["multiple"]["menus"]) == 2, feature
            assert feature["multiple"]["menu_borders"] == [[0, 0, 0, 0]] * 2, feature
            assert feature["single"]["bar"] == feature["multiple"]["bar"] == feature["remaining"]["bar"] == feature["bar_before"], feature
            requests = [json.loads(line) for line in (OUTPUT / f"{name}.rpc.jsonl").read_text().splitlines()]
            assert any(r.get("id") == "plan-a" and r.get("value") == "Stay in plan mode" for r in requests), requests
        elif scenario == "plan-single":
            check_inline_plan(feature["initial"])
            check_inline_plan(feature["confirm"], ("Execute", "Stay in plan"))
            check_inline_plan(feature["final"])
            assert feature["before"] == feature["initial"]["bar"] == feature["confirm"]["bar"] == feature["final"]["bar"], feature
            assert feature["select_answered"] and feature["confirm_answered"], feature
            requests = [json.loads(line) for line in (OUTPUT / f"{name}.rpc.jsonl").read_text().splitlines()]
            assert any(r.get("id") == "plan-single-select" and r.get("value") == "Refine the plan" for r in requests), requests
            assert any(r.get("id") == "plan-single-confirm" and r.get("confirmed") is False for r in requests), requests
        elif scenario == "plan-single-390":
            assert [feature["width"], feature["height"]] == [390, 48], feature
            check_inline_plan(feature["plan"])
            assert struct.unpack(">II", (OUTPUT / f"{name}.png").read_bytes()[16:24]) == (390, 48)
        elif scenario == "models":
            assert feature == {"focused": True, "text": "test/fixture-cheap", "matches": 1}, feature
        elif scenario in ("gallery", "viewer"):
            assert feature["next"][2] >= 44 and feature["next"][3] >= 44, feature
            if scenario == "gallery":
                assert feature["page_two"] and feature["window_width"] == 1280, feature
                x, _, width, _ = feature["gallery"]
                assert x >= 0 and x + width <= 1280, feature
            else:
                assert feature["width"] == 390, feature
                x, y, width, height = feature["next"]
                assert x >= 0 and x + width <= 390 and y + height <= feature["height"], feature
        elif scenario.startswith("compact-"):
            assert feature == {"applied": True}, feature
            requests = [json.loads(line) for line in (OUTPUT / f"{name}.rpc.jsonl").read_text().splitlines()]
            models = [request["model"] for request in requests if request["type"] == "fixture_compact"]
            assert models == ["test/fixture-cheap"] * (2 if scenario == "compact-memory" else 1), models
            assert (memory / "USER.md").read_text() == original
            assert (memory / "MEMORY.md").read_text() == original
        elif scenario == "updates":
            assert feature == {"release_link": True}, feature
        assert not report["raw_snapshot_visible"]
        assert report["app_version"] == tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"], report
        assert report["footer_version"] == f"Pi Desktop {report['app_version']}", report
        assert not report["connected_visible"]
        assert report["tabs_visible"] == (scenario == "tabs")
        assert report["settings_visible"]
        assert not report["horizontal_overflow"], report
        assert all(border == [0, 0, 0, 0] for border in report["header_borders"]), report["header_borders"]
        assert report["header_outer_border"] == [1, 1, 1, 1], report["header_outer_border"]
        for icon in report["header_icons"]:
            size = 16 if icon["detail"] else 24
            assert icon["bounds"][2:] == [size, size], icon
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
    parser = argparse.ArgumentParser()
    parser.add_argument("scenarios", nargs="*")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--version")
    parser.add_argument("--temp-root", type=Path)
    parser.add_argument("--font-scale", type=int, default=100)
    parser.add_argument("--live-updates", action="store_true")
    parser.add_argument("--live-stream", action="store_true", help="Two ephemeral calls to the configured OpenAI provider; no tools or saved sessions")
    parser.add_argument("--interactive", action="store_true", help="Explicitly allow windows on the current desktop instead of a private compositor")
    args = parser.parse_args()
    if not args.interactive and os.environ.get("PI_DESKTOP_QA_ISOLATED") != "1":
        raise SystemExit(subprocess.call([sys.executable, str(ROOT / "tools/verify-ui-headless.py"), *sys.argv[1:]]))
    if args.output:
        OUTPUT = args.output.resolve()
    elif args.version:
        assert args.version.replace(".", "").replace("-", "").isalnum(), "invalid version"
        OUTPUT = ROOT / "dist/qa" / args.version
    TEMP_ROOT = args.temp_root
    FONT_SCALE = args.font_scale
    OUTPUT.mkdir(parents=True, exist_ok=True)
    subprocess.run(["cargo", "build", "--quiet"], cwd=ROOT, check=True)
    cases = [
        ("code-dark-1280", "code", "midnight-fudge"),
        ("code-light-1280", "code", "oat-milk"),
        ("chat-1280", "chat", "midnight-fudge"),
        ("optional-tabs-1280", "tabs", "midnight-fudge"),
        ("reasoning-390", "reasoning", "midnight-fudge"),
        ("personalization", "settings", "oat-milk"),
        ("model-search", "models", "midnight-fudge"),
        ("gallery-1280", "gallery", "midnight-fudge"),
        ("viewer-390", "viewer", "midnight-fudge"),
        ("compact-memory", "compact-memory", "midnight-fudge"),
        ("compact-personality", "compact-personality", "midnight-fudge"),
        ("compact-system", "compact-system", "midnight-fudge"),
        ("git-dark-1280", "git", "midnight-fudge"),
        ("git-light-1280", "git", "oat-milk"),
        ("git-unborn", "git-unborn", "midnight-fudge"),
        ("git-actions", "git-actions", "midnight-fudge"),
        ("git-large", "git-large", "midnight-fudge"),
        ("git-recovery", "git-recovery", "midnight-fudge"),
        ("git-workbench", "git-workbench", "midnight-fudge"),
        ("git-history", "git-history", "midnight-fudge"),
        ("git-390", "git-390", "midnight-fudge"),
        ("git-repository", "git-repository", "midnight-fudge"),
        ("git-ux", "git-ux", "midnight-fudge"),
        ("git-clean", "git-clean", "midnight-fudge"),
        ("stream", "stream", "midnight-fudge"),
        ("thinking", "thinking", "midnight-fudge"),
        ("wait", "wait", "midnight-fudge"),
        ("plan-bar", "plan-bar", "midnight-fudge"),
        ("plan-single", "plan-single", "midnight-fudge"),
        ("plan-single-light", "plan-single", "oat-milk"),
        ("plan-single-390", "plan-single-390", "midnight-fudge"),
        ("status", "status", "midnight-fudge"),
        ("geometry", "geometry", "midnight-fudge"),
        ("tools", "tools", "midnight-fudge"),
    ]
    if args.live_updates:
        cases.append(("updates", "updates", "midnight-fudge"))
    if args.live_stream:
        cases.extend([("live-stream", "live-stream", "midnight-fudge"), ("live-wait", "live-wait", "midnight-fudge")])
    selected = set(args.scenarios)
    cases = [case for case in cases if not selected or case[1] in selected]
    assert cases, "no matching scenarios"
    for name, scenario, theme in cases:
        run(name, scenario, theme)
    print(f"{len(cases)} native UI scenarios passed. PNGs and measured reports: {OUTPUT}")
