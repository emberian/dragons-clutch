#!/usr/bin/env python3
"""Create and authenticate source-bound SBF link provenance.

One descriptor joins the named link, source tree, exact plain build log and
invocation, shipped ELF, and independent frame object/report.  Consumers select
an ELF through that descriptor or through a checked gate that binds it; a bare
path or matching filename is not provenance.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile
from typing import Any, Mapping, NoReturn, Sequence


SCHEMA = "dclutch-sbf-link-provenance-v1"
GATE_SCHEMA = "dclutch-checked-upgrade-gate-v1"
SAFE_NAME = re.compile(r"^[a-z0-9][a-z0-9_-]*$")
HEX40 = re.compile(r"^[0-9a-f]{40}$")
HEX64 = re.compile(r"^[0-9a-f]{64}$")
ROLE_PACKAGES = {
    "core": "dclutch-core-sbf",
    "claims": "dclutch-claims-sbf",
    "trading": "dclutch-trading-sbf",
    "resolution": "dclutch-resolution-proof-sbf",
    "custody": "dclutch-custody-sbf",
    "registry": "dclutch-registry-sbf",
    "rent": "dclutch-rent-sbf",
}
SHIPPED_LINKS = (
    ("claims", "dclutch-claims-sbf", True),
    ("core", "dclutch-core-sbf", True),
    ("custody", "dclutch-custody-sbf", True),
    ("dealer-accelerator", "dclutch-dealer-accelerator-sbf", True),
    ("dclutch-dealer-sbf", "dclutch-dealer-sbf", False),
    ("dclutch-direct-aot-sbf", "dclutch-direct-aot-sbf", False),
    ("general-accelerator", "dclutch-general-accelerator-sbf", True),
    ("dclutch-product-runtime-v2-sbf", "dclutch-product-runtime-v2-sbf", False),
    ("registry", "dclutch-registry-sbf", True),
    ("rent", "dclutch-rent-sbf", True),
    ("resolution", "dclutch-resolution-proof-sbf", True),
    ("series-shadow", "dclutch-series-shadow-sbf", True),
    ("trading", "dclutch-trading-sbf", True),
)
MAX_JSON_BYTES = 16 * 1024 * 1024


class Refusal(RuntimeError):
    """The proposed evidence is not one exact attributable link."""


def refuse(message: str) -> NoReturn:
    raise Refusal(message)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            refuse(f"JSON repeats key {key!r}")
        value[key] = item
    return value


def read_json(path: Path, label: str) -> dict[str, Any]:
    regular(path, label)
    raw = path.read_bytes()
    if not raw or len(raw) > MAX_JSON_BYTES:
        refuse(f"{label} is empty or exceeds {MAX_JSON_BYTES} bytes")
    try:
        value = json.loads(raw, object_pairs_hook=unique_object)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        refuse(f"{label} is not unique-key JSON: {error}")
    if not isinstance(value, dict):
        refuse(f"{label} is not a JSON object")
    return value


def exact_keys(value: Mapping[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        refuse(
            f"{label} fields differ: missing={sorted(expected - actual)} "
            f"unknown={sorted(actual - expected)}"
        )


def regular(path: Path, label: str) -> None:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        refuse(f"missing {label}: {path}")
    if not stat.S_ISREG(mode):
        refuse(f"{label} is not a regular non-symlink file: {path}")


def root_path(root: Path, relative: str, label: str) -> Path:
    if not isinstance(relative, str) or not relative or Path(relative).is_absolute():
        refuse(f"{label} path is not canonical root-relative text")
    candidate = root / relative
    regular(candidate, label)
    resolved = candidate.resolve(strict=True)
    if resolved == root or root not in resolved.parents or resolved != candidate:
        refuse(f"{label} path escapes, aliases, or traverses a symlink: {relative}")
    return candidate


def evidence(root: Path, relative: str, label: str) -> dict[str, Any]:
    path = root_path(root, relative, label)
    return {
        "canonical_path": relative,
        "bytes": path.stat().st_size,
        "sha256": sha256_file(path),
    }


def verify_evidence(root: Path, value: Any, label: str) -> Path:
    if not isinstance(value, dict):
        refuse(f"{label} evidence is not an object")
    exact_keys(value, {"canonical_path", "bytes", "sha256"}, f"{label} evidence")
    path = root_path(root, value["canonical_path"], label)
    if (
        not isinstance(value["bytes"], int)
        or isinstance(value["bytes"], bool)
        or value["bytes"] < 0
        or path.stat().st_size != value["bytes"]
    ):
        refuse(f"{label} byte count differs")
    if not isinstance(value["sha256"], str) or not HEX64.fullmatch(value["sha256"]):
        refuse(f"{label} SHA-256 is malformed")
    if sha256_file(path) != value["sha256"]:
        refuse(f"{label} SHA-256 differs")
    return path


def read_lines(path: Path, label: str) -> list[str]:
    try:
        raw = path.read_bytes()
        text = raw.decode()
    except (OSError, UnicodeDecodeError) as error:
        refuse(f"{label} is not UTF-8: {error}")
    if not raw or not text.endswith("\n"):
        refuse(f"{label} is empty or not newline terminated")
    return text.splitlines()


def validate_log(
    path: Path,
    kind: str,
    run_id: str,
    invocation: str,
    compile_marker: str,
    package: str,
) -> None:
    lines = read_lines(path, f"{kind} build log")
    expected_header = f"dclutch-sbf-{kind}-run-v1={run_id}"
    expected_invocation = f"dclutch-sbf-{kind}-invocation-v1={invocation}"
    if len(lines) < 3 or lines[0] != expected_header or lines[1] != expected_invocation:
        refuse(f"{kind} build log run/invocation stamp differs")
    marker_pattern = re.compile(rf"^\s*Compiling\s+{re.escape(package)}\s+v\S+(?:\s|$)")
    matches = [line for line in lines[2:] if marker_pattern.match(line)]
    if not matches or matches[-1] != compile_marker:
        refuse(f"{kind} build log compile marker differs for {package}")


def frame_fields(path: Path) -> dict[str, str]:
    lines = read_lines(path, "frame report")
    if not lines or lines[0] != "dclutch-sbf-frame-report-v1":
        refuse("frame report schema differs")
    fields: dict[str, str] = {}
    for line in lines[1:]:
        if line == "measurement_output:":
            break
        if line.count("=") != 1:
            refuse("frame report field is malformed")
        key, value = line.split("=", 1)
        if key in fields:
            refuse(f"frame report repeats {key}")
        fields[key] = value
    expected = {
        "label",
        "package",
        "source_tree_sha256",
        "build_run_id",
        "frame_count",
        "frame_bound_bytes",
        "frames_at_or_over_bound",
        "deepest_frame_bytes",
        "object_sha256",
    }
    if set(fields) != expected:
        refuse("frame report provenance fields differ")
    return fields


def atomic_new(path: Path, value: Mapping[str, Any]) -> None:
    if path.exists() or path.is_symlink():
        refuse(f"output already exists: {path}")
    parent = path.parent.resolve(strict=True)
    if parent != path.parent:
        refuse("output parent is not canonical")
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(canonical_json(value))
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def emit(arguments: argparse.Namespace) -> None:
    root = Path(arguments.root).resolve(strict=True)
    if not root.is_dir() or Path(arguments.root) != root:
        refuse("--root must be an exact canonical directory")
    for value, label in ((arguments.label, "label"), (arguments.package, "package")):
        if not SAFE_NAME.fullmatch(value):
            refuse(f"{label} is unsafe")
    if not HEX40.fullmatch(arguments.source_revision):
        refuse("source revision is not full 40-digit lowercase hex")
    if not HEX64.fullmatch(arguments.source_tree_sha256):
        refuse("source tree SHA-256 is malformed")
    if not HEX64.fullmatch(arguments.build_run_id):
        refuse("build run ID is malformed")
    if arguments.diagnostics_count != 0:
        refuse("release provenance refuses nonzero plain-build diagnostics")
    build_log = root_path(root, arguments.build_log, "plain build log")
    frame_log = root_path(root, arguments.frame_build_log, "frame build log")
    frame_object = root_path(root, arguments.frame_object, "frame object")
    report = root_path(root, arguments.frame_report, "frame report")
    validate_log(
        build_log,
        "build",
        arguments.build_run_id,
        arguments.build_invocation,
        arguments.build_compile_marker,
        arguments.package,
    )
    validate_log(
        frame_log,
        "frame",
        arguments.build_run_id,
        arguments.frame_invocation,
        arguments.frame_compile_marker,
        arguments.package,
    )
    fields = frame_fields(report)
    if (
        fields["label"] != arguments.label
        or fields["package"] != arguments.package
        or fields["source_tree_sha256"] != arguments.source_tree_sha256
        or fields["build_run_id"] != arguments.build_run_id
        or fields["object_sha256"] != sha256_file(frame_object)
        or fields["frames_at_or_over_bound"] != "0"
        or fields["frame_bound_bytes"] != "4096"
    ):
        refuse("frame report does not bind this exact link/source/object")
    shipped = None
    artifact_stem = arguments.artifact_stem
    if arguments.elf is not None:
        if not artifact_stem or not SAFE_NAME.fullmatch(artifact_stem):
            refuse("shipped ELF requires one safe artifact stem")
        if arguments.elf != f"elf/{arguments.label}.so":
            refuse("shipped ELF path is not the canonical named-role path")
        elf = root_path(root, arguments.elf, "shipped ELF")
        if elf.read_bytes()[:4] != b"\x7fELF":
            refuse("shipped ELF is not an ELF file")
        shipped = evidence(root, arguments.elf, "shipped ELF")
    elif artifact_stem is not None:
        refuse("frame-only link must not claim an artifact stem")

    descriptor = {
        "schema": SCHEMA,
        "label": arguments.label,
        "package": arguments.package,
        "artifact_stem": artifact_stem,
        "source_revision": arguments.source_revision,
        "source_tree_sha256": arguments.source_tree_sha256,
        "build_run_id": arguments.build_run_id,
        "plain_build": {
            "invocation": arguments.build_invocation,
            "log": evidence(root, arguments.build_log, "plain build log"),
            "compile_marker": arguments.build_compile_marker,
            "sbf_diagnostics_count": arguments.diagnostics_count,
        },
        "shipped_elf": shipped,
        "frame_measurement": {
            "invocation": arguments.frame_invocation,
            "build_log": evidence(root, arguments.frame_build_log, "frame build log"),
            "compile_marker": arguments.frame_compile_marker,
            "object": evidence(root, arguments.frame_object, "frame object"),
            "report": evidence(root, arguments.frame_report, "frame report"),
        },
    }
    output = Path(arguments.output)
    if not output.is_absolute() or output.parent != root / "provenance":
        refuse("output must be ROOT/provenance/LABEL.json")
    if output.name != f"{arguments.label}.json":
        refuse("output filename differs from its named label")
    atomic_new(output, descriptor)


def verify_descriptor(
    root: Path,
    descriptor_path: Path,
    *,
    expected_label: str | None = None,
    expected_package: str | None = None,
    expected_source_revision: str | None = None,
    expected_source_tree_sha256: str | None = None,
    expected_build_run_id: str | None = None,
) -> dict[str, Any]:
    if descriptor_path.parent != root / "provenance":
        refuse("descriptor is not under the canonical provenance directory")
    value = read_json(descriptor_path, "link provenance descriptor")
    exact_keys(
        value,
        {
            "schema",
            "label",
            "package",
            "artifact_stem",
            "source_revision",
            "source_tree_sha256",
            "build_run_id",
            "plain_build",
            "shipped_elf",
            "frame_measurement",
        },
        "link provenance descriptor",
    )
    if value["schema"] != SCHEMA:
        refuse("link provenance schema differs")
    if not SAFE_NAME.fullmatch(value["label"]) or not SAFE_NAME.fullmatch(
        value["package"]
    ):
        refuse("link provenance label/package is unsafe")
    if descriptor_path.name != f"{value['label']}.json":
        refuse("descriptor filename differs from its named label")
    if (
        not HEX40.fullmatch(value["source_revision"])
        or not HEX64.fullmatch(value["source_tree_sha256"])
        or not HEX64.fullmatch(value["build_run_id"])
    ):
        refuse("link provenance source/run identity is malformed")
    expectations = (
        (value["label"], expected_label, "label"),
        (value["package"], expected_package, "package"),
        (value["source_revision"], expected_source_revision, "source revision"),
        (
            value["source_tree_sha256"],
            expected_source_tree_sha256,
            "source tree SHA-256",
        ),
        (value["build_run_id"], expected_build_run_id, "build run ID"),
    )
    for actual, expected, label in expectations:
        if expected is not None and actual != expected:
            refuse(f"descriptor {label} differs from its consumer")

    plain = value["plain_build"]
    frame = value["frame_measurement"]
    if not isinstance(plain, dict) or not isinstance(frame, dict):
        refuse("descriptor build sections are malformed")
    exact_keys(
        plain,
        {"invocation", "log", "compile_marker", "sbf_diagnostics_count"},
        "plain build provenance",
    )
    exact_keys(
        frame,
        {"invocation", "build_log", "compile_marker", "object", "report"},
        "frame measurement provenance",
    )
    if plain["sbf_diagnostics_count"] != 0:
        refuse("descriptor admits nonzero plain-build diagnostics")
    build_log = verify_evidence(root, plain["log"], "plain build log")
    frame_log = verify_evidence(root, frame["build_log"], "frame build log")
    frame_object = verify_evidence(root, frame["object"], "frame object")
    report = verify_evidence(root, frame["report"], "frame report")
    validate_log(
        build_log,
        "build",
        value["build_run_id"],
        plain["invocation"],
        plain["compile_marker"],
        value["package"],
    )
    validate_log(
        frame_log,
        "frame",
        value["build_run_id"],
        frame["invocation"],
        frame["compile_marker"],
        value["package"],
    )
    fields = frame_fields(report)
    if (
        fields["label"] != value["label"]
        or fields["package"] != value["package"]
        or fields["source_tree_sha256"] != value["source_tree_sha256"]
        or fields["build_run_id"] != value["build_run_id"]
        or fields["object_sha256"] != sha256_file(frame_object)
        or fields["frames_at_or_over_bound"] != "0"
        or fields["frame_bound_bytes"] != "4096"
    ):
        refuse("descriptor frame report no longer binds its source/object")
    elf_path = None
    if value["shipped_elf"] is not None:
        if value["artifact_stem"] is None or not SAFE_NAME.fullmatch(
            value["artifact_stem"]
        ):
            refuse("shipped link omitted a safe artifact stem")
        if value["shipped_elf"].get("canonical_path") != f"elf/{value['label']}.so":
            refuse("descriptor shipped ELF path is not its canonical named-role path")
        elf_path = verify_evidence(root, value["shipped_elf"], "shipped ELF")
        if elf_path.read_bytes()[:4] != b"\x7fELF":
            refuse("descriptor shipped file is not an ELF")
    elif value["artifact_stem"] is not None:
        refuse("frame-only descriptor claims an artifact stem")
    return {
        "descriptor": value,
        "descriptor_path": descriptor_path,
        "elf_path": elf_path,
    }


def select_gate_role(
    gate_path: Path, expected_gate_sha256: str, role: str
) -> dict[str, Any]:
    if role not in ROLE_PACKAGES:
        refuse(f"unknown permanent role: {role}")
    regular(gate_path, "checked Upgrade gate")
    if not HEX64.fullmatch(expected_gate_sha256):
        refuse("expected gate SHA-256 is malformed")
    if sha256_file(gate_path) != expected_gate_sha256:
        refuse("checked Upgrade gate SHA-256 differs")
    root = gate_path.parent.resolve(strict=True)
    if gate_path != root / "CHECKED_UPGRADE_GATE.json":
        refuse("gate is not the canonical root CHECKED_UPGRADE_GATE.json")
    gate = read_json(gate_path, "checked Upgrade gate")
    if gate.get("schema") != GATE_SCHEMA:
        refuse("checked Upgrade gate schema differs")
    source_revision = gate.get("source_revision")
    source_tree = gate.get("source_tree_sha256")
    run_id = gate.get("build_run_id")
    if (
        not HEX40.fullmatch(source_revision or "")
        or not HEX64.fullmatch(source_tree or "")
        or not HEX64.fullmatch(run_id or "")
    ):
        refuse("checked Upgrade gate source/run identity is malformed")
    links = gate.get("links")
    if (
        not isinstance(links, list)
        or gate.get("link_count") != len(links)
        or len(links) != len(SHIPPED_LINKS)
    ):
        refuse("checked Upgrade gate is not the exact all-13 link set")
    identities = [
        (link.get("label"), link.get("package")) if isinstance(link, dict) else None
        for link in links
    ]
    expected_identities = [(label, package) for label, package, _ in SHIPPED_LINKS]
    if identities != expected_identities:
        refuse("checked Upgrade gate link order/identity is not canonical all-13")
    verified_links: dict[str, tuple[dict[str, Any], Path | None]] = {}
    for link, (label, package, produces_artifact) in zip(links, SHIPPED_LINKS):
        if (
            link.get("frames_at_or_over_bound") != 0
            or link.get("sbf_diagnostics_count") != 0
            or link.get("frame_bound_bytes") != 4096
        ):
            refuse(f"checked Upgrade gate {label} link is not frame/diagnostic clean")
        provenance_path = verify_evidence(
            root, link.get("artifact_provenance"), f"gate {label} provenance"
        )
        verified = verify_descriptor(
            root,
            provenance_path,
            expected_label=label,
            expected_package=package,
            expected_source_revision=source_revision,
            expected_source_tree_sha256=source_tree,
            expected_build_run_id=run_id,
        )
        if produces_artifact:
            elf = verify_evidence(root, link.get("elf"), f"gate {label} ELF")
            if (
                verified["elf_path"] != elf
                or verified["descriptor"]["shipped_elf"] != link["elf"]
            ):
                refuse(f"gate {label} ELF differs from its link provenance")
        elif link.get("elf") is not None or verified["elf_path"] is not None:
            refuse(f"gate frame-only {label} link claims a shipped ELF")
        verified_links[label] = (verified, provenance_path)
    matches = [
        link for link in links if isinstance(link, dict) and link.get("label") == role
    ]
    if len(matches) != 1:
        refuse(f"checked Upgrade gate does not contain exactly one {role} link")
    link = matches[0]
    if link.get("package") != ROLE_PACKAGES[role]:
        refuse(f"checked Upgrade gate maps {role} to the wrong package")
    verified, provenance_path = verified_links[role]
    elf = verified["elf_path"]
    if elf is None:
        refuse(f"gate {role} omitted its shipped ELF")
    return {
        "schema": "dclutch-checked-gate-role-selection-v1",
        "role": role,
        "package": ROLE_PACKAGES[role],
        "source_revision": source_revision,
        "source_tree_sha256": source_tree,
        "build_run_id": run_id,
        "gate_path": str(gate_path),
        "gate_sha256": expected_gate_sha256,
        "provenance_path": str(provenance_path),
        "provenance_sha256": sha256_file(provenance_path),
        "elf_path": str(elf),
        "elf_bytes": elf.stat().st_size,
        "elf_sha256": sha256_file(elf),
    }


def emit_gate(arguments: argparse.Namespace) -> None:
    """Emit the canonical all-link gate from already verified link evidence.

    This is the reusable form of the byte-for-byte gate construction formerly
    embedded in checked-release-candidate.sh.  It performs no builds and does
    not infer evidence from adjacent target directories.
    """

    root = Path(arguments.root).resolve(strict=True)
    if not root.is_dir() or Path(arguments.root) != root:
        refuse("--root must be an exact canonical directory")
    if not HEX40.fullmatch(arguments.source_revision):
        refuse("source revision is not full 40-digit lowercase hex")
    if not HEX64.fullmatch(arguments.source_tree_sha256):
        refuse("source tree SHA-256 is malformed")
    if not HEX64.fullmatch(arguments.build_run_id):
        refuse("build run ID is malformed")
    if not arguments.solana_cli_version or "\n" in arguments.solana_cli_version:
        refuse("Solana CLI version is empty or multiline")

    source_tree = root_path(root, "source-tree.txt", "source tree manifest")
    if sha256_file(source_tree) != arguments.source_tree_sha256:
        refuse("source tree manifest SHA-256 differs")
    build_links_path = root_path(root, "build-links.tsv", "build links manifest")
    build_run_path = root_path(root, "build-run.txt", "build run manifest")
    diagnostics_path = root_path(
        root, "build-diagnostics.txt", "build diagnostics manifest"
    )
    if read_lines(build_run_path, "build run manifest") != [
        f"dclutch-sbf-build-run-v1={arguments.build_run_id}"
    ]:
        refuse("build run manifest differs from the admitted run")

    build_links: list[tuple[str, str]] = []
    for line in read_lines(build_links_path, "build links manifest"):
        if line.count("\t") != 1:
            refuse("build links manifest row is malformed")
        label, package = line.split("\t", 1)
        build_links.append((label, package))
    expected_links = [(label, package) for label, package, _ in SHIPPED_LINKS]
    if build_links != expected_links:
        refuse("build links manifest is not the canonical all-13 order")

    diagnostics: dict[str, int] = {}
    for line in read_lines(diagnostics_path, "build diagnostics manifest"):
        if line.count("=") != 1:
            refuse("build diagnostics row is malformed")
        label, count_text = line.split("=", 1)
        if label in diagnostics or not count_text.isascii() or not count_text.isdigit():
            refuse("build diagnostics row is duplicated or noncanonical")
        diagnostics[label] = int(count_text)
    if list(diagnostics) != [label for label, _ in expected_links]:
        refuse("build diagnostics manifest is not the canonical all-13 order")

    artifact_roles = {
        package: label
        for label, package, produces_artifact in SHIPPED_LINKS
        if produces_artifact
    }
    links: list[dict[str, Any]] = []
    for label, package, produces_artifact in SHIPPED_LINKS:
        if diagnostics[label] != 0:
            refuse(f"gate refuses nonzero SBF diagnostics for {label}")
        build_log_relative = f"build-{label}.log"
        frame_log_relative = f"frame-build-{label}.log"
        frame_report_relative = f"frame/{label}.txt"
        provenance_relative = f"provenance/{label}.json"
        build_log = root_path(root, build_log_relative, f"{label} build log")
        frame_log = root_path(root, frame_log_relative, f"{label} frame build log")
        frame_report = root_path(
            root, frame_report_relative, f"{label} frame report"
        )
        descriptor_path = root_path(
            root, provenance_relative, f"{label} provenance"
        )
        descriptor = verify_descriptor(
            root,
            descriptor_path,
            expected_label=label,
            expected_package=package,
            expected_source_revision=arguments.source_revision,
            expected_source_tree_sha256=arguments.source_tree_sha256,
            expected_build_run_id=arguments.build_run_id,
        )
        build_pattern = re.compile(
            rf"^\s*Compiling\s+{re.escape(package)}\s+v\S+(?:\s|$)"
        )
        build_markers = [
            line
            for line in read_lines(build_log, f"{label} build log")
            if build_pattern.match(line)
        ]
        frame_markers = [
            line
            for line in read_lines(frame_log, f"{label} frame build log")
            if build_pattern.match(line)
        ]
        if not build_markers or not frame_markers:
            refuse(f"missing canonical compile marker for {label}")
        fields = frame_fields(frame_report)
        try:
            frame_count = int(fields["frame_count"])
            frame_bound = int(fields["frame_bound_bytes"])
            frames_over = int(fields["frames_at_or_over_bound"])
            deepest = int(fields["deepest_frame_bytes"])
        except ValueError:
            refuse(f"frame report integers are malformed for {label}")
        if frame_bound != 4096 or frames_over != 0:
            refuse(f"frame report is not admitted for {label}")

        role = artifact_roles.get(package)
        elf_evidence = evidence(root, f"elf/{role}.so", f"{label} ELF") if role else None
        if produces_artifact:
            if descriptor["elf_path"] != root / f"elf/{role}.so":
                refuse(f"{label} descriptor selects a different ELF")
            checked_manifest = evidence(
                root, f"evidence/{role}/checked.bin", f"{label} checked manifest"
            )
        else:
            if descriptor["elf_path"] is not None:
                refuse(f"frame-only {label} descriptor claims an ELF")
            checked_manifest = None
        links.append(
            {
                "label": label,
                "package": package,
                "build_log": evidence(root, build_log_relative, f"{label} build log"),
                "compile_marker": build_markers[-1],
                "sbf_diagnostics_count": diagnostics[label],
                "frame_build_log": evidence(
                    root, frame_log_relative, f"{label} frame build log"
                ),
                "frame_compile_marker": frame_markers[-1],
                "frame_report": evidence(
                    root, frame_report_relative, f"{label} frame report"
                ),
                "artifact_provenance": evidence(
                    root, provenance_relative, f"{label} provenance"
                ),
                "frame_count": frame_count,
                "frame_bound_bytes": frame_bound,
                "frames_at_or_over_bound": frames_over,
                "deepest_frame_bytes": deepest,
                "elf": elf_evidence,
                "checked_manifest": checked_manifest,
            }
        )

    gate = {
        "schema": GATE_SCHEMA,
        "source_revision": arguments.source_revision,
        "source_tree_sha256": arguments.source_tree_sha256,
        "solana_cli_version": arguments.solana_cli_version,
        "build_run_id": arguments.build_run_id,
        "link_count": len(links),
        "source_tree_manifest": evidence(
            root, "source-tree.txt", "source tree manifest"
        ),
        "build_links_manifest": evidence(
            root, "build-links.tsv", "build links manifest"
        ),
        "build_run_manifest": evidence(root, "build-run.txt", "build run manifest"),
        "diagnostics_manifest": evidence(
            root, "build-diagnostics.txt", "build diagnostics manifest"
        ),
        "links": links,
    }
    target = root / "CHECKED_UPGRADE_GATE.json"
    atomic_new(target, gate)
    print(f"checked Upgrade gate sha256={sha256_file(target)}")


def parser() -> argparse.ArgumentParser:
    top = argparse.ArgumentParser(description=__doc__)
    commands = top.add_subparsers(dest="command", required=True)
    create = commands.add_parser("emit")
    create.add_argument("--root", required=True)
    create.add_argument("--output", required=True)
    create.add_argument("--label", required=True)
    create.add_argument("--package", required=True)
    create.add_argument("--artifact-stem")
    create.add_argument("--source-revision", required=True)
    create.add_argument("--source-tree-sha256", required=True)
    create.add_argument("--build-run-id", required=True)
    create.add_argument("--build-invocation", required=True)
    create.add_argument("--build-log", required=True)
    create.add_argument("--build-compile-marker", required=True)
    create.add_argument("--diagnostics-count", required=True, type=int)
    create.add_argument("--frame-invocation", required=True)
    create.add_argument("--frame-build-log", required=True)
    create.add_argument("--frame-compile-marker", required=True)
    create.add_argument("--frame-object", required=True)
    create.add_argument("--frame-report", required=True)
    create.add_argument("--elf")

    verify = commands.add_parser("verify")
    verify.add_argument("--root", required=True)
    verify.add_argument("--descriptor", required=True)
    verify.add_argument("--label")
    verify.add_argument("--package")
    verify.add_argument("--elf")

    select = commands.add_parser("select-gate-role")
    select.add_argument("--gate", required=True)
    select.add_argument("--gate-sha256", required=True)
    select.add_argument("--role", required=True)
    gate = commands.add_parser("emit-gate")
    gate.add_argument("--root", required=True)
    gate.add_argument("--source-revision", required=True)
    gate.add_argument("--source-tree-sha256", required=True)
    gate.add_argument("--solana-cli-version", required=True)
    gate.add_argument("--build-run-id", required=True)
    return top


def main(argv: Sequence[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
        if arguments.command == "emit":
            emit(arguments)
            return 0
        if arguments.command == "verify":
            root = Path(arguments.root).resolve(strict=True)
            descriptor = Path(arguments.descriptor).resolve(strict=True)
            verified = verify_descriptor(
                root,
                descriptor,
                expected_label=arguments.label,
                expected_package=arguments.package,
            )
            if arguments.elf is not None:
                supplied = Path(arguments.elf)
                regular(supplied, "consumer ELF")
                if (
                    supplied.resolve(strict=True) != supplied
                    or supplied != verified["elf_path"]
                ):
                    refuse(
                        "consumer ELF is stale, adjacent, renamed, or not the descriptor path"
                    )
            print(
                json.dumps(
                    {
                        "schema": "dclutch-sbf-artifact-selection-v1",
                        "label": verified["descriptor"]["label"],
                        "package": verified["descriptor"]["package"],
                        "source_revision": verified["descriptor"]["source_revision"],
                        "source_tree_sha256": verified["descriptor"][
                            "source_tree_sha256"
                        ],
                        "build_run_id": verified["descriptor"]["build_run_id"],
                        "provenance_path": str(descriptor),
                        "provenance_sha256": sha256_file(descriptor),
                        "elf_path": str(verified["elf_path"])
                        if verified["elf_path"]
                        else None,
                        "elf_sha256": sha256_file(verified["elf_path"])
                        if verified["elf_path"]
                        else None,
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
            return 0
        if arguments.command == "emit-gate":
            emit_gate(arguments)
            return 0
        selected = select_gate_role(
            Path(arguments.gate).resolve(strict=True),
            arguments.gate_sha256,
            arguments.role,
        )
        print(json.dumps(selected, indent=2, sort_keys=True))
        return 0
    except (OSError, Refusal) as error:
        print(f"SBF ARTIFACT PROVENANCE REFUSED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
