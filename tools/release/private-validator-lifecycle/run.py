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
RUN_SCHEMA = "dclutch-private-validator-lifecycle-run-v1"
PARTICIPANT_PROBE_RUN_SCHEMA = "dclutch-private-validator-participant-probe-run-v1"
SEED_DOMAIN = b"dclutch/private-validator-lifecycle/named-seed/v1\0"
FOUNDING_PARTICIPANT_COMMANDS = (
    "local-mutable-prepare-v1",
    "local-mutable-plan-authenticate-v1",
    "local-private-validator-market-v1",
    "campaign",
    "local-private-validator-user-position-admission-v1",
)
PYTH_PROVISION_COMMAND = "local-private-validator-pyth-vaa-provision-v1"
FULL_LIFECYCLE_COMMAND = "local-private-validator-lifecycle-v1"
FULL_LIFECYCLE_BLOCKERS = (
    "owned-loopback Direct producer",
    "caller-backed wallet payout executor",
    "aggregate lifecycle receipt authenticator",
)
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
ROLE_ORDER = ("registry", "rent", "custody", "resolution", "claims", "trading", "core")
DEVELOPMENT_FEE_BASIS_POINTS = 50
FEE_BASIS_POINTS_DENOMINATOR = 10_000
VALIDATOR_MINT_ROLE = "core-upgrade-authority"
DEVELOPMENT_FEE_RECIPIENT_ROLE = "founding-source-funder"
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


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_file(value: str | Path, label: str) -> Path:
    path = Path(value)
    if not path.is_absolute() or path.is_symlink() or not path.is_file():
        raise Refusal(f"{label} must be one existing absolute non-symlink file: {path}")
    return path.resolve(strict=True)


def canonical_directory(value: str | Path, label: str) -> Path:
    path = Path(value)
    if not path.is_absolute() or path.is_symlink() or not path.is_dir():
        raise Refusal(f"{label} must be one existing absolute non-symlink directory: {path}")
    return path.resolve(strict=True)


def read_unique_json(path: Path, label: str) -> Any:
    def pairs(rows: list[tuple[str, Any]]) -> dict[str, Any]:
        output: dict[str, Any] = {}
        for key, value in rows:
            if key in output:
                raise Refusal(f"{label} duplicated JSON key {key!r}")
            output[key] = value
        return output

    try:
        return json.loads(path.read_text(), object_pairs_hook=pairs)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
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
        raise Refusal("private-validator lifecycle requires one clean source commit; worktree is dirty")
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()


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
        or Path(str(elf.get("canonical_path", ""))).name == BANISHED_RESOLUTION_ELF_BASENAME
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
    gate_path = canonical_file(paths.release_root / "CHECKED_UPGRADE_GATE.json", "checked release gate")
    gate = read_unique_json(gate_path, "checked release gate")
    if gate.get("schema") != "dclutch-checked-upgrade-gate-v1":
        raise Refusal("checked release gate schema is not dclutch-checked-upgrade-gate-v1")
    if gate.get("source_revision") != commit:
        raise Refusal(
            f"checked release gate commit {gate.get('source_revision')} differs from clean source {commit}"
        )
    if gate.get("link_count") != 13 or len(gate.get("links", [])) != 13:
        raise Refusal("checked release gate does not carry the exact thirteen-link closure")
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
            raise Refusal(f"checked release link {link.get('label')} is not below the frame bound")
    tree = gate.get("source_tree_manifest", {})
    tree_path = canonical_file(paths.release_root / tree.get("canonical_path", ""), "source tree manifest")
    if sha256_file(tree_path) != gate.get("source_tree_sha256") or sha256_file(tree_path) != tree.get("sha256"):
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
        raise Refusal(f"successor help failed before validator launch:\n{result.stdout[-4000:]}")
    required = list(FOUNDING_PARTICIPANT_COMMANDS)
    if through == "full":
        required.extend((PYTH_PROVISION_COMMAND, FULL_LIFECYCLE_COMMAND))
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
    source_summary_path = canonical_file(source_work / "SUMMARY.json", "bootstrap source summary")
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
        raise Refusal("reused bootstrap is not bound to this exact source and checked release gate")
    if (
        receipt.get("schema") != "dclutch-private-validator-host-build-receipt-v1"
        or receipt.get("exit_status") != 0
        or not isinstance(receipt.get("rustup_toolchain"), str)
        or not isinstance(receipt.get("rustc"), str)
    ):
        raise Refusal("reused bootstrap lacks a successful pinned-toolchain build receipt")
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
        raise Refusal("successor manifest did not resolve canonically inside the clean source")
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
        target / "release/dclutch-local-successor-bootstrap", "built successor bootstrap"
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
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": list(params)}).encode()
    call = request.Request(url, data=body, headers={"content-type": "application/json"})
    with request.urlopen(call, timeout=5) as response:  # noqa: S310 - URL is constructed loopback
        decoded = json.load(response)
    if decoded.get("error") is not None:
        raise Refusal(f"localhost RPC {method} refused: {decoded['error']}")
    return decoded.get("result")


