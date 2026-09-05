#!/usr/bin/env python3
"""Run and verify both public, read-only Direct route wrappers from one pack.

This is deliberately a devnet-only evidence campaign.  It builds the Rust
producer and TypeScript CLI from the checked candidate's archived source,
invokes the two public CLI commands without a signer, and joins their actual
machine reports to the checked release pack and durable Direct journal.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from typing import Any, Mapping, NoReturn, Sequence


_PACK_TOOL_PATH = Path(__file__).with_name("successor_campaign_pack.py")
_PACK_TOOL_SPEC = importlib.util.spec_from_file_location(
    "dclutch_source_pinned_successor_campaign_pack", _PACK_TOOL_PATH
)
if _PACK_TOOL_SPEC is None or _PACK_TOOL_SPEC.loader is None:
    raise RuntimeError(f"cannot load source-pinned pack verifier: {_PACK_TOOL_PATH}")
pack_tool = importlib.util.module_from_spec(_PACK_TOOL_SPEC)
_PACK_TOOL_SPEC.loader.exec_module(pack_tool)


SCHEMA = "dclutch-public-route-campaign-v1"
EVIDENCE_BASENAME = "PUBLIC_ROUTE_CAMPAIGN.json"
DEVNET_GENESIS_HASH = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
RELEASE_REPORT_SCHEMA = "dclutch-devnet-checked-execution-release-report-v1"
DIRECT_REPORT_SCHEMA = "dclutch-devnet-direct-hot-route-manifest-report-v1"
DIRECT_MANIFEST_FORMAT = "dclutch-direct-hot-route-manifest-v3"
DIRECT_SESSION_SCHEMA = "dclutch-devnet-direct-trade-private-session-v1"
DIRECT_PRODUCER_JOURNAL_SCHEMA = "dclutch-devnet-direct-trade-producer-journal-v1"
MAX_REPORT_BYTES = 1024 * 1024
SOURCE_FILES = {
    "cli_route": "source/packages/dclutch-cli/src/commands/route.ts",
    "cli_main": "source/packages/dclutch-cli/src/main.ts",
    "cli_package": "source/packages/dclutch-cli/package.json",
    "cli_lock": "source/packages/dclutch-cli/package-lock.json",
    "sdk_package": "source/packages/dclutch-sdk/package.json",
    "sdk_lock": "source/packages/dclutch-sdk/package-lock.json",
    "rust_producer": (
        "source/tools/local-validator/bootstrap/successor/src/"
        "direct_hot_route_manifest.rs"
    ),
    "rust_manifest": "source/tools/local-validator/bootstrap/successor/Cargo.toml",
    # One workspace, one Cargo.lock: the successor stopped being its own
    # workspace root in the fold, so the root lock is what pins this producer.
    "rust_lock": "source/Cargo.lock",
}
PRODUCER_JOURNAL_FIELDS = {
    "schema",
    "phase",
    "cluster",
    "genesisHash",
    "plan",
    "planSha256",
    "marketInput",
    "marketInputSha256",
    "campaignReport",
    "campaignReportSha256",
    "buyerParticipant",
    "buyerParticipantSha256",
    "checkedExecutionRelease",
    "checkedExecutionReleaseSha256",
    "sellerTicket",
    "sellerTicketSha256",
    "buyerTicket",
    "buyerTicketSha256",
    "payer",
    "payerKeypair",
    "observationSlot",
    "publicManifest",
    "publicManifestSha256",
    "publicManifestBase64",
    "privateSession",
    "privateSessionSha256",
    "privateSessionBase64",
    "journalDir",
    "evidenceFile",
    "previousStateSha256",
    "stateSha256",
}


class Refusal(RuntimeError):
    """The campaign did not execute the exact two-wrapper evidence path."""


def refuse(message: str) -> NoReturn:
    raise Refusal(message)


def canonical_directory(value: str | Path, label: str) -> Path:
    supplied = Path(value)
    if not supplied.is_absolute():
        refuse(f"{label} must be absolute")
    resolved = supplied.resolve(strict=True)
    if supplied != resolved or not resolved.is_dir():
        refuse(f"{label} must be one exact canonical directory")
    return resolved


def new_root(value: str | Path) -> Path:
    output = Path(value)
    if not output.is_absolute() or output.exists() or output.is_symlink():
        refuse("--output-root must be an absolute new path")
    parent = output.parent.resolve(strict=True)
    if parent != output.parent:
        refuse("--output-root parent must be canonical")
    output.mkdir(mode=0o755)
    return output


def write_new_bytes(path: Path, value: bytes) -> None:
    if path.exists() or path.is_symlink():
        refuse(f"output already exists: {path}")
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(value)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def version(arguments: Sequence[str], label: str, cwd: Path | None = None) -> str:
    result = subprocess.run(arguments, cwd=cwd, capture_output=True, check=False)
    if result.returncode != 0 or not result.stdout:
        refuse(f"could not identify {label}")
    try:
        text = result.stdout.decode().strip()
    except UnicodeDecodeError:
        refuse(f"{label} version is not UTF-8")
    if not text or "\n" in text:
        refuse(f"{label} version is not one line")
    return text


def require_node_runtime() -> tuple[Path, str]:
    node_command = shutil.which("node")
    if node_command is None:
        refuse("Node.js is not installed")
    node_binary = Path(node_command).resolve(strict=True)
    pack_tool.regular(node_binary, "Node.js runtime")
    node_version = version([str(node_binary), "--version"], "Node.js")
    matched = re.fullmatch(r"v([0-9]+)\.([0-9]+)\.([0-9]+)", node_version)
    if matched is None or tuple(map(int, matched.groups())) < (22, 13, 0):
        refuse("the public CLI requires Node.js 22.13.0 or newer")
    return node_binary, node_version


def run_logged(arguments: Sequence[str], cwd: Path, log: Path, label: str) -> None:
    with log.open("xb") as output:
        output.write((json.dumps({"argv": list(arguments), "cwd": str(cwd)}) + "\n").encode())
        output.flush()
        result = subprocess.run(arguments, cwd=cwd, stdout=output, stderr=subprocess.STDOUT)
    if result.returncode != 0:
        refuse(f"{label} exited {result.returncode}; inspect {log}")


def run_report(
    arguments: Sequence[str], cwd: Path, report_path: Path, stderr_path: Path, label: str
) -> dict[str, Any]:
    result = subprocess.run(arguments, cwd=cwd, capture_output=True, check=False)
    write_new_bytes(stderr_path, result.stderr)
    if result.returncode != 0:
        detail = result.stderr.decode(errors="replace").strip()[:4096]
        refuse(f"{label} exited {result.returncode}: {detail}")
    if not result.stdout or len(result.stdout) > MAX_REPORT_BYTES:
        refuse(f"{label} emitted an empty or oversized machine report")
    write_new_bytes(report_path, result.stdout)
    return pack_tool.read_json(report_path, f"{label} machine report")


def journal_manifest_bytes(root: Path) -> tuple[bytes, int]:
    rows: list[str] = []
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root).as_posix()
        if "\t" in relative or "\n" in relative:
            refuse("Direct journal contains a path unsafe for its tree manifest")
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode):
            refuse(f"Direct journal contains a symlink: {path}")
        if stat.S_ISDIR(mode):
            continue
        if not stat.S_ISREG(mode):
            refuse(f"Direct journal contains a non-regular entry: {path}")
        resolved = path.resolve(strict=True)
        if resolved != path or root not in resolved.parents:
            refuse(f"Direct journal entry aliases or escapes its root: {path}")
        rows.append(f"{relative}\t{path.stat().st_size}\t{pack_tool.sha256_file(path)}\n")
    if not rows:
        refuse("Direct journal is empty")
    return "".join(rows).encode(), len(rows)


def staged_source_manifest_bytes(root: Path) -> bytes:
    rows: list[str] = []
    for current_text, directories, files in os.walk(root):
        current = Path(current_text)
        kept: list[str] = []
        for name in sorted(directories):
            if name in {"node_modules", "dist"}:
                continue
            directory = current / name
            if directory.is_symlink():
                refuse(f"staged first-party source contains a symlink: {directory}")
            kept.append(name)
        directories[:] = kept
        for name in sorted(files):
            path = current / name
            relative = path.relative_to(root).as_posix()
            if "\t" in relative or "\n" in relative:
                refuse("staged first-party source contains a path unsafe for its manifest")
            mode = path.lstat().st_mode
            if not stat.S_ISREG(mode):
                refuse(f"staged first-party source contains a non-regular file: {path}")
            rows.append(
                f"{relative}\t{path.stat().st_size}\t{pack_tool.sha256_file(path)}\n"
            )
    if not rows:
        refuse("staged first-party CLI/SDK source is empty")
    return "".join(rows).encode()


def artifact_by_role(pack: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    return {item["role"]: item for item in pack["artifacts"]}


def canonical_base64(value: Any, label: str) -> bytes:
    if not isinstance(value, str) or not value:
        refuse(f"{label} is not nonempty base64 text")
    try:
        decoded = base64.b64decode(value, validate=True)
    except (ValueError, binascii.Error):
        refuse(f"{label} is not canonical base64")
    if base64.b64encode(decoded).decode() != value:
        refuse(f"{label} is not canonical base64")
    return decoded


def producer_journal_binding(
    producer_path: Path,
    plan: Path,
    session: Path,
    journal_root: Path,
    pack_root: Path,
    pack: Mapping[str, Any],
) -> dict[str, Any]:
    producer_path = pack_tool.canonical_file(producer_path, "finalized Direct producer journal")
    producer = pack_tool.read_json(producer_path, "finalized Direct producer journal")
    if set(producer) != PRODUCER_JOURNAL_FIELDS:
        refuse("finalized Direct producer journal fields differ")
    observation = producer.get("observationSlot")
    if (
        producer.get("schema") != DIRECT_PRODUCER_JOURNAL_SCHEMA
        or producer.get("phase") != "finalized"
        or producer.get("cluster") != "devnet"
        or producer.get("genesisHash") != DEVNET_GENESIS_HASH
        or isinstance(observation, bool)
        or not isinstance(observation, int)
        or observation <= 0
    ):
        refuse("Direct producer journal is not one finalized devnet producer")
    for field in (
        "planSha256",
        "marketInputSha256",
        "campaignReportSha256",
        "buyerParticipantSha256",
        "checkedExecutionReleaseSha256",
        "sellerTicketSha256",
        "buyerTicketSha256",
        "publicManifestSha256",
        "privateSessionSha256",
        "previousStateSha256",
        "stateSha256",
    ):
        pack_tool.require_hex(producer.get(field), 64, f"Direct producer {field}")

    session_value = pack_tool.read_json(session, "exact devnet Direct session")
    if (
        producer.get("plan") != str(plan)
        or producer.get("planSha256") != pack_tool.sha256_file(plan)
        or producer.get("privateSession") != str(session)
        or producer.get("privateSessionSha256") != pack_tool.sha256_file(session)
        or producer.get("journalDir") != str(journal_root)
        or producer.get("payerKeypair") != session_value.get("payerKeypair")
        or producer.get("publicManifest") != session_value.get("publicManifest")
        or producer.get("publicManifestSha256")
        != session_value.get("publicManifestSha256")
        or producer.get("marketInput") != session_value.get("marketInput")
    ):
        refuse("Direct producer journal does not bind the exact plan, session, and journal")

    file_fields = {
        "market_input": ("marketInput", "marketInputSha256"),
        "campaign_report": ("campaignReport", "campaignReportSha256"),
        "buyer_participant": ("buyerParticipant", "buyerParticipantSha256"),
        "checked_execution_release": (
            "checkedExecutionRelease",
            "checkedExecutionReleaseSha256",
        ),
        "seller_ticket": ("sellerTicket", "sellerTicketSha256"),
        "buyer_ticket": ("buyerTicket", "buyerTicketSha256"),
        "public_manifest": ("publicManifest", "publicManifestSha256"),
    }
    sources: dict[str, Any] = {}
    paths: dict[str, Path] = {}
    for label, (path_field, digest_field) in file_fields.items():
        path = pack_tool.canonical_file(
            producer.get(path_field, ""), f"Direct producer {label}"
        )
        if pack_tool.sha256_file(path) != producer[digest_field]:
            refuse(f"Direct producer {label} digest differs")
        paths[label] = path
        sources[label] = pack_tool.absolute_evidence(path, f"Direct producer {label}")
    if canonical_base64(
        producer.get("publicManifestBase64"), "Direct producer public manifest"
    ) != paths["public_manifest"].read_bytes():
        refuse("Direct producer embedded public manifest differs from its file")
    if canonical_base64(
        producer.get("privateSessionBase64"), "Direct producer private session"
    ) != session.read_bytes():
        refuse("Direct producer embedded private session differs from its file")
    pack_checked = pack_tool.root_path(
        pack_root,
        pack["release"]["checked_execution_release_set"]["canonical_path"],
        "pack checked execution release",
    )
    if paths["checked_execution_release"].read_bytes() != pack_checked.read_bytes():
        refuse("Direct producer checked execution release is not byte-identical to the pack")
    return {
        "journal": pack_tool.absolute_evidence(
            producer_path, "finalized Direct producer journal"
        ),
        "sources": sources,
        "observation_slot": observation,
        "state_sha256": producer["stateSha256"],
        "previous_state_sha256": producer["previousStateSha256"],
    }


def validate_release_report(
    report: Mapping[str, Any], output: Path, pack: Mapping[str, Any]
) -> None:
    expected_fields = {
        "schema",
        "output",
        "bytes",
        "sha256",
        "executionReleaseSetId",
        "checkedExecutionReleaseSetId",
    }
    if set(report) != expected_fields or report.get("schema") != RELEASE_REPORT_SCHEMA:
        refuse("public checked-execution report fields differ")
    if report.get("output") != str(output):
        refuse("public checked-execution report names another output")
    pack_tool.regular(output, "public checked execution release")
    if (
        report.get("bytes") != output.stat().st_size
        or report.get("sha256") != pack_tool.sha256_file(output)
        or report.get("executionReleaseSetId")
        != pack["release"]["execution_release_set_id"]
        or report.get("checkedExecutionReleaseSetId")
        != pack["release"]["checked_execution_release_set_id"]
    ):
        refuse("public checked-execution report differs from its output or pack")


def validate_direct_report(
    report: Mapping[str, Any], output: Path, pack: Mapping[str, Any]
) -> None:
    expected_fields = {
        "schema",
        "format",
        "output",
        "bytes",
        "sha256",
        "market",
        "payer",
        "lookupTable",
        "lookupTableCreationSlot",
        "checkedInfrastructureSha256",
    }
    if (
        set(report) != expected_fields
        or report.get("schema") != DIRECT_REPORT_SCHEMA
        or report.get("format") != DIRECT_MANIFEST_FORMAT
    ):
        refuse("public Direct route report fields differ")
    if report.get("output") != str(output):
        refuse("public Direct route report names another output")
    pack_tool.regular(output, "public Direct route manifest")
    manifest = pack_tool.read_json(output, "public Direct route manifest")
    if manifest.get("format") != DIRECT_MANIFEST_FORMAT:
        refuse("public Direct route output has another format")
    if (
        report.get("bytes") != output.stat().st_size
        or report.get("sha256") != pack_tool.sha256_file(output)
        or report.get("checkedInfrastructureSha256")
        != pack["release"]["checked_infrastructure"]["sha256"]
    ):
        refuse("public Direct route report differs from its output or pack")


def source_evidence(root: Path) -> dict[str, Any]:
    return {
        name: pack_tool.absolute_evidence(
            pack_tool.root_path(root, relative, f"public route {name} source"),
            f"public route {name} source",
        )
        for name, relative in SOURCE_FILES.items()
    }


def command_vectors(
    *,
    node: str,
    launcher: Path,
    bootstrap: Path,
    rpc_url: str,
    acknowledgment: str,
    plan: Path,
    session: Path,
    artifacts: Mapping[str, Mapping[str, Any]],
    pack_root: Path,
    checked_output: Path,
    direct_output: Path,
) -> tuple[list[str], list[str]]:
    release = [
        node,
        str(launcher),
        "--json",
        "--rpc",
        rpc_url,
        "--i-mean-devnet",
        acknowledgment,
        "--bootstrap-bin",
        str(bootstrap),
        "route",
        "release-set",
        "--plan",
        str(plan),
        "--expected-plan-sha256",
        pack_tool.sha256_file(plan),
    ]
    for role in ("core", "claims", "trading", "resolution", "custody"):
        checked = artifacts[role]["checked_manifest"]
        release.extend(
            [
                f"--{role}-checked",
                str(
                    pack_tool.root_path(
                        pack_root, checked["canonical_path"], f"{role} checked release"
                    )
                ),
                f"--expected-{role}-checked-sha256",
                checked["sha256"],
            ]
        )
    release.extend(["--output", str(checked_output)])

    direct = [
        node,
        str(launcher),
        "--json",
        "--rpc",
        rpc_url,
        "--session",
        str(session),
        "--i-mean-devnet",
        acknowledgment,
        "--bootstrap-bin",
        str(bootstrap),
        "route",
        "direct",
        "--checked-execution-release",
        str(checked_output),
        "--expected-checked-execution-release-sha256",
        pack_tool.sha256_file(checked_output),
    ]
    for role in ("registry", "rent"):
        checked = artifacts[role]["checked_manifest"]
        direct.extend(
            [
                f"--{role}-checked",
                str(
                    pack_tool.root_path(
                        pack_root, checked["canonical_path"], f"{role} checked release"
                    )
                ),
                f"--expected-{role}-checked-sha256",
                checked["sha256"],
            ]
        )
    direct.extend(["--output", str(direct_output)])
    return release, direct


def assert_source_pinned_runner(
    root: Path, pack: Mapping[str, Any], executing: Path | None = None
) -> None:
    pinned = pack_tool.verify_evidence(
        root,
        pack["verifier"]["public_route_campaign"],
        "source-pinned public route campaign runner",
    )
    current = (executing or Path(__file__)).resolve(strict=True)
    pack_tool.regular(current, "executing public route campaign runner")
    if current != pinned or current.read_bytes() != pinned.read_bytes():
        refuse("execute the public route campaign runner from the pack's exact source")


def run(arguments: argparse.Namespace) -> None:
    if arguments.i_mean_devnet != DEVNET_GENESIS_HASH:
        refuse(f"--i-mean-devnet must equal {DEVNET_GENESIS_HASH}")
    pack_path = pack_tool.canonical_file(arguments.pack, "successor campaign release pack")
    pack_root, pack = pack_tool.verify_pack(pack_path)
    assert_source_pinned_runner(pack_root, pack)
    plan = pack_tool.canonical_file(arguments.plan, "exact devnet successor plan")
    session = pack_tool.canonical_file(arguments.session, "exact devnet Direct session")
    journal = canonical_directory(arguments.direct_journal, "exact devnet Direct journal")
    producer_journal = pack_tool.canonical_file(
        arguments.producer_journal, "finalized Direct producer journal"
    )
    session_value = pack_tool.read_json(session, "exact devnet Direct session")
    if (
        session_value.get("schema") != DIRECT_SESSION_SCHEMA
        or session_value.get("plan") != str(plan)
        or session_value.get("journalDir") != str(journal)
    ):
        refuse("devnet Direct session does not bind the exact plan and journal inputs")
    producer_binding = producer_journal_binding(
        producer_journal, plan, session, journal, pack_root, pack
    )
    output = new_root(arguments.output_root)
    build = output / "build"
    reports = output / "reports"
    products = output / "products"
    build.mkdir()
    reports.mkdir()
    products.mkdir()
    node_binary, node_version = require_node_runtime()
    npm_version = version(["npm", "--version"], "npm")
    cargo_version = version(
        ["cargo", "--version"], "Cargo", pack_root / "source"
    )
    rustc_version = version(
        ["rustc", "--version"], "Rust compiler", pack_root / "source"
    )

    journal_bytes, journal_files = journal_manifest_bytes(journal)
    journal_manifest = output / "direct-journal-files.tsv"
    write_new_bytes(journal_manifest, journal_bytes)

    staged = build / "cli-source" / "packages"
    staged.mkdir(parents=True)
    copy_ignore = shutil.ignore_patterns("node_modules", "dist", ".DS_Store")
    for package in ("dclutch-cli", "dclutch-sdk"):
        source = pack_root / "source" / "packages" / package
        shutil.copytree(source, staged / package, ignore=copy_ignore, symlinks=False)
    cli_root = staged / "dclutch-cli"
    sdk_root = staged / "dclutch-sdk"
    source_before = build / "staged-source-before.tsv"
    source_after = build / "staged-source-after.tsv"
    write_new_bytes(source_before, staged_source_manifest_bytes(staged.parent))
    sdk_npm_install_log = build / "sdk-npm-ci.log"
    npm_install_log = build / "npm-ci.log"
    npm_build_log = build / "npm-build.log"
    run_logged(
        ["npm", "ci", "--no-audit", "--no-fund"],
        sdk_root,
        sdk_npm_install_log,
        "source-pinned SDK dependency installation",
    )
    run_logged(
        ["npm", "ci", "--no-audit", "--no-fund"],
        cli_root,
        npm_install_log,
        "source-pinned CLI dependency installation",
    )
    run_logged(["npm", "run", "build"], cli_root, npm_build_log, "source-pinned CLI build")
    write_new_bytes(source_after, staged_source_manifest_bytes(staged.parent))
    if source_before.read_bytes() != source_after.read_bytes():
        refuse("npm dependency installation or CLI build changed first-party staged source")
    launcher = cli_root / "bin" / "dclutch-terminal.mjs"
    bundle = cli_root / "dist" / "dclutch-terminal.mjs"
    pack_tool.regular(launcher, "built public CLI launcher")
    pack_tool.regular(bundle, "built public CLI bundle")

    bootstrap_target = build / "bootstrap-target"
    bootstrap_log = build / "bootstrap-build.log"
    bootstrap_manifest = pack_root / SOURCE_FILES["rust_manifest"]
    run_logged(
        [
            "cargo",
            "build",
            "--release",
            "--locked",
            "--offline",
            "--manifest-path",
            str(bootstrap_manifest),
            "--target-dir",
            str(bootstrap_target),
        ],
        pack_root / "source",
        bootstrap_log,
        "source-pinned Rust producer build",
    )
    bootstrap = bootstrap_target / "release" / "dclutch-local-successor-bootstrap"
    pack_tool.regular(bootstrap, "built Rust route producer")

    checked_output = products / "checked-execution-release.bin"
    direct_output = products / "direct-hot-route.json"
    artifacts = artifact_by_role(pack)
    release_command, direct_command = command_vectors(
        node=str(node_binary),
        launcher=launcher,
        bootstrap=bootstrap,
        rpc_url=arguments.rpc_url,
        acknowledgment=arguments.i_mean_devnet,
        plan=plan,
        session=session,
        artifacts=artifacts,
        pack_root=pack_root,
        checked_output=checked_output,
        direct_output=direct_output,
    )
    release_report_path = reports / "checked-execution-release.json"
    release_stderr = reports / "checked-execution-release.stderr"
    release_report = run_report(
        release_command,
        output,
        release_report_path,
        release_stderr,
        "public checked-execution wrapper",
    )
    validate_release_report(release_report, checked_output, pack)
    pack_checked = pack_tool.root_path(
        pack_root,
        pack["release"]["checked_execution_release_set"]["canonical_path"],
        "pack checked execution release",
    )
    if checked_output.read_bytes() != pack_checked.read_bytes():
        refuse("public checked-execution output is not byte-identical to the pack")

    direct_report_path = reports / "direct-hot-route.json"
    direct_stderr = reports / "direct-hot-route.stderr"
    direct_report = run_report(
        direct_command,
        output,
        direct_report_path,
        direct_stderr,
        "public Direct route wrapper",
    )
    validate_direct_report(direct_report, direct_output, pack)

    evidence = {
        "schema": SCHEMA,
        "evidence_level": "exact-source-public-cli-read-only-devnet-route",
        "not_a_deployment": True,
        "read_only_devnet": True,
        "no_key_read_or_transaction_submitted": True,
        "rpc_url": arguments.rpc_url,
        "devnet_genesis_acknowledgment": arguments.i_mean_devnet,
        "source_revision": pack["source"]["revision"],
        "source_tree_sha256": pack["source"]["tree_sha256"],
        "release_pack": pack_tool.absolute_evidence(
            pack_path, "successor campaign release pack"
        ),
        "inputs": {
            "plan": pack_tool.absolute_evidence(plan, "exact devnet successor plan"),
            "direct_session": pack_tool.absolute_evidence(
                session, "exact devnet Direct session"
            ),
            "direct_producer": producer_binding,
            "direct_journal": {
                "canonical_path": str(journal),
                "file_count": journal_files,
                "tree_manifest": pack_tool.absolute_evidence(
                    journal_manifest, "Direct journal tree manifest"
                ),
            },
        },
        "source": source_evidence(pack_root),
        "toolchains": {
            "node": node_version,
            "npm": npm_version,
            "cargo": cargo_version,
            "rustc": rustc_version,
        },
        "build": {
            "node_runtime": pack_tool.absolute_evidence(node_binary, "Node.js runtime"),
            "staged_source_before": pack_tool.absolute_evidence(
                source_before, "staged first-party source before build"
            ),
            "staged_source_after": pack_tool.absolute_evidence(
                source_after, "staged first-party source after build"
            ),
            "sdk_npm_install_log": pack_tool.absolute_evidence(
                sdk_npm_install_log, "source-pinned SDK dependency installation log"
            ),
            "npm_install_log": pack_tool.absolute_evidence(
                npm_install_log, "source-pinned CLI dependency installation log"
            ),
            "npm_build_log": pack_tool.absolute_evidence(
                npm_build_log, "source-pinned CLI build log"
            ),
            "bootstrap_build_log": pack_tool.absolute_evidence(
                bootstrap_log, "source-pinned Rust producer build log"
            ),
            "cli_launcher": pack_tool.absolute_evidence(launcher, "built public CLI launcher"),
            "cli_bundle": pack_tool.absolute_evidence(bundle, "built public CLI bundle"),
            "bootstrap_binary": pack_tool.absolute_evidence(
                bootstrap, "built Rust route producer"
            ),
        },
        "wrappers": {
            "checked_execution_release": {
                "command": release_command,
                "report": pack_tool.absolute_evidence(
                    release_report_path, "public checked-execution machine report"
                ),
                "stderr": pack_tool.absolute_evidence(
                    release_stderr, "public checked-execution stderr"
                ),
                "output": pack_tool.absolute_evidence(
                    checked_output, "public checked execution release"
                ),
            },
            "direct_hot_route": {
                "command": direct_command,
                "report": pack_tool.absolute_evidence(
                    direct_report_path, "public Direct route machine report"
                ),
                "stderr": pack_tool.absolute_evidence(
                    direct_stderr, "public Direct route stderr"
                ),
                "output": pack_tool.absolute_evidence(
                    direct_output, "public Direct route manifest"
                ),
            },
        },
    }
    evidence_path = output / EVIDENCE_BASENAME
    pack_tool.atomic_new(evidence_path, evidence)
    verify_value(evidence_path, evidence)
    print(f"public route campaign evidence={evidence_path}")
    print(f"public route campaign sha256={pack_tool.sha256_file(evidence_path)}")


def evidence_path(value: Any, label: str) -> Path:
    try:
        return pack_tool.verify_absolute_evidence(value, label)
    except pack_tool.Refusal as error:
        refuse(str(error))


def verify_value(path: Path, evidence: Mapping[str, Any]) -> None:
    expected_top = {
        "schema",
        "evidence_level",
        "not_a_deployment",
        "read_only_devnet",
        "no_key_read_or_transaction_submitted",
        "rpc_url",
        "devnet_genesis_acknowledgment",
        "source_revision",
        "source_tree_sha256",
        "release_pack",
        "inputs",
        "source",
        "toolchains",
        "build",
        "wrappers",
    }
    if set(evidence) != expected_top or evidence.get("schema") != SCHEMA:
        refuse("public route campaign evidence header fields differ")
    if (
        evidence.get("evidence_level")
        != "exact-source-public-cli-read-only-devnet-route"
        or evidence.get("not_a_deployment") is not True
        or evidence.get("read_only_devnet") is not True
        or evidence.get("no_key_read_or_transaction_submitted") is not True
        or evidence.get("devnet_genesis_acknowledgment") != DEVNET_GENESIS_HASH
    ):
        refuse("public route campaign evidence header differs")
    pack_path = evidence_path(evidence["release_pack"], "successor campaign release pack")
    try:
        pack_root, pack = pack_tool.verify_pack(pack_path)
    except pack_tool.Refusal as error:
        refuse(str(error))
    assert_source_pinned_runner(pack_root, pack)
    if (
        evidence.get("source_revision") != pack["source"]["revision"]
        or evidence.get("source_tree_sha256") != pack["source"]["tree_sha256"]
    ):
        refuse("public route campaign source differs from its pack")
    if evidence.get("source") != source_evidence(pack_root):
        refuse("public route campaign source evidence differs from its pack")

    inputs = evidence.get("inputs")
    if not isinstance(inputs, dict) or set(inputs) != {
        "plan",
        "direct_session",
        "direct_producer",
        "direct_journal",
    }:
        refuse("public route campaign input fields differ")
    plan = evidence_path(inputs["plan"], "exact devnet successor plan")
    session = evidence_path(inputs["direct_session"], "exact devnet Direct session")
    journal = inputs["direct_journal"]
    if not isinstance(journal, dict) or set(journal) != {
        "canonical_path",
        "file_count",
        "tree_manifest",
    }:
        refuse("public route campaign journal fields differ")
    journal_root = canonical_directory(journal["canonical_path"], "exact devnet Direct journal")
    session_value = pack_tool.read_json(session, "exact devnet Direct session")
    if (
        session_value.get("schema") != DIRECT_SESSION_SCHEMA
        or session_value.get("plan") != str(plan)
        or session_value.get("journalDir") != str(journal_root)
    ):
        refuse("devnet Direct session does not bind the exact plan and journal inputs")
    manifest_path = evidence_path(journal["tree_manifest"], "Direct journal tree manifest")
    manifest_bytes, count = journal_manifest_bytes(journal_root)
    if journal.get("file_count") != count or manifest_path.read_bytes() != manifest_bytes:
        refuse("public route campaign Direct journal tree differs")
    producer_value = inputs["direct_producer"]
    if not isinstance(producer_value, dict) or set(producer_value) != {
        "journal",
        "sources",
        "observation_slot",
        "state_sha256",
        "previous_state_sha256",
    }:
        refuse("public route campaign Direct producer fields differ")
    producer_path = evidence_path(
        producer_value["journal"], "finalized Direct producer journal"
    )
    expected_producer = producer_journal_binding(
        producer_path, plan, session, journal_root, pack_root, pack
    )
    if producer_value != expected_producer:
        refuse("public route campaign Direct producer binding differs")

    build = evidence.get("build")
    expected_build = {
        "node_runtime",
        "staged_source_before",
        "staged_source_after",
        "sdk_npm_install_log",
        "npm_install_log",
        "npm_build_log",
        "bootstrap_build_log",
        "cli_launcher",
        "cli_bundle",
        "bootstrap_binary",
    }
    if not isinstance(build, dict) or set(build) != expected_build:
        refuse("public route campaign build fields differ")
    build_paths = {key: evidence_path(build[key], f"public route {key}") for key in expected_build}
    if (
        build_paths["staged_source_before"].read_bytes()
        != build_paths["staged_source_after"].read_bytes()
    ):
        refuse("public route staged first-party source changed during build")
    staged_root = canonical_directory(
        build_paths["cli_launcher"].parents[3], "staged public CLI source root"
    )
    if build_paths["staged_source_after"].read_bytes() != staged_source_manifest_bytes(
        staged_root
    ):
        refuse("public route staged first-party source changed after evidence creation")
    toolchains = evidence.get("toolchains")
    if not isinstance(toolchains, dict) or set(toolchains) != {"node", "npm", "cargo", "rustc"}:
        refuse("public route campaign toolchain fields differ")
    if not all(isinstance(value, str) and value for value in toolchains.values()):
        refuse("public route campaign toolchain value is empty")

    wrappers = evidence.get("wrappers")
    if not isinstance(wrappers, dict) or set(wrappers) != {
        "checked_execution_release",
        "direct_hot_route",
    }:
        refuse("public route campaign wrapper fields differ")
    checked = wrappers["checked_execution_release"]
    direct = wrappers["direct_hot_route"]
    wrapper_fields = {"command", "report", "stderr", "output"}
    if (
        not isinstance(checked, dict)
        or set(checked) != wrapper_fields
        or not isinstance(direct, dict)
        or set(direct) != wrapper_fields
    ):
        refuse("public route campaign wrapper evidence fields differ")
    checked_report_path = evidence_path(
        checked["report"], "public checked-execution machine report"
    )
    evidence_path(checked["stderr"], "public checked-execution stderr")
    checked_output = evidence_path(checked["output"], "public checked execution release")
    direct_report_path = evidence_path(direct["report"], "public Direct route machine report")
    evidence_path(direct["stderr"], "public Direct route stderr")
    direct_output = evidence_path(direct["output"], "public Direct route manifest")
    artifacts = artifact_by_role(pack)
    expected_release, expected_direct = command_vectors(
        node=str(build_paths["node_runtime"]),
        launcher=build_paths["cli_launcher"],
        bootstrap=build_paths["bootstrap_binary"],
        rpc_url=evidence["rpc_url"],
        acknowledgment=evidence["devnet_genesis_acknowledgment"],
        plan=plan,
        session=session,
        artifacts=artifacts,
        pack_root=pack_root,
        checked_output=checked_output,
        direct_output=direct_output,
    )
    if checked.get("command") != expected_release or direct.get("command") != expected_direct:
        refuse("public route campaign command vectors differ")
    checked_report = pack_tool.read_json(
        checked_report_path, "public checked-execution machine report"
    )
    direct_report = pack_tool.read_json(direct_report_path, "public Direct route machine report")
    validate_release_report(checked_report, checked_output, pack)
    validate_direct_report(direct_report, direct_output, pack)
    pack_checked = pack_tool.root_path(
        pack_root,
        pack["release"]["checked_execution_release_set"]["canonical_path"],
        "pack checked execution release",
    )
    if checked_output.read_bytes() != pack_checked.read_bytes():
        refuse("public checked-execution output is not byte-identical to the pack")
    if path.name != EVIDENCE_BASENAME:
        refuse(f"public route campaign evidence must be named {EVIDENCE_BASENAME}")


def verify(arguments: argparse.Namespace) -> None:
    path = pack_tool.canonical_file(arguments.evidence, "public route campaign evidence")
    evidence = pack_tool.read_json(path, "public route campaign evidence")
    verify_value(path, evidence)
    print(f"public route campaign verified sha256={pack_tool.sha256_file(path)}")


def parser() -> argparse.ArgumentParser:
    top = argparse.ArgumentParser(description=__doc__)
    commands = top.add_subparsers(dest="command", required=True)
    execute = commands.add_parser("run", help="build and invoke both exact public CLI wrappers")
    execute.add_argument("--pack", required=True)
    execute.add_argument("--rpc-url", required=True)
    execute.add_argument("--i-mean-devnet", required=True)
    execute.add_argument("--plan", required=True)
    execute.add_argument("--session", required=True)
    execute.add_argument("--direct-journal", required=True)
    execute.add_argument("--producer-journal", required=True)
    execute.add_argument("--output-root", required=True)
    check = commands.add_parser("verify", help="rehash and reverify a completed wrapper campaign")
    check.add_argument("--evidence", required=True)
    return top


def main(argv: Sequence[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
        if arguments.command == "run":
            run(arguments)
        else:
            verify(arguments)
        return 0
    except (OSError, Refusal, pack_tool.Refusal, ValueError) as error:
        print(f"PUBLIC ROUTE CAMPAIGN REFUSED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
