"""What every gate shares: one verdict vocabulary, one clean-revision export, one subprocess wrapper.

Exit codes, the tree's one convention (adopted from tools/seam-audit):
  0  the gate ran and passed
  1  the tree has the defect the gate detects
  2  a prerequisite is missing; nothing was proven either way
  64 usage
"""

from __future__ import annotations

import contextlib
import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

EXIT_PASS, EXIT_FAIL, EXIT_PREREQ, EXIT_USAGE = 0, 1, 2, 64

GATES = Path(__file__).resolve().parent
REPO = GATES.parents[1]

# `cargo build-sbf` exits 0 when the SBF backend reports that a call overwrites
# its own stack frame. Every gate that builds a link greps its log for this.
FRAME_DIAGNOSTIC = "overwrites values in the frame"


class Prereq(Exception):
    """Nothing was proven: a prerequisite is missing."""


class Failed(Exception):
    """The measurement ran and the tree has the defect."""


def have(command: str) -> bool:
    return shutil.which(command) is not None


def say(text: str) -> None:
    print(f"\n=== {text} ===", flush=True)


def note(text: str) -> None:
    print(f"    {text}", flush=True)


def sh(args, *, cwd=None, env=None, capture=False, quiet=False, timeout=None,
       stdin=None) -> subprocess.CompletedProcess:
    """Run a command. Never raises on a nonzero exit; raises Prereq if the binary is absent."""
    kwargs: dict = {"cwd": str(cwd) if cwd else None, "env": env, "text": True,
                    "timeout": timeout, "stdin": stdin}
    if capture:
        kwargs.update(stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    elif quiet:
        kwargs.update(stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        return subprocess.run([str(a) for a in args], **kwargs)
    except FileNotFoundError as error:
        raise Prereq(f"{args[0]} is not on PATH") from error


def git(*args: str, repo: Path = REPO) -> str:
    result = sh(["git", "-C", repo, *args], capture=True)
    if result.returncode != 0:
        first = (result.stderr.strip().splitlines() or ["(no message)"])[0]
        raise Prereq(f"git {' '.join(args)} failed in {repo}: {first}")
    return result.stdout


def resolve_commit(rev: str, repo: Path = REPO) -> str:
    result = sh(["git", "-C", repo, "rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}"],
                capture=True)
    if result.returncode != 0:
        raise Prereq(f"{rev} does not name a commit in {repo}")
    return result.stdout.strip()


def repo_top(path: Path) -> Path | None:
    result = sh(["git", "-C", path, "rev-parse", "--show-toplevel"], capture=True)
    return Path(result.stdout.strip()) if result.returncode == 0 else None


def dirty(*paths: str, repo: Path = REPO) -> int:
    """Tracked files under `paths` that differ from HEAD. Untracked files are not counted."""
    out = sh(["git", "-C", repo, "status", "--porcelain", "--untracked-files=no", "--", *paths],
             capture=True).stdout
    return len([line for line in out.splitlines() if line])


def tracked_files(repo: Path = REPO) -> list[str]:
    """`git ls-files`, sorted. A vendored or exported copy of the tree is not a checkout."""
    result = sh(["git", "-C", repo, "ls-files"], capture=True)
    if result.returncode != 0:
        raise Prereq(f"{repo} is not a git checkout, so nothing tracked can be enumerated")
    return sorted(line for line in result.stdout.splitlines() if line)


@contextlib.contextmanager
def scratch(prefix: str):
    path = Path(tempfile.mkdtemp(prefix=f"dclutch-{prefix}."))
    try:
        yield path
    finally:
        shutil.rmtree(path, ignore_errors=True)


@contextlib.contextmanager
def archived(rev: str, repo: Path = REPO):
    """A clean export of REV, `git archive | tar -xm`, removed afterwards.

    `git archive` touches no repository state, so it cannot contend on `.git`
    locks with other lanes. `-m` extracts at NOW: a tree stamped with the
    commit's time looks older than a surviving build directory, cargo rebuilds
    nothing, and the previous commit's artifact wears this one's name.

    DCLUTCH_GATE_BUILD_ROOT names a directory to export into and keep, so one
    export can serve several gates in one invocation.
    """
    sha = resolve_commit(rev, repo)
    keep = os.environ.get("DCLUTCH_GATE_BUILD_ROOT")
    with scratch("archive") as tmp:
        root = Path(keep) if keep else tmp / "src"
        if root.exists():
            shutil.rmtree(root)
        root.mkdir(parents=True)
        export = subprocess.Popen(["git", "-C", str(repo), "archive", sha], stdout=subprocess.PIPE)
        unpack = subprocess.run(["tar", "-xm", "-C", str(root)], stdin=export.stdout)
        export.wait()
        if export.returncode or unpack.returncode:
            raise Prereq(f"could not export {sha} from {repo}")
        yield root, sha


@contextlib.contextmanager
def checked_out(rev: str, repo: Path = REPO):
    """A detached worktree at REV, for a gate whose subject needs a real `.git`."""
    sha = resolve_commit(rev, repo)
    with scratch("worktree") as tmp:
        root = tmp / "wt"
        result = sh(["git", "-C", repo, "worktree", "add", "--detach", "--quiet", root, sha],
                    capture=True)
        if result.returncode != 0:
            raise Prereq(f"could not check out {sha}: {result.stderr.strip()}")
        try:
            yield root, sha
        finally:
            sh(["git", "-C", repo, "worktree", "remove", "--force", root], quiet=True)
            sh(["git", "-C", repo, "worktree", "prune"], quiet=True)


@contextlib.contextmanager
def measured_tree(ctx, *dirty_paths: str):
    """The tree a compiling gate measures: an archive of --commit, else the working tree.

    A working-tree measurement says how many tracked files under `dirty_paths`
    differ from HEAD, because on a shared checkout a red may be a neighbouring
    lane's half-written file. Yields (root, commit-or-None).
    """
    if ctx.commit:
        with archived(ctx.commit) as (root, sha):
            note(f"measuring commit {sha} (clean export)")
            yield root, sha
        return
    count = dirty(*dirty_paths) if dirty_paths else 0
    if count:
        note(f"measuring the WORKING TREE: {count} uncommitted file(s) under "
             f"{' '.join(dirty_paths)}; a red here may be another lane's. Use --commit to report one.")
    else:
        note("measuring the working tree")
    yield REPO, None


def atomic_write(path: Path, text: str) -> None:
    """Temp file + rename in the destination directory, so a failed writer leaves the old bytes."""
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as handle:
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except BaseException:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(temporary)
        raise


@dataclass
class Context:
    commit: str | None = None
    require: bool = False
    dry_run: bool = False
    jobs: str = field(default_factory=lambda: os.environ.get("CARGO_BUILD_JOBS", "4"))


@dataclass
class Verdict:
    rows: list[str] = field(default_factory=list)
    worst: int = EXIT_PASS

    def record(self, name: str, code: int, detail: str = "", *, require: bool = False) -> None:
        if code == EXIT_PREREQ and require:
            code, detail = EXIT_FAIL, f"{detail} (--require: an unrun gate is not a passing gate)"
        if code == EXIT_PASS:
            self.rows.append(f"PASS      {name}")
        elif code == EXIT_PREREQ:
            self.rows.append(f"NOT RUN   {name} -- {detail}")
            self.worst = max(self.worst, EXIT_PREREQ) if self.worst != EXIT_FAIL else self.worst
        else:
            self.rows.append(f"FAILED    {name}{' -- ' + detail if detail else ''}")
            self.worst = EXIT_FAIL

    def render(self) -> str:
        tail = {
            EXIT_PASS: "all requested gates ran and passed",
            EXIT_PREREQ: "no gate failed, but a gate above DID NOT RUN. That is an unmeasured tree, not a passing one. Exit 2.",
        }.get(self.worst, "a gate failed. Exit 1.")
        return "\n".join(self.rows) + f"\n\n{tail}\n"


def read_tsv(path: Path, columns: int):
    """Non-comment rows of a tab-separated register, each with at least `columns` fields."""
    if not path.is_file():
        raise Prereq(f"{path.relative_to(REPO) if path.is_relative_to(REPO) else path} is absent")
    for number, line in enumerate(path.read_text().splitlines(), start=1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) < columns:
            raise Prereq(f"{path.name}:{number}: expected {columns} tab-separated fields, got {len(fields)}")
        yield number, fields


def ledger_lock(ledger: Path, *, timeout_s: int = 300):
    """The gauntlet ledger's lock: an atomic `mkdir` beside the file, holder pid recorded."""
    import time

    lock = ledger.with_name(ledger.name + ".lock")

    @contextlib.contextmanager
    def held():
        waited = 0
        while True:
            try:
                lock.mkdir(parents=True)
                break
            except FileExistsError:
                if waited >= timeout_s:
                    holder = (lock / "pid").read_text().strip() if (lock / "pid").is_file() else "unknown"
                    note(f"ledger lock {lock} held over {timeout_s}s by pid {holder}; breaking it")
                    shutil.rmtree(lock, ignore_errors=True)
                    continue
                if waited == 0:
                    note(f"waiting for the ledger lock at {lock}")
                time.sleep(1)
                waited += 1
        (lock / "pid").write_text(f"{os.getpid()}\n")
        try:
            yield
        finally:
            shutil.rmtree(lock, ignore_errors=True)

    return held()
