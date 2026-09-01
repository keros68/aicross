#!/usr/bin/env python3
# aicross Hook Forwarder
#
# Registered in Claude Code settings only after explicit consent.
# Receives hook events on stdin (Claude Code format), maps them to
# aicross's 3-state agent status, and forwards a compact JSON payload
# to the aicross backend over local TCP.
#
# Single payload kind emitted (one TCP connection per event):
#   status   {tab_id, tool, status, event}
#            Drives the tab status dot. Every relevant hook fires this.
#
# Per-tool file-edit attribution was removed in v2.7.x — ChangesBoard
# now sources from a folder snapshot diff (`compute_folder_stats`),
# which is tool-agnostic by construction. So nothing in this script
# emits `path` / `action` anymore.
#
# Env vars (injected by aicross when spawning Claude in a tab):
#   AICROSS_TAB_ID    — tab/session UUID the agent belongs to
#   AICROSS_HOOK_PORT — loopback port of the Rust hook server
#   AICROSS_TOOL      — always "claude" today (only Claude is supported)
#
# Exit 0 silently on any error. A flaky hook must never block the agent.

import json
import os
import socket
import sys


def post(port: int, payload: dict) -> None:
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(1.0)
        s.connect(("127.0.0.1", port))
        s.sendall((json.dumps(payload) + "\n").encode("utf-8"))
        try:
            s.recv(256)
        except Exception:
            pass
        s.close()
    except Exception:
        pass


def main() -> None:
    try:
        data = json.load(sys.stdin)
    except Exception:
        sys.exit(0)

    tab_id = os.environ.get("AICROSS_TAB_ID")
    port = os.environ.get("AICROSS_HOOK_PORT")
    tool = os.environ.get("AICROSS_TOOL", "")
    if not tab_id or not port:
        sys.exit(0)
    try:
        port_n = int(port)
    except Exception:
        sys.exit(0)

    event = data.get("hook_event_name", "")
    status = None

    if event in ("Stop", "StopFailure"):
        status = "idle"
    elif event == "Notification":
        # Claude Code may expose the notification subtype under different keys
        ntype = (
            data.get("notification_type")
            or data.get("type")
            or (data.get("notification") or {}).get("type")
        )
        if ntype == "permission_prompt":
            status = "wait_input"
        elif ntype == "idle_prompt":
            status = "idle"
    else:
        # UserPromptSubmit / PreToolUse / PostToolUse / SubagentStart /
        # PreCompact / etc. — anything not explicitly idle or waiting means
        # Claude is busy. One bucket, one color (orange).
        status = "working"

    if status is not None:
        post(port_n, {"tab_id": tab_id, "tool": tool, "status": status, "event": event})

    sys.exit(0)


if __name__ == "__main__":
    main()
