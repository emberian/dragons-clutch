#!/usr/bin/env python3
"""Replay every command a runbook publishes, in its safest form.

A runbook is an instruction to a reader NOW, and nothing in this tree checks
that its instructions still work. A flag gets renamed, a script grows a required
argument, a binary is renamed -- and the sentence telling a stranger to type it
stays exactly where it was, keeping all of its authority. Nothing goes red. The
cost is paid by the one person least able to absorb it: someone who does not
already know what the command was supposed to do.

This is the doc half of `tools/doc-citations`, which closed the same shape for
symbols named in doc comments. There, a citation outlived the thing cited. Here,
an instruction outlives the interface it instructs.

    python3 tools/doc-commands/doc_commands.py --root .
    python3 tools/doc-commands/doc_commands.py --root . \
        --baseline tools/doc-commands/baseline.json --check

SCOPE, and it is deliberate: RUNBOOKS ONLY -- `README.md`, `docs/guides/`,
`docs/operators/`. `docs/evidence/` records what a past run did and its commands
are dated by construction; holding a record to today's interface would be
holding the wrong thing to the wrong standard. Scope is stated with `--roots`
rather than assumed, so widening it is a decision somebody makes.

WHAT IT CHECKS, in two tiers, because they cost differently and fail
differently:

  RESOLVED -- the program the command names exists. A repo-relative path is
  tracked and executable; a bare name is a bin this repository declares (npm
  `bin` maps, cargo `[[bin]]` names) or a third-party program named in
  THIRD_PARTY. An unresolved program is a finding: the reader has nothing to
  run.

  PROBED -- the program accepts the subcommand and the long flags the runbook
  passes it. Established by running `<program> --help` and reading the output,
  never by a list kept here: a list would be a second authority for the thing
  being checked, which is this project's signature defect.

WHAT IT WILL NOT DO. It runs `--help` and nothing else. It never runs a command
a runbook publishes, never touches a chain, and probes ONLY a program whose own
source declares a help flag -- safety by declaration rather than by hope. A
program it cannot probe is reported as unprobed WITH THE REASON, never as
passing. "Could not be checked" and "checked and fine" are different answers and
this tool keeps them apart.

EXIT CODES, the tree's own (`tools/gate`, `tools/seam-audit`):
  0  every command resolved, and every probe that ran agreed
  1  this tree has the defect -- a runbook publishes something that will not run
  2  a prerequisite is missing; nothing was proven either way
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

EXIT_PASS, EXIT_FAIL, EXIT_PREREQ, EXIT_USAGE = 0, 1, 2, 64

DEFAULT_ROOTS = ("README.md", "docs/guides", "docs/operators")

FENCE = re.compile(r"^```(sh|bash|shell|console)\s*$")
FENCE_END = re.compile(r"^```\s*$")
ENV_ASSIGN = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")
LONG_FLAG = re.compile(r"^--[a-z0-9][a-z0-9-]*$")

# Programs that belong to somebody else. Naming them is the point: this list is
# the boundary of what this repository can be held responsible for, and a
# command outside it is DECLINED rather than silently passed.
THIRD_PARTY = frozenset({
    "cargo", "rustup", "rustc", "npm", "npx", "node", "python3", "pip", "uv",
    "git", "gh", "solana", "solana-keygen", "solana-test-validator", "spl-token",
    "jq", "curl", "shasum", "sha256sum", "cmp", "diff", "sed", "awk", "grep",
    "tail", "head", "cat", "ls", "mkdir", "rm", "cp", "mv", "tar", "unzip",
    "ssh", "scp", "bash", "sh", "zsh", "export", "cd", "echo", "printf", "env",
    "lake", "lean", "elan", "make", "open", "mktemp", "chmod", "test",
})

# Shell control words a line can start with. They are not programs and a runbook
# that uses them is writing shell, not publishing a command.
SHELL_WORDS = frozenset({"for", "do", "done", "if", "then", "fi", "while", "case", "esac", "(", ")", "{", "}", "|", "||", "&&", ";;"})

# A program is probed only if its own source shows it HANDLES a help flag, not
# merely that it mentions one. The distinction is not pedantic: measured while
# writing this, `tools/ticket-board/run-local.sh` mentions `--help` in a comment
# and passes every argument through to a binary it CARGO-BUILDS first, so a
# checker that trusted the mention spent sixty seconds compiling. Probing is
# safe by declaration, and a comment is not a declaration.
HELP_ARM = re.compile(
    r"(?:^|[|(\s])(?:-h\s*\|\s*)?[\"']?--help[\"']?\s*\)"   # sh/bash case arm
    r"|--help[\"']?\s*(?:==|=~|\]\])"                          # bash [[ ... ]] test
    r"|\badd_argument\b|\bargparse\b|\bArgumentParser\b"     # python, which always handles it
    r"|\bclap\b|\bStructOpt\b",                                # rust
    re.M,
)


@dataclass
class Command:
    """One command a runbook publishes, with where a reader would find it."""
    source: str
    line: int
    text: str
    program: str
    words: list[str] = field(default_factory=list)


@dataclass
class Finding:
    kind: str
    source: str
    line: int
    detail: str

    def key(self) -> str:
        return f"{self.source}:{self.line}\t{self.kind}\t{self.detail}"


def tracked_files(root: Path) -> set[str]:
    out = subprocess.run(["git", "-C", str(root), "ls-files"], capture_output=True, text=True, check=True)
    return set(out.stdout.split("\n"))


def declared_bins(root: Path, tracked: set[str]) -> dict[str, tuple[str, str | None]]:
    """Every executable name this repository declares, and where it declares it.

    Read from the manifests rather than listed here, for the same reason the
    flag check reads `--help`: a list of binary names kept in a checker is a
    second authority for a fact the manifests own, and it is wrong the day
    somebody renames one -- which is exactly what happened to `dclutch`.
    """
    found: dict[str, tuple[str, str | None]] = {}
    for path in sorted(tracked):
        package = str(Path(path).parent)
        if path.endswith("package.json"):
            try:
                manifest = json.loads((root / path).read_text())
            except (OSError, json.JSONDecodeError):
                continue
            bins = manifest.get("bin")
            entries: dict[str, str] = {}
            if isinstance(bins, str):
                entries[str(manifest.get("name", path))] = bins
            elif isinstance(bins, dict):
                entries = {name: target for name, target in bins.items() if isinstance(target, str)}
            for name, target in entries.items():
                found.setdefault(name, (path, os.path.normpath(os.path.join(package, target))))
        elif path.endswith("Cargo.toml"):
            try:
                text = (root / path).read_text()
            except OSError:
                continue
            for name in re.findall(r'\[\[bin\]\][^\[]*?name\s*=\s*"([^"]+)"', text, re.S):
                found.setdefault(name, (path, None))
    return found


def fence_body(lines: list[str], after: int) -> list[str]:
    """The lines of the fence that opens at `after`, exclusive of its close."""
    body: list[str] = []
    for line in lines[after:]:
        if FENCE_END.match(line.strip()):
            break
        body.append(line)
    return body


def split_commands(text: str, source: str) -> list[Command]:
    """Every command in one document's shell fences, continuations joined.

    A `\\`-continued command is ONE command. Reading its continuation lines as
    commands of their own is how a naive survey reports `--price` as a missing
    program and buries the real findings under its own noise.
    """
    commands: list[Command] = []
    lines = text.split("\n")
    inside = False
    prompted = False
    pending: list[str] = []
    start = 0
    for number, raw in enumerate(lines, 1):
        stripped = raw.strip()
        if not inside:
            if FENCE.match(stripped):
                inside = True
                # A fence that prompts with `$` is a TRANSCRIPT: its unprompted
                # lines are the program's output, not more commands. Reading
                # them as commands is how a survey reports `claims` and `were`
                # as missing programs and buries its real findings in its own
                # noise.
                prompted = any(later.strip().startswith("$ ") for later in fence_body(lines, number))
            continue
        if FENCE_END.match(stripped):
            inside = False
            prompted = False
            pending = []
            continue
        if not pending:
            if not stripped or stripped.startswith("#"):
                continue
            if prompted and not stripped.startswith("$ "):
                continue
            start = number
        body = stripped[2:].strip() if stripped.startswith("$ ") else stripped
        if body.endswith("\\"):
            pending.append(body[:-1].strip())
            continue
        pending.append(body)
        joined = " ".join(pending).strip()
        pending = []
        if joined:
            commands.extend(one_command(joined, source, start))
    return commands


def one_command(text: str, source: str, line: int) -> list[Command]:
    """Strip the shell around a command and name the program it runs."""
    for separator in ("&&", "||", "|", ";"):
        if separator in text:
            parts = [part.strip() for part in text.split(separator)]
            return [held for part in parts if part for held in one_command(part, source, line)]
    body = text.strip().lstrip("(").strip()
    try:
        words = shlex.split(body)
    except ValueError:
        return []
    while words and (ENV_ASSIGN.match(words[0]) or words[0] in {"sudo", "time", "exec"}):
        words = words[1:]
    if not words:
        return []
    program = words[0]
    # `python3 tools/x.py` and `node scripts/y.mjs` run a repo file; the
    # interpreter is not the thing a runbook is instructing about.
    if program in {"python3", "python", "node", "bash", "sh"} and len(words) > 1 and not words[1].startswith("-"):
        words = words[1:]
        program = words[0]
    if program in SHELL_WORDS:
        return []
    return [Command(source=source, line=line, text=body, program=program, words=words)]


def collect(root: Path, roots: tuple[str, ...], tracked: set[str]) -> list[Command]:
    documents: list[str] = []
    for entry in roots:
        if entry in tracked and entry.endswith(".md"):
            documents.append(entry)
            continue
        prefix = entry.rstrip("/") + "/"
        documents.extend(sorted(path for path in tracked if path.startswith(prefix) and path.endswith(".md")))
    commands: list[Command] = []
    for document in documents:
        commands.extend(split_commands((root / document).read_text(), document))
    return commands


def resolve(command: Command, root: Path, tracked: set[str],
            bins: dict[str, tuple[str, str | None]]) -> tuple[str, str]:
    """Where a reader's shell would find this command's program.

    Returns `(disposition, detail)`. `repo` and `bin` are ours to check; `third
    party` is somebody else's and declined by name; `unresolved` is the finding.
    """
    program = command.program
    if program.startswith("./"):
        program = program[2:]
    if "/" in program:
        if program.startswith("target/") or "/target/" in program:
            # A build output. Its absence says the tree is unbuilt, which is a
            # fact about this checkout rather than about the runbook.
            return "build output", program
        if program in tracked:
            path = root / program
            if not os.access(path, os.X_OK) and not program.endswith((".py", ".mjs", ".js", ".json")):
                return "unresolved", f"{program} is tracked but not executable"
            return "repo", program
        if program.startswith(("/", "$", "<")) or "<" in program:
            return "placeholder", program
        return "unresolved", f"{program} is not a tracked file"
    if program in bins:
        return "bin", bins[program][0]
    if program in THIRD_PARTY:
        return "third party", program
    if program.startswith(("$", "<")):
        return "placeholder", program
    return "unresolved", f"{program} is neither a tracked file nor a bin this repository declares"


def help_text(root: Path, disposition: str, target: str, program: str,
              bins: dict[str, tuple[str, str | None]],
              subcommands: tuple[str, ...] = ()) -> tuple[str | None, str]:
    """`<program> --help`, when the program's own source declares one.

    Safety by declaration: a program whose source never mentions a help flag is
    not run at all. The alternative is executing an unknown script and hoping
    `--help` is inert, which is not a thing to hope.
    """
    if disposition == "bin":
        # An npm bin map already says where the launcher lives, so this probes
        # the launcher IN THE TREE rather than requiring a global install: a
        # reader's PATH is not the thing under test, the published interface is.
        launcher = bins.get(program, (target, None))[1]
        if launcher is not None and (root / launcher).exists():
            executable = ["node", str(root / launcher)] if launcher.endswith((".mjs", ".js")) else [str(root / launcher)]
        else:
            # Beside its own manifest FIRST, then the root target dir. Not a
            # guess: `tools/dclutch-cli` declares `[workspace]` of its own, so
            # cargo puts `dclutch` under `tools/dclutch-cli/target/`, and a
            # checker that only looked at the root one reported the repository's
            # published binary as unbuilt while it sat there compiled.
            beside = root / Path(target).parent / "target"
            for candidate in (beside / "release" / program, beside / "debug" / program,
                              root / "target" / "release" / program, root / "target" / "debug" / program):
                if candidate.exists():
                    executable = [str(candidate)]
                    break
            else:
                found = subprocess.run(["sh", "-c", f"command -v {shlex.quote(program)}"], capture_output=True, text=True)
                if found.returncode != 0:
                    return None, f"{program} is declared by {target}, and is neither built under target/ nor on PATH; build it to probe this command"
                executable = [program]
    elif disposition == "repo":
        path = root / target
        try:
            source = path.read_text(errors="replace")
        except OSError as error:
            return None, f"{target} could not be read: {error}"
        if HELP_ARM.search(source) is None:
            return None, f"{target} handles no help flag of its own, so it is not run"
        executable = [sys.executable, str(path)] if target.endswith(".py") else [str(path)]
    else:
        return None, f"{disposition} programs are not probed"
    # Its own session, so a timeout kills whatever the probe started rather than
    # leaving it holding a port or a build slot behind the checker's back.
    try:
        result = subprocess.run(
            executable + list(subcommands) + ["--help"], capture_output=True, text=True, timeout=30,
            cwd=str(root), env={**os.environ, "NO_COLOR": "1"}, start_new_session=True,
        )
    except subprocess.TimeoutExpired as error:
        return None, f"{target} --help did not finish in 30s: it is doing more than printing usage"
    except OSError as error:
        return None, f"{target} --help could not be run: {error}"
    output = f"{result.stdout}\n{result.stderr}"
    if not output.strip():
        return None, f"{target} --help printed nothing"
    return output, ""


USAGE = re.compile(r"^\s*usage:\s*(.*?)(?:\n\s*\n|\Z)", re.S | re.I | re.M)


def required_flags(text: str) -> frozenset[str]:
    """Long options the program's own usage line says are NOT optional.

    argparse prints optional things inside `[...]` and required things bare, so
    stripping every bracketed span from the usage block leaves exactly the
    arguments a reader must pass. Read from the program's own output rather than
    declared here, which is the whole posture of this checker.

    A usage line in any other shape yields nothing, and nothing is the honest
    answer: this infers a requirement, it never invents one.
    """
    found = USAGE.search(text)
    if found is None:
        return frozenset()
    depth = 0
    bare: list[str] = []
    for character in found.group(1):
        if character == "[":
            depth += 1
        elif character == "]":
            depth = max(0, depth - 1)
        elif depth == 0:
            bare.append(character)
    return frozenset(word for word in "".join(bare).split() if LONG_FLAG.match(word))


def probe(command: Command, text: str, consumed: int) -> list[str]:
    """Every subcommand and long flag the runbook uses that help does not name.

    `consumed` is how many leading words the descent already validated on its
    way into the subcommand's own page; re-checking them against the page they
    SELECTED is how a correct `dclutch market show` gets reported as a market
    verb `dclutch market --help` never mentions.
    """
    missing: list[str] = []
    rest = command.words[1 + consumed:]
    for index, word in enumerate(rest):
        if LONG_FLAG.match(word):
            if not re.search(rf"(?<![\w-]){re.escape(word)}(?![\w-])", text):
                missing.append(word)
            continue
        if index == 0 and re.match(r"^[a-z][a-z0-9-]*$", word):
            if not re.search(rf"(?<![\w-]){re.escape(word)}(?![\w-])", text):
                missing.append(word)
    return missing


def survey(root: Path, roots: tuple[str, ...], run_probes: bool) -> tuple[list[Finding], dict[str, int]]:
    tracked = tracked_files(root)
    bins = declared_bins(root, tracked)
    commands = collect(root, roots, tracked)
    findings: list[Finding] = []
    counts = {"commands": len(commands), "repo": 0, "bin": 0, "third party": 0, "placeholder": 0,
              "build output": 0, "probed": 0, "unprobed": 0}
    helps: dict[tuple[str, tuple[str, ...]], tuple[str | None, str]] = {}
    for command in commands:
        disposition, detail = resolve(command, root, tracked, bins)
        if disposition == "unresolved":
            findings.append(Finding("unresolved program", command.source, command.line, detail))
            continue
        counts[disposition] += 1
        if not run_probes or disposition not in {"repo", "bin"}:
            continue
        if (detail, ()) not in helps:
            helps[(detail, ())] = help_text(root, disposition, detail, command.program, bins)
        text, reason = helps[(detail, ())]
        if text is None:
            counts["unprobed"] += 1
            findings.append(Finding("unprobed", command.source, command.line, reason))
            continue
        counts["probed"] += 1
        # Descend into the subcommand's OWN help before judging its flags. A
        # CLI whose subcommands document themselves is healthy, not broken, and
        # demanding every flag in the top-level page would report a defect where
        # there is none -- `dclutch ticket author` takes fourteen flags that
        # `dclutch ticket --help` names and `dclutch --help` rightly does not.
        # The descent goes only through words the CURRENT page already names,
        # so it never invents a subcommand to run.
        path: list[str] = []
        # Every page the descent passed through, because a reader passes
        # through them too. `dclutch market decode --file` documents `--file`
        # on the TOP page and `decode` on the market page; a flag named at any
        # level of the path a reader walks is a flag they can find.
        pages: list[str] = [text]
        for word in command.words[1:]:
            if not re.match(r"^[a-z][a-z0-9-]*$", word) or len(path) >= 2:
                break
            if not re.search(rf"(?<![\w-]){re.escape(word)}(?![\w-])", text):
                break
            candidate = (*path, word)
            key = (detail, candidate)
            if key not in helps:
                helps[key] = help_text(root, disposition, detail, command.program, bins, candidate)
            deeper, _ = helps[key]
            if deeper is None:
                break
            path = list(candidate)
            text = deeper
            pages.append(deeper)
        missing = probe(command, "\n".join(pages), len(path))
        if missing:
            findings.append(Finding(
                "rejected by its own program", command.source, command.line,
                f"{command.program} --help names none of: {' '.join(missing)}",
            ))
        omitted = sorted(required_flags(pages[-1]) - set(command.words))
        if omitted:
            findings.append(Finding(
                "incomplete as published", command.source, command.line,
                f"{command.program} requires {' '.join(omitted)}, which this command does not pass",
            ))
    findings.sort(key=Finding.key)
    return findings, counts


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="tools/gate commands", description=__doc__.split("\n")[0])
    parser.add_argument("--root", default=".", help="repository root")
    parser.add_argument("--roots", nargs="*", default=list(DEFAULT_ROOTS),
                        help="documents and directories surveyed (default: the runbooks)")
    parser.add_argument("--baseline", default=str(Path(__file__).resolve().parent / "commands-baseline.json"), help="triaged findings, as JSON")
    parser.add_argument("--check", action="store_true", help="fail on any finding outside the baseline")
    parser.add_argument("--write", action="store_true", help="rewrite the baseline from this survey")
    parser.add_argument("--no-probe", action="store_true", help="resolve programs only; run nothing")
    parser.add_argument("--json", action="store_true", help="machine-readable survey")
    arguments = parser.parse_args(argv)

    root = Path(arguments.root).resolve()
    if not (root / ".git").exists():
        print(f"{root} is not a git repository root", file=sys.stderr)
        return EXIT_PREREQ
    try:
        findings, counts = survey(root, tuple(arguments.roots), not arguments.no_probe)
    except FileNotFoundError as error:
        print(f"prerequisite missing: {error}", file=sys.stderr)
        return EXIT_PREREQ

    if arguments.json:
        print(json.dumps({
            "counts": counts,
            "findings": [finding.__dict__ for finding in findings],
        }, indent=2, sort_keys=True))
        return EXIT_PASS

    accepted: set[str] = set()
    if arguments.baseline:
        baseline_path = Path(arguments.baseline)
        if arguments.write:
            baseline_path.write_text(json.dumps(
                {"accepted": sorted(finding.key() for finding in findings)}, indent=2) + "\n")
            print(f"wrote {len(findings)} accepted finding(s) to {baseline_path}")
            return EXIT_PASS
        if baseline_path.exists():
            accepted = set(json.loads(baseline_path.read_text()).get("accepted", []))

    print(f"doc-commands: {counts['commands']} command(s) in {' '.join(arguments.roots)}")
    print(f"  resolved: {counts['repo']} repo path(s), {counts['bin']} declared bin(s), "
          f"{counts['third party']} third-party, {counts['placeholder']} placeholder(s), "
          f"{counts['build output']} build output(s)")
    print(f"  probed:   {counts['probed']} against their own --help, {counts['unprobed']} not probed")

    fresh = [finding for finding in findings if finding.key() not in accepted]
    stale = sorted(accepted - {finding.key() for finding in findings})
    for finding in fresh:
        print(f"  {finding.kind}: {finding.source}:{finding.line} — {finding.detail}")
    for entry in stale:
        print(f"  baseline entry no longer found (delete it): {entry}")

    if not arguments.check:
        return EXIT_PASS
    # "Could not be checked" is not "checked and fine", and the two get
    # different exit codes: a defect is 1, an unrun probe is 2. Collapsing them
    # is the failure this tree already paid for once, when a missing `ast-grep`
    # made a clean tree look broken.
    defects = [finding for finding in fresh if finding.kind != "unprobed"]
    unproven = [finding for finding in fresh if finding.kind == "unprobed"]
    if defects:
        print(f"\n{len(defects)} runbook command(s) a reader cannot run as published", file=sys.stderr)
        return EXIT_FAIL
    if stale:
        print(f"\n{len(stale)} baseline entry(ies) outlived their finding; delete them", file=sys.stderr)
        return EXIT_FAIL
    if unproven:
        print(f"\n{len(unproven)} command(s) were not probed; nothing is claimed about them", file=sys.stderr)
        return EXIT_PREREQ
    return EXIT_PASS


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
