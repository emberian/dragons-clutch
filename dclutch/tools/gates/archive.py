"""tools/gate archive REV DIR -- export a clean tree of REV into DIR (git archive | tar -xm).

The one implementation of "measure a commit, not the tree around it", for
runners outside this package to call instead of carrying their own.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

from .common import EXIT_PASS, EXIT_USAGE, REPO, resolve_commit


def export(rev: str, into: Path, repo: Path = REPO) -> str:
    sha = resolve_commit(rev, repo)
    if into.exists():
        shutil.rmtree(into)
    into.mkdir(parents=True)
    archive = subprocess.Popen(["git", "-C", str(repo), "archive", sha], stdout=subprocess.PIPE)
    unpack = subprocess.run(["tar", "-xm", "-C", str(into)], stdin=archive.stdout)
    archive.wait()
    if archive.returncode or unpack.returncode:
        raise SystemExit(f"archive: could not export {sha}")
    return sha


def main(argv: list[str]) -> int:
    if len(argv) != 2 or argv[0] in ("-h", "--help"):
        print(__doc__.strip(), file=sys.stderr)
        return EXIT_USAGE
    sha = export(argv[0], Path(argv[1]).resolve())
    print(sha)
    return EXIT_PASS
