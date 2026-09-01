#!/usr/bin/env python3
"""Emit, verify, and materialize a checked successor-campaign release pack.

The checked-release candidate already owns fresh SBF compilation, exact frame
measurement, Loader images, checked release manifests, and their all-link gate.
This tool closes the next seam: it binds the candidate to the source-pinned
budget/licence authorities and turns the seven execution/infrastructure roles
into an exact ``SuccessorRunSpec`` input.  It never launches a validator,
signs, submits, deploys, funds, or publishes.

The pack is the candidate directory, kept as one tree.  All paths in the pack
manifest are root-relative; materialized run specs and attestations use the
current canonical absolute location, so the pack may be moved as a whole and
verified again before use.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from typing import Any, Mapping, NoReturn, Sequence


SCHEMA = "dclutch-successor-campaign-release-pack-v1"
SPEC_SCHEMA = "dclutch-local-successor-run-spec-v2"
LINEAGE_SCHEMA = "dclutch-current-source-infrastructure-lineage-v1"
LINEAGE_BINDING_SCHEMA = "dclutch-release-pack-lineage-binding-v1"
REPRODUCTION_SCHEMA = "dclutch-successor-release-pack-reproduction-v1"
PACK_BASENAME = "SUCCESSOR_CAMPAIGN_PACK.json"
MAX_JSON_BYTES = 32 * 1024 * 1024
HEX40 = re.compile(r"^[0-9a-f]{40}$")
HEX64 = re.compile(r"^[0-9a-f]{64}$")
CAMPAIGN_ROLES = ("registry", "core", "claims", "trading", "resolution", "custody", "rent")
ARTIFACT_ROLES = (
    "claims",
    "core",
    "custody",
    "dealer-accelerator",
    "general-accelerator",
    "registry",
    "rent",
    "resolution",
    "series-shadow",
    "trading",
)
GENESIS_SLOTS = {
    "registry": 11,
    "core": 13,
    "claims": 17,
    "trading": 19,
    "resolution": 23,
    "custody": 29,
    "rent": 31,
}
NODE_DISTRIBUTIONS = {
    "https://nodejs.org/dist/v26.4.0/node-v26.4.0-linux-x64.tar.xz": (
        "node-v26.4.0-linux-x64.tar.xz",
        "5c4286dcd5bbd5acb1ccc7eb0e088bd5eb1e3affad671ee9364004f8f6a4a431",
    ),
    "https://nodejs.org/dist/v26.4.0/node-v26.4.0-darwin-arm64.tar.xz": (
        "node-v26.4.0-darwin-arm64.tar.xz",
        "bef4c7e75087c029835f519a7ba640eba52fa617fadb3a9049828ff3b45b57dd",
    ),
}
SPLINE_PRODUCT_FILES = (
    "portfolio.bin",
    "price-gate.bin",
    "product-basis.bin",
    "product.bin",
    "result-domain.bin",
)


class Refusal(RuntimeError):
    """Input is not one exact, source-bound campaign release pack."""


def refuse(message: str) -> NoReturn:
    raise Refusal(message)


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            refuse(f"JSON repeats key {key!r}")
        value[key] = item
    return value


def regular(path: Path, label: str) -> None:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        refuse(f"missing {label}: {path}")
    if not stat.S_ISREG(mode):
        refuse(f"{label} is not a regular non-symlink file: {path}")


def canonical_root(path: str | Path) -> Path:
    supplied = Path(path)
    root = supplied.resolve(strict=True)
    if supplied != root or not root.is_dir():
        refuse("pack root must be an exact canonical directory")
    return root


def root_path(root: Path, relative: str, label: str) -> Path:
    if (
        not isinstance(relative, str)
        or not relative
        or Path(relative).is_absolute()
        or ".." in Path(relative).parts
    ):
        refuse(f"{label} path is not canonical root-relative text")
    candidate = root / relative
    regular(candidate, label)
    resolved = candidate.resolve(strict=True)
    if resolved != candidate or root not in resolved.parents:
        refuse(f"{label} path escapes, aliases, or traverses a symlink")
    return candidate


def evidence(root: Path, relative: str, label: str) -> dict[str, Any]:
    path = root_path(root, relative, label)
    return {
        "canonical_path": relative,
        "bytes": path.stat().st_size,
        "sha256": sha256_file(path),
    }


def verify_evidence(root: Path, value: Any, label: str) -> Path:
    if not isinstance(value, dict) or set(value) != {"canonical_path", "bytes", "sha256"}:
        refuse(f"{label} evidence fields differ")
    path = root_path(root, value.get("canonical_path"), label)
    size = value.get("bytes")
    digest = value.get("sha256")
    if isinstance(size, bool) or not isinstance(size, int) or size < 0 or path.stat().st_size != size:
        refuse(f"{label} byte count differs")
    if not isinstance(digest, str) or not HEX64.fullmatch(digest) or sha256_file(path) != digest:
        refuse(f"{label} SHA-256 differs")
    return path


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


def read_kv(
    path: Path, label: str, *, repeatable: frozenset[str] = frozenset()
) -> dict[str, str]:
    regular(path, label)
    try:
        text = path.read_text()
    except UnicodeDecodeError as error:
        refuse(f"{label} is not UTF-8: {error}")
    if not text or not text.endswith("\n"):
        refuse(f"{label} is empty or not newline terminated")
    fields: dict[str, str] = {}
    for line in text.splitlines():
        if line.count("=") < 1:
            refuse(f"{label} contains a non key/value row")
        key, value = line.split("=", 1)
        if not key:
            refuse(f"{label} repeats or omits a key")
        if key in fields:
            if key in repeatable:
                continue
            refuse(f"{label} repeats or omits a key")
        fields[key] = value
    return fields


def require_hex(value: Any, width: int, label: str) -> str:
    pattern = HEX40 if width == 40 else HEX64
    if not isinstance(value, str) or not pattern.fullmatch(value):
        refuse(f"{label} is not {width}-digit lowercase hex")
    return value


def base58_32(hex_text: str) -> str:
    raw = bytes.fromhex(require_hex(hex_text, 64, "program identity"))
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    value = int.from_bytes(raw, "big")
    output = ""
    while value:
        value, remainder = divmod(value, 58)
        output = alphabet[remainder] + output
    leading = len(raw) - len(raw.lstrip(b"\0"))
    output = "1" * leading + output
    if not output or len(output) > 44:
        refuse("program identity does not encode as one canonical public key")
    return output


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


def canonical_file(path: str | Path, label: str) -> Path:
    supplied = Path(path)
    if not supplied.is_absolute():
        refuse(f"{label} must be absolute")
    resolved = supplied.resolve(strict=True)
    regular(resolved, label)
    if supplied != resolved:
        refuse(f"{label} path is not exact and canonical")
    return resolved


def absolute_evidence(path: Path, label: str) -> dict[str, Any]:
    canonical = canonical_file(path, label)
    return {
        "canonical_path": str(canonical),
        "bytes": canonical.stat().st_size,
        "sha256": sha256_file(canonical),
    }


def verify_absolute_evidence(value: Any, label: str) -> Path:
    if not isinstance(value, dict) or set(value) != {"canonical_path", "bytes", "sha256"}:
        refuse(f"{label} evidence fields differ")
    path = canonical_file(value.get("canonical_path", ""), label)
    if absolute_evidence(path, label) != value:
        refuse(f"{label} evidence differs")
    return path


def verify_checked_gate(root: Path, gate_path: Path, gate_sha256: str) -> dict[str, Any]:
    """Use the source-pinned gate verifier and return its exact gate JSON."""

    tool = root_path(
        root,
        "source/tools/release/artifact_provenance.py",
        "source-pinned artifact provenance verifier",
    )
    command = [
        sys.executable,
        str(tool),
        "select-gate-role",
        "--gate",
        str(gate_path),
        "--gate-sha256",
        gate_sha256,
        "--role",
        "core",
    ]
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if result.returncode != 0:
        refuse(f"checked Upgrade gate did not reauthenticate: {result.stderr.strip()}")
    return read_json(gate_path, "checked Upgrade gate")


def authority_values(root: Path) -> tuple[str, int, int]:
    toolchain_path = root_path(root, "source/rust-toolchain.toml", "host toolchain pin")
    try:
        toolchain = tomllib.loads(toolchain_path.read_text())
        host_channel = toolchain["toolchain"]["channel"]
    except (KeyError, tomllib.TOMLDecodeError, UnicodeDecodeError) as error:
        refuse(f"host toolchain pin is malformed: {error}")
    if not isinstance(host_channel, str) or not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", host_channel):
        refuse("host toolchain channel is not an exact stable version")

    budget_path = root_path(root, "source/tools/gauntlet/CU_BUDGETS.json", "CU budget authority")
    budgets = read_json(budget_path, "CU budget authority")
    try:
        compute = budgets["ceiling"]["compute_units"]
    except (KeyError, TypeError):
        refuse("CU budget authority omitted ceiling.compute_units")
    if isinstance(compute, bool) or not isinstance(compute, int) or compute <= 0:
        refuse("CU budget authority has an invalid compute ceiling")

    packet_path = root_path(
        root,
        "source/crates/dclutch-versioned-message-operator/src/lib.rs",
        "packet bound authority",
    )
    packet_text = packet_path.read_text()
    matches = re.findall(r"pub const PACKET_DATA_BYTES: usize = ([0-9_]+);", packet_text)
    if len(matches) != 1:
        refuse("packet authority does not expose exactly one PACKET_DATA_BYTES")
    packet = int(matches[0].replace("_", ""))
    if packet <= 0:
        refuse("packet authority has an invalid packet ceiling")
    return host_channel, compute, packet


def resolution_semantic_id(root: Path) -> str:
    path = root_path(
        root,
        "source/crates/dclutch-resolution-codec/src/lib.rs",
        "Resolution release authority",
    )
    text = path.read_text()
    matches = re.findall(
        r"RESOLUTION_CONTROLLER_RELEASE_PREIMAGE_V4: &\[u8\] =\s*\n?\s*b\"([^\"]+)\";",
        text,
    )
    if len(matches) != 1:
        refuse("Resolution release authority does not expose one canonical V4 preimage")
    return sha256_bytes(matches[0].encode())



def _lineage_release_fields(root: pathlib.Path, summary: Mapping[str, str]) -> dict:
    """The release fields the candidate's STATED lineage requires.

    A genesis cohort succeeds nothing, so it has no predecessor account to pin
    and no `infrastructure/predecessor-profile.bin` on disk. It says so by
    carrying `infrastructure_lineage`, rather than by quietly lacking a field
    -- which is the same rule the candidate summary itself follows.
    """
    lineage = summary_required(summary, "infrastructure_lineage")
    if lineage == "genesis":
        return {"infrastructure_lineage": "genesis"}
    if lineage != "succession":
        refuse(f"checked candidate summary states an unknown lineage: {lineage}")
    return {
        "predecessor_infrastructure_profile_sha256": require_hex(
            summary_required(summary, "predecessor_infrastructure_profile_sha256"),
            64,
            "predecessor infrastructure profile digest",
        ),
        "predecessor_infrastructure_profile": evidence(
            root,
            "infrastructure/predecessor-profile.bin",
            "predecessor infrastructure profile",
        ),
    }


def summary_required(summary: Mapping[str, str], key: str) -> str:
    value = summary.get(key)
    if value is None or value == "":
        refuse(f"checked candidate summary omitted {key}")
    return value


def archive_member_sha256(archive: Path, member: str, label: str) -> str:
    try:
        with tarfile.open(archive, "r:xz") as distribution:
            matches = [item for item in distribution.getmembers() if item.name == member]
            if len(matches) != 1 or not matches[0].isfile():
                refuse(f"Node archive does not contain one regular {label}")
            source = distribution.extractfile(matches[0])
            if source is None:
                refuse(f"Node archive {label} is unreadable")
            digest = hashlib.sha256()
            while chunk := source.read(1024 * 1024):
                digest.update(chunk)
            return digest.hexdigest()
    except (tarfile.TarError, EOFError, OSError) as error:
        refuse(f"Node archive is not one readable xz tar: {error}")


def node_toolchain_value(root: Path, summary: Mapping[str, str]) -> dict[str, Any]:
    source = summary_required(summary, "node_archive_source")
    distribution = NODE_DISTRIBUTIONS.get(source)
    if distribution is None:
        refuse("candidate Node archive source is not one pinned official v26.4.0 distribution")
    archive_name, official_sha = distribution
    archive = evidence(
        root,
        f"toolchain/{archive_name}",
        "preserved official Node distribution",
    )
    if (
        archive["sha256"] != official_sha
        or summary_required(summary, "node_archive_sha256") != official_sha
    ):
        refuse("candidate Node archive differs from the pinned official SHA-256")
    archive_path = root_path(
        root, archive["canonical_path"], "preserved official Node distribution"
    )
    distribution_root = archive_name.removesuffix(".tar.xz")
    node_sha = archive_member_sha256(
        archive_path, f"{distribution_root}/bin/node", "Node executable"
    )
    npm_sha = archive_member_sha256(
        archive_path,
        f"{distribution_root}/lib/node_modules/npm/bin/npm-cli.js",
        "npm CLI",
    )
    if (
        summary_required(summary, "node_version") != "v26.4.0"
        or summary_required(summary, "node_binary_sha256") != node_sha
        or summary_required(summary, "npm_cli_sha256") != npm_sha
    ):
        refuse("candidate Node/npm runtime differs from its preserved distribution")
    npm_version = summary_required(summary, "npm_version")
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", npm_version):
        refuse("candidate npm version is not canonical")
    return {
        "node_version": "v26.4.0",
        "npm_version": npm_version,
        "archive_source": source,
        "archive": archive,
        "node_binary_sha256": node_sha,
        "npm_cli_sha256": npm_sha,
    }


def host_substrate_value(
    summary: Mapping[str, str], host_channel: str
) -> dict[str, str]:
    rustc = summary_required(summary, "host_rustc_version")
    if not rustc.startswith(f"rustc {host_channel} "):
        refuse("executed host rustc differs from the source toolchain pin")
    values = {
        "rustc": rustc,
        "rustc_verbose_sha256": require_hex(
            summary_required(summary, "host_rustc_verbose_sha256"),
            64,
            "host rustc verbose-version digest",
        ),
        "cargo": summary_required(summary, "host_cargo_version"),
        "cc": summary_required(summary, "host_cc_version"),
        "linker": summary_required(summary, "host_linker_version"),
        "libc": summary_required(summary, "host_libc_version"),
        "os": summary_required(summary, "host_os"),
        "arch": summary_required(summary, "host_arch"),
        "kernel": summary_required(summary, "host_kernel"),
    }
    if values["os"] not in {"Linux", "Darwin"} or values["arch"] not in {
        "x86_64",
        "arm64",
    }:
        refuse("candidate host OS/architecture is not a supported builder substrate")
    return values


def spline_product_handoff_value(
    root: Path, summary: Mapping[str, str]
) -> dict[str, Any]:
    if summary.get("spline_product_handoff") != "passed":
        refuse("candidate did not pass the public spline Product handoff")
    smoke_path = root_path(
        root,
        "product-handoff/smoke/smoke-report.json",
        "spline Product handoff smoke report",
    )
    smoke = read_json(smoke_path, "spline Product handoff smoke report")
    if set(smoke) != {
        "schema",
        "key_free",
        "signs",
        "submits",
        "fixture",
        "fixture_sha256",
        "compiler_completion_sha256",
        "compiler_report_sha256",
        "sdk_inspection_sha256",
        "semantic_basis_id",
        "found_records",
    }:
        refuse("spline Product handoff smoke report fields differ")
    if (
        smoke.get("schema") != "dclutch/product-spline-handoff-smoke/v1"
        or smoke.get("key_free") is not True
        or smoke.get("signs") is not False
        or smoke.get("submits") is not False
    ):
        refuse("spline Product handoff smoke header differs")

    fixture_path = root_path(
        root,
        "source/docs/operator/examples/spline-product-degree2.json",
        "canonical spline Product fixture",
    )
    if (
        smoke.get("fixture") != str(fixture_path)
        or smoke.get("fixture_sha256") != sha256_file(fixture_path)
    ):
        refuse("spline Product handoff differs from its canonical source fixture")
    smoke_relative = "product-handoff/smoke"
    completion_path = root_path(
        root, f"{smoke_relative}/completion.json", "spline compiler completion"
    )
    compiler_report_path = root_path(
        root, f"{smoke_relative}/product/report.json", "spline compiler report"
    )
    inspection_path = root_path(
        root, f"{smoke_relative}/inspection.json", "spline SDK inspection"
    )
    for field, path, label in (
        ("compiler_completion_sha256", completion_path, "compiler completion"),
        ("compiler_report_sha256", compiler_report_path, "compiler report"),
        ("sdk_inspection_sha256", inspection_path, "SDK inspection"),
    ):
        if smoke.get(field) != sha256_file(path):
            refuse(f"spline Product handoff {label} digest differs")

    completion = read_json(completion_path, "spline compiler completion")
    compiler_output = root / f"{smoke_relative}/product"
    if set(completion) != {"schema", "output_dir", "report", "report_sha256"} or (
        completion.get("schema") != "dclutch/product-spline-authoring-completion/v1"
        or completion.get("output_dir") != str(compiler_output)
        or completion.get("report") != str(compiler_report_path)
        or completion.get("report_sha256") != smoke["compiler_report_sha256"]
    ):
        refuse("spline compiler completion differs from the archived output")

    compiler_report = read_json(compiler_report_path, "spline compiler report")
    compiler_report_fields = {
        "schema",
        "command",
        "key_free",
        "signs",
        "submits",
        "input_sha256",
        "registry_program",
        "product_outcome_count",
        "basis_width",
        "degree",
        "interior_multiplicity",
        "payout_scale",
        "rounding_boundary",
        "semantic_basis_id",
        "records",
        "verified_price_gate",
        # Added 2026-09-01 with the founding-band gate: how much of the ex-ante
        # question each cell takes, or `measured: false` and the reason. Always
        # present in the report, so it is required here rather than tolerated —
        # an archive produced before that commit will refuse, which is the
        # correct answer for an archive whose markets were never measured.
        "partition_quality",
    }
    if set(compiler_report) != compiler_report_fields or (
        compiler_report.get("schema") != "dclutch/product-spline-authoring-report/v1"
        or compiler_report.get("command") != "product-spline-compile-v1"
        or compiler_report.get("key_free") is not True
        or compiler_report.get("signs") is not False
        or compiler_report.get("submits") is not False
        or compiler_report.get("input_sha256") != smoke["fixture_sha256"]
        or compiler_report.get("semantic_basis_id") != smoke["semantic_basis_id"]
    ):
        refuse("spline compiler report fields or source binding differ")

    inspection = read_json(inspection_path, "spline SDK inspection")
    # `partition_quality` is subtracted because the SDK inspection does not yet
    # carry it: `packages/dclutch-cli/src/commands/product.ts:112` builds its
    # document field by field and stops at `verified_price_gate`. Routed to the
    # SDK/CLI lane; until it lands, requiring the key here would refuse a
    # correct inspection for a field its producer never had.
    # BOTH surfaces carry `partition_quality`; an earlier sweep subtracted it
    # here, which asserted the SDK does not emit it and made the pack refuse
    # every genesis candidate. Measured on the real artifacts: the only
    # asymmetries are `command` (compiler only) and `report`/`found_records`
    # (SDK only).
    inspection_fields = compiler_report_fields - {"command"} | {"report", "found_records"}
    found = smoke.get("found_records")
    if not isinstance(found, dict) or set(found) != {
        "productRecord",
        "resultDomainRecord",
        "portfolioRecord",
        "linkedBasisRecord",
        "priceGateRecord",
    } or any(not isinstance(item, str) or item == "" for item in found.values()):
        refuse("spline Product handoff Found record coordinates differ")
    semantic = require_hex(
        smoke.get("semantic_basis_id"), 64, "spline Product semantic basis"
    )
    if (
        set(inspection) != inspection_fields
        or inspection.get("schema") != "dclutch/product-spline-inspection/v1"
        or inspection.get("report") != str(compiler_report_path)
        or inspection.get("key_free") is not True
        or inspection.get("signs") is not False
        or inspection.get("submits") is not False
        or inspection.get("input_sha256") != smoke["fixture_sha256"]
        or inspection.get("semantic_basis_id") != semantic
        or inspection.get("found_records") != found
        or any(
            inspection.get(key) != compiler_report.get(key)
            for key in compiler_report_fields
            - {"schema", "command", "verified_price_gate", "partition_quality"}
        )
    ):
        refuse("spline SDK inspection differs from the smoke report")
    # `partition_quality` is the same fact in two spellings -- the TypeScript
    # SDK emits camelCase, the Rust compiler snake_case -- so raw equality can
    # never hold and EXCLUDING it would let the two diverge unnoticed. Compare
    # it normalised instead, which is the only form that actually checks the
    # SDK reports the partition the compiler measured.
    sdk_quality = _normalise_keys(inspection.get("partition_quality"))
    compiler_quality = _normalise_keys(compiler_report.get("partition_quality"))
    if not isinstance(sdk_quality, dict) or not isinstance(compiler_quality, dict):
        refuse("spline partition quality is missing from a surface that must carry it")
    # The SDK adds one DERIVED field the compiler does not: `degenerate`. Every
    # measured fact must agree exactly, and the derived one must follow from
    # those facts -- ignoring it would let the SDK tell a client a partition is
    # fine while the numbers beside it say otherwise.
    shared = set(sdk_quality) & set(compiler_quality)
    if set(compiler_quality) - shared or any(
        sdk_quality[key] != compiler_quality[key] for key in shared
    ):
        refuse("spline SDK partition quality differs from the compiler's")
    if set(sdk_quality) - shared != {"degenerate"}:
        refuse("spline SDK partition quality carries an unexpected field")
    dominant = compiler_quality.get("dominant_share_bps")
    ceiling = compiler_quality.get("max_cell_share_bps")
    if not isinstance(dominant, int) or not isinstance(ceiling, int):
        refuse("spline partition quality shares are not exact integers")
    if sdk_quality.get("degenerate") is not (dominant >= ceiling):
        refuse("spline SDK degeneracy disagrees with the compiler's own shares")
    compiler_gate = compiler_report.get("verified_price_gate")
    inspected_gate = inspection.get("verified_price_gate")
    if (
        not isinstance(compiler_gate, dict)
        or set(compiler_gate) != {"scale", "mass", "degree", "width", "atom_count", "prices"}
        or not isinstance(inspected_gate, dict)
        or set(inspected_gate) != {"scale", "mass", "degree", "width", "atomCount", "prices"}
        or inspected_gate
        != {
            "scale": compiler_gate["scale"],
            "mass": compiler_gate["mass"],
            "degree": compiler_gate["degree"],
            "width": compiler_gate["width"],
            "atomCount": compiler_gate["atom_count"],
            "prices": compiler_gate["prices"],
        }
    ):
        refuse("spline SDK price-gate inspection differs from the compiler report")

    smoke_evidence = evidence(
        root, f"{smoke_relative}/smoke-report.json", "spline Product handoff smoke report"
    )
    cli_bundle = evidence(root, "product-handoff/dclutch-terminal.mjs", "built public CLI bundle")
    successor = evidence(
        root,
        "product-handoff/dclutch-local-successor-bootstrap",
        "built spline Product producer",
    )
    if (
        summary_required(summary, "spline_product_handoff_report_sha256")
        != smoke_evidence["sha256"]
        or summary_required(summary, "spline_product_cli_bundle_sha256")
        != cli_bundle["sha256"]
        or summary_required(summary, "spline_product_successor_sha256")
        != successor["sha256"]
    ):
        refuse("candidate summary differs from spline Product handoff evidence")
    products = {
        name: evidence(
            root,
            f"{smoke_relative}/product/{name}",
            f"spline Product output {name}",
        )
        for name in SPLINE_PRODUCT_FILES
    }
    records = compiler_report.get("records")
    record_files = {
        "portfolio": "portfolio.bin",
        "price_gate": "price-gate.bin",
        "product_basis": "product-basis.bin",
        "product": "product.bin",
        "result_domain": "result-domain.bin",
    }
    if not isinstance(records, dict) or set(records) != set(record_files):
        refuse("spline compiler report record set differs")
    for record_name, file_name in record_files.items():
        record = records[record_name]
        if not isinstance(record, dict) or set(record) != {
            "file",
            "bytes",
            "schema_id",
            "content_sha256",
            "raw_account",
            "staging_account",
        } or (
            record.get("file") != file_name
            or record.get("bytes") != products[file_name]["bytes"]
            or record.get("content_sha256") != products[file_name]["sha256"]
        ):
            refuse(f"spline compiler report record differs for {record_name}")
        require_hex(record.get("schema_id"), 64, f"spline {record_name} schema identity")
    expected_found = {
        "productRecord": records["product"]["raw_account"],
        "resultDomainRecord": records["result_domain"]["raw_account"],
        "portfolioRecord": records["portfolio"]["raw_account"],
        "linkedBasisRecord": records["product_basis"]["raw_account"],
        "priceGateRecord": records["price_gate"]["raw_account"],
    }
    if found != expected_found:
        refuse("spline Found coordinates differ from the compiler records")
    return {
        "schema": smoke["schema"],
        "key_free": True,
        "signs": False,
        "submits": False,
        "source": {
            "node_archive_member_lister": evidence(
                root,
                "source/tools/release/node_archive_members.py",
                "source-pinned Node archive member lister",
            ),
            "runner": evidence(
                root,
                "source/tools/release/spline-product-handoff-smoke.sh",
                "source-pinned spline Product smoke runner",
            ),
            "verifier": evidence(
                root,
                "source/tools/release/verify-spline-product-handoff.mjs",
                "source-pinned spline Product smoke verifier",
            ),
            "fixture": evidence(
                root,
                "source/docs/operator/examples/spline-product-degree2.json",
                "canonical spline Product fixture",
            ),
            "sdk_lock": evidence(
                root, "source/packages/dclutch-sdk/package-lock.json", "SDK package lock"
            ),
            "cli_lock": evidence(
                root, "source/packages/dclutch-cli/package-lock.json", "CLI package lock"
            ),
            "successor_lock": evidence(
                root,
                "source/tools/local-validator/bootstrap/successor/Cargo.lock",
                "spline Product producer lock",
            ),
        },
        "build": {
            "cli_bundle": cli_bundle,
            "successor": successor,
            "log": evidence(
                root, "product-handoff/build.log", "spline Product handoff build log"
            ),
        },
        "execution": {
            "smoke_report": smoke_evidence,
            "compiler_completion": evidence(
                root, f"{smoke_relative}/completion.json", "spline compiler completion"
            ),
            "compiler_report": evidence(
                root, f"{smoke_relative}/product/report.json", "spline compiler report"
            ),
            "sdk_inspection": evidence(
                root, f"{smoke_relative}/inspection.json", "spline SDK inspection"
            ),
            "products": products,
        },
        "fixture_sha256": smoke["fixture_sha256"],
        "compiler_report_sha256": smoke["compiler_report_sha256"],
        "semantic_basis_id": semantic,
        "found_records": found,
    }



def _normalise_keys(value: object) -> object:
    """Lower-case and de-camel one mapping's keys, recursively.

    `dominantShareBps` and `dominant_share_bps` are one fact spelled by two
    languages. Normalising is what lets the pack compare the SDK's report to
    the compiler's rather than giving up and excluding the field.
    """
    import re

    if isinstance(value, dict):
        return {
            re.sub(r"(?<!^)(?=[A-Z])", "_", key).lower(): _normalise_keys(item)
            for key, item in value.items()
        }
    if isinstance(value, list):
        return [_normalise_keys(item) for item in value]
    return value


def emit(arguments: argparse.Namespace) -> None:
    root = canonical_root(arguments.root)
    output = Path(arguments.output) if arguments.output else root / PACK_BASENAME
    if not output.is_absolute() or output.parent != root or output.name != PACK_BASENAME:
        refuse(f"pack output must be exactly {root / PACK_BASENAME}")

    summary_path = root_path(root, "SUMMARY.txt", "checked candidate summary")
    summary = read_kv(summary_path, "checked candidate summary")
    required_summary = {
        "format": "dclutch-checked-release-candidate-summary-v1",
        "evidence_level": "local-reproducible-release-candidate",
        "not_a_deployment": "true",
        "cargo_lock_immutability": "passed",
        "sbf_build_freshness": "passed",
        "sbf_build_freshness_links": "13",
        "sbf_build_diagnostics_total": "0",
        "sbf_build_diagnostics_accepted": "false",
    }
    for key, expected in required_summary.items():
        if summary.get(key) != expected:
            refuse(f"checked candidate summary {key} is not {expected!r}")
    revision = require_hex(summary_required(summary, "source_revision"), 40, "source revision")
    tree_sha = require_hex(summary_required(summary, "source_digest"), 64, "source tree digest")
    gate_sha = require_hex(
        summary_required(summary, "checked_upgrade_gate_sha256"),
        64,
        "checked Upgrade gate digest",
    )
    gate_path = root_path(root, "CHECKED_UPGRADE_GATE.json", "checked Upgrade gate")
    if sha256_file(gate_path) != gate_sha:
        refuse("checked Upgrade gate digest differs from candidate summary")
    gate = verify_checked_gate(root, gate_path, gate_sha)
    if gate.get("source_revision") != revision or gate.get("source_tree_sha256") != tree_sha:
        refuse("checked Upgrade gate differs from candidate source")
    links = gate.get("links")
    if not isinstance(links, list) or gate.get("link_count") != 13 or len(links) != 13:
        refuse("checked Upgrade gate is not the exact 13-link set")

    host_channel, compute_ceiling, packet_ceiling = authority_values(root)
    resolution_semantic = resolution_semantic_id(root)
    gate_by_label: dict[str, dict[str, Any]] = {}
    frames: list[dict[str, Any]] = []
    for link in links:
        if not isinstance(link, dict) or not isinstance(link.get("label"), str):
            refuse("checked Upgrade gate link is malformed")
        label = link["label"]
        if label in gate_by_label:
            refuse("checked Upgrade gate repeats a link label")
        gate_by_label[label] = link
        frame_bound = link.get("frame_bound_bytes")
        deepest = link.get("deepest_frame_bytes")
        if frame_bound != 4096 or link.get("frames_at_or_over_bound") != 0:
            refuse(f"{label} is not admitted by the 4096-byte frame gate")
        if isinstance(deepest, bool) or not isinstance(deepest, int) or deepest < 0 or deepest >= frame_bound:
            refuse(f"{label} deepest frame is malformed or over the bound")
        frames.append(
            {
                "label": label,
                "package": link.get("package"),
                "deepest_frame_bytes": deepest,
                "frame_count": link.get("frame_count"),
                "frame_report": link.get("frame_report"),
            }
        )
    if set(gate_by_label) != {
        "claims", "core", "custody", "dealer-accelerator", "dclutch-dealer-sbf",
        "dclutch-direct-aot-sbf", "general-accelerator", "dclutch-product-runtime-v2-sbf",
        "registry", "rent", "resolution", "series-shadow", "trading",
    }:
        refuse("checked Upgrade gate link labels differ from the shipped set")

    artifacts: list[dict[str, Any]] = []
    campaign: list[dict[str, Any]] = []
    platform_match = re.search(r"platform-tools v([^\)]+)", summary_required(summary, "rustc_version"))
    if not platform_match:
        refuse("candidate rustc version omitted platform-tools version")
    actual_builder = summary_required(summary, "builder")
    builder_scheduler = summary_required(summary, "builder_scheduler")
    if actual_builder not in {"local", "persvati", "hbox"} or builder_scheduler != (
        "swarm-build" if actual_builder == "hbox" else "direct"
    ):
        refuse("candidate summary builder/scheduler declaration differs")
    for role in ARTIFACT_ROLES:
        link = gate_by_label[role]
        elf = link.get("elf")
        manifest = link.get("checked_manifest")
        if not isinstance(elf, dict) or not isinstance(manifest, dict):
            refuse(f"artifact-producing role {role} omitted ELF or checked manifest")
        elf_path = verify_evidence(root, elf, f"{role} ELF")
        manifest_path = verify_evidence(root, manifest, f"{role} checked manifest")
        checked_text_relative = f"evidence/{role}/checked.txt"
        checked_text = read_kv(
            root_path(root, checked_text_relative, f"{role} checked projection"),
            f"{role} checked projection",
            repeatable=frozenset({"assumption"}),
        )
        for key, expected in {
            "format": "dclutch-checked-release-v1",
            "artifact_sha256": elf["sha256"],
            "artifact_bytes": str(elf["bytes"]),
            "checked_release_id": manifest["sha256"],
            "source_revision": revision,
            "source_digest": tree_sha,
        }.items():
            if checked_text.get(key) != expected:
                refuse(f"{role} checked projection {key} differs")
        program_hex = require_hex(checked_text.get("program_id"), 64, f"{role} program identity")
        semantic = require_hex(
            checked_text.get("semantic_release_id"), 64, f"{role} semantic release identity"
        )
        for suffix, actual in (
            ("elf_sha256", elf["sha256"]),
            ("elf_bytes", str(elf["bytes"])),
            ("program_id", program_hex),
            ("checked_manifest_sha256", manifest["sha256"]),
        ):
            if summary.get(f"{role}_{suffix}") != actual:
                refuse(f"candidate summary differs from {role} {suffix}")
        artifact = {
            "role": role,
            "package": link.get("package"),
            "program_id_hex": program_hex,
            "program_id_base58": base58_32(program_hex),
            "semantic_release_id": semantic,
            "checked_release_id": manifest["sha256"],
            "elf": elf,
            "checked_manifest": manifest,
            "checked_projection": evidence(root, checked_text_relative, f"{role} checked projection"),
            "artifact_provenance": link.get("artifact_provenance"),
            "build_log": link.get("build_log"),
        }
        artifacts.append(artifact)
        if role in CAMPAIGN_ROLES:
            campaign_semantic = resolution_semantic if role == "resolution" else semantic
            campaign.append(
                {
                    "role": role,
                    "spec_key": "rent_credit" if role == "rent" else role,
                    "program_id": artifact["program_id_base58"],
                    "semantic_release_id": campaign_semantic,
                    "semantic_source": (
                        "protocol-resolution-v4" if role == "resolution" else "checked-release-manifest"
                    ),
                    "elf": elf,
                    "build_log": link.get("build_log"),
                    "package": link.get("package"),
                }
            )
    if tuple(item["role"] for item in campaign) != tuple(role for role in ARTIFACT_ROLES if role in CAMPAIGN_ROLES):
        refuse("internal campaign-role order differs")
    campaign.sort(key=lambda item: CAMPAIGN_ROLES.index(item["role"]))
    node_toolchain = node_toolchain_value(root, summary)
    host_substrate = host_substrate_value(summary, host_channel)
    product_handoff = spline_product_handoff_value(root, summary)

    pack = {
        "schema": SCHEMA,
        "evidence_level": "local-reproducible-checked-release-campaign-input",
        "not_a_deployment": True,
        "source": {
            "revision": revision,
            "tree_sha256": tree_sha,
            "tree_manifest": gate["source_tree_manifest"],
            "root_cargo_lock_sha256": require_hex(
                summary_required(summary, "root_cargo_lock_digest"), 64, "root Cargo.lock digest"
            ),
            "cargo_lock_count": int(summary_required(summary, "cargo_lock_count")),
            "cargo_lock_set_sha256": require_hex(
                summary_required(summary, "cargo_lock_set_sha256"), 64, "Cargo.lock set digest"
            ),
            "cargo_lock_immutability": "passed",
        },
        "toolchains": {
            "host_rust_channel": host_channel,
            "sbf_rustc": summary_required(summary, "rustc_version"),
            "solana_cli": summary_required(summary, "solana_version"),
            "cargo_build_sbf": summary_required(summary, "cargo_build_sbf_version"),
            "platform_tools": platform_match.group(1),
            "target_triple": summary_required(summary, "target_triple"),
            "supported_builders": ["local", "persvati", "hbox-through-swarm-build"],
            "hbox_scheduler": "swarm-build",
            "actual_builder": actual_builder,
            "actual_builder_scheduler": builder_scheduler,
            "host_pin": evidence(root, "source/rust-toolchain.toml", "host toolchain pin"),
            "host_substrate": host_substrate,
            "node": node_toolchain,
        },
        "checked_upgrade_gate": evidence(root, "CHECKED_UPGRADE_GATE.json", "checked Upgrade gate"),
        "candidate_summary": evidence(root, "SUMMARY.txt", "checked candidate summary"),
        "artifacts": artifacts,
        "release": {
            "execution_release_set_id": require_hex(
                summary_required(summary, "multiprogram.execution_release_set_id"),
                64,
                "execution release-set ID",
            ),
            "checked_execution_release_set_id": require_hex(
                summary_required(summary, "multiprogram.checked_execution_release_set_id"),
                64,
                "checked execution release-set ID",
            ),
            "execution_release_set": evidence(
                root, "set/execution-release-set.bin", "execution release-set preimage"
            ),
            "checked_execution_release_set": evidence(
                root, "set/multiprogram.checked", "checked execution release set"
            ),
            **_lineage_release_fields(root, summary),
            "infrastructure_profile_sha256": require_hex(
                summary_required(summary, "infrastructure.profile_sha256"),
                64,
                "infrastructure profile digest",
            ),
            "infrastructure_profile_pda_hex": require_hex(
                summary_required(summary, "infrastructure.profile_pda"),
                64,
                "infrastructure profile PDA",
            ),
            "infrastructure_profile": evidence(
                root, "infrastructure/profile.bin", "infrastructure profile"
            ),
            "checked_infrastructure_id": require_hex(
                summary_required(summary, "infrastructure.checked_infrastructure_id"),
                64,
                "checked infrastructure ID",
            ),
            "checked_infrastructure": evidence(
                root, "infrastructure/infrastructure.checked", "checked infrastructure"
            ),
        },
        "ceilings": {
            "compute_units": compute_ceiling,
            "compute_authority": evidence(
                root, "source/tools/gauntlet/CU_BUDGETS.json", "CU budget authority"
            ),
            "packet_bytes": packet_ceiling,
            "packet_authority": evidence(
                root,
                "source/crates/dclutch-versioned-message-operator/src/lib.rs",
                "packet bound authority",
            ),
            "frame_bytes": 4096,
            "frames": frames,
        },
        "compliance": {
            "repository_license": "AGPL-3.0-or-later",
            "workspace_manifest": evidence(root, "source/Cargo.toml", "workspace licence authority"),
            "sbom": evidence(root, "source/tools/sbom/SBOM.md", "dependency SBOM"),
            "notices": evidence(root, "source/tools/sbom/NOTICES.md", "dependency notices"),
            "sbom_verifier": evidence(
                root, "source/tools/sbom/sbom_check.py", "SBOM verifier"
            ),
        },
        "campaign": {
            "run_spec_schema": SPEC_SCHEMA,
            "launcher": evidence(
                root,
                "source/tools/local-validator/dclutch-successor-validator",
                "successor validator launcher",
            ),
            "successor_manifest": evidence(
                root,
                "source/tools/local-validator/bootstrap/successor/Cargo.toml",
                "successor campaign manifest",
            ),
            "successor_lock": evidence(
                root,
                "source/tools/local-validator/bootstrap/successor/Cargo.lock",
                "successor campaign lock",
            ),
            "resolution_release_authority": evidence(
                root,
                "source/crates/dclutch-resolution-codec/src/lib.rs",
                "Resolution release authority",
            ),
            "roles": campaign,
        },
        "product_handoff": product_handoff,
        "verifier": {
            "pack": evidence(
                root,
                "source/tools/release/successor_campaign_pack.py",
                "source-pinned campaign pack verifier",
            ),
            "artifact_provenance": evidence(
                root,
                "source/tools/release/artifact_provenance.py",
                "source-pinned artifact provenance verifier",
            ),
            "public_route_campaign": evidence(
                root,
                "source/tools/release/public_route_campaign.py",
                "source-pinned public route campaign runner",
            ),
            "devnet_direct_lifecycle": evidence(
                root,
                "source/tools/release/devnet_direct_lifecycle.py",
                "source-pinned devnet Direct lifecycle runner",
            ),
        },
    }
    atomic_new(output, pack)
    print(f"successor campaign release pack sha256={sha256_file(output)}")


def verify_pack(pack_path: Path) -> tuple[Path, dict[str, Any]]:
    supplied = pack_path
    resolved = supplied.resolve(strict=True)
    regular(resolved, "successor campaign release pack")
    if supplied != resolved or resolved.name != PACK_BASENAME:
        refuse("pack path must be exact, canonical, and use the canonical basename")
    root = resolved.parent
    pack = read_json(resolved, "successor campaign release pack")
    expected_top = {
        "schema", "evidence_level", "not_a_deployment", "source", "toolchains",
        "checked_upgrade_gate", "candidate_summary", "artifacts", "release", "ceilings",
        "compliance", "campaign", "product_handoff", "verifier",
    }
    if set(pack) != expected_top:
        refuse("successor campaign release pack top-level fields differ")
    if (
        pack["schema"] != SCHEMA
        or pack["evidence_level"] != "local-reproducible-checked-release-campaign-input"
        or pack["not_a_deployment"] is not True
    ):
        refuse("successor campaign release pack header differs")

    gate_path = verify_evidence(root, pack["checked_upgrade_gate"], "checked Upgrade gate")
    gate = verify_checked_gate(root, gate_path, pack["checked_upgrade_gate"]["sha256"])
    source = pack["source"]
    if not isinstance(source, dict) or source.get("revision") != gate.get("source_revision") or source.get("tree_sha256") != gate.get("source_tree_sha256"):
        refuse("pack source differs from checked Upgrade gate")
    require_hex(source.get("revision"), 40, "pack source revision")
    require_hex(source.get("tree_sha256"), 64, "pack source digest")
    verify_evidence(root, source.get("tree_manifest"), "source tree manifest")
    if source.get("cargo_lock_immutability") != "passed":
        refuse("pack does not carry passed Cargo.lock immutability")

    summary_path = verify_evidence(root, pack["candidate_summary"], "candidate summary")
    summary = read_kv(summary_path, "candidate summary")
    if summary.get("source_revision") != source["revision"] or summary.get("source_digest") != source["tree_sha256"]:
        refuse("candidate summary differs from pack source")
    if summary.get("checked_upgrade_gate_sha256") != pack["checked_upgrade_gate"]["sha256"]:
        refuse("candidate summary differs from pack gate")
    for key, expected in (
        ("cargo_lock_immutability", "passed"),
        ("sbf_build_freshness", "passed"),
        ("sbf_build_freshness_links", "13"),
        ("sbf_build_diagnostics_total", "0"),
        ("sbf_build_diagnostics_accepted", "false"),
    ):
        if summary.get(key) != expected:
            refuse(f"candidate summary no longer proves {key}={expected}")

    host_channel, compute, packet = authority_values(root)
    toolchains = pack["toolchains"]
    if not isinstance(toolchains, dict) or toolchains.get("host_rust_channel") != host_channel:
        refuse("pack host toolchain differs from source pin")
    if toolchains.get("supported_builders") != [
        "local",
        "persvati",
        "hbox-through-swarm-build",
    ] or toolchains.get("hbox_scheduler") != "swarm-build":
        refuse("pack supported-builder policy differs")
    if toolchains.get("actual_builder") != summary.get("builder") or toolchains.get(
        "actual_builder_scheduler"
    ) != summary.get("builder_scheduler"):
        refuse("pack actual builder differs from checked summary")
    verify_evidence(root, toolchains.get("host_pin"), "host toolchain pin")
    if toolchains.get("sbf_rustc") != summary.get("rustc_version") or toolchains.get("solana_cli") != summary.get("solana_version") or toolchains.get("cargo_build_sbf") != summary.get("cargo_build_sbf_version") or toolchains.get("target_triple") != summary.get("target_triple"):
        refuse("pack build toolchains differ from checked summary")
    if toolchains.get("node") != node_toolchain_value(root, summary):
        refuse("pack Node/npm toolchain differs from the preserved official distribution")
    if toolchains.get("host_substrate") != host_substrate_value(summary, host_channel):
        refuse("pack host substrate differs from the executed candidate builder")

    gate_links = {link["label"]: link for link in gate["links"]}
    artifacts = pack["artifacts"]
    if not isinstance(artifacts, list) or [item.get("role") for item in artifacts] != list(ARTIFACT_ROLES):
        refuse("pack artifact roles are not canonical")
    artifact_by_role: dict[str, dict[str, Any]] = {}
    for item in artifacts:
        role = item["role"]
        if set(item) != {
            "role", "package", "program_id_hex", "program_id_base58", "semantic_release_id",
            "checked_release_id", "elf", "checked_manifest", "checked_projection",
            "artifact_provenance", "build_log",
        }:
            refuse(f"{role} artifact fields differ")
        link = gate_links.get(role)
        if link is None or item["package"] != link["package"] or item["elf"] != link["elf"] or item["checked_manifest"] != link["checked_manifest"] or item["artifact_provenance"] != link["artifact_provenance"] or item["build_log"] != link["build_log"]:
            refuse(f"{role} artifact differs from checked Upgrade gate")
        elf_path = verify_evidence(root, item["elf"], f"{role} ELF")
        verify_evidence(root, item["checked_manifest"], f"{role} checked manifest")
        checked_path = verify_evidence(root, item["checked_projection"], f"{role} checked projection")
        checked = read_kv(
            checked_path,
            f"{role} checked projection",
            repeatable=frozenset({"assumption"}),
        )
        if (
            checked.get("artifact_sha256") != sha256_file(elf_path)
            or checked.get("checked_release_id") != item["checked_release_id"]
            or checked.get("program_id") != item["program_id_hex"]
            or checked.get("semantic_release_id") != item["semantic_release_id"]
            or base58_32(item["program_id_hex"]) != item["program_id_base58"]
        ):
            refuse(f"{role} artifact projection differs")
        artifact_by_role[role] = item

    release = pack["release"]
    base_fields = {
        "execution_release_set_id",
        "checked_execution_release_set_id",
        "execution_release_set",
        "checked_execution_release_set",
        "infrastructure_profile_sha256",
        "infrastructure_profile_pda_hex",
        "infrastructure_profile",
        "checked_infrastructure_id",
        "checked_infrastructure",
    }
    # The lineage decides which fields MUST be present, in both directions. A
    # succession without its predecessor digest and a genesis carrying one are
    # both refused, so neither shape can be reached by omission.
    if not isinstance(release, dict):
        refuse("pack release section fields differ")
    if release.get("infrastructure_lineage") == "genesis":
        expected_fields = base_fields | {"infrastructure_lineage"}
        evidence_keys = (
            ("execution_release_set", "execution release-set preimage"),
            ("checked_execution_release_set", "checked execution release set"),
            ("infrastructure_profile", "infrastructure profile"),
            ("checked_infrastructure", "checked infrastructure"),
        )
    else:
        expected_fields = base_fields | {
            "predecessor_infrastructure_profile_sha256",
            "predecessor_infrastructure_profile",
        }
        evidence_keys = (
            ("execution_release_set", "execution release-set preimage"),
            ("checked_execution_release_set", "checked execution release set"),
            ("predecessor_infrastructure_profile", "predecessor infrastructure profile"),
            ("infrastructure_profile", "infrastructure profile"),
            ("checked_infrastructure", "checked infrastructure"),
        )
    if set(release) != expected_fields:
        refuse("pack release section fields differ")
    for key, label in evidence_keys:
        verify_evidence(root, release.get(key), label)
    for release_key, summary_key in (
        ("execution_release_set_id", "multiprogram.execution_release_set_id"),
        (
            "checked_execution_release_set_id",
            "multiprogram.checked_execution_release_set_id",
        ),
        (
            "predecessor_infrastructure_profile_sha256",
            "predecessor_infrastructure_profile_sha256",
        ),
        ("infrastructure_profile_sha256", "infrastructure.profile_sha256"),
        ("infrastructure_profile_pda_hex", "infrastructure.profile_pda"),
        ("checked_infrastructure_id", "infrastructure.checked_infrastructure_id"),
    ):
        if release.get(release_key) != summary.get(summary_key):
            refuse("pack release/profile identities differ from checked summary")
    if (
        release["predecessor_infrastructure_profile"]["sha256"]
        != release["predecessor_infrastructure_profile_sha256"]
    ):
        refuse("pack predecessor profile digest differs from its preserved bytes")

    ceilings = pack["ceilings"]
    if not isinstance(ceilings, dict) or ceilings.get("compute_units") != compute or ceilings.get("packet_bytes") != packet or ceilings.get("frame_bytes") != 4096:
        refuse("pack compute/frame/packet ceilings differ from their authorities")
    verify_evidence(root, ceilings.get("compute_authority"), "CU budget authority")
    verify_evidence(root, ceilings.get("packet_authority"), "packet bound authority")
    frames = ceilings.get("frames")
    if not isinstance(frames, list) or len(frames) != 13:
        refuse("pack does not carry all 13 frame measurements")
    for frame, gate_link in zip(frames, gate["links"], strict=True):
        expected = {
            "label": gate_link["label"],
            "package": gate_link["package"],
            "deepest_frame_bytes": gate_link["deepest_frame_bytes"],
            "frame_count": gate_link["frame_count"],
            "frame_report": gate_link["frame_report"],
        }
        if frame != expected:
            refuse(f"frame metadata differs for {gate_link['label']}")
        verify_evidence(root, frame["frame_report"], f"{frame['label']} frame report")

    compliance = pack["compliance"]
    if not isinstance(compliance, dict) or compliance.get("repository_license") != "AGPL-3.0-or-later":
        refuse("pack repository licence declaration differs")
    for key, label in (
        ("workspace_manifest", "workspace licence authority"),
        ("sbom", "dependency SBOM"),
        ("notices", "dependency notices"),
        ("sbom_verifier", "SBOM verifier"),
    ):
        verify_evidence(root, compliance.get(key), label)

    campaign = pack["campaign"]
    if not isinstance(campaign, dict) or campaign.get("run_spec_schema") != SPEC_SCHEMA:
        refuse("pack campaign header differs")
    for key, label in (
        ("launcher", "successor validator launcher"),
        ("successor_manifest", "successor campaign manifest"),
        ("successor_lock", "successor campaign lock"),
        ("resolution_release_authority", "Resolution release authority"),
    ):
        verify_evidence(root, campaign.get(key), label)
    roles = campaign.get("roles")
    if not isinstance(roles, list) or [item.get("role") for item in roles] != list(CAMPAIGN_ROLES):
        refuse("pack campaign roles are not canonical")
    resolution_semantic = resolution_semantic_id(root)
    for item in roles:
        role = item["role"]
        expected_semantic = resolution_semantic if role == "resolution" else artifact_by_role[role]["semantic_release_id"]
        expected_source = "protocol-resolution-v4" if role == "resolution" else "checked-release-manifest"
        if (
            set(item) != {"role", "spec_key", "program_id", "semantic_release_id", "semantic_source", "elf", "build_log", "package"}
            or item["spec_key"] != ("rent_credit" if role == "rent" else role)
            or item["program_id"] != artifact_by_role[role]["program_id_base58"]
            or item["semantic_release_id"] != expected_semantic
            or item["semantic_source"] != expected_source
            or item["elf"] != artifact_by_role[role]["elf"]
            or item["build_log"] != artifact_by_role[role]["build_log"]
            or item["package"] != artifact_by_role[role]["package"]
        ):
            refuse(f"campaign role binding differs for {role}")
    if pack["product_handoff"] != spline_product_handoff_value(root, summary):
        refuse("pack spline Product handoff differs from reverified source/build/execution evidence")
    verifier = pack["verifier"]
    if not isinstance(verifier, dict) or set(verifier) != {
        "pack",
        "artifact_provenance",
        "public_route_campaign",
        "devnet_direct_lifecycle",
    }:
        refuse("pack verifier fields differ")
    pinned_pack_verifier = verify_evidence(
        root, verifier["pack"], "source-pinned campaign pack verifier"
    )
    executing_verifier = Path(__file__).resolve(strict=True)
    regular(executing_verifier, "executing campaign pack verifier")
    if executing_verifier.read_bytes() != pinned_pack_verifier.read_bytes():
        refuse("executing campaign pack verifier differs from the pack's exact source revision")
    verify_evidence(
        root,
        verifier["artifact_provenance"],
        "source-pinned artifact provenance verifier",
    )
    verify_evidence(
        root,
        verifier["public_route_campaign"],
        "source-pinned public route campaign runner",
    )
    verify_evidence(
        root,
        verifier["devnet_direct_lifecycle"],
        "source-pinned devnet Direct lifecycle runner",
    )
    return root, pack


def attestation(root: Path, pack_path: Path, pack: Mapping[str, Any], role: Mapping[str, Any]) -> dict[str, Any]:
    elf = root_path(root, role["elf"]["canonical_path"], f"{role['role']} ELF")
    build_log = root_path(root, role["build_log"]["canonical_path"], f"{role['role']} build log")
    return {
        "schema": "dclutch-gauntlet-artifact-attestation-v1",
        "elf_path": str(elf),
        "elf_sha256": role["elf"]["sha256"],
        "program_id": role["program_id"],
        "commit": pack["source"]["revision"],
        "archive_sha256": pack["source"]["tree_sha256"],
        "cargo_build_sbf_version": pack["toolchains"]["cargo_build_sbf"],
        "platform_tools_version": pack["toolchains"]["platform_tools"],
        "rustc_version": pack["toolchains"]["sbf_rustc"],
        "solana_version": pack["toolchains"]["solana_cli"],
        "build_command": f"cargo build-sbf --manifest-path programs/{role['package']}/Cargo.toml -- --locked",
        "build_log_sha256": sha256_file(build_log),
        "verifier": {"status": "clean", "diagnostic_count": 0},
        "sbf_backend_frame_diagnostics": 0,
        "release_pack_sha256": sha256_file(pack_path),
        "assumptions": [
            "program_id is candidate-local and has no private key",
            "the checked Upgrade gate reauthenticated all thirteen fresh builds and frame reports before this projection",
            "this attestation is a campaign launcher input, not deployment or mainnet evidence",
        ],
    }


def validate_materialized_spec(root: Path, pack_path: Path, pack: Mapping[str, Any], spec_path: Path) -> None:
    spec = read_json(spec_path, "materialized successor run spec")
    if spec.get("schema") != SPEC_SCHEMA:
        refuse("materialized run spec schema differs")
    roles = pack["campaign"]["roles"]
    for role in roles:
        item = spec.get(role["spec_key"])
        if not isinstance(item, dict):
            refuse(f"materialized run spec omitted {role['spec_key']}")
        elf = root_path(root, role["elf"]["canonical_path"], f"{role['role']} ELF")
        expected = {
            "program_id": role["program_id"],
            "elf_path": str(elf),
            "elf_sha256": role["elf"]["sha256"],
            "semantic_release_id": role["semantic_release_id"],
        }
        for key, value in expected.items():
            if item.get(key) != value:
                refuse(f"materialized {role['role']} {key} differs from release pack")
        attestation_path = Path(item.get("attestation", ""))
        if not attestation_path.is_absolute() or attestation_path.resolve(strict=True) != attestation_path:
            refuse(f"materialized {role['role']} attestation is not canonical")
        if read_json(attestation_path, f"{role['role']} attestation") != attestation(root, pack_path, pack, role):
            refuse(f"materialized {role['role']} attestation differs from release pack")


def lineage_binding_value(
    root: Path,
    pack_path: Path,
    pack: Mapping[str, Any],
    lineage_path: Path,
) -> dict[str, Any]:
    """Join the pack to the campaign-owned, already-authenticated lineage envelope."""

    lineage_path = canonical_file(lineage_path, "infrastructure lineage evidence")
    lineage = read_json(lineage_path, "infrastructure lineage evidence")
    expected_top = {
        "schema",
        "evidenceLevel",
        "cluster",
        "genesisHash",
        "planSha256",
        "campaignEvidencePath",
        "source",
        "checkedArtifacts",
        "profiles",
        "artifactLineage",
        "activation",
        "migration",
    }
    if set(lineage) != expected_top:
        refuse("infrastructure lineage top-level fields differ")
    if (
        lineage["schema"] != LINEAGE_SCHEMA
        or lineage["evidenceLevel"] != "local-validator-finalized-chain-state"
        or lineage["cluster"] != "owned-loopback"
    ):
        refuse("infrastructure lineage header differs")
    require_hex(lineage.get("planSha256"), 64, "lineage plan digest")
    if not isinstance(lineage.get("genesisHash"), str) or not re.fullmatch(
        r"[1-9A-HJ-NP-Za-km-z]{32,44}", lineage["genesisHash"]
    ):
        refuse("lineage genesis hash is not canonical base58")

    source = lineage.get("source")
    if not isinstance(source, dict) or set(source) != {
        "revision",
        "treeSha256",
        "checkedReleaseGatePath",
        "checkedReleaseGateSha256",
        "checkedLocalMutableSetSha256",
        "solanaCliVersion",
    }:
        refuse("lineage source fields differ")
    gate_path = canonical_file(source["checkedReleaseGatePath"], "lineage checked release gate")
    expected_gate = root_path(
        root,
        pack["checked_upgrade_gate"]["canonical_path"],
        "pack checked Upgrade gate",
    )
    if (
        source["revision"] != pack["source"]["revision"]
        or source["treeSha256"] != pack["source"]["tree_sha256"]
        or source["checkedReleaseGateSha256"] != pack["checked_upgrade_gate"]["sha256"]
        or gate_path != expected_gate
        or sha256_file(gate_path) != source["checkedReleaseGateSha256"]
        or source["solanaCliVersion"] != pack["toolchains"]["solana_cli"]
    ):
        refuse("campaign lineage differs from the release pack source or gate")
    require_hex(
        source.get("checkedLocalMutableSetSha256"),
        64,
        "lineage checked-local mutable-set digest",
    )

    pack_roles = {item["role"]: item for item in pack["campaign"]["roles"]}
    checked = lineage.get("checkedArtifacts")
    if not isinstance(checked, list) or [row.get("role") for row in checked if isinstance(row, dict)] != list(CAMPAIGN_ROLES):
        refuse("lineage checked artifacts changed canonical seven-role order")
    for row in checked:
        role = row["role"]
        expected = pack_roles[role]
        if set(row) != {
            "role",
            "program",
            "programData",
            "checkedCandidateElfPath",
            "checkedCandidateElfSha256",
            "genesisLiveElfSha256",
            "genesisProgramDataAccountSha256",
            "genesisDeploymentSlot",
            "semanticReleaseId",
        }:
            refuse(f"lineage checked {role} artifact fields differ")
        elf_path = canonical_file(row["checkedCandidateElfPath"], f"lineage {role} ELF")
        expected_elf = root_path(root, expected["elf"]["canonical_path"], f"pack {role} ELF")
        if (
            row["program"] != expected["program_id"]
            or elf_path != expected_elf
            or row["checkedCandidateElfSha256"] != expected["elf"]["sha256"]
            or row["genesisLiveElfSha256"] != expected["elf"]["sha256"]
            or row["semanticReleaseId"] != expected["semantic_release_id"]
        ):
            refuse(f"lineage checked {role} artifact differs from release pack")
        for field in ("genesisProgramDataAccountSha256",):
            require_hex(row[field], 64, f"lineage {role} {field}")
        if isinstance(row["genesisDeploymentSlot"], bool) or not isinstance(
            row["genesisDeploymentSlot"], int
        ) or row["genesisDeploymentSlot"] <= 0:
            refuse(f"lineage {role} deployment slot is not positive")

    activation = lineage.get("activation")
    if not isinstance(activation, dict) or set(activation) != {
        "releaseSetId",
        "checkedExecutionReleaseSetId",
        "checkedMultiprogramEnvelopeSha256",
        "account",
        "roles",
    }:
        refuse("lineage activation fields differ")
    if (
        activation["releaseSetId"] != pack["release"]["execution_release_set_id"]
        or activation["checkedExecutionReleaseSetId"]
        != pack["release"]["checked_execution_release_set_id"]
        or activation["checkedMultiprogramEnvelopeSha256"]
        != pack["release"]["checked_execution_release_set"]["sha256"]
    ):
        refuse("lineage activation differs from release pack")

    profiles = lineage.get("profiles")
    if not isinstance(profiles, dict) or set(profiles) != {
        "predecessorV1",
        "successorV2",
        "v1PreservedByteIdentical",
    } or profiles["v1PreservedByteIdentical"] is not True:
        refuse("lineage profile succession fields differ or V1 was not preserved")
    v1 = profiles.get("predecessorV1")
    v2 = profiles.get("successorV2")
    if not isinstance(v1, dict) or not isinstance(v2, dict):
        refuse("lineage profile succession is malformed")
    for label, profile in (("predecessorV1", v1), ("successorV2", v2)):
        for field in ("registryArtifactReleaseId", "rentArtifactReleaseId"):
            require_hex(profile.get(field), 64, f"lineage {label} {field}")
    for field in (
        "predecessorRegistryArtifactReleaseId",
        "predecessorRentArtifactReleaseId",
    ):
        require_hex(v2.get(field), 64, f"lineage successorV2 {field}")
    if (
        v2["predecessorRegistryArtifactReleaseId"] != v1["registryArtifactReleaseId"]
        or v2["predecessorRentArtifactReleaseId"] != v1["rentArtifactReleaseId"]
        or v2["rentArtifactReleaseId"] != v1["rentArtifactReleaseId"]
        or v2["registryArtifactReleaseId"] == v1["registryArtifactReleaseId"]
    ):
        refuse("lineage profile succession does not move Registry forward and carry Rent")

    campaign_evidence = canonical_file(
        lineage["campaignEvidencePath"], "lineage campaign evidence"
    )
    return {
        "schema": LINEAGE_BINDING_SCHEMA,
        "evidence_level": "local-validator-finalized-lineage-bound-to-checked-release-pack",
        "not_a_deployment": True,
        "release_pack": absolute_evidence(pack_path, "successor campaign release pack"),
        "infrastructure_lineage": absolute_evidence(
            lineage_path, "infrastructure lineage evidence"
        ),
        "campaign_evidence": absolute_evidence(
            campaign_evidence, "lineage campaign evidence"
        ),
        "source_revision": pack["source"]["revision"],
        "source_tree_sha256": pack["source"]["tree_sha256"],
        "checked_release_gate_sha256": pack["checked_upgrade_gate"]["sha256"],
        "genesis_hash": lineage["genesisHash"],
        "plan_sha256": lineage["planSha256"],
        "checked_local_mutable_set_sha256": source["checkedLocalMutableSetSha256"],
        "execution_release_set_id": activation["releaseSetId"],
        "checked_execution_release_set_id": activation["checkedExecutionReleaseSetId"],
        "profiles": {
            "predecessor_v1_address": v1.get("address"),
            "predecessor_registry_artifact_release_id": v1["registryArtifactReleaseId"],
            "predecessor_rent_artifact_release_id": v1["rentArtifactReleaseId"],
            "successor_v2_address": v2.get("address"),
            "successor_registry_artifact_release_id": v2["registryArtifactReleaseId"],
            "successor_rent_artifact_release_id": v2["rentArtifactReleaseId"],
            "v1_preserved_byte_identical": True,
        },
    }


def bind_lineage(arguments: argparse.Namespace) -> None:
    pack_path = canonical_file(arguments.pack, "successor campaign release pack")
    root, pack = verify_pack(pack_path)
    lineage_path = canonical_file(arguments.lineage, "infrastructure lineage evidence")
    output = Path(arguments.output)
    if not output.is_absolute() or output.exists() or output.is_symlink():
        refuse("--output must be an absolute new path")
    parent = output.parent.resolve(strict=True)
    if parent != output.parent:
        refuse("--output parent is not canonical")
    value = lineage_binding_value(root, pack_path, pack, lineage_path)
    atomic_new(output, value)
    print(f"release-pack lineage binding={output}")
    print(f"release-pack lineage binding sha256={sha256_file(output)}")


def verify_lineage_binding(arguments: argparse.Namespace) -> None:
    pack_path = canonical_file(arguments.pack, "successor campaign release pack")
    root, pack = verify_pack(pack_path)
    binding_path = canonical_file(arguments.binding, "release-pack lineage binding")
    binding = read_json(binding_path, "release-pack lineage binding")
    lineage_value = binding.get("infrastructure_lineage")
    if not isinstance(lineage_value, dict):
        refuse("release-pack lineage binding omitted infrastructure lineage evidence")
    lineage_path = canonical_file(
        lineage_value.get("canonical_path", ""), "infrastructure lineage evidence"
    )
    expected = lineage_binding_value(root, pack_path, pack, lineage_path)
    if binding != expected:
        refuse("release-pack lineage binding differs from current pack/campaign evidence")
    print(f"release-pack lineage binding verified sha256={sha256_file(binding_path)}")


def reproduction_projection(pack: Mapping[str, Any]) -> dict[str, Any]:
    """Select only deterministic release outputs, excluding run/path evidence."""

    artifacts = []
    for item in pack["artifacts"]:
        artifacts.append(
            {
                "role": item["role"],
                "package": item["package"],
                "program_id_hex": item["program_id_hex"],
                "semantic_release_id": item["semantic_release_id"],
                "checked_release_id": item["checked_release_id"],
                "elf_bytes": item["elf"]["bytes"],
                "elf_sha256": item["elf"]["sha256"],
                "checked_manifest_bytes": item["checked_manifest"]["bytes"],
                "checked_manifest_sha256": item["checked_manifest"]["sha256"],
            }
        )
    frames = [
        {
            "label": item["label"],
            "package": item["package"],
            "frame_count": item["frame_count"],
            "deepest_frame_bytes": item["deepest_frame_bytes"],
        }
        for item in pack["ceilings"]["frames"]
    ]
    release = pack["release"]
    return {
        "source_revision": pack["source"]["revision"],
        "source_tree_sha256": pack["source"]["tree_sha256"],
        "root_cargo_lock_sha256": pack["source"]["root_cargo_lock_sha256"],
        "cargo_lock_set_sha256": pack["source"]["cargo_lock_set_sha256"],
        "toolchains": {
            key: pack["toolchains"][key]
            for key in (
                "host_rust_channel",
                "sbf_rustc",
                "solana_cli",
                "cargo_build_sbf",
                "platform_tools",
                "target_triple",
            )
        } | {"node": pack["toolchains"]["node"]},
        "artifacts": artifacts,
        "release": {
            "execution_release_set_id": release["execution_release_set_id"],
            "checked_execution_release_set_id": release[
                "checked_execution_release_set_id"
            ],
            "execution_release_set_sha256": release["execution_release_set"]["sha256"],
            "checked_execution_release_set_sha256": release[
                "checked_execution_release_set"
            ]["sha256"],
            "predecessor_infrastructure_profile_sha256": release[
                "predecessor_infrastructure_profile_sha256"
            ],
            "infrastructure_profile_sha256": release[
                "infrastructure_profile_sha256"
            ],
            "infrastructure_profile_pda_hex": release[
                "infrastructure_profile_pda_hex"
            ],
            "checked_infrastructure_id": release["checked_infrastructure_id"],
            "checked_infrastructure_sha256": release["checked_infrastructure"]["sha256"],
        },
        "ceilings": {
            "compute_units": pack["ceilings"]["compute_units"],
            "packet_bytes": pack["ceilings"]["packet_bytes"],
            "frame_bytes": pack["ceilings"]["frame_bytes"],
            "frames": frames,
        },
        "compliance": {
            "repository_license": pack["compliance"]["repository_license"],
            "workspace_manifest_sha256": pack["compliance"]["workspace_manifest"][
                "sha256"
            ],
            "sbom_sha256": pack["compliance"]["sbom"]["sha256"],
            "notices_sha256": pack["compliance"]["notices"]["sha256"],
            "sbom_verifier_sha256": pack["compliance"]["sbom_verifier"]["sha256"],
        },
        "product_handoff": {
            "schema": pack["product_handoff"]["schema"],
            "fixture_sha256": pack["product_handoff"]["fixture_sha256"],
            "compiler_report_sha256": pack["product_handoff"][
                "compiler_report_sha256"
            ],
            "semantic_basis_id": pack["product_handoff"]["semantic_basis_id"],
            "found_records": pack["product_handoff"]["found_records"],
            "source_sha256": {
                key: value["sha256"]
                for key, value in pack["product_handoff"]["source"].items()
            },
            "cli_bundle_sha256": pack["product_handoff"]["build"]["cli_bundle"][
                "sha256"
            ],
            "product_sha256": {
                key: value["sha256"]
                for key, value in pack["product_handoff"]["execution"][
                    "products"
                ].items()
            },
        },
        "verifiers": {
            "campaign_pack_sha256": pack["verifier"]["pack"]["sha256"],
            "artifact_provenance_sha256": pack["verifier"]["artifact_provenance"][
                "sha256"
            ],
            "public_route_campaign_sha256": pack["verifier"][
                "public_route_campaign"
            ]["sha256"],
            "devnet_direct_lifecycle_sha256": pack["verifier"][
                "devnet_direct_lifecycle"
            ]["sha256"],
        },
    }


def reproduction_value(
    left_path: Path,
    right_path: Path,
    left: Mapping[str, Any],
    right: Mapping[str, Any],
) -> dict[str, Any]:
    if left_path == right_path:
        refuse("release-pack reproduction requires two distinct pack files")
    left_projection = reproduction_projection(left)
    right_projection = reproduction_projection(right)
    if left_projection != right_projection:
        sections = [
            key
            for key in left_projection
            if left_projection.get(key) != right_projection.get(key)
        ]
        refuse(
            "supported-builder release outputs differ in deterministic sections: "
            + ", ".join(sections)
        )
    return {
        "schema": REPRODUCTION_SCHEMA,
        "verdict": "byte-identical-shipped-artifacts-and-release-identities",
        "source_revision": left_projection["source_revision"],
        "source_tree_sha256": left_projection["source_tree_sha256"],
        "left": {
            "builder": left["toolchains"]["actual_builder"],
            "scheduler": left["toolchains"]["actual_builder_scheduler"],
            "pack": absolute_evidence(left_path, "left successor campaign release pack"),
        },
        "right": {
            "builder": right["toolchains"]["actual_builder"],
            "scheduler": right["toolchains"]["actual_builder_scheduler"],
            "pack": absolute_evidence(right_path, "right successor campaign release pack"),
        },
        "reproduced": left_projection,
        "excluded_nondeterminism": [
            "absolute work paths in compiler logs",
            "fresh random build-run identifier",
            "per-run provenance and gate JSON hashes",
            "actual builder label",
            "host OS, kernel, C toolchain, and libc identity",
            "source-built host successor binary (its exact source and deterministic Product outputs are compared)",
        ],
    }


def compare_packs(arguments: argparse.Namespace) -> None:
    left_path = canonical_file(arguments.left, "left successor campaign release pack")
    right_path = canonical_file(arguments.right, "right successor campaign release pack")
    _, left = verify_pack(left_path)
    _, right = verify_pack(right_path)
    report = reproduction_value(left_path, right_path, left, right)
    output = Path(arguments.output)
    if not output.is_absolute() or output.exists() or output.is_symlink():
        refuse("--output must be an absolute new path")
    parent = output.parent.resolve(strict=True)
    if parent != output.parent:
        refuse("--output parent is not canonical")
    atomic_new(output, report)
    print(f"supported-builder reproduction={output}")
    print(f"supported-builder reproduction sha256={sha256_file(output)}")


def verify_reproduction(arguments: argparse.Namespace) -> None:
    report_path = canonical_file(arguments.report, "supported-builder reproduction")
    report = read_json(report_path, "supported-builder reproduction")
    expected_top = {
        "schema",
        "verdict",
        "source_revision",
        "source_tree_sha256",
        "left",
        "right",
        "reproduced",
        "excluded_nondeterminism",
    }
    if set(report) != expected_top or report.get("schema") != REPRODUCTION_SCHEMA:
        refuse("supported-builder reproduction header fields differ")
    left = report.get("left")
    right = report.get("right")
    if (
        not isinstance(left, dict)
        or set(left) != {"builder", "scheduler", "pack"}
        or not isinstance(right, dict)
        or set(right) != {"builder", "scheduler", "pack"}
    ):
        refuse("supported-builder reproduction pack references differ")
    left_path = verify_absolute_evidence(
        left["pack"], "left successor campaign release pack"
    )
    right_path = verify_absolute_evidence(
        right["pack"], "right successor campaign release pack"
    )
    _, left_pack = verify_pack(left_path)
    _, right_pack = verify_pack(right_path)
    expected = reproduction_value(left_path, right_path, left_pack, right_pack)
    if report != expected:
        refuse("supported-builder reproduction differs from its verified release packs")
    print(f"supported-builder reproduction verified sha256={sha256_file(report_path)}")


def materialize(arguments: argparse.Namespace) -> None:
    pack_path = Path(arguments.pack)
    root, pack = verify_pack(pack_path)
    run_root = Path(arguments.run_root)
    if not run_root.is_absolute() or run_root.exists() or run_root.is_symlink():
        refuse("--run-root must be an absolute new path")
    parent = run_root.parent.resolve(strict=True)
    if parent != run_root.parent:
        refuse("--run-root parent is not canonical")
    market_path = Path(arguments.market)
    if not market_path.is_absolute() or market_path.resolve(strict=True) != market_path:
        refuse("--market must be an exact canonical file")
    market = read_json(market_path, "campaign Market input")
    if arguments.record_publication not in {"genesis", "transaction"}:
        refuse("--record-publication must be genesis or transaction")
    if arguments.rpc_port < 1024 or arguments.rpc_port > 65494:
        refuse("--rpc-port must leave room for the launcher's 42-port block")

    run_root.mkdir(mode=0o755)
    attestations = run_root / "attestation"
    attestations.mkdir(mode=0o755)
    role_inputs: dict[str, dict[str, Any]] = {}
    for role in pack["campaign"]["roles"]:
        attestation_path = attestations / f"{role['role']}.json"
        atomic_new(attestation_path, attestation(root, pack_path, pack, role))
        item = {
            "program_id": role["program_id"],
            "elf_path": str(root_path(root, role["elf"]["canonical_path"], f"{role['role']} ELF")),
            "elf_sha256": role["elf"]["sha256"],
            "semantic_release_id": role["semantic_release_id"],
            "attestation": str(attestation_path),
        }
        if arguments.record_publication == "transaction":
            item["genesis_deployment_slot"] = GENESIS_SLOTS[role["role"]]
        role_inputs[role["spec_key"]] = item

    spec: dict[str, Any] = {
        "schema": SPEC_SCHEMA,
        "rpc_url": f"http://127.0.0.1:{arguments.rpc_port}/",
        "launcher": str(
            root_path(
                root,
                pack["campaign"]["launcher"]["canonical_path"],
                "successor validator launcher",
            )
        ),
        "ledger": str(run_root / "ledger"),
        "account_dir": str(run_root / "accounts"),
        "plan": str(run_root / "plan.json"),
        "output": str(run_root / "evidence.json"),
        **role_inputs,
        "market": market,
        "record_publication": arguments.record_publication,
    }
    spec_path = run_root / "spec.json"
    atomic_new(spec_path, spec)
    validate_materialized_spec(root, pack_path, pack, spec_path)
    print(f"successor run spec={spec_path}")
    print(f"release pack sha256={sha256_file(pack_path)}")


def parser() -> argparse.ArgumentParser:
    top = argparse.ArgumentParser(description=__doc__)
    commands = top.add_subparsers(dest="command", required=True)
    create = commands.add_parser("emit", help="add the campaign pack manifest to a checked candidate")
    create.add_argument("--root", required=True)
    create.add_argument("--output")
    verify = commands.add_parser("verify", help="rehash and reauthenticate the complete pack")
    verify.add_argument("--pack", required=True)
    material = commands.add_parser(
        "materialize-spec", help="write an exact existing-successor campaign run spec from the pack"
    )
    material.add_argument("--pack", required=True)
    material.add_argument("--market", required=True)
    material.add_argument("--run-root", required=True)
    material.add_argument("--rpc-port", type=int, default=20890)
    material.add_argument("--record-publication", default="transaction")
    check_spec = commands.add_parser("verify-spec", help="verify an already materialized spec")
    check_spec.add_argument("--pack", required=True)
    check_spec.add_argument("--spec", required=True)
    bind = commands.add_parser(
        "bind-lineage",
        help="bind the campaign-owned post-succession lineage artifact back to this pack",
    )
    bind.add_argument("--pack", required=True)
    bind.add_argument("--lineage", required=True)
    bind.add_argument("--output", required=True)
    binding = commands.add_parser(
        "verify-lineage-binding", help="reverify a release-pack/campaign-lineage binding"
    )
    binding.add_argument("--pack", required=True)
    binding.add_argument("--binding", required=True)
    compare = commands.add_parser(
        "compare-packs",
        help="prove deterministic release outputs match across two verified builder packs",
    )
    compare.add_argument("--left", required=True)
    compare.add_argument("--right", required=True)
    compare.add_argument("--output", required=True)
    reproduce = commands.add_parser(
        "verify-reproduction",
        help="rehash both packs and reverify a supported-builder reproduction report",
    )
    reproduce.add_argument("--report", required=True)
    return top


def main(argv: Sequence[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
        if arguments.command == "emit":
            emit(arguments)
        elif arguments.command == "verify":
            _, _ = verify_pack(Path(arguments.pack))
            print(f"successor campaign release pack verified sha256={sha256_file(Path(arguments.pack))}")
        elif arguments.command == "materialize-spec":
            materialize(arguments)
        elif arguments.command == "verify-spec":
            pack_path = Path(arguments.pack)
            root, pack = verify_pack(pack_path)
            spec_path = Path(arguments.spec)
            if not spec_path.is_absolute() or spec_path.resolve(strict=True) != spec_path:
                refuse("--spec must be an exact canonical file")
            validate_materialized_spec(root, pack_path, pack, spec_path)
            print(f"successor campaign spec verified={spec_path}")
        elif arguments.command == "bind-lineage":
            bind_lineage(arguments)
        elif arguments.command == "verify-lineage-binding":
            verify_lineage_binding(arguments)
        elif arguments.command == "compare-packs":
            compare_packs(arguments)
        else:
            verify_reproduction(arguments)
        return 0
    except (OSError, Refusal, ValueError) as error:
        print(f"SUCCESSOR CAMPAIGN RELEASE PACK REFUSED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
