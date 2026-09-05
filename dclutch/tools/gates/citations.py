"""tools/gate citations -- a commit cited by a decision or an evidence document exists.

  tools/gate citations               survey and print; refuse on an unadjudicated dangling citation
  tools/gate citations --check       the same (the tier's spelling)
  tools/gate citations --list        print every adjudicated token with its class
  --root DIR                         survey DIR instead of the repository

WHY.  Decision records are the tree's durable rulings and evidence documents are
its dated measurements, and both cite commits.  Nothing checks those citations,
so when a commit never lands -- rebased away, or never in this repository at all
-- the sentence keeps its authority and the next reader reasons from a pointer
to nothing.  The third C-16 walk measured 18.5 % of `docs/decisions/`'s path
citations dangling and named cited commits that do not exist, in records the
tree calls CONFIRMED.  A confirmed ruling whose evidence does not resolve is an
unfalsifiable ruling.

WHAT IT DOES.  Every backticked ``[0-9a-f]{8,40}`` token under the surveyed
roots is a CANDIDATE.  A candidate that `git cat-file -e <token>^{commit}`
resolves is a citation that holds.  A candidate that does not resolve must be
ADJUDICATED by name in `citations-register.json`, which says what the token is
INSTEAD of a commit of this tree.  An unadjudicated dangling candidate is a
refusal.

A register row is an adjudication, never a suppression, so the register is
gated in both directions:

  - a row whose token has started resolving is a refusal (the row is now false);
  - a row no surveyed document cites any more is a refusal (the row is dead).

WHAT IT CANNOT SEE, said out loud.  Eight hex characters is 4.3e9 and this
repository holds thousands of commits, so a citation to ANOTHER repository can
resolve here by accident and be counted as holding -- `DEVNET_ITERATION_2.md`
cites `3b2c7bdd`, a dragons-clutch commit, and this tree has a commit with that
prefix.  The instrument proves that a cited token names a commit, never that it
names the RIGHT one.  It also sees only backticked tokens: a bare sha in prose
is invisible to it, which is the tree's own citation convention held as a rule.

EXIT.  0 clean, 1 a refusal, 2 the register or git is unreadable.
"""

from __future__ import annotations

import argparse
import collections
import json
import re
import subprocess
import sys
from pathlib import Path

from .common import EXIT_FAIL, EXIT_PASS, EXIT_PREREQ, GATES, REPO, Prereq, note

REGISTER = GATES / "citations-register.json"
ROOTS = ("docs/decisions", "docs/evidence")
# The tree's citation convention: a sha is written in backticks. Eight is the
# shortest abbreviation any document here uses; forty is a whole sha.
CANDIDATE = re.compile(r"`([0-9a-f]{8,40})`")


def survey(root: Path) -> dict[str, list[str]]:
    """Every candidate token under the roots, to the sorted documents citing it."""
    cited: dict[str, set[str]] = collections.defaultdict(set)
    for name in ROOTS:
        base = root / name
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*.md")):
            for match in CANDIDATE.finditer(path.read_text(errors="replace")):
                cited[match.group(1)].add(str(path.relative_to(root)))
    return {token: sorted(where) for token, where in sorted(cited.items())}


def resolving(tokens: list[str], root: Path) -> set[str]:
    """Which tokens name a commit. One `cat-file --batch-check`, not one process per token.

    `--batch-check` answers each input line in order: an object line for a token
    that peels to a commit, and `<input> missing`/`ambiguous` for one that does
    not. Ambiguity is NOT resolution -- a prefix several commits share names none
    of them -- so only the object lines count.
    """
    if not tokens:
        return set()
    query = "".join(f"{token}^{{commit}}\n" for token in tokens)
    result = subprocess.run(
        ["git", "-C", str(root), "cat-file", "--batch-check"],
        input=query, capture_output=True, text=True,
    )
    if result.returncode and not result.stdout:
        raise Prereq(f"git cat-file refused in {root}: {result.stderr.strip()}")
    answers = result.stdout.splitlines()
    if len(answers) != len(tokens):
        raise Prereq(
            f"git answered {len(answers)} of {len(tokens)} citations; the reading is not aligned"
        )
    return {token for token, answer in zip(tokens, answers)
            if len(answer.split()) == 3 and answer.split()[1] == "commit"}


