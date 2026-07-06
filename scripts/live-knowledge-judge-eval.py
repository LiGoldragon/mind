#!/usr/bin/env python3
"""Deprecated wrapper for the Rust Mind live accepted-knowledge judge eval.

The previous implementation in this path hand-parsed NOTA replies with regular
expressions. The supported harness is now the Rust
`mind-live-knowledge-judge-eval` binary, which parses replies through
`nota` and generated `signal-mind` types.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def option_value(arguments: list[str], name: str) -> str | None:
    prefix = f"--{name}="
    for index, argument in enumerate(arguments):
        if argument.startswith(prefix):
            return argument[len(prefix) :]
        if argument == f"--{name}" and index + 1 < len(arguments):
            return arguments[index + 1]
    return None


def main() -> int:
    repository = Path(__file__).resolve().parents[1]
    arguments = sys.argv[1:]
    if (
        option_value(arguments, "agent-daemon") is None
        and option_value(arguments, "agent-configuration-writer") is None
    ):
        agent_repository = Path(
            option_value(arguments, "agent-repository")
            or "/git/github.com/LiGoldragon/agent"
        )
        subprocess.run(
            [
                os.environ.get("CARGO", "cargo"),
                "build",
                "--manifest-path",
                str(agent_repository / "Cargo.toml"),
                "--features",
                "live-provider",
                "--bins",
            ],
            check=True,
        )
    command = [
        os.environ.get("CARGO", "cargo"),
        "run",
        "--manifest-path",
        str(repository / "Cargo.toml"),
        "--features",
        "eval-fixture-prepopulation",
        "--bin",
        "mind-live-knowledge-judge-eval",
        "--",
        *arguments,
    ]
    os.execvp(command[0], command)
    return 127


if __name__ == "__main__":
    raise SystemExit(main())
