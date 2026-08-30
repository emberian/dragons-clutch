#!/usr/bin/env python3
"""Normalize one authenticated mixed-gate Trading projection for Hot CU.

The mixed-gate semantic owner remains ``compose-mixed-gate.py``.  This adapter
does not decode the v2 gate.  It re-runs that owner, requires its output to be
byte-identical to the separately hashed projection supplied by the caller, and
then exposes only the five canonical role ELF paths the Hot fixture needs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile


HEX64 = set("0123456789abcdef")
ROLES = ("registry", "trading", "core", "claims", "custody")
FIELDS = {
    "schema",
    "gate_path",
    "gate_sha256",
    "source_revision",
    "source_tree_sha256",
    "solana_cli_version",
    "label",
    "package",
    "disposition",
    "artifact_source_revision",
    "artifact_source_tree_sha256",
    "artifact_build_run_id",
    "artifact_provenance",
    "elf",
    "checked_manifest",
    "carry_forward_plan",
}


def refuse(message: str) -> None:
    raise ValueError(message)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def hex64(value: str, label: str) -> str:
    if len(value) != 64 or any(char not in HEX64 for char in value):
        refuse(f"{label} is not canonical lowercase SHA-256")
    return value


def regular(path: Path, label: str) -> Path:
    if not path.is_absolute() or path.is_symlink() or not path.is_file():
        refuse(f"{label} is not one absolute canonical regular file")
    resolved = path.resolve(strict=True)
    if resolved != path:
        refuse(f"{label} path is not canonical")
    return path


def read_json(path: Path, label: str) -> tuple[bytes, dict]:
    source = regular(path, label).read_bytes()
    try:
        value = json.loads(source)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        refuse(f"{label} is not JSON: {error}")
    if not isinstance(value, dict):
        refuse(f"{label} is not one object")
    return source, value


def evidence_path(root: Path, value: object, label: str) -> Path:
    if not isinstance(value, dict) or set(value) != {"bytes", "canonical_path", "sha256"}:
        refuse(f"{label} evidence fields differ")
    relative = value["canonical_path"]
    if not isinstance(relative, str) or relative.startswith("/") or ".." in Path(relative).parts:
        refuse(f"{label} evidence path is not canonical relative text")
    path = regular(root / relative, label)
    if path.stat().st_size != value["bytes"] or sha256(path) != hex64(value["sha256"], label):
        refuse(f"{label} evidence bytes or SHA-256 differ")
    return path


def normalize(arguments: argparse.Namespace) -> dict:
    selection_path = Path(arguments.selection)
    gate_path = Path(arguments.gate)
    source, selection = read_json(selection_path, "mixed-gate role projection")
    if sha256(selection_path) != hex64(arguments.selection_sha256, "projection SHA-256"):
        refuse("mixed-gate role projection SHA-256 differs")
    regular(gate_path, "mixed checked gate")
    gate_sha = hex64(arguments.gate_sha256, "checked gate SHA-256")
    if sha256(gate_path) != gate_sha:
        refuse("mixed checked gate SHA-256 differs")
    if set(selection) != FIELDS:
        refuse("mixed-gate role projection fields differ")
    if (
        selection["schema"] != "dclutch-checked-mixed-gate-link-selection-v1"
        or selection["label"] != "trading"
        or selection["package"] != "dclutch-trading-sbf"
        or selection["gate_path"] != str(gate_path)
        or selection["gate_sha256"] != gate_sha
    ):
        refuse("mixed-gate projection role, package, gate path, or gate SHA-256 differs")

    repo = Path(arguments.repo).resolve(strict=True)
    verifier = regular(repo / "tools/release/compose-mixed-gate.py", "mixed-gate verifier")
    with tempfile.TemporaryDirectory(prefix="dclutch-hot-cu-mixed-") as temporary:
        replay = Path(temporary) / "selection.json"
        completed = subprocess.run(
            [
                sys.executable,
                str(verifier),
                "verify",
                "--gate",
                str(gate_path),
                "--expected-gate-sha256",
                gate_sha,
                "--expected-source-revision",
                selection["source_revision"],
                "--expected-source-tree-sha256",
                selection["source_tree_sha256"],
                "--selected-link",
                "trading",
                "--output",
                str(replay),
            ],
            cwd=repo,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if completed.returncode != 0:
            refuse(
                "mixed-gate semantic owner refused replay: "
                + completed.stderr.decode("utf-8", errors="replace").strip()
            )
        if replay.read_bytes() != source:
            refuse("mixed-gate semantic-owner replay differs from the accepted projection")

    root = gate_path.parent
    trading = evidence_path(root, selection["elf"], "mixed-gate Trading ELF")
    if trading.name != "trading.so" or trading.parent != root / "elf":
        refuse("mixed-gate Trading ELF is not its canonical role path")
    role_paths = {}
    for role in ROLES:
        path = regular(root / "elf" / f"{role}.so", f"mixed-gate {role} ELF")
        if path.read_bytes()[:4] != b"\x7fELF":
            refuse(f"mixed-gate {role} artifact is not an ELF")
        role_paths[role] = str(path)
    if role_paths["trading"] != str(trading):
        refuse("mixed-gate Trading role path differs from its accepted projection")
    return {
        "schema": "dclutch-hot-cu-mixed-gate-selection-v1",
        "source_revision": selection["source_revision"],
        "source_tree_sha256": selection["source_tree_sha256"],
        "gate_path": str(gate_path),
        "gate_sha256": gate_sha,
        "projection_path": str(selection_path),
        "projection_sha256": arguments.selection_sha256,
        "role_elf_paths": role_paths,
        "trading_elf_sha256": selection["elf"]["sha256"],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--gate", required=True)
    parser.add_argument("--gate-sha256", required=True)
    parser.add_argument("--selection", required=True)
    parser.add_argument("--selection-sha256", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    try:
        value = normalize(args)
        output = Path(args.output)
        if not output.is_absolute() or output.exists() or output.is_symlink():
            refuse("normalized mixed-gate output must be one new absolute path")
        output.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        return 0
    except (OSError, KeyError, TypeError, ValueError) as error:
        print(f"HOT CU MIXED GATE REFUSED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