def load_register(path: Path = REGISTER) -> dict:
    if not path.is_file():
        raise Prereq(f"{path} is absent; the register is the gate's other half")
    return json.loads(path.read_text())


def check(root: Path = REPO, *, register_path: Path = REGISTER) -> tuple[int, str]:
    register = load_register(register_path)
    adjudicated = register["adjudicated"]
    classes = register["classes"]
    unknown_class = sorted({row["class"] for row in adjudicated.values()} - set(classes))
    cited = survey(root)
    if not cited:
        raise Prereq(f"no candidate citation under {root}/{{{','.join(ROOTS)}}}")

    tokens = list(cited)
    found = resolving(tokens, root)
    holds = [t for t in tokens if t in found]
    dangling = [t for t in tokens if t not in found]

    unadjudicated = [t for t in dangling if t not in adjudicated]
    now_resolving = [t for t in adjudicated if t in holds]
    uncited = [t for t in adjudicated if t not in cited]
    # `cited_by` is the register's own claim about where the token appears, and a
    # claim nobody re-derives is the defect this gate exists for.
    drifted = [t for t in adjudicated
               if t in cited and adjudicated[t].get("cited_by") != cited[t]]

    counted = collections.Counter(
        adjudicated[t]["class"] for t in dangling if t in adjudicated
    )
    print(f"citations: {len(cited)} candidate tokens under {', '.join(ROOTS)}")
    print(f"  resolve as a commit of this tree:  {len(holds)}")
    print(f"  adjudicated as something else:     {len(dangling) - len(unadjudicated)}")
    for name, count in sorted(counted.items()):
        print(f"      {name:<18}{count:>4}  -- {classes.get(name, '(no class note)')}")

    for token in sorted(unadjudicated):
        print(f"  DANGLING {token}: no commit, and no register row -- cited by "
              + ", ".join(cited[token]), file=sys.stderr)
    for token in sorted(now_resolving):
        print(f"  STALE ROW {token}: the register calls it "
              f"{adjudicated[token]['class']} and it now resolves as a commit", file=sys.stderr)
    for token in sorted(uncited):
        print(f"  DEAD ROW {token}: no surveyed document cites it any more", file=sys.stderr)
    for token in sorted(drifted):
        print(f"  DRIFTED ROW {token}: register says {adjudicated[token].get('cited_by')}, "
              f"the tree says {cited[token]}", file=sys.stderr)
    for name in unknown_class:
        print(f"  UNDECLARED CLASS {name}: rows use it and `classes` does not define it", file=sys.stderr)

    problems = len(unadjudicated) + len(now_resolving) + len(uncited) + len(drifted) + len(unknown_class)
    if problems:
        return EXIT_FAIL, (
            f"{len(unadjudicated)} cited commit(s) do not exist and are not adjudicated; "
            f"{len(now_resolving)} register row(s) went false; {len(uncited)} row(s) cite nothing; "
            f"{len(drifted)} row(s) name the wrong documents; "
            f"{len(unknown_class)} undeclared class(es)"
        )
    print(f"citations: every cited commit resolves, and all {len(adjudicated)} "
          "non-commit tokens are adjudicated by name.")
    return EXIT_PASS, ""


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="tools/gate citations", add_help=False)
    parser.add_argument("--root", default=str(REPO))
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--register", default=str(REGISTER))
    parser.add_argument("-h", "--help", action="store_true")
    args = parser.parse_args(argv)
    if args.help:
        print(__doc__.strip())
        return EXIT_PASS
    if args.list:
        register = load_register(Path(args.register))
        for token, row in register["adjudicated"].items():
            print(f"{token:<42}{row['class']:<18}{', '.join(row['cited_by'])}")
        return EXIT_PASS
    try:
        code, detail = check(Path(args.root), register_path=Path(args.register))
    except Prereq as error:
        print(f"citations: COULD NOT RUN -- {error}", file=sys.stderr)
        return EXIT_PREREQ
    if detail:
        note(f"citations: REFUSING -- {detail}")
    return code
