#!/usr/bin/env python3
"""Localhost-only dClutch mutable lifecycle supervisor.

The supervisor never accepts an RPC URL.  It creates one fresh loopback
validator per named seed, provisions only seed-derived disposable local keys,
invokes the accepted exterior commands, preserves every stage artifact, and
always tears down the complete validator process group.

The one-seed founding/participant probe is accepted today.  The twenty-seed
terminal lifecycle remains fail-closed until its Direct producer, payout
executor, and aggregate receipt authenticator are all present in the exact
source revision.  Keeping the future full path visible here must never make it
dispatchable before those semantic owners converge.
"""

from __future__ import annotations

import argparse
import base64
import contextlib
import dataclasses
import hashlib
import json
import os
from pathlib import Path
import signal
import shutil
import socket
import subprocess
import sys
import time
from typing import Any, Iterable, Sequence
from urllib import request

SCHEMA = "dclutch-private-validator-lifecycle-summary-v1"
PARTICIPANT_PROBE_SCHEMA = "dclutch-private-validator-participant-probe-summary-v1"
FULL_PROBE_SCHEMA = "dclutch-private-validator-full-lifecycle-probe-summary-v1"
RUN_SCHEMA = "dclutch-private-validator-lifecycle-run-v1"
PARTICIPANT_PROBE_RUN_SCHEMA = "dclutch-private-validator-participant-probe-run-v1"
FULL_PROBE_RUN_SCHEMA = "dclutch-private-validator-full-lifecycle-probe-run-v1"
SEED_DOMAIN = b"dclutch/private-validator-lifecycle/named-seed/v1\0"
FOUNDING_PARTICIPANT_COMMANDS = (
    "local-mutable-prepare-v1",
    "local-mutable-plan-authenticate-v1",
    "local-private-validator-market-v1",
    "campaign",
    "local-private-validator-user-position-admission-v1",
)
PYTH_PROVISION_COMMAND = "local-private-validator-pyth-vaa-provision-v1"
FLAGSHIP_RESOLUTION_COMMAND = "local-private-validator-flagship-resolution-v1"
PAYOUT_INPUT_COMMAND = "local-private-validator-wallet-terminal-payout-input-v1"
PAYOUT_EXECUTE_COMMAND = "local-private-validator-wallet-terminal-payout-v1"
TERMINAL_RETIREMENT_COMMAND = "local-private-validator-terminal-sequence-v1"
FULL_LIFECYCLE_BLOCKERS = (
    "owned-loopback Direct producer",
    "Direct-owned canonical nonzero-claim payout schedule",
)
PYTH_JOURNAL_SCHEMA = "dclutch-owned-loopback-pyth-prerequisite-transaction-v1"
RESOLUTION_PRODUCER_SCHEMA = "dclutch-owned-loopback-flagship-resolution-producer-v1"
RESOLUTION_TABLE_SCHEMA = "dclutch-owned-loopback-flagship-resolution-alt-journal-v2"
RESOLUTION_INPUT_SCHEMA = "dclutch-owned-loopback-flagship-resolution-input-v1"
RESOLUTION_CHECKPOINT_SCHEMA = (
    "dclutch-owned-loopback-flagship-resolution-checkpoint-v2"
)
PAYOUT_INPUT_SCHEMA = "dclutch-wallet-terminal-payout-plan-input-v1"
PAYOUT_EVIDENCE_SCHEMA = (
    "dclutch-local-private-validator-wallet-terminal-payout-evidence-v1"
)
TERMINAL_SESSION_SCHEMA = "dclutch-owned-loopback-terminal-sequence-session-v1"
TERMINAL_JOURNAL_SCHEMA = "dclutch-owned-loopback-terminal-sequence-journal-v1"
TERMINAL_COMPLETION_SCHEMA = "dclutch-owned-loopback-terminal-sequence-completion-v1"
MAX_RESOLUTION_TABLE_INVOCATIONS = 64
MAX_RESOLUTION_STAGE_INVOCATIONS = 16
MAX_PAYOUT_INVOCATIONS = 24
MAX_TERMINAL_INVOCATIONS = 32
MAX_PAYOUT_TARGETS = 32
PYTH_JOURNAL_FILES = (
    "00-router-initialize.json",
    "01-receiver-initialize.json",
    "02-treasury-capitalize.json",
    "03-encoded-vaa-create.json",
    "04-encoded-vaa-initialize.json",
    "05-encoded-vaa-write-0000.json",
    "06-encoded-vaa-write-0600.json",
    "07-encoded-vaa-verify.json",
)
PYTH_ACTIONS = (
    "router-initialize",
    "receiver-initialize",
    "treasury-capitalize",
    "encoded-vaa-create",
    "encoded-vaa-initialize",
    "encoded-vaa-write-0000",
    "encoded-vaa-write-0600",
    "encoded-vaa-verify",
)
ROLE_ORDER = ("registry", "rent", "custody", "resolution", "claims", "trading", "core")
DEVELOPMENT_FEE_BASIS_POINTS = 50
FEE_BASIS_POINTS_DENOMINATOR = 10_000
PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS = 100_000_000
VALIDATOR_MINT_ROLE = "core-upgrade-authority"
DEVELOPMENT_FEE_RECIPIENT_ROLE = "founding-source-funder"
PARTICIPANT_ROLE = "participant"
PARTICIPANT_FIXTURE_SOURCE_ROLE = "direct-buyer"
LOCAL_AIRDROP_ROLES: tuple[str, ...] = ()
PROTOCOL_CREATED_KEY_ROLES = (
    "collateral-mint",
    "collateral-wallet",
    "founding-source-funder",
)
CANONICAL_RESOLUTION_GATE_LABEL = "resolution"
CANONICAL_RESOLUTION_PACKAGE = "dclutch-resolution-proof-sbf"
CANONICAL_RESOLUTION_ELF_PATH = "elf/resolution.so"
BANISHED_RESOLUTION_ELF_BASENAME = "dclutch_sbf.so"
BANISHED_RESOLUTION_ELF_BYTES = 9_034_536


class Refusal(RuntimeError):
    """A fail-closed release refusal with a stable operator-facing reason."""


@dataclasses.dataclass(frozen=True)
class Paths:
    repo: Path
    release_root: Path
    bootstrap: Path
    reuse_bootstrap_work: Path | None
    validator: Path
    solana: Path
    work: Path


@dataclasses.dataclass(frozen=True, order=True)
class PayoutTarget:
    """Direct-owned routing for one live nonzero claim, without a Direct DTO."""

    owner: str
    claim_index: int
    recipient: str


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
BASE58_VALUES = {character: index for index, character in enumerate(BASE58_ALPHABET)}


