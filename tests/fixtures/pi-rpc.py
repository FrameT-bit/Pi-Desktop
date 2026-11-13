#!/usr/bin/env python3
"""Offline RPC fixture for native UI checks. Never calls a model or a tool."""
import json
import os
import sys

if "--mode" not in sys.argv:
    print("[]")
    raise SystemExit(0)

model = {"provider": "openai", "id": "gpt-6.1-sol", "name": "GPT-6.1 Sol", "contextWindow": 200000}
level = "high"
for line in sys.stdin:
    request = json.loads(line)
    command = request.get("type", "")
    if command == "set_thinking_level":
        level = request["level"]
    data = {
        "get_state": {"sessionId": "ui-fixture", "sessionName": "UI regression checks", "isStreaming": False, "model": model, "thinkingLevel": level},
        "get_available_models": {"models": [model]},
        "get_available_thinking_levels": {"levels": ["off", "low", "medium", "high", "xhigh", "max"]},
        "get_messages": {"messages": []},
        "get_commands": {"commands": []},
        "get_fork_messages": {"messages": []},
        "get_session_stats": {"contextUsage": {"percent": 24, "tokens": 48000, "contextWindow": 200000}},
    }.get(command, {})
    if log := os.environ.get("PI_DESKTOP_FIXTURE_LOG"):
        with open(log, "a") as stream:
            stream.write(json.dumps(request) + "\n")
    print(json.dumps({"type": "response", "id": request.get("id"), "command": command, "success": True, "data": data}), flush=True)
