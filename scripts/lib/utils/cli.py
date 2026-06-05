from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path
from typing import Callable


class CliError(Exception):
    pass


Command = Callable[[list[str]], int | None]


def die(message: str, code: int = 1) -> int:
    print(message, file=os.sys.stderr)
    return code


def dispatch(
    argv: list[str] | None,
    *,
    usage: Callable[[], None],
    commands: dict[str, Command],
    name: str,
) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if not args or args[0] in {"-h", "--help", "help"}:
        usage()
        return 0
    command = args[0]
    handler = commands.get(command)
    if handler is None:
        usage()
        return 2
    try:
        result = handler(args[1:])
    except (CliError, RuntimeError, OSError, subprocess.CalledProcessError) as exc:
        return die(f"{name} {command}: {exc}")
    return int(result or 0)


def run_checked(
    argv: list[str],
    *,
    cwd: Path | None = None,
    input_bytes: bytes | None = None,
    stdout: int | None = None,
) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(argv, cwd=cwd, input=input_bytes, stdout=stdout, check=True)
