#!/usr/bin/env python3
"""Kill one exact local validator if its supervisor disappears."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import signal
import subprocess
import time


def alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def command(pid: int) -> str:
    proc = Path(f"/proc/{pid}/cmdline")
    if proc.is_file():
        return proc.read_bytes().replace(b"\0", b" ").decode(errors="replace")
    result = subprocess.run(
        ["ps", "-o", "command=", "-p", str(pid)],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    return result.stdout


def exact_validator(pid: int, ledger: Path) -> bool:
    value = command(pid)
    return "solana-test-validator" in value and f"--ledger {ledger}" in value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--supervisor-pid", type=int, required=True)
    parser.add_argument("--validator-pid", type=int, required=True)
    parser.add_argument("--ledger", required=True)
    args = parser.parse_args()
    ledger = Path(args.ledger)
    if not ledger.is_absolute() or args.supervisor_pid <= 0 or args.validator_pid <= 0:
        return 2
    while alive(args.supervisor_pid):
        if not alive(args.validator_pid) or not exact_validator(args.validator_pid, ledger):
            return 0
        time.sleep(0.5)
    if not exact_validator(args.validator_pid, ledger):
        return 0
    with suppress_missing():
        os.killpg(args.validator_pid, signal.SIGTERM)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if not alive(args.validator_pid) or not exact_validator(args.validator_pid, ledger):
            return 0
        time.sleep(0.25)
    if exact_validator(args.validator_pid, ledger):
        with suppress_missing():
            os.killpg(args.validator_pid, signal.SIGKILL)
    return 0


class suppress_missing:
    def __enter__(self) -> None:
        return None

    def __exit__(self, kind: object, value: object, traceback: object) -> bool:
        return kind is ProcessLookupError


if __name__ == "__main__":
    raise SystemExit(main())
