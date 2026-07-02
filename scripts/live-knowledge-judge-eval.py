#!/usr/bin/env python3
"""Deprecated wrapper for the Rust Mind live accepted-knowledge judge eval.

The previous implementation in this path hand-parsed NOTA replies with regular
expressions. The supported harness is now the Rust
`mind-live-knowledge-judge-eval` binary, which parses replies through
`nota_next` and generated `signal-mind` types.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def main() -> int:
    repository = Path(__file__).resolve().parents[1]
    binary = repository / "target" / "debug" / "mind-live-knowledge-judge-eval"
    if binary.exists():
        command = [str(binary), *sys.argv[1:]]
    else:
        command = [
            "cargo",
            "run",
            "--bin",
            "mind-live-knowledge-judge-eval",
            "--",
            *sys.argv[1:],
        ]
    os.execvp(command[0], command)
    return 127


if __name__ == "__main__":
    raise SystemExit(main())