def wait_ready(url: str, child: subprocess.Popen[bytes], timeout: float = 60.0) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise Refusal(f"validator exited before readiness with status {child.returncode}")
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


def run_stage(run: Path, ordinal: int, label: str, argv: Sequence[str]) -> subprocess.CompletedProcess[bytes]:
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
        [str(solana), "address", "--keypair", str(keypair)], text=True, stderr=subprocess.STDOUT
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
        raise Refusal(f"localhost balance for {address} was not an integer lamport amount")
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
        raise Refusal("fresh validator did not fund its owned development payer identity")

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
    for offset, file_name in enumerate(PYTH_JOURNAL_FILES):
        label = "pyth-" + file_name.removesuffix(".json")
        run_stage(run, first_ordinal + offset, label, argv)
        expected = set(PYTH_JOURNAL_FILES[: offset + 1])
        observed = {path.name for path in journal_dir.glob("*.json")}
        if observed != expected:
            raise Refusal(f"local Pyth action {label} produced a noncanonical journal prefix")
        path = journal_dir / file_name
        journal = read_unique_json(path, f"local Pyth {label} journal")
        if (
            journal.get("schema")
            != "dclutch-owned-loopback-pyth-prerequisite-transaction-v1"
            or journal.get("cluster") != "owned-loopback"
            or journal.get("authorizedMutation") is not True
            or journal.get("phase") != "finalized"
            or not isinstance(journal.get("finalized"), dict)
        ):
            raise Refusal(f"local Pyth action {label} did not preserve finalized evidence")
        signature = journal["finalized"].get("signature")
        compute_units = journal["finalized"].get("computeUnitsConsumed")
        if not isinstance(signature, str) or not signature or not isinstance(compute_units, int):
            raise Refusal(f"local Pyth action {label} omitted signature or compute units")
        journal_rows.append(
            {
                "action": journal.get("intent", {}).get("action"),
                "path": str(path),
                "sha256": sha256_file(path),
                "signature": signature,
                "compute_units_consumed": compute_units,
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
        or set(document) != {"format", "encodedVaa", "updateAccount", "postUpdateBodyBase64"}
    ):
        raise Refusal("local Pyth final summary or exact four-field facts projection changed")
    try:
        post_update_body = base64.b64decode(document["postUpdateBodyBase64"], validate=True)
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


def checked_mutable_slot_floor(plan_path: Path) -> int:
    plan = read_unique_json(plan_path, "checked local mutable plan")
    checked_set = plan.get("checked_local_mutable_set")
    roles = checked_set.get("roles") if isinstance(checked_set, dict) else None
    if not isinstance(roles, list) or len(roles) != len(ROLE_ORDER):
        raise Refusal("checked local mutable plan omitted its exact seven-role slot projection")
    slots = [role.get("deployment_slot") for role in roles if isinstance(role, dict)]
    if len(slots) != len(ROLE_ORDER) or any(not isinstance(slot, int) or slot < 0 for slot in slots):
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


def dcltgmf2_metric(evidence: Path) -> int:
    document = read_unique_json(evidence, "founding evidence")
    transactions = document.get("execution", {}).get("transactions", [])
    rows = [row for row in transactions if "DCLTGMF2" in row.get("label", "")]
    if len(rows) != 1 or not isinstance(rows[0].get("compute_units_consumed"), int):
        raise Refusal("founding evidence does not carry one exact DCLTGMF2 compute measurement")
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
        [str(paths.bootstrap), "local-mutable-plan-authenticate-v1", "--plan", str(plan)],
    )

    ledger = run / "ledger"
    fixture = paths.repo / "fixtures" / "pyth" / "local-upgraded-2026-08-22"
    validator_log = (run / "validator.log").open("wb")
    child: subprocess.Popen[bytes] | None = None
    watchdog: subprocess.Popen[bytes] | None = None
    try:
        mint_key = require_role_key(report, VALIDATOR_MINT_ROLE)
        mint_address = key_address(paths.solana, mint_key)
        child = subprocess.Popen(
            [
                str(paths.validator),
                "--config",
                "/dev/null",
                "--ledger",
                str(ledger),
                "--account-dir",
                report["account_dir"],
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
                "--upgradeable-program",
                "rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp",
                str(fixture / "receiver.so"),
                "none",
                "--upgradeable-program",
                "HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL",
                str(fixture / "router.so"),
                "none",
            ],
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
            raise Refusal("fresh private validator reported the mainnet-beta genesis hash")
        funding_poststate = provision_disposable_funding(run, paths, report, url)
        slot_floor = checked_mutable_slot_floor(plan)
        funding_poststate["finalized_slot"] = wait_finalized_slot(url, child, slot_floor)
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
        participant_key = Path(report["keypairs"]["participant"])
        payer_key = require_role_key(report, VALIDATOR_MINT_ROLE)
        minimum_slot = rpc(url, "getSlot", [{"commitment": "finalized"}])
        if not isinstance(minimum_slot, int) or minimum_slot <= 0:
            raise Refusal("localhost finalized slot after founding was not a positive integer")
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
                key_address(paths.solana, participant_key),
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
                "--execute",
            ],
        )
        participant_report = read_unique_json(participant, "participant admission evidence")
        if (
            participant_report.get("schema")
            != "dclutch-owned-loopback-user-position-admission-execution-v1"
            or participant_report.get("cluster") != "owned-loopback"
            or participant_report.get("phase") != "finalized"
            or not participant_report.get("authorizedMutation")
            or not isinstance(participant_report.get("finalized"), dict)
        ):
            raise Refusal("participant admission did not preserve one finalized loopback report")

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
        pyth_facts, pyth_evidence, next_ordinal = run_pyth_provisioning(
            run, paths, report, url, 8
        )
        lifecycle_session = run / "lifecycle-session.json"
        run_stage(
            run,
            next_ordinal,
            "full-lifecycle",
            [
                str(paths.bootstrap),
                "local-private-validator-lifecycle-v1",
                "--rpc-url",
                url,
                "--plan",
                str(plan),
                "--market-input",
                str(market),
                "--campaign-evidence",
                str(evidence),
                "--participant-evidence",
                str(participant),
                "--direct-evidence",
                str(direct_evidence),
                "--pyth-facts",
                str(pyth_facts),
                "--key-dir",
                str(prepare_work / "keys"),
                "--session",
                str(lifecycle_session),
                "--journal-dir",
                str(run / "journals"),
                "--execute",
            ],
        )
        session = read_unique_json(lifecycle_session, "full lifecycle session")
        required = ["participant", "direct", "resolution", "payout", "retire"]
        if session.get("schema") != "dclutch-local-private-validator-lifecycle-v1" or [
            stage.get("stage") for stage in session.get("stages", [])
        ] != required or not all(stage.get("status") == "finalized" for stage in session["stages"]):
            raise Refusal("full lifecycle session omitted a finalized participant→retire stage")
        result = {
            "schema": RUN_SCHEMA,
            "name": name,
            "seed_sha256": sha256_bytes(bytes.fromhex(seed)),
            "status": "passed",
            "dcltgmf2_compute_units": metric,
            "compute_units": {
                "founding-dcltgmf2": metric,
                **{
                    f"pyth-{label}": value
                    for label, value in pyth_evidence["compute_units"].items()
                },
            },
            "fee_profile": fee_profile,
            "plan": str(plan),
            "founding_evidence": str(evidence),
            "participant_evidence": str(participant),
            "direct_evidence": str(direct_evidence),
            "pyth_provisioning": pyth_evidence,
            "lifecycle_session": str(lifecycle_session),
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
            raise Refusal("passed lifecycle run omitted its named compute-unit projection")
        for label, value in metrics.items():
            if not isinstance(label, str) or not label or not isinstance(value, int) or value < 0:
                raise Refusal("passed lifecycle run carried an invalid named compute-unit value")
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
    parser.add_argument("--through", choices=("participant", "full"), default="full")
    args = parser.parse_args(argv)
    if args.through == "full" and args.seeds != 20:
        raise Refusal("full release evidence requires exactly 20 named seeds")
    if args.through == "full":
        raise Refusal(
            "full private-validator lifecycle is not accepted in this revision; "
            "missing semantic owners: " + ", ".join(FULL_LIFECYCLE_BLOCKERS)
        )
    if args.through == "participant" and args.seeds != 1:
        raise Refusal("the founding/participant development probe requires exactly one named seed")
    work = Path(args.work)
    if not work.is_absolute() or work.exists() or not work.parent.is_dir():
        raise Refusal("--work must be a fresh absolute directory with an existing parent")
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
        "schema": SCHEMA if through == "full" else PARTICIPANT_PROBE_SCHEMA,
        "evidence_level": (
            "local-private-validator"
            if through == "full"
            else "local-private-validator-founding-participant-probe"
        ),
        "source_revision": commit,
        "checked_release_gate": str(gate_path),
        "checked_release_gate_sha256": gate_digest,
        "bootstrap_sha256": sha256_file(paths.bootstrap),
        "bootstrap_help_sha256": help_sha256,
        "orchestrator_files": {
            "runner": {"path": str(runner_path), "sha256": sha256_file(runner_path)},
            "watchdog": {"path": str(watchdog_path), "sha256": sha256_file(watchdog_path)},
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
    raise Refusal(f"interrupted by signal {signum}; contained children are being terminated")


if __name__ == "__main__":
    try:
        signal.signal(signal.SIGINT, interrupted)
        signal.signal(signal.SIGTERM, interrupted)
        raise SystemExit(main(sys.argv[1:]))
    except Refusal as error:
        print(f"private-validator-lifecycle: REFUSED: {error}", file=sys.stderr)
        raise SystemExit(2) from error
