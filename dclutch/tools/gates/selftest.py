"""tools/gate selftest -- the gates' own refusal tests, so a gate that cannot fail is found before it is trusted.

Runs: the python tests under tools/gates/tests (frames, reference, commands,
budgets, emission), tools/lane/test.sh, tools/gauntlet/test-run-cli.sh, and
tools/seam-audit/test-seam-audit.py. Each is hermetic: scratch repositories and
stub toolchains, never the real tree.
"""

from __future__ import annotations

from .common import EXIT_FAIL, EXIT_PASS, GATES, REPO, note, sh

SUITES = (
    ("python tests", ["python3", "-m", "unittest", "discover", "-s", str(GATES / "tests"), "-p", "test_*.py"], REPO),
    ("tools/lane/test.sh", ["bash", str(REPO / "tools/lane/test.sh")], REPO),
    ("tools/gauntlet/test-run-cli.sh", ["bash", str(REPO / "tools/gauntlet/test-run-cli.sh")], REPO),
    ("tools/seam-audit/test-seam-audit.py", ["python3", str(REPO / "tools/seam-audit/test-seam-audit.py")], REPO / "tools/seam-audit"),
)


def run(*, dry_run: bool = False):
    failed = []
    for name, args, cwd in SUITES:
        if dry_run:
            note(f"$ cd {cwd} && " + " ".join(args))
            continue
        note(name)
        if sh(args, cwd=cwd, quiet=True).returncode:
            failed.append(name)
            sh(args, cwd=cwd)  # once more, loudly, so the failure is readable
    if failed:
        return EXIT_FAIL, "self-tests failed: " + ", ".join(failed)
    return EXIT_PASS, ""


def main(argv: list[str]) -> int:
    if argv and argv[0] in ("-h", "--help"):
        print(__doc__.strip())
        return EXIT_PASS
    code, detail = run()
    if detail:
        print(f"selftest: {detail}")
    return code
