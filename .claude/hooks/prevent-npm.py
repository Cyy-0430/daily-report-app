#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Prevent npm usage — remind to use pnpm.

PreToolUse hook for the Bash tool. When Claude Code tries to run an ``npm``
command, this hook blocks the call (exit code 2) and feeds a reminder through
stderr so Claude switches to pnpm instead.

Exits 0 (allow) for everything else: non-Bash tools, commands without npm,
malformed input — never block on something we can't understand.

Note: ``pnpm`` / ``npm_config`` / ``npmjs`` are NOT matched — only ``npm`` as a
standalone command token (preceded by start/separator, followed by space/end).
"""
from __future__ import annotations

import json
import re
import sys

# Force UTF-8 on Windows so the Chinese reminder survives stderr output.
if sys.platform.startswith("win"):
    for _stream in (sys.stdout, sys.stderr):
        if hasattr(_stream, "reconfigure"):
            _stream.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[union-attr]

# `npm` as a command token: preceded by start or a non-word char (excludes the
# leading `p` in `pnpm`), followed by whitespace or end (excludes `npm_config`,
# `npmjs`). MULTILINE so `^` also matches after embedded newlines.
NPM_RE = re.compile(r"(?:^|[^\w-])npm(?:\s|$)", re.MULTILINE)

REMINDER = (
    "检测到 npm 命令。本项目统一使用 pnpm，请改用 pnpm 执行。\n"
    "常用对应关系：\n"
    "  npm install        -> pnpm install\n"
    "  npm install -D x   -> pnpm add -D x\n"
    "  npm run dev        -> pnpm dev\n"
    "  npm run <script>   -> pnpm <script>\n"
    "  npx <pkg>          -> pnpm dlx <pkg>   （或 pnpm exec <pkg>）"
)


def main() -> None:
    try:
        data = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        sys.exit(0)

    # The PreToolUse matcher already restricts to Bash; stay defensive anyway.
    tool_name = data.get("tool_name") or data.get("toolName") or ""
    if tool_name.lower() != "bash":
        sys.exit(0)

    command = data.get("tool_input", {}).get("command")
    if not isinstance(command, str):
        sys.exit(0)

    if NPM_RE.search(command):
        # Exit code 2 blocks the tool call; stderr is fed back to Claude.
        print(REMINDER, file=sys.stderr)
        sys.exit(2)

    sys.exit(0)


if __name__ == "__main__":
    main()