def base58_bytes(value: Any, width: int, label: str) -> bytes:
    if not isinstance(value, str) or not value:
        raise Refusal(f"{label} must be nonempty base58 text")
    number = 0
    try:
        for character in value:
            number = number * 58 + BASE58_VALUES[character]
    except KeyError as error:
        raise Refusal(f"{label} contains a non-base58 character") from error
    body = (
        b"" if number == 0 else number.to_bytes((number.bit_length() + 7) // 8, "big")
    )
    decoded = b"\0" * (len(value) - len(value.lstrip("1"))) + body
    if len(decoded) != width or decoded == b"\0" * width:
        raise Refusal(f"{label} must decode to one nonzero {width}-byte value")
    return decoded


def canonical_pubkey(value: Any, label: str) -> str:
    base58_bytes(value, 32, label)
    return value


def canonical_signature(value: Any, label: str) -> str:
    base58_bytes(value, 64, label)
    return value


def positive_integer(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise Refusal(f"{label} must be one positive integer")
    return value


def canonical_decimal(value: Any, label: str, *, positive: bool = True) -> int:
    if (
        not isinstance(value, str)
        or not value
        or (len(value) > 1 and value.startswith("0"))
        or not value.isascii()
        or not value.isdecimal()
    ):
        raise Refusal(f"{label} must be canonical unsigned decimal text")
    number = int(value)
    if positive and number == 0:
        raise Refusal(f"{label} must be positive")
    return number


def finalized_fact(
    row: Any,
    label: str,
    *,
    signature_key: str = "signature",
    slot_key: str = "slot",
    fee_key: str = "feeLamports",
    compute_key: str = "computeUnitsConsumed",
    decimal_text: bool = False,
) -> dict[str, Any]:
    if not isinstance(row, dict):
        raise Refusal(f"{label} must be one finalized evidence object")
    number = canonical_decimal if decimal_text else positive_integer
    return {
        "signature": canonical_signature(row.get(signature_key), f"{label} signature"),
        "slot": number(row.get(slot_key), f"{label} slot"),
        "fee_lamports": number(row.get(fee_key), f"{label} fee"),
        "compute_units_consumed": number(
            row.get(compute_key), f"{label} compute units"
        ),
    }


def canonical_payout_schedule(rows: Sequence[PayoutTarget]) -> tuple[PayoutTarget, ...]:
    targets = tuple(rows)
    if not 1 <= len(targets) <= MAX_PAYOUT_TARGETS:
        raise Refusal(
            "Direct payout schedule must contain one through 32 nonzero claims"
        )
    seen: set[tuple[str, int]] = set()
    for target in targets:
        if not isinstance(target, PayoutTarget):
            raise Refusal("Direct payout schedule contains a caller-crafted row shape")
        canonical_pubkey(target.owner, "payout owner")
        canonical_pubkey(target.recipient, "payout recipient")
        if (
            isinstance(target.claim_index, bool)
            or not isinstance(target.claim_index, int)
            or not 0 <= target.claim_index <= 0xFFFFFFFF
        ):
            raise Refusal("payout claim index must be one canonical u32")
        identity = (target.owner, target.claim_index)
        if identity in seen:
            raise Refusal("Direct payout schedule repeats an owner/claim pair")
        seen.add(identity)
    canonical = tuple(
        sorted(
            targets,
            key=lambda row: (
                base58_bytes(row.owner, 32, "owner"),
                row.claim_index,
                base58_bytes(row.recipient, 32, "recipient"),
            ),
        )
    )
    if targets != canonical:
        raise Refusal(
            "Direct payout schedule is not canonical owner/claim/recipient order"
        )
    return targets


def canonical_file(value: str | Path, label: str) -> Path:
    path = Path(value)
    if not path.is_absolute() or path.is_symlink() or not path.is_file():
        raise Refusal(f"{label} must be one existing absolute non-symlink file: {path}")
    return path.resolve(strict=True)


def canonical_directory(value: str | Path, label: str) -> Path:
    path = Path(value)
    if not path.is_absolute() or path.is_symlink() or not path.is_dir():
        raise Refusal(
            f"{label} must be one existing absolute non-symlink directory: {path}"
        )
    return path.resolve(strict=True)


def decode_unique_json(text: str, label: str) -> Any:
    def pairs(rows: list[tuple[str, Any]]) -> dict[str, Any]:
        output: dict[str, Any] = {}
        for key, value in rows:
            if key in output:
                raise Refusal(f"{label} duplicated JSON key {key!r}")
            output[key] = value
        return output

    try:
        return json.loads(text, object_pairs_hook=pairs)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise Refusal(f"{label} is not exact JSON: {error}") from error


def read_unique_json(path: Path, label: str) -> Any:
    try:
        return decode_unique_json(path.read_text(), label)
    except OSError as error:
        raise Refusal(f"{label} is not exact JSON: {error}") from error


def clean_commit(repo: Path) -> str:
    result = subprocess.run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=repo,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.stdout:
        raise Refusal(
            "private-validator lifecycle requires one clean source commit; worktree is dirty"
        )
    return subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=repo, text=True
    ).strip()


def canonical_resolution_link(gate: dict[str, Any]) -> dict[str, Any]:
    """Select Resolution only through the checked release's canonical role.

    The private controller has no manual ELF argument.  This extra join keeps a
    checked gate from relabelling the banished aggregate `dclutch_sbf.so` as
    Resolution while preserving otherwise plausible frame and digest fields.
    """

    links = gate.get("links")
    if not isinstance(links, list):
        raise Refusal("checked release gate links are not one array")
    rows = [row for row in links if row.get("label") == CANONICAL_RESOLUTION_GATE_LABEL]
    if len(rows) != 1:
        raise Refusal("checked release gate must carry one canonical Resolution role")
    row = rows[0]
    elf = row.get("elf")
    marker = row.get("compile_marker")
    if (
        row.get("package") != CANONICAL_RESOLUTION_PACKAGE
        or not isinstance(marker, str)
        or CANONICAL_RESOLUTION_PACKAGE not in marker
        or row.get("checked_manifest") is None
        or not isinstance(elf, dict)
        or elf.get("canonical_path") != CANONICAL_RESOLUTION_ELF_PATH
        or Path(str(elf.get("canonical_path", ""))).name
        == BANISHED_RESOLUTION_ELF_BASENAME
        or not isinstance(elf.get("bytes"), int)
        or elf["bytes"] <= 0
        or elf["bytes"] == BANISHED_RESOLUTION_ELF_BYTES
    ):
        raise Refusal(
            "Resolution is not the checked dclutch-resolution-proof-sbf role; "
            "aggregate dclutch_sbf.so substitution is banished"
        )
    return row


def checked_gate(paths: Paths, commit: str) -> tuple[Path, dict[str, Any], str]:
    gate_path = canonical_file(
        paths.release_root / "CHECKED_UPGRADE_GATE.json", "checked release gate"
    )
    gate = read_unique_json(gate_path, "checked release gate")
    if gate.get("schema") != "dclutch-checked-upgrade-gate-v1":
        raise Refusal(
            "checked release gate schema is not dclutch-checked-upgrade-gate-v1"
        )
    if gate.get("source_revision") != commit:
        raise Refusal(
            f"checked release gate commit {gate.get('source_revision')} differs from clean source {commit}"
        )
    if gate.get("link_count") != 13 or len(gate.get("links", [])) != 13:
        raise Refusal(
            "checked release gate does not carry the exact thirteen-link closure"
        )
    labels = [link.get("label") for link in gate["links"]]
    if len(set(labels)) != 13:
        raise Refusal("checked release gate link labels are not unique")
    canonical_resolution_link(gate)
    for link in gate["links"]:
        if (
            link.get("sbf_diagnostics_count") != 0
            or link.get("frame_bound_bytes") != 4096
            or link.get("frames_at_or_over_bound") != 0
            or not isinstance(link.get("deepest_frame_bytes"), int)
            or link["deepest_frame_bytes"] >= 4096
        ):
            raise Refusal(
                f"checked release link {link.get('label')} is not below the frame bound"
            )
    tree = gate.get("source_tree_manifest", {})
    tree_path = canonical_file(
        paths.release_root / tree.get("canonical_path", ""), "source tree manifest"
    )
    if sha256_file(tree_path) != gate.get("source_tree_sha256") or sha256_file(
        tree_path
    ) != tree.get("sha256"):
        raise Refusal("checked release source-tree manifest digest changed")
    return gate_path, gate, sha256_file(gate_path)


def command_surface(bootstrap: Path, through: str) -> str:
    result = subprocess.run(
        [str(bootstrap), "help"],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    if result.returncode != 0:
        raise Refusal(
            f"successor help failed before validator launch:\n{result.stdout[-4000:]}"
        )
    required = list(FOUNDING_PARTICIPANT_COMMANDS)
    if through in ("full", "full-probe"):
        required.extend(
            (
                PYTH_PROVISION_COMMAND,
                FLAGSHIP_RESOLUTION_COMMAND,
                PAYOUT_INPUT_COMMAND,
                PAYOUT_EXECUTE_COMMAND,
                TERMINAL_RETIREMENT_COMMAND,
            )
        )
    missing = [command for command in required if command not in result.stdout]
    if missing:
        raise Refusal(
            "full localhost lifecycle has no accepted caller for: " + ", ".join(missing)
        )
    return sha256_bytes(result.stdout.encode())


def reuse_bootstrap(paths: Paths, commit: str, gate_digest: str) -> Paths:
    source_work = paths.reuse_bootstrap_work
    if source_work is None:
        raise Refusal("internal bootstrap reuse omitted its source work directory")
    source_bootstrap = canonical_file(
        source_work / "host-target/release/dclutch-local-successor-bootstrap",
        "reused successor bootstrap",
    )
    source_summary_path = canonical_file(
        source_work / "SUMMARY.json", "bootstrap source summary"
    )
    source_receipt_path = canonical_file(
        source_work / "host-build/receipt.json", "bootstrap source build receipt"
    )
    summary = read_unique_json(source_summary_path, "bootstrap source summary")
    receipt = read_unique_json(source_receipt_path, "bootstrap source build receipt")
    binary_digest = sha256_file(source_bootstrap)
    if (
        summary.get("source_revision") != commit
        or summary.get("checked_release_gate_sha256") != gate_digest
        or summary.get("bootstrap_sha256") != binary_digest
    ):
        raise Refusal(
            "reused bootstrap is not bound to this exact source and checked release gate"
        )
    if (
        receipt.get("schema") != "dclutch-private-validator-host-build-receipt-v1"
        or receipt.get("exit_status") != 0
        or not isinstance(receipt.get("rustup_toolchain"), str)
        or not isinstance(receipt.get("rustc"), str)
    ):
        raise Refusal(
            "reused bootstrap lacks a successful pinned-toolchain build receipt"
        )
    canonical_file(receipt["rustc"], "reused bootstrap rustc")

    target = paths.work / "host-target/release"
    target.mkdir(parents=True)
    bootstrap = target / "dclutch-local-successor-bootstrap"
    shutil.copyfile(source_bootstrap, bootstrap)
    bootstrap.chmod(0o755)
    stage = paths.work / "host-build"
    stage.mkdir()
    write_json_new(
        stage / "receipt.json",
        {
            "schema": "dclutch-private-validator-host-build-reuse-receipt-v1",
            "source_revision": commit,
            "checked_release_gate_sha256": gate_digest,
            "bootstrap_sha256": binary_digest,
            "source_work": str(source_work),
            "source_summary": str(source_summary_path),
            "source_summary_sha256": sha256_file(source_summary_path),
            "source_build_receipt": str(source_receipt_path),
            "source_build_receipt_sha256": sha256_file(source_receipt_path),
            "rustup_toolchain": receipt["rustup_toolchain"],
            "rustc": receipt["rustc"],
        },
    )
    return dataclasses.replace(paths, bootstrap=bootstrap.resolve())


def build_bootstrap(paths: Paths, commit: str, gate_digest: str) -> Paths:
    if paths.reuse_bootstrap_work is not None:
        return reuse_bootstrap(paths, commit, gate_digest)
    cargo_text = shutil.which("cargo")
    if cargo_text is None:
        raise Refusal("cargo is unavailable for the source-pinned successor host build")
    cargo_candidate = Path(cargo_text)
    selected_toolchain: str | None = None
    selected_rustc: Path | None = None
    if cargo_candidate.is_symlink():
        rustup_text = shutil.which("rustup")
        if rustup_text is None:
            raise Refusal("cargo is a rustup selector but rustup is unavailable")
        selected_toolchain = subprocess.check_output(
            [rustup_text, "show", "active-toolchain"],
            cwd=paths.repo,
            text=True,
            stderr=subprocess.STDOUT,
        ).split()[0]
        selected = subprocess.check_output(
            [rustup_text, "which", "--toolchain", selected_toolchain, "cargo"],
            text=True,
            stderr=subprocess.STDOUT,
        ).strip()
        cargo = canonical_file(selected, "rustup-selected cargo")
        rustc_text = subprocess.check_output(
            [rustup_text, "which", "--toolchain", selected_toolchain, "rustc"],
            text=True,
            stderr=subprocess.STDOUT,
        ).strip()
        selected_rustc = canonical_file(rustc_text, "rustup-selected rustc")
    else:
        cargo = canonical_file(cargo_candidate, "cargo")
    stage = paths.work / "host-build"
    stage.mkdir()
    manifest = paths.repo / "tools/local-validator/bootstrap/successor/Cargo.toml"
    if canonical_file(manifest, "successor manifest") != manifest.resolve():
        raise Refusal(
            "successor manifest did not resolve canonically inside the clean source"
        )
    target = paths.work / "host-target"
    argv = [
        str(cargo),
        "build",
        "--locked",
        "--release",
        "--manifest-path",
        str(manifest),
    ]
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target)
    if selected_toolchain is not None and selected_rustc is not None:
        # Directly invoking the selected Cargo binary otherwise lets its child
        # `rustc` shim fall back to the host default whenever Cargo compiles a
        # registry source outside the repository's override directory.
        environment["RUSTUP_TOOLCHAIN"] = selected_toolchain
        environment["RUSTC"] = str(selected_rustc)
    started = time.monotonic_ns()
    result = subprocess.run(
        argv,
        cwd=paths.repo,
        env=environment,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    elapsed = time.monotonic_ns() - started
    write_bytes_new(stage / "stdout.bin", result.stdout)
    write_bytes_new(stage / "stderr.bin", result.stderr)
    write_json_new(
        stage / "receipt.json",
        {
            "schema": "dclutch-private-validator-host-build-receipt-v1",
            "argv": argv,
            "exit_status": result.returncode,
            "elapsed_nanoseconds": elapsed,
            "rustup_toolchain": selected_toolchain,
            "rustc": None if selected_rustc is None else str(selected_rustc),
            "stdout_sha256": sha256_bytes(result.stdout),
            "stderr_sha256": sha256_bytes(result.stderr),
        },
    )
    if result.returncode != 0:
        raise Refusal(
            "source-pinned successor host build failed:\n"
            + (result.stdout + result.stderr).decode(errors="replace")[-6000:]
        )
    bootstrap = canonical_file(
        target / "release/dclutch-local-successor-bootstrap",
        "built successor bootstrap",
    )
    return dataclasses.replace(paths, bootstrap=bootstrap)


def allocate_port_block(seed_index: int) -> int:
    low, high, stride = 21000, 48000, 64
    count = (high - low) // stride
    start = (os.getpid() + seed_index * 17) % count
    for step in range(count):
        base = low + ((start + step) % count) * stride
        held: list[socket.socket] = []
        try:
            for offset in (0, 2, 3, *range(10, 42)):
                member = socket.socket()
                member.bind(("127.0.0.1", base + offset))
                held.append(member)
        except OSError:
            for member in held:
                member.close()
            continue
        for member in held:
            member.close()
        return base
    raise Refusal("no free complete 42-port localhost block")


def rpc(url: str, method: str, params: Sequence[Any] = ()) -> Any:
    if not url.startswith("http://127.0.0.1:"):
        raise Refusal(f"private-validator RPC escaped loopback: {url}")
    body = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": list(params)}
    ).encode()
    call = request.Request(url, data=body, headers={"content-type": "application/json"})
    with request.urlopen(
        call, timeout=5
    ) as response:  # noqa: S310 - URL is constructed loopback
        decoded = json.load(response)
    if decoded.get("error") is not None:
        raise Refusal(f"localhost RPC {method} refused: {decoded['error']}")
    return decoded.get("result")


def wait_ready(url: str, child: subprocess.Popen[bytes], timeout: float = 60.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise Refusal(
                f"validator exited before readiness with status {child.returncode}"
            )
        try:
            if rpc(url, "getHealth") == "ok":
                return
        except Exception:
            pass
        time.sleep(0.25)
    raise Refusal("validator did not become healthy within 60 seconds")


def wait_finalized_slot(
    url: str,
    child: subprocess.Popen[bytes],
    minimum_slot: int,
    timeout: float = 60.0,
) -> int:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise Refusal(f"validator exited before finalized slot {minimum_slot}")
        observed = rpc(url, "getSlot", [{"commitment": "finalized"}])
        if isinstance(observed, int) and observed >= minimum_slot:
            return observed
        time.sleep(0.25)
    raise Refusal(f"validator did not finalize checked deployment slot {minimum_slot}")


def terminate_group(child: subprocess.Popen[bytes] | None) -> None:
    if child is None or child.poll() is not None:
        return
    with contextlib.suppress(ProcessLookupError):
        os.killpg(child.pid, signal.SIGTERM)
    try:
        child.wait(timeout=10)
    except subprocess.TimeoutExpired:
        with contextlib.suppress(ProcessLookupError):
            os.killpg(child.pid, signal.SIGKILL)
        child.wait(timeout=10)


def write_json_new(path: Path, value: Any) -> None:
    data = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
    except Exception:
        with contextlib.suppress(OSError):
            os.close(descriptor)
        raise


def write_bytes_new(path: Path, data: bytes) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
    except Exception:
        with contextlib.suppress(OSError):
            os.close(descriptor)
        raise


def run_stage(
    run: Path, ordinal: int, label: str, argv: Sequence[str]
) -> subprocess.CompletedProcess[bytes]:
    stage = run / "stages" / f"{ordinal:02d}-{label}"
    stage.mkdir(parents=True, exist_ok=False)
    intent = {
        "schema": "dclutch-private-validator-stage-intent-v1",
        "label": label,
        "argv": list(argv),
        "started_unix_ns": time.time_ns(),
    }
    write_json_new(stage / "intent.json", intent)
    started = time.monotonic_ns()
    child = subprocess.Popen(
        argv,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    interrupted_error: BaseException | None = None
    try:
        stdout, stderr = child.communicate()
    except BaseException as error:
        interrupted_error = error
        terminate_group(child)
        stdout, stderr = child.communicate()
    finally:
        terminate_group(child)
    elapsed = time.monotonic_ns() - started
    result = subprocess.CompletedProcess(argv, child.returncode, stdout, stderr)
    write_bytes_new(stage / "stdout.bin", result.stdout)
    write_bytes_new(stage / "stderr.bin", result.stderr)
    receipt = {
        "schema": "dclutch-private-validator-stage-receipt-v1",
        "label": label,
        "exit_status": result.returncode,
        "elapsed_nanoseconds": elapsed,
        "stdout_sha256": sha256_bytes(result.stdout),
        "stderr_sha256": sha256_bytes(result.stderr),
        "interrupted": interrupted_error is not None,
    }
    write_json_new(stage / "receipt.json", receipt)
    if interrupted_error is not None:
        raise interrupted_error
    if result.returncode != 0:
        tail = result.stderr.decode(errors="replace")[-6000:]
        raise Refusal(f"stage {label} failed with status {result.returncode}:\n{tail}")
    return result


def key_address(solana: Path, keypair: Path) -> str:
    return subprocess.check_output(
        [str(solana), "address", "--keypair", str(keypair)],
        text=True,
        stderr=subprocess.STDOUT,
    ).strip()


def funding_argv(solana: Path, url: str, address: str) -> list[str]:
    return [
        str(solana),
        "--config",
        "/dev/null",
        "--url",
        url,
        "airdrop",
        "100",
        address,
        "--commitment",
        "finalized",
    ]


def validator_argv(
    validator: Path,
    ledger: Path,
    account_dir: str,
    mint_address: str,
    port: int,
) -> list[str]:
    """Launch only the exact plan-owned eighteen-account transaction genesis.

    Provider programs are already present in ``account_dir`` as immutable
    Loader-v3 Program/ProgramData pairs. Adding ``--upgradeable-program`` here
    would replace their tag-0 authority with Agave's tag-1 default pubkey.
    """

    return [
        str(validator),
        "--config",
        "/dev/null",
        "--ledger",
        str(ledger),
        "--account-dir",
        account_dir,
        "--mint",
        mint_address,
        "--ticks-per-slot",
        "16",
        "--bind-address",
        "127.0.0.1",
        "--rpc-port",
        str(port),
        "--faucet-port",
        str(port + 2),
        "--gossip-port",
        str(port + 3),
        "--dynamic-port-range",
        f"{port + 10}-{port + 41}",
    ]


def require_role_key(report: dict[str, Any], role: str) -> Path:
    keypairs = report.get("keypairs")
    if not isinstance(keypairs, dict) or not isinstance(keypairs.get(role), str):
        raise Refusal(f"local mutable preparation omitted disposable role {role}")
    return Path(keypairs[role])


def account_is_absent(url: str, address: str) -> bool:
    observed = rpc(url, "getAccountInfo", [address, {"commitment": "finalized"}])
    return isinstance(observed, dict) and observed.get("value") is None


def balance_lamports(url: str, address: str) -> int:
    observed = rpc(url, "getBalance", [address, {"commitment": "finalized"}])
    value = observed.get("value") if isinstance(observed, dict) else None
    if not isinstance(value, int) or value < 0:
        raise Refusal(
            f"localhost balance for {address} was not an integer lamport amount"
        )
    return value


def provision_disposable_funding(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
) -> dict[str, Any]:
    funded: list[dict[str, Any]] = []
    for offset, role in enumerate(LOCAL_AIRDROP_ROLES):
        key = require_role_key(report, role)
        address = key_address(paths.solana, key)
        run_stage(
            run,
            3 + offset,
            f"fund-{role}",
            funding_argv(paths.solana, url, address),
        )
        lamports = balance_lamports(url, address)
        if lamports == 0:
            raise Refusal(f"local faucet left disposable payer role {role} unfunded")
        funded.append({"role": role, "address": address, "lamports": lamports})

    genesis_key = require_role_key(report, VALIDATOR_MINT_ROLE)
    genesis_address = key_address(paths.solana, genesis_key)
    genesis_lamports = balance_lamports(url, genesis_address)
    if genesis_lamports == 0:
        raise Refusal(
            "fresh validator did not fund its owned development payer identity"
        )

    vacant: list[dict[str, str]] = []
    for role in PROTOCOL_CREATED_KEY_ROLES:
        key = require_role_key(report, role)
        address = key_address(paths.solana, key)
        if not account_is_absent(url, address):
            raise Refusal(
                f"protocol-created disposable role {role} already exists before founding"
            )
        vacant.append({"role": role, "address": address})
    poststate = {
        "schema": "dclutch-private-validator-disposable-funding-v1",
        "funded_by_local_faucet": funded,
        "funded_by_validator_genesis": {
            "role": VALIDATOR_MINT_ROLE,
            "address": genesis_address,
            "lamports": genesis_lamports,
        },
        "protocol_created_roles_proved_vacant": vacant,
        "external_writes": False,
    }
    write_json_new(run / "provisioning-poststate.json", poststate)
    return poststate


def run_pyth_provisioning(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
    first_ordinal: int,
) -> tuple[Path, dict[str, Any], int]:
    """Execute and reauthenticate the exact eight-action local Pyth owner."""

    payer_key = require_role_key(report, VALIDATOR_MINT_ROLE)
    encoded_key = require_role_key(report, "pyth-encoded-vaa")
    update_key = require_role_key(report, "pyth-update-account")
    payer = key_address(paths.solana, payer_key)
    encoded = key_address(paths.solana, encoded_key)
    update = key_address(paths.solana, update_key)
    if len({payer, encoded, update}) != 3:
        raise Refusal("local Pyth payer, EncodedVaa, and update roles are not distinct")
    if not account_is_absent(url, encoded) or not account_is_absent(url, update):
        raise Refusal("local Pyth EncodedVaa and update roles must both begin vacant")

    journal_dir = run / "journals" / "pyth"
    journal_dir.mkdir(parents=True, exist_ok=False)
    facts = run / "pyth-update-facts.json"
    argv = [
        str(paths.bootstrap),
        PYTH_PROVISION_COMMAND,
        "--rpc-url",
        url,
        "--payer",
        payer,
        "--encoded-vaa",
        encoded,
        "--update-account",
        update,
        "--journal-dir",
        str(journal_dir),
        "--facts-output",
        str(facts),
        "--payer-keypair",
        str(payer_key),
        "--encoded-vaa-keypair",
        str(encoded_key),
        "--execute",
    ]
    journal_rows: list[dict[str, Any]] = []
    prior_slot = 0
    for offset, file_name in enumerate(PYTH_JOURNAL_FILES):
        label = "pyth-" + file_name.removesuffix(".json")
        run_stage(run, first_ordinal + offset, label, argv)
        expected = set(PYTH_JOURNAL_FILES[: offset + 1])
        observed = {path.name for path in journal_dir.glob("*.json")}
        if observed != expected:
            raise Refusal(
                f"local Pyth action {label} produced a noncanonical journal prefix"
            )
        path = journal_dir / file_name
        journal = read_unique_json(path, f"local Pyth {label} journal")
        if (
            journal.get("schema") != PYTH_JOURNAL_SCHEMA
            or journal.get("cluster") != "owned-loopback"
            or journal.get("authorizedMutation") is not True
            or journal.get("phase") != "finalized"
            or not isinstance(journal.get("finalized"), dict)
            or not isinstance(journal.get("intent"), dict)
            or journal["intent"].get("action") != PYTH_ACTIONS[offset]
        ):
            raise Refusal(
                f"local Pyth action {label} did not preserve finalized evidence"
            )
        finalized = finalized_fact(journal["finalized"], f"local Pyth {label}")
        if (
            journal.get("expectedSignature") != finalized["signature"]
            or journal["intent"].get("exactFeeLamports") != finalized["fee_lamports"]
            or finalized["slot"] < prior_slot
        ):
            raise Refusal(
                f"local Pyth action {label} changed signature, exact fee, or slot order"
            )
        prior_slot = finalized["slot"]
        journal_rows.append(
            {
                "action": PYTH_ACTIONS[offset],
                "path": str(path),
                "sha256": sha256_file(path),
                **finalized,
            }
        )

    final_stage = run_stage(
        run,
        first_ordinal + len(PYTH_JOURNAL_FILES),
        "pyth-finalize",
        argv,
    )
    final_summary = json.loads(final_stage.stdout)
    document = read_unique_json(facts, "local Pyth update facts")
    if (
        final_summary.get("schema") != "dclutch-owned-loopback-pyth-prerequisites-v1"
        or final_summary.get("status") != "finalized"
        or final_summary.get("encodedVaa") != encoded
        or final_summary.get("updateAccount") != update
        or final_summary.get("facts") != str(facts)
        or document.get("format") != "dclutch-flagship-pyth-update-facts-v1"
        or document.get("encodedVaa") != encoded
        or document.get("updateAccount") != update
        or set(document)
        != {"format", "encodedVaa", "updateAccount", "postUpdateBodyBase64"}
    ):
        raise Refusal(
            "local Pyth final summary or exact four-field facts projection changed"
        )
    try:
        post_update_body = base64.b64decode(
            document["postUpdateBodyBase64"], validate=True
        )
    except (ValueError, TypeError) as error:
        raise Refusal("local Pyth PostUpdate body is not canonical base64") from error
    if not post_update_body:
        raise Refusal("local Pyth PostUpdate body is empty")
    if not account_is_absent(url, update):
        raise Refusal("local Pyth provisioner populated the Receiver update account")
    evidence = {
        "schema": "dclutch-private-validator-pyth-provisioning-v1",
        "status": "finalized",
        "payer": payer,
        "encoded_vaa": encoded,
        "update_account": update,
        "update_account_remained_vacant": True,
        "facts": str(facts),
        "facts_sha256": sha256_file(facts),
        "journals": journal_rows,
        "compute_units": {
            row["action"]: row["compute_units_consumed"] for row in journal_rows
        },
    }
    write_json_new(run / "pyth-provisioning.json", evidence)
    return facts, evidence, first_ordinal + len(PYTH_JOURNAL_FILES) + 1


def run_json_stage(
    run: Path,
    ordinal: int,
    label: str,
    argv: Sequence[str],
) -> dict[str, Any]:
    result = run_stage(run, ordinal, label, argv)
    try:
        text = result.stdout.decode("utf-8")
    except UnicodeDecodeError as error:
        raise Refusal(f"stage {label} stdout was not UTF-8 JSON") from error
    document = decode_unique_json(text, f"stage {label} stdout")
    if not isinstance(document, dict):
        raise Refusal(f"stage {label} stdout was not one JSON object")
    return document


def exact_keys(document: Any, expected: set[str], label: str) -> dict[str, Any]:
    if not isinstance(document, dict) or set(document) != expected:
        raise Refusal(f"{label} fields changed from its exact semantic-owner schema")
    return document


def lowercase_sha256(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise Refusal(f"{label} must be one lowercase SHA-256")
    return value


def authenticate_resolution_producer(
    document: Any,
    *,
    require_complete: bool,
) -> dict[str, Any]:
    producer = exact_keys(
        document,
        {
            "format",
            "planSha256",
            "campaignEvidenceSha256",
            "pythFactsSha256",
            "observationSlot",
            "observationUnixTimestamp",
            "market",
            "generation",
            "payer",
            "authority",
            "tables",
            "routes",
            "plannedInput",
            "flagshipInput",
        },
        "Resolution producer checkpoint",
    )
    for field in ("planSha256", "campaignEvidenceSha256", "pythFactsSha256"):
        lowercase_sha256(producer.get(field), f"Resolution {field}")
    if producer.get("format") != RESOLUTION_PRODUCER_SCHEMA:
        raise Refusal(
            "Resolution producer checkpoint used another owned-loopback schema"
        )
    market = canonical_pubkey(producer.get("market"), "Resolution Market")
    payer = canonical_pubkey(producer.get("payer"), "Resolution payer")
    if payer != canonical_pubkey(producer.get("authority"), "Resolution authority"):
        raise Refusal("Resolution table payer and authority differ")
    positive_integer(producer.get("observationSlot"), "Resolution observation slot")
    if (
        isinstance(producer.get("generation"), bool)
        or not isinstance(producer.get("generation"), int)
        or producer["generation"] < 0
    ):
        raise Refusal("Resolution generation is not one u64")
    tables = producer.get("tables")
    routes = producer.get("routes")
    if not isinstance(tables, dict) or set(tables) != {"submit", "execute", "reclaim"}:
        raise Refusal("Resolution producer omitted its exact three table plans")
    if not isinstance(routes, dict) or set(routes) != set(tables):
        raise Refusal("Resolution producer omitted its exact three table routes")
    planned = producer.get("plannedInput")
    if (
        not isinstance(planned, dict)
        or planned.get("format") != RESOLUTION_INPUT_SCHEMA
    ):
        raise Refusal("Resolution producer planned another input schema")
    if planned.get("accounts", {}).get("market") != market:
        raise Refusal("Resolution producer planned input changed its Market")
    complete = all(
        isinstance(route, dict) and route.get("action") == "complete"
        for route in routes.values()
    )
    if require_complete and (not complete or producer.get("flagshipInput") != planned):
        raise Refusal("Resolution producer has not frozen all three exact tables")
    if not require_complete and producer.get("flagshipInput") not in (None, planned):
        raise Refusal("Resolution producer carried a substituted flagship input")
    return producer


def authenticate_resolution_table_journal(
    document: Any,
    *,
    require_complete: bool,
) -> list[dict[str, Any]]:
    journal = exact_keys(
        document,
        {
            "format",
            "producerIdentitySha256",
            "phase",
            "intent",
            "intentSha256",
            "signedTransactionBase64",
            "signedTransactionSha256",
            "expectedSignature",
            "finalized",
            "receipts",
        },
        "Resolution table journal",
    )
    if journal.get("format") != RESOLUTION_TABLE_SCHEMA:
        raise Refusal("Resolution table journal used another owned-loopback schema")
    lowercase_sha256(
        journal.get("producerIdentitySha256"), "Resolution producer identity"
    )
    receipts = journal.get("receipts")
    if not isinstance(receipts, list):
        raise Refusal("Resolution table journal receipts are not one array")
    signatures: set[str] = set()
    prior_slot = 0
    facts: list[dict[str, Any]] = []
    for index, receipt in enumerate(receipts):
        fact = finalized_fact(receipt, f"Resolution table receipt {index}")
        if fact["signature"] in signatures or fact["slot"] < prior_slot:
            raise Refusal(
                "Resolution table receipts repeat a signature or regress slots"
            )
        signatures.add(fact["signature"])
        prior_slot = fact["slot"]
        facts.append(fact)
    if require_complete and (
        journal.get("phase") != "finalized"
        or journal.get("intent") is not None
        or journal.get("finalized") is not None
    ):
        raise Refusal("Resolution table journal still carries an unfinished action")
    return facts


def authenticate_resolution_checkpoint(document: Any) -> list[dict[str, Any]]:
    checkpoint = exact_keys(
        document,
        {"format", "inputSha256", "stagePlan", "receipts", "verifiedTerminal"},
        "Resolution checkpoint",
    )
    if (
        checkpoint.get("format") != RESOLUTION_CHECKPOINT_SCHEMA
        or checkpoint.get("stagePlan") is not None
        or checkpoint.get("verifiedTerminal") is not True
    ):
        raise Refusal("Resolution checkpoint is not verified terminal")
    lowercase_sha256(checkpoint.get("inputSha256"), "Resolution input digest")
    receipts = checkpoint.get("receipts")
    if not isinstance(receipts, list) or [row.get("stage") for row in receipts] != [
        "submit",
        "execute",
        "reclaim",
    ]:
        raise Refusal("Resolution checkpoint omitted submit/execute/reclaim receipts")
    facts = [
        finalized_fact(row, f"Resolution {row.get('stage')} receipt")
        for row in receipts
    ]
    if len({fact["signature"] for fact in facts}) != len(facts) or any(
        left["slot"] > right["slot"] for left, right in zip(facts, facts[1:])
    ):
        raise Refusal("Resolution receipts repeat a signature or regress slots")
    return facts


def keypairs_by_address(paths: Paths, report: dict[str, Any]) -> dict[str, Path]:
    rows = report.get("keypairs")
    if not isinstance(rows, dict) or not rows:
        raise Refusal("local mutable preparation omitted disposable keypairs")
    output: dict[str, Path] = {}
    for role, value in sorted(rows.items()):
        if not isinstance(role, str) or not isinstance(value, str):
            raise Refusal("local mutable keypair projection changed shape")
        key = canonical_file(value, f"disposable keypair {role}")
        address = canonical_pubkey(
            key_address(paths.solana, key), f"disposable key {role}"
        )
        if address in output:
            raise Refusal("two disposable keypair roles alias one signer")
        output[address] = key
    return output


def require_keypair(keys: dict[str, Path], address: str, label: str) -> Path:
    canonical_pubkey(address, label)
    try:
        return keys[address]
    except KeyError as error:
        raise Refusal(
            f"{label} is not one supervisor-owned disposable signer"
        ) from error


def authenticate_payout_input(
    document: Any,
    target: PayoutTarget,
    market: str,
) -> dict[str, Any]:
    payout = exact_keys(
        document,
        {
            "format",
            "market",
            "owner",
            "recipientOwner",
            "recipient",
            "collateralMint",
            "tokenProgram",
            "quantity",
            "claimIndex",
            "transferIndex",
            "parentContext",
            "custodyContext",
            "releaseSet",
            "terminalCertificate",
            "programs",
            "records",
        },
        "wallet payout input",
    )
    if (
        payout.get("format") != PAYOUT_INPUT_SCHEMA
        or payout.get("market") != market
        or payout.get("owner") != target.owner
        or payout.get("recipientOwner") != target.owner
        or payout.get("recipient") != target.recipient
        or payout.get("claimIndex") != target.claim_index
        or payout.get("transferIndex") != 0
    ):
        raise Refusal("wallet payout input changed Direct's canonical target identity")
    canonical_decimal(payout.get("quantity"), "wallet payout quantity")
    for field in ("collateralMint", "tokenProgram", "terminalCertificate"):
        canonical_pubkey(payout.get(field), f"wallet payout {field}")
    return payout


def authenticate_payout_evidence(
    document: Any,
    input_path: Path,
    target: PayoutTarget,
    market: str,
) -> dict[str, Any]:
    evidence = exact_keys(
        document,
        {
            "schema",
            "cluster",
            "inputSha256",
            "payoutIntentSha256",
            "journalStateSha256",
            "signature",
            "finalizedSlot",
            "feeLamports",
            "computeUnitsConsumed",
            "feePayer",
            "owner",
            "market",
            "recipient",
            "payout",
            "lookupTable",
            "lookupAddressesSha256",
            "payoutInstructionSha256",
            "custodyRequestSha256",
            "returnDataProducer",
            "returnDataBase64",
            "poststates",
            "evidenceSha256",
        },
        "wallet payout evidence",
    )
    if (
        evidence.get("schema") != PAYOUT_EVIDENCE_SCHEMA
        or evidence.get("cluster") != "owned-loopback"
        or evidence.get("inputSha256") != sha256_file(input_path)
        or evidence.get("owner") != target.owner
        or evidence.get("market") != market
        or evidence.get("recipient") != target.recipient
    ):
        raise Refusal(
            "wallet payout evidence changed its exact input/owner/market/recipient join"
        )
    finalized_fact(
        evidence,
        "wallet payout evidence",
        slot_key="finalizedSlot",
    )
    for field in (
        "inputSha256",
        "payoutIntentSha256",
        "journalStateSha256",
        "lookupAddressesSha256",
        "payoutInstructionSha256",
        "evidenceSha256",
    ):
        lowercase_sha256(evidence.get(field), f"wallet payout {field}")
    return evidence


def authenticate_terminal_completion(
    document: Any,
    *,
    market: str,
    payer: str,
) -> dict[str, Any]:
    completion = exact_keys(
        document,
        {
            "schema",
            "status",
            "cluster",
            "genesisHash",
            "invocation",
            "session",
            "journalDirectory",
            "market",
            "payer",
            "lookupTable",
            "journals",
            "finalizedSlot",
            "transactionFeesLamports",
            "computeUnitsConsumed",
        },
        "terminal retirement completion",
    )
    if (
        completion.get("schema") != TERMINAL_COMPLETION_SCHEMA
        or completion.get("status") != "finalized"
        or completion.get("cluster") != "owned-loopback"
        or completion.get("market") != market
        or completion.get("payer") != payer
        or completion.get("invocation", {}).get("command")
        != TERMINAL_RETIREMENT_COMMAND
    ):
        raise Refusal(
            "terminal retirement completion changed cluster, command, Market, or payer"
        )
    canonical_pubkey(completion.get("genesisHash"), "terminal genesis")
    canonical_pubkey(completion.get("lookupTable"), "terminal lookup table")
    journals = completion.get("journals")
    if not isinstance(journals, list) or not journals:
        raise Refusal("terminal retirement completion omitted finalized journals")
    fixed_tail = [
        "core-begin-retiring",
        "direct-begin-retiring",
        "resolution-close-fund",
        "direct-close-capability",
        "retirement-replay-handoff",
        "aggregate-retirement",
    ]
    kinds = [row.get("mutation", {}).get("kind") for row in journals]
    if kinds[-len(fixed_tail) :] != fixed_tail:
        raise Refusal(
            "terminal retirement completion omitted the exact six protocol mutations"
        )
    facts: list[dict[str, Any]] = []
    prior_slot = 0
    signatures: set[str] = set()
    total_fees = 0
    total_compute = 0
    for index, row in enumerate(journals):
        if (
            row.get("schema") != TERMINAL_JOURNAL_SCHEMA
            or row.get("phase") != "finalized"
        ):
            raise Refusal(
                "terminal completion references a nonfinal or substituted journal"
            )
        fact = finalized_fact(
            row,
            f"terminal journal {index}",
            slot_key="finalizedSlot",
            fee_key="transactionFeeLamports",
            decimal_text=True,
        )
        if fact["signature"] in signatures or fact["slot"] < prior_slot:
            raise Refusal("terminal journal signatures repeat or slots regress")
        signatures.add(fact["signature"])
        prior_slot = fact["slot"]
        total_fees += fact["fee_lamports"]
        total_compute += fact["compute_units_consumed"]
        facts.append(fact)
    if (
        canonical_decimal(completion.get("finalizedSlot"), "terminal finalized slot")
        != max(fact["slot"] for fact in facts)
        or canonical_decimal(
            completion.get("transactionFeesLamports"), "terminal fee total"
        )
        != total_fees
        or canonical_decimal(
            completion.get("computeUnitsConsumed"), "terminal CU total"
        )
        != total_compute
    ):
        raise Refusal(
            "terminal completion aggregate slot, fee, or compute arithmetic changed"
        )
    return completion


def authenticate_terminal_stdout(
    document: Any,
    *,
    completion: dict[str, Any],
    completion_path: Path,
    journal_dir: Path,
    market: str,
) -> dict[str, Any]:
    summary = exact_keys(
        document,
        {
            "status",
            "market",
            "lookupTable",
            "journalDirectory",
            "completion",
            "completionSha256",
            "message",
        },
        "terminal retirement stdout",
    )
    if (
        summary.get("status") != "complete"
        or summary.get("market") != market
        or summary.get("lookupTable") != completion.get("lookupTable")
        or summary.get("journalDirectory") != str(journal_dir)
        or summary.get("completion") != str(completion_path)
        or summary.get("completionSha256") != sha256_file(completion_path)
        or summary.get("message")
        != "Every exact terminal journal reverified at finalized and the aggregate Market account is closed."
    ):
        raise Refusal(
            "terminal retirement stdout changed its exact completion path, hash, or semantic summary"
        )
    return summary


def run_flagship_resolution(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
    plan: Path,
    campaign_evidence: Path,
    pyth_facts: Path,
    first_ordinal: int,
) -> tuple[dict[str, Any], int]:
    """Drive only the accepted Resolution semantic owners through Complete."""

    root = run / "resolution"
    root.mkdir(mode=0o700)
    producer_path = root / "producer.json"
    table_path = root / "tables.json"
    input_path = root / "input.json"
    checkpoint_path = root / "checkpoint.json"
    for path in (producer_path, table_path, input_path, checkpoint_path):
        if path.exists():
            raise Refusal(
                "fresh Resolution controller root contains a preexisting artifact"
            )
    keys = keypairs_by_address(paths, report)
    ordinal = first_ordinal

    def produce() -> dict[str, Any]:
        nonlocal ordinal
        document = run_json_stage(
            run,
            ordinal,
            f"resolution-produce-{ordinal - first_ordinal:02d}",
            [
                str(paths.bootstrap),
                FLAGSHIP_RESOLUTION_COMMAND,
                "--produce-input",
                "--rpc-url",
                url,
                "--plan",
                str(plan),
                "--campaign-evidence",
                str(campaign_evidence),
                "--pyth-facts",
                str(pyth_facts),
                "--producer-checkpoint",
                str(producer_path),
                "--output",
                str(input_path),
            ],
        )
        ordinal += 1
        if read_unique_json(producer_path, "persisted Resolution producer") != document:
            raise Refusal(
                "Resolution producer stdout differs from its persisted checkpoint"
            )
        return document

    producer = produce()
    for _attempt in range(MAX_RESOLUTION_TABLE_INVOCATIONS):
        authenticated = authenticate_resolution_producer(
            producer, require_complete=False
        )
        if authenticated.get("flagshipInput") is not None:
            authenticate_resolution_producer(authenticated, require_complete=True)
            if (
                read_unique_json(input_path, "Resolution input")
                != authenticated["plannedInput"]
            ):
                raise Refusal(
                    "Resolution input differs from the completed producer checkpoint"
                )
            break
        authority = authenticated["authority"]
        document = run_json_stage(
            run,
            ordinal,
            f"resolution-table-{ordinal - first_ordinal:02d}",
            [
                str(paths.bootstrap),
                FLAGSHIP_RESOLUTION_COMMAND,
                "--provision-tables",
                "--rpc-url",
                url,
                "--producer-checkpoint",
                str(producer_path),
                "--table-journal",
                str(table_path),
                "--authority-keypair",
                str(require_keypair(keys, authority, "Resolution table authority")),
                "--execute",
            ],
        )
        ordinal += 1
        if (
            read_unique_json(table_path, "persisted Resolution table journal")
            != document
        ):
            raise Refusal("Resolution table stdout differs from its persisted journal")
        authenticate_resolution_table_journal(document, require_complete=False)
        producer = produce()
    else:
        raise Refusal(
            "Resolution tables exceeded the bounded 64-invocation controller loop"
        )

    table = read_unique_json(table_path, "completed Resolution table journal")
    table_facts = authenticate_resolution_table_journal(table, require_complete=True)
    planned = producer["plannedInput"]
    submitter = canonical_pubkey(planned.get("submitter"), "Resolution submitter")
    resolver = canonical_pubkey(planned.get("resolver"), "Resolution resolver")
    update_account = canonical_pubkey(
        planned.get("accounts", {}).get("updateAccount"), "Resolution update account"
    )
    execute_argv = [
        str(paths.bootstrap),
        FLAGSHIP_RESOLUTION_COMMAND,
        "--rpc-url",
        url,
        "--input",
        str(input_path),
        "--checkpoint",
        str(checkpoint_path),
        "--through",
        "complete",
        "--submitter-keypair",
        str(require_keypair(keys, submitter, "Resolution submitter")),
        "--resolver-keypair",
        str(require_keypair(keys, resolver, "Resolution resolver")),
        "--update-keypair",
        str(require_keypair(keys, update_account, "Resolution update signer")),
        "--execute",
    ]
    checkpoint: dict[str, Any] | None = None
    for attempt in range(MAX_RESOLUTION_STAGE_INVOCATIONS):
        checkpoint = run_json_stage(
            run,
            ordinal,
            f"resolution-execute-{attempt:02d}",
            execute_argv,
        )
        ordinal += 1
        if (
            read_unique_json(checkpoint_path, "persisted Resolution checkpoint")
            != checkpoint
        ):
            raise Refusal(
                "Resolution executor stdout differs from its persisted checkpoint"
            )
        if checkpoint.get("verifiedTerminal") is True:
            stage_facts = authenticate_resolution_checkpoint(checkpoint)
            return {
                "schema": "dclutch-private-validator-resolution-controller-v1",
                "status": "finalized",
                "producer": str(producer_path),
                "producer_sha256": sha256_file(producer_path),
                "table_journal": str(table_path),
                "table_journal_sha256": sha256_file(table_path),
                "input": str(input_path),
                "input_sha256": sha256_file(input_path),
                "checkpoint": str(checkpoint_path),
                "checkpoint_sha256": sha256_file(checkpoint_path),
                "market": producer["market"],
                "table_transactions": table_facts,
                "stage_transactions": stage_facts,
            }, ordinal
    raise Refusal("Resolution exceeded the bounded 16-invocation stage controller loop")


def run_wallet_payouts(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
    plan: Path,
    campaign_evidence: Path,
    market: str,
    schedule: Sequence[PayoutTarget],
    first_ordinal: int,
) -> tuple[list[dict[str, Any]], int]:
    """Produce and execute exactly one semantic-owner payout per nonzero claim."""

    targets = canonical_payout_schedule(schedule)
    canonical_pubkey(market, "payout Market")
    root = run / "payouts"
    root.mkdir(mode=0o700)
    keys = keypairs_by_address(paths, report)
    payer_key = require_role_key(report, VALIDATOR_MINT_ROLE)
    payer = canonical_pubkey(key_address(paths.solana, payer_key), "payout fee payer")
    ordinal = first_ordinal
    rows: list[dict[str, Any]] = []
    for index, target in enumerate(targets):
        payout_root = root / f"{index:03d}"
        payout_root.mkdir(mode=0o700)
        input_path = payout_root / "input.json"
        evidence_path = payout_root / "evidence.json"
        journal_dir = payout_root / "journals"
        journal_dir.mkdir(mode=0o700)
        input_result = run_stage(
            run,
            ordinal,
            f"payout-input-{index:03d}",
            [
                str(paths.bootstrap),
                PAYOUT_INPUT_COMMAND,
                "--rpc-url",
                url,
                "--plan",
                str(plan),
                "--evidence",
                str(campaign_evidence),
                "--market",
                market,
                "--owner",
                target.owner,
                "--recipient",
                target.recipient,
                "--claim-index",
                str(target.claim_index),
            ],
        )
        ordinal += 1
        try:
            input_document = decode_unique_json(
                input_result.stdout.decode("utf-8"), f"payout input {index}"
            )
        except UnicodeDecodeError as error:
            raise Refusal(f"payout input {index} was not UTF-8") from error
        authenticate_payout_input(input_document, target, market)
        write_bytes_new(input_path, input_result.stdout)
        execute_argv = [
            str(paths.bootstrap),
            PAYOUT_EXECUTE_COMMAND,
            "--rpc-url",
            url,
            "--input",
            str(input_path),
            "--fee-payer",
            payer,
            "--fee-payer-keypair",
            str(payer_key),
            "--owner-keypair",
            str(require_keypair(keys, target.owner, "payout owner")),
            "--journal-dir",
            str(journal_dir),
            "--evidence",
            str(evidence_path),
            "--execute",
        ]
        for attempt in range(MAX_PAYOUT_INVOCATIONS):
            stdout = run_json_stage(
                run,
                ordinal,
                f"payout-{index:03d}-{attempt:02d}",
                execute_argv,
            )
            ordinal += 1
            if evidence_path.is_file():
                persisted = read_unique_json(evidence_path, f"payout evidence {index}")
                if stdout != persisted:
                    # One extra semantic-owner call must reauthenticate and print exact evidence.
                    stdout = run_json_stage(
                        run,
                        ordinal,
                        f"payout-authenticate-{index:03d}",
                        execute_argv,
                    )
                    ordinal += 1
                if stdout != persisted:
                    raise Refusal(
                        "payout reauthentication differs from persisted evidence"
                    )
                evidence = authenticate_payout_evidence(
                    persisted, input_path, target, market
                )
                rows.append(
                    {
                        "target": dataclasses.asdict(target),
                        "input": str(input_path),
                        "input_sha256": sha256_file(input_path),
                        "evidence": str(evidence_path),
                        "evidence_sha256": sha256_file(evidence_path),
                        "finalized": finalized_fact(
                            evidence,
                            f"payout {index}",
                            slot_key="finalizedSlot",
                        ),
                    }
                )
                break
        else:
            raise Refusal(f"payout {index} exceeded the bounded 24-invocation loop")
    return rows, ordinal


def run_terminal_retirement(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
    plan: Path,
    market_input: Path,
    campaign_evidence: Path,
    market: str,
    first_ordinal: int,
) -> tuple[dict[str, Any], int]:
    """Advance the accepted terminal semantic owner until create-new completion."""

    root = run / "retirement"
    root.mkdir(mode=0o700)
    journal_dir = root / "journals"
    journal_dir.mkdir(mode=0o700)
    session_path = root / "session.json"
    completion_path = root / "completion.json"
    payer_key = require_role_key(report, VALIDATOR_MINT_ROLE)
    payer = canonical_pubkey(
        key_address(paths.solana, payer_key), "retirement fee payer"
    )
    argv = [
        str(paths.bootstrap),
        TERMINAL_RETIREMENT_COMMAND,
        "--rpc-url",
        url,
        "--plan",
        str(plan),
        "--market-input",
        str(market_input),
        "--evidence",
        str(campaign_evidence),
        "--market",
        market,
        "--fee-payer",
        payer,
        "--fee-payer-keypair",
        str(payer_key),
        "--session",
        str(session_path),
        "--journal-dir",
        str(journal_dir),
        "--completion",
        str(completion_path),
        "--execute",
    ]
    ordinal = first_ordinal
    for attempt in range(MAX_TERMINAL_INVOCATIONS):
        stdout = run_json_stage(
            run,
            ordinal,
            f"retirement-{attempt:02d}",
            argv,
        )
        ordinal += 1
        if completion_path.is_file():
            completion = read_unique_json(
                completion_path, "terminal retirement completion"
            )
            authenticate_terminal_completion(completion, market=market, payer=payer)
            authenticate_terminal_stdout(
                stdout,
                completion=completion,
                completion_path=completion_path,
                journal_dir=journal_dir,
                market=market,
            )
            transactions = [
                {
                    "mutation": row["mutation"]["kind"],
                    **finalized_fact(
                        row,
                        f"terminal journal {index}",
                        slot_key="finalizedSlot",
                        fee_key="transactionFeeLamports",
                        decimal_text=True,
                    ),
                }
                for index, row in enumerate(completion["journals"])
            ]
            return {
                "schema": "dclutch-private-validator-retirement-controller-v1",
                "status": "finalized",
                "session": str(session_path),
                "session_sha256": sha256_file(session_path),
                "journal_dir": str(journal_dir),
                "completion": str(completion_path),
                "completion_sha256": sha256_file(completion_path),
                "transactions": transactions,
            }, ordinal
    raise Refusal("terminal retirement exceeded the bounded 32-invocation loop")


def run_post_direct_lifecycle(
    run: Path,
    paths: Paths,
    report: dict[str, Any],
    url: str,
    plan: Path,
    market_input: Path,
    campaign_evidence: Path,
    payout_schedule: Sequence[PayoutTarget],
    first_ordinal: int,
) -> tuple[dict[str, Any], int]:
    """Post-Direct seam; caller must come from the future frozen Direct adapter."""

    pyth_facts, pyth, ordinal = run_pyth_provisioning(
        run, paths, report, url, first_ordinal
    )
    resolution, ordinal = run_flagship_resolution(
        run,
        paths,
        report,
        url,
        plan,
        campaign_evidence,
        pyth_facts,
        ordinal,
    )
    payouts, ordinal = run_wallet_payouts(
        run,
        paths,
        report,
        url,
        plan,
        campaign_evidence,
        resolution["market"],
        payout_schedule,
        ordinal,
    )
    retirement, ordinal = run_terminal_retirement(
        run,
        paths,
        report,
        url,
        plan,
        market_input,
        campaign_evidence,
        resolution["market"],
        ordinal,
    )
    result = {
        "schema": "dclutch-private-validator-post-direct-lifecycle-v1",
        "status": "finalized",
        "pyth": pyth,
        "resolution": resolution,
        "payouts": payouts,
        "retirement": retirement,
    }
    result["compute_units"] = post_direct_compute_units(result)
    return result, ordinal


def post_direct_compute_units(result: dict[str, Any]) -> dict[str, int]:
    metrics: dict[str, int] = {}
    for action, value in result["pyth"]["compute_units"].items():
        metrics[f"pyth-{action}"] = positive_integer(value, f"Pyth {action} CU")
    for family in ("table_transactions", "stage_transactions"):
        for index, fact in enumerate(result["resolution"][family]):
            metrics[f"resolution-{family}-{index:02d}"] = positive_integer(
                fact.get("compute_units_consumed"), f"Resolution {family} CU"
            )
    for index, payout in enumerate(result["payouts"]):
        metrics[f"payout-{index:03d}"] = positive_integer(
            payout["finalized"].get("compute_units_consumed"), f"payout {index} CU"
        )
    for index, transaction in enumerate(result["retirement"]["transactions"]):
        metrics[f"retirement-{index:02d}-{transaction['mutation']}"] = positive_integer(
            transaction.get("compute_units_consumed"), f"retirement {index} CU"
        )
    return metrics


def accepted_direct_payout_schedule(_direct_evidence: Path) -> tuple[PayoutTarget, ...]:
    """Intentional seam: no provisional Direct schema may become controller authority."""

    raise Refusal(
        "Direct payout schedule adapter is not frozen; require exact ordered "
        "nonzero owner/recipient/claimIndex facts from the Direct semantic owner"
    )


def checked_mutable_slot_floor(plan_path: Path) -> int:
    plan = read_unique_json(plan_path, "checked local mutable plan")
    checked_set = plan.get("checked_local_mutable_set")
    roles = checked_set.get("roles") if isinstance(checked_set, dict) else None
    if not isinstance(roles, list) or len(roles) != len(ROLE_ORDER):
        raise Refusal(
            "checked local mutable plan omitted its exact seven-role slot projection"
        )
    slots = [role.get("deployment_slot") for role in roles if isinstance(role, dict)]
    if len(slots) != len(ROLE_ORDER) or any(
        not isinstance(slot, int) or slot < 0 for slot in slots
    ):
        raise Refusal("checked local mutable plan carried an invalid deployment slot")
    return max(slots)


def named_seed(index: int) -> tuple[str, str]:
    name = f"seed-{index:02d}"
    return name, hashlib.sha256(SEED_DOMAIN + name.encode()).hexdigest()


def key_flags(report: dict[str, Any]) -> list[str]:
    flags: list[str] = []
    campaign_keypairs = report.get("campaign_keypairs")
    if not isinstance(campaign_keypairs, dict) or not campaign_keypairs:
        raise Refusal(
            "local mutable preparation omitted its Rust-owned campaign keypair projection"
        )
    for role, path in sorted(campaign_keypairs.items()):
        flags.extend((f"--keypair-{role}", path))
    return flags


def authenticate_participant_fixture_liquidity(
    campaign: dict[str, Any],
    market: dict[str, Any],
    participant: str,
    source_account: str,
) -> dict[str, Any]:
    execution = campaign.get("execution")
    fixture = (
        execution.get("localParticipantFixtureLiquidity")
        if isinstance(execution, dict)
        else None
    )
    expected_keys = {
        "sourceTokenAccount",
        "sourceOwner",
        "quantityAtoms",
        "foundingCollateralAtoms",
        "totalSupplyAtoms",
        "mint",
        "mintAuthorityRemoved",
        "transactionSignature",
        "finalizedSlot",
        "computeUnitsConsumed",
    }
    founding_atoms = market.get("initial_collateral_atoms")
    expected_mint = campaign.get("founding_targets", {}).get("collateral_mint")
    if (
        not isinstance(fixture, dict)
        or set(fixture) != expected_keys
        or fixture.get("sourceTokenAccount") != source_account
        or fixture.get("sourceOwner") != participant
        or fixture.get("quantityAtoms") != PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS
        or not isinstance(founding_atoms, int)
        or founding_atoms <= 0
        or fixture.get("foundingCollateralAtoms") != founding_atoms
        or fixture.get("totalSupplyAtoms")
        != founding_atoms + PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS
        or fixture.get("mint") != expected_mint
        or fixture.get("mintAuthorityRemoved") is not True
        or not isinstance(fixture.get("transactionSignature"), str)
        or not fixture["transactionSignature"]
        or not isinstance(fixture.get("finalizedSlot"), int)
        or fixture["finalizedSlot"] <= 0
        or not isinstance(fixture.get("computeUnitsConsumed"), int)
        or fixture["computeUnitsConsumed"] <= 0
    ):
        raise Refusal(
            "founding evidence omitted the exact 100,000,000-atom, "
            "authority-removed local participant liquidity receipt"
        )
    return fixture


def dcltgmf2_metric(evidence: Path) -> int:
    document = read_unique_json(evidence, "founding evidence")
    transactions = document.get("execution", {}).get("transactions", [])
    rows = [row for row in transactions if "DCLTGMF2" in row.get("label", "")]
    if len(rows) != 1 or not isinstance(rows[0].get("compute_units_consumed"), int):
        raise Refusal(
            "founding evidence does not carry one exact DCLTGMF2 compute measurement"
        )
    return rows[0]["compute_units_consumed"]


def run_one(
    paths: Paths,
    gate_path: Path,
    gate: dict[str, Any],
    gate_digest: str,
    index: int,
    through: str,
) -> dict[str, Any]:
    name, seed = named_seed(index)
    run = paths.work / "runs" / name
    run.mkdir(parents=True, exist_ok=False)
    (run / "stages").mkdir()
    port = allocate_port_block(index)
    url = f"http://127.0.0.1:{port}"
    prepare_work = run / "mutable"
    plan = prepare_work / "plan.json"
    prepare = run_stage(
        run,
        1,
        "prepare-mutable",
        [
            str(paths.bootstrap),
            "local-mutable-prepare-v1",
            "--work",
            str(prepare_work),
            "--output",
            str(plan),
            "--checked-release-gate",
            str(gate_path),
            "--expected-checked-release-gate-sha256",
            gate_digest,
            "--expected-source-revision",
            gate["source_revision"],
            "--expected-source-tree-sha256",
            gate["source_tree_sha256"],
            "--seed",
            seed,
        ],
    )
    report = json.loads(prepare.stdout)
    if report.get("schema") != "dclutch-local-mutable-prepare-report-v1":
        raise Refusal("local mutable preparation returned another report schema")
    expected_key_root = (prepare_work / "keys").resolve()
    for role, key_text in report.get("keypairs", {}).items():
        key = canonical_file(key_text, f"disposable local key {role}")
        if key.parent != expected_key_root:
            raise Refusal(f"local mutable key {role} escaped the owned seed directory")
    run_stage(
        run,
        2,
        "authenticate-mutable",
        [
            str(paths.bootstrap),
            "local-mutable-plan-authenticate-v1",
            "--plan",
            str(plan),
        ],
    )

    ledger = run / "ledger"
    validator_log = (run / "validator.log").open("wb")
    child: subprocess.Popen[bytes] | None = None
    watchdog: subprocess.Popen[bytes] | None = None
    try:
        mint_key = require_role_key(report, VALIDATOR_MINT_ROLE)
        mint_address = key_address(paths.solana, mint_key)
        child = subprocess.Popen(
            validator_argv(
                paths.validator,
                ledger,
                report["account_dir"],
                mint_address,
                port,
            ),
            stdin=subprocess.DEVNULL,
            stdout=validator_log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        watchdog = subprocess.Popen(
            [
                sys.executable,
                str(Path(__file__).with_name("watchdog.py")),
                "--supervisor-pid",
                str(os.getpid()),
                "--validator-pid",
                str(child.pid),
                "--ledger",
                str(ledger),
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        wait_ready(url, child)
        if rpc(url, "getGenesisHash") == "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d":
            raise Refusal(
                "fresh private validator reported the mainnet-beta genesis hash"
            )
        funding_poststate = provision_disposable_funding(run, paths, report, url)
        slot_floor = checked_mutable_slot_floor(plan)
        funding_poststate["finalized_slot"] = wait_finalized_slot(
            url, child, slot_floor
        )
        funding_poststate["checked_mutable_slot_floor"] = slot_floor
        # Preserve the finalized-slot observation separately because the
        # funding poststate was already fsynced before this wait began.
        write_json_new(
            run / "validator-readiness.json",
            {
                "schema": "dclutch-private-validator-readiness-v1",
                "checked_mutable_slot_floor": slot_floor,
                "observed_finalized_slot": funding_poststate["finalized_slot"],
            },
        )

        market = run / "market.json"
        fee_recipient_key = require_role_key(report, DEVELOPMENT_FEE_RECIPIENT_ROLE)
        fee_recipient = key_address(paths.solana, fee_recipient_key)
        market_stage = run_stage(
            run,
            4,
            "market-input",
            [
                str(paths.bootstrap),
                "local-private-validator-market-v1",
                "--plan",
                str(plan),
                "--rpc-url",
                url,
                "--fee-basis-points",
                str(DEVELOPMENT_FEE_BASIS_POINTS),
                "--fee-recipient-keypair",
                str(fee_recipient_key),
            ],
        )
        write_bytes_new(market, market_stage.stdout)
        evidence = run / "founding.json"
        run_stage(
            run,
            5,
            "founding",
            [
                str(paths.bootstrap),
                "campaign",
                "--rpc-url",
                url,
                "--plan",
                str(plan),
                "--market",
                str(market),
                "--evidence",
                str(evidence),
                "--through",
                "founding",
                "--execute",
                *key_flags(report),
            ],
        )

        participant = run / "participant.json"
        participant_key = require_role_key(report, PARTICIPANT_ROLE)
        fixture_source_key = require_role_key(report, PARTICIPANT_FIXTURE_SOURCE_ROLE)
        participant_address = key_address(paths.solana, participant_key)
        fixture_source_address = key_address(paths.solana, fixture_source_key)
        campaign_report = read_unique_json(evidence, "finalized founding evidence")
        market_report = read_unique_json(market, "market input")
        fixture_liquidity = authenticate_participant_fixture_liquidity(
            campaign_report,
            market_report,
            participant_address,
            fixture_source_address,
        )
        payer_key = require_role_key(report, VALIDATOR_MINT_ROLE)
        minimum_slot = rpc(url, "getSlot", [{"commitment": "finalized"}])
        if not isinstance(minimum_slot, int) or minimum_slot <= 0:
            raise Refusal(
                "localhost finalized slot after founding was not a positive integer"
            )
        run_stage(
            run,
            6,
            "participant",
            [
                str(paths.bootstrap),
                "local-private-validator-user-position-admission-v1",
                "--rpc-url",
                url,
                "--plan",
                str(plan),
                "--campaign-evidence",
                str(evidence),
                "--position-owner",
                participant_address,
                "--position-owner-keypair",
                str(participant_key),
                "--fee-payer",
                key_address(paths.solana, payer_key),
                "--fee-payer-keypair",
                str(payer_key),
                "--minimum-finalized-slot",
                str(minimum_slot),
                "--output",
                str(participant),
                "--collateral-source-owner",
                participant_address,
                "--collateral-source-owner-keypair",
                str(participant_key),
                "--collateral-source-account",
                fixture_source_address,
                "--collateral-quantity-atoms",
                str(PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS),
                "--execute",
            ],
        )
        participant_report = read_unique_json(
            participant, "participant admission evidence"
        )
        if (
            participant_report.get("schema")
            != "dclutch-owned-loopback-user-position-admission-execution-v1"
            or participant_report.get("cluster") != "owned-loopback"
            or participant_report.get("phase") != "finalized"
            or not participant_report.get("authorizedMutation")
            or not isinstance(participant_report.get("finalized"), dict)
            or not isinstance(participant_report.get("collateral"), dict)
            or participant_report["collateral"].get("phase") != "finalized"
            or not isinstance(participant_report["collateral"].get("finalized"), dict)
            or participant_report["collateral"].get("intent", {}).get("sourceAccount")
            != fixture_source_address
            or participant_report["collateral"].get("intent", {}).get("sourceOwner")
            != participant_address
            or participant_report["collateral"].get("intent", {}).get("quantityAtoms")
            != PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS
        ):
            raise Refusal(
                "participant admission did not preserve finalized admission plus exact "
                "100,000,000-atom collateral preparation"
            )

        metric = dcltgmf2_metric(evidence)
        fee_profile = {
            "basis_points_numerator": DEVELOPMENT_FEE_BASIS_POINTS,
            "basis_points_denominator": FEE_BASIS_POINTS_DENOMINATOR,
            "gross_unit": "collateral-atoms",
            "rounding": "floor-per-side",
            "seller_fee_atoms": {
                "gross_multiplier": DEVELOPMENT_FEE_BASIS_POINTS,
                "divisor": FEE_BASIS_POINTS_DENOMINATOR,
            },
            "buyer_fee_atoms": {
                "gross_multiplier": DEVELOPMENT_FEE_BASIS_POINTS,
                "divisor": FEE_BASIS_POINTS_DENOMINATOR,
            },
            "recipient_credit": "seller_fee_atoms + buyer_fee_atoms",
            "recipient": fee_recipient,
            "recipient_role": "founding-source-funder",
            "market_input_sha256": sha256_file(market),
        }
        if through == "participant":
            result = {
                "schema": PARTICIPANT_PROBE_RUN_SCHEMA,
                "name": name,
                "seed_sha256": sha256_bytes(bytes.fromhex(seed)),
                "status": "passed",
                "finalized_stages": ["founding", "participant"],
                "dcltgmf2_compute_units": metric,
                "compute_units": {"founding-dcltgmf2": metric},
                "fee_profile": fee_profile,
                "participant_fixture_liquidity": fixture_liquidity,
                "plan": str(plan),
                "founding_evidence": str(evidence),
                "participant_evidence": str(participant),
                "validator_log": str(run / "validator.log"),
            }
            write_json_new(run / "RESULT.json", result)
            return result

        # Exterior stages intentionally have their own driver commands.  The
        # exact argument producers are frozen beside those commands; this
        # supervisor never reconstructs their packets or persisted facts.
        # Direct is executed by its accepted exterior immediately before this
        # provisioning boundary. Its exact caller supplies direct-finalized.json.
        direct_evidence = run / "direct-finalized.json"
        if not direct_evidence.is_file():
            raise Refusal(
                "accepted Direct exterior did not preserve run/direct-finalized.json before Pyth"
            )
        schedule = accepted_direct_payout_schedule(direct_evidence)
        post_direct, _next_ordinal = run_post_direct_lifecycle(
            run,
            paths,
            report,
            url,
            plan,
            market,
            evidence,
            schedule,
            8,
        )
        result = {
            "schema": RUN_SCHEMA if through == "full" else FULL_PROBE_RUN_SCHEMA,
            "name": name,
            "seed_sha256": sha256_bytes(bytes.fromhex(seed)),
            "status": "passed",
            "dcltgmf2_compute_units": metric,
            "compute_units": {
                "founding-dcltgmf2": metric,
                **post_direct["compute_units"],
            },
            "fee_profile": fee_profile,
            "plan": str(plan),
            "founding_evidence": str(evidence),
            "participant_evidence": str(participant),
            "direct_evidence": str(direct_evidence),
            "post_direct": post_direct,
            "validator_log": str(run / "validator.log"),
        }
        write_json_new(run / "RESULT.json", result)
        return result
    finally:
        terminate_group(child)
        if watchdog is not None:
            try:
                watchdog.wait(timeout=5)
            except subprocess.TimeoutExpired:
                terminate_group(watchdog)
        validator_log.close()


def arithmetic_mean(values: Iterable[int]) -> dict[str, int]:
    rows = list(values)
    if not rows:
        return {"numerator": 0, "denominator": 0, "floor": 0, "remainder": 0}
    total = sum(rows)
    count = len(rows)
    return {
        "numerator": total,
        "denominator": count,
        "floor": total // count,
        "remainder": total % count,
    }


def compute_unit_report(runs: Sequence[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    rows: dict[str, list[int]] = {}
    for run in runs:
        metrics = run.get("compute_units", {})
        if not isinstance(metrics, dict):
            raise Refusal(
                "passed lifecycle run omitted its named compute-unit projection"
            )
        for label, value in metrics.items():
            if (
                not isinstance(label, str)
                or not label
                or not isinstance(value, int)
                or value < 0
            ):
                raise Refusal(
                    "passed lifecycle run carried an invalid named compute-unit value"
                )
            rows.setdefault(label, []).append(value)
    return {
        label: {"pass_count": len(values), "arithmetic_mean": arithmetic_mean(values)}
        for label, values in sorted(rows.items())
    }


def parse(argv: Sequence[str]) -> tuple[Paths, int, str]:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--release-root", required=True)
    parser.add_argument("--validator", required=True)
    parser.add_argument("--solana", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--reuse-bootstrap-work")
    parser.add_argument("--seeds", type=int, default=20)
    parser.add_argument(
        "--through", choices=("participant", "full-probe", "full"), default="full"
    )
    args = parser.parse_args(argv)
    if args.through == "full" and args.seeds != 20:
        raise Refusal("full release evidence requires exactly 20 named seeds")
    if args.through == "full-probe" and args.seeds != 1:
        raise Refusal(
            "the full-lifecycle development probe requires exactly one named seed"
        )
    if args.through in ("full", "full-probe"):
        raise Refusal(
            "full private-validator lifecycle is not accepted in this revision; "
            "missing semantic owners: " + ", ".join(FULL_LIFECYCLE_BLOCKERS)
        )
    if args.through == "participant" and args.seeds != 1:
        raise Refusal(
            "the founding/participant development probe requires exactly one named seed"
        )
    work = Path(args.work)
    if not work.is_absolute() or work.exists() or not work.parent.is_dir():
        raise Refusal(
            "--work must be a fresh absolute directory with an existing parent"
        )
    paths = Paths(
        repo=canonical_directory(args.repo, "repository"),
        release_root=canonical_directory(args.release_root, "checked release root"),
        bootstrap=work / "host-target/release/dclutch-local-successor-bootstrap",
        reuse_bootstrap_work=(
            None
            if args.reuse_bootstrap_work is None
            else canonical_directory(args.reuse_bootstrap_work, "bootstrap source work")
        ),
        validator=canonical_file(args.validator, "solana-test-validator"),
        solana=canonical_file(args.solana, "solana CLI"),
        work=work,
    )
    return paths, args.seeds, args.through


def main(argv: Sequence[str]) -> int:
    paths, seeds, through = parse(argv)
    commit = clean_commit(paths.repo)
    gate_path, gate, gate_digest = checked_gate(paths, commit)
    paths.work.mkdir(mode=0o700)
    (paths.work / "runs").mkdir()
    paths = build_bootstrap(paths, commit, gate_digest)
    help_sha256 = command_surface(paths.bootstrap, through)
    runner_path = canonical_file(Path(__file__), "private-validator lifecycle runner")
    watchdog_path = canonical_file(
        Path(__file__).with_name("watchdog.py"), "private-validator lifecycle watchdog"
    )
    runs: list[dict[str, Any]] = []
    failures: list[dict[str, str]] = []
    for index in range(1, seeds + 1):
        try:
            runs.append(run_one(paths, gate_path, gate, gate_digest, index, through))
        except Exception as error:
            failures.append({"seed": f"seed-{index:02d}", "error": str(error)})
    mean = arithmetic_mean(row["dcltgmf2_compute_units"] for row in runs)
    compute_units = compute_unit_report(runs)
    summary = {
        "schema": (
            SCHEMA
            if through == "full"
            else (
                FULL_PROBE_SCHEMA
                if through == "full-probe"
                else PARTICIPANT_PROBE_SCHEMA
            )
        ),
        "evidence_level": (
            "local-private-validator"
            if through == "full"
            else (
                "local-private-validator-full-lifecycle-development-probe"
                if through == "full-probe"
                else "local-private-validator-founding-participant-probe"
            )
        ),
        "source_revision": commit,
        "checked_release_gate": str(gate_path),
        "checked_release_gate_sha256": gate_digest,
        "bootstrap_sha256": sha256_file(paths.bootstrap),
        "bootstrap_help_sha256": help_sha256,
        "orchestrator_files": {
            "runner": {"path": str(runner_path), "sha256": sha256_file(runner_path)},
            "watchdog": {
                "path": str(watchdog_path),
                "sha256": sha256_file(watchdog_path),
            },
        },
        "named_seed_count": seeds,
        "through": through,
        "pass_count": len(runs),
        "fail_count": len(failures),
        "dcltgmf2_compute_units_arithmetic_mean": mean,
        "compute_unit_report": compute_units,
        "runs": runs,
        "failures": failures,
        "external_writes": False,
        "network": "fresh-loopback-validator-only",
    }
    write_json_new(paths.work / "SUMMARY.json", summary)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0 if len(runs) == seeds else 1


def interrupted(signum: int, _frame: Any) -> None:
    raise Refusal(
        f"interrupted by signal {signum}; contained children are being terminated"
    )


if __name__ == "__main__":
    try:
        signal.signal(signal.SIGINT, interrupted)
        signal.signal(signal.SIGTERM, interrupted)
        raise SystemExit(main(sys.argv[1:]))
    except Refusal as error:
        print(f"private-validator-lifecycle: REFUSED: {error}", file=sys.stderr)
        raise SystemExit(2) from error
