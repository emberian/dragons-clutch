"""tools/gate lane ... -- the lane wrapper (tools/lane.sh), reachable from the one entry point."""

from __future__ import annotations

import os

from .common import REPO


def main(argv: list[str]) -> int:
    os.execv("/usr/bin/env", ["/usr/bin/env", "bash", str(REPO / "tools" / "lane.sh"), *argv])
