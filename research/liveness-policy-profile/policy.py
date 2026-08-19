#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Verify the sealed R1 liveness evidence and derive its fail-closed profile.

The checker deliberately separates measured bank facts, policy inputs, runtime
reward constants, and terminal admission.  It never clamps an impossible CU
headroom request into a finite lamport quote and never emits a protocol-wide
``LivenessPolicy`` while a mandatory route is stopped.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from admission_math import (
    QuotePolicy,
    exact_unique_labels,
    quote_route,
    require_runtime_schedule_covers_policy,
    resolution_path_quote,
)
from terminal_admission import validate_terminal_admission
from terminal_profile import ACCOUNT_ROWS, EXPECTED_ACCOUNTS, build_terminal


PROFILE_DIR = Path(__file__).resolve().parent
REPO = PROFILE_DIR.parents[1]
EVIDENCE_PATH = PROFILE_DIR / "evidence.json"
SEALED_PROBE_PATHS = {
    "research/liveness-policy-profile/Cargo.toml",
    "research/liveness-policy-profile/Cargo.lock",
    "research/liveness-policy-profile/src/main.rs",
}


class CheckError(RuntimeError):
    """A pinned identity or arithmetic check failed."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run(argv: list[str], *, cwd: Path = REPO, env: dict[str, str] | None = None) -> str:
    result = subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if result.returncode != 0:
        raise CheckError(f"command failed ({result.returncode}): {' '.join(argv)}\n{result.stdout}")
    return result.stdout


def require_equal(actual: Any, expected: Any, label: str) -> None:
    if actual != expected:
        raise CheckError(f"{label}: expected {expected!r}, got {actual!r}")


def load_evidence(path: Path = EVIDENCE_PATH) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as stream:
        evidence = json.load(stream)
    if evidence.get("schema") != "dragons-clutch/liveness-policy-evidence/v2":
        raise CheckError("unexpected evidence schema")
    return evidence


def quote_policy(evidence: dict[str, Any]) -> QuotePolicy:
    row = evidence["policy_inputs"]
    return QuotePolicy(
        headroom_numerator=row["cu_headroom_numerator"],
        headroom_denominator=row["cu_headroom_denominator"],
        rounding_quantum_cu=row["cu_rounding_quantum"],
        transaction_ceiling_cu=row["transaction_cu_ceiling"],
        base_fee_cap_lamports=row["base_transaction_fee_cap_lamports"],
        micro_lamports_per_cu_cap=row["micro_lamports_per_cu_cap"],
        keeper_tip_lamports=row["keeper_surplus_lamports"],
    )


def route_dict(measured_cu: int, policy: QuotePolicy) -> dict[str, Any]:
    route = quote_route(measured_cu, policy)
    return {
        "measured_cu": route.measured_cu,
        "required_headroom_cu": route.required_headroom_cu,
        "selected_limit_cu": route.selected_limit_cu,
        "external_fee_cap_lamports": route.external_fee_cap_lamports,
        "keeper_reward_lamports": route.keeper_reward_lamports,
        "status": route.status,
    }


def derive(evidence: dict[str, Any]) -> dict[str, Any]:
    """Derive the only promoted subsystem quote and protocol STOP."""

    policy = quote_policy(evidence)
    measurements = evidence["measurements"]
    work = measurements["resolution_work"]
    exact_unique_labels(work["fold_widths"], [1, 2, 3, 4], "ResolutionWork Fold")
    exact_unique_labels(
        [row["degree"] for row in measurements["occupation_v4"]["degree_rows"]],
        [1, 2, 3],
        "occupation-v4 degree",
    )
    exact_unique_labels(
        [row["degree"] for row in measurements["native_point_v3"]["degree_rows"]],
        [1, 2, 3],
        "native point-v3 degree",
    )
    exact_unique_labels(
        [row["degree"] for row in measurements["native_bearer_redeem"]["degree_rows"]],
        [1, 2, 3],
        "native bearer degree",
    )

    begin = quote_route(max(work["begin_cu"]), policy)
    folds = {
        width: quote_route(max(work[f"fold_{width}_cu"]), policy)
        for width in range(1, 5)
    }
    finalize = quote_route(max(work["finalize_cu"]), policy)
    abort = quote_route(max(work["abort_cu"]), policy)
    path = resolution_path_quote(
        record_count=evidence["resolution_work"]["maximum_records"],
        begin=begin,
        fold_quotes=folds,
        finalize=finalize,
        abort=abort,
        rent_principal_lamports=(
            evidence["accounts"]["resolution.work.v1"]["rent_lamports"]
            + evidence["accounts"]["resolution.reserve.v1"]["rent_lamports"]
        ),
    )

    runtime_schedule = evidence["resolution_work"]["runtime_reward_schedule"]
    require_runtime_schedule_covers_policy(
        fold_quotes=folds,
        fold_base_reward=runtime_schedule["fold_base_lamports"],
        fold_per_record_reward=runtime_schedule["fold_per_record_lamports"],
        finalize_quote=finalize,
        finalize_reward=runtime_schedule["finalize_lamports"],
        abort_quote=abort,
        abort_reward=runtime_schedule["abort_lamports"],
    )

    direct = measurements["direct_v2"]
    native_point_routes = [
        {"degree": row["degree"], "resolve": route_dict(row["resolve_cu"], policy)}
        for row in measurements["native_point_v3"]["degree_rows"]
    ]
    occupation_routes = [
        {
            "degree": row["degree"],
            "initial": route_dict(row["initial_cu"], policy),
            "retry": route_dict(row["retry_cu"], policy),
        }
        for row in measurements["occupation_v4"]["degree_rows"]
    ]
    terminal = build_terminal(evidence["runtime_ref"])
    terminal_status = validate_terminal_admission(
        terminal,
        expected_accounts=EXPECTED_ACCOUNTS,
    )

    return {
        "status": "MEASURED_RUNTIME_ECONOMIC_ADMISSION_STOP",
        "complete_liveness_policy": "NOT_EMITTED_STOP",
        "maximum_raw_cu_with_requested_headroom": (
            policy.transaction_ceiling_cu
            * policy.headroom_denominator
            // policy.headroom_numerator
        ),
        "resolution_work": {
            "status": path.status,
            "routes": {
                "begin": route_dict(max(work["begin_cu"]), policy),
                **{
                    f"fold_{width}": route_dict(max(work[f"fold_{width}_cu"]), policy)
                    for width in range(1, 5)
                },
                "finalize": route_dict(max(work["finalize_cu"]), policy),
                "abort": route_dict(max(work["abort_cu"]), policy),
            },
            "maximum_records": path.record_count,
            "fold_path_lamports": path.fold_path_lamports,
            "success_rewards_lamports": path.success_rewards_lamports,
            "worst_abort_rewards_lamports": path.worst_abort_rewards_lamports,
            "spendable_reserve_lamports": path.spendable_reserve_lamports,
            "rent_principal_lamports": path.rent_principal_lamports,
            "persistent_reserve_lamports": path.persistent_reserve_lamports,
            "begin_external_lamports": path.begin_external_lamports,
            "payer_cold_outlay_lamports": path.payer_cold_outlay_lamports,
            "runtime_schedule_matches_policy": True,
        },
        "source_value_admission": {
            "status": "FAIL_CLOSED_STOP",
            "default_release_available": False,
            "endow_refusal_code": measurements["source_endow"]["default_refusal_code"],
            "refusal_cu_not_priced_as_success": True,
        },
        "direct_v2": {
            "status": "STOP",
            "select": route_dict(max(direct["select_cu"]), policy),
            "select_result": direct["select_result"],
            "empty_frozen_lapse": direct["empty_frozen_lapse"],
            "live_v3": False,
        },
        "native_point_v3": {
            "status": "PASS" if all(row["resolve"]["status"] == "PASS" for row in native_point_routes) else "STOP",
            "degree_routes": native_point_routes,
        },
        "occupation_v4_monolithic": {
            "status": "STOP_HEADROOM",
            "degree_routes": occupation_routes,
            "live_action": "USE_MEASURED_RESOLUTIONWORK_OR_FAIL_CLOSED",
        },
        "terminal_status": terminal_status,
        "terminal_blocking_ids": terminal["blocking_ids"],
    }


def parse_probe(output: str) -> tuple[dict[str, dict[str, int]], dict[str, int]]:
    accounts: dict[str, dict[str, int]] = {}
    metadata: dict[str, int] = {}
    lines = output.splitlines()
    if not lines or lines[0] != "schema\tdragons-clutch/liveness-account-inventory/v2":
        raise CheckError("historical probe schema mismatch")
    for line in lines[1:]:
        fields = line.split("\t")
        if len(fields) == 2 and fields[1].isdigit():
            metadata[fields[0]] = int(fields[1])
        elif len(fields) == 3 and fields[1].isdigit() and fields[2].isdigit():
            accounts[fields[0]] = {
                "bytes": int(fields[1]),
                "rent_lamports": int(fields[2]),
            }
        else:
            raise CheckError(f"malformed historical probe row: {line!r}")
    return accounts, metadata


def historical_probe(evidence: dict[str, Any]) -> str:
    """Execute only the manifest/lock/source archived at ``runtime_ref``."""

    with tempfile.TemporaryDirectory(prefix="clutch-liveness-profile-") as temp_name:
        temp = Path(temp_name)
        archive = subprocess.Popen(
            ["git", "archive", "--format=tar", evidence["runtime_ref"]],
            cwd=REPO,
            stdout=subprocess.PIPE,
        )
        assert archive.stdout is not None
        extracted = subprocess.run(["tar", "-xf", "-"], cwd=temp, stdin=archive.stdout, check=False)
        archive.stdout.close()
        if archive.wait() != 0 or extracted.returncode != 0:
            raise CheckError("could not materialize historical evidence tree")
        if not all((temp / path).is_file() for path in SEALED_PROBE_PATHS):
            raise CheckError("historical tree does not contain the sealed probe")
        manifest = temp / "research/liveness-policy-profile/Cargo.toml"
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(PROFILE_DIR / "target" / evidence["runtime_ref"])
        return run(
            ["cargo", "run", "--offline", "--locked", "--quiet", "--manifest-path", str(manifest)],
            cwd=temp,
            env=env,
        )


def check_source_identity(evidence: dict[str, Any]) -> None:
    missing = SEALED_PROBE_PATHS - set(evidence["source_blobs"])
    if missing:
        raise CheckError("unsealed historical probe inputs: " + ", ".join(sorted(missing)))
    for path, expected in evidence["source_blobs"].items():
        actual = run(["git", "rev-parse", f"{evidence['runtime_ref']}:{path}"]).strip()
        require_equal(actual, expected, f"source blob {path}")
    for path, expected in evidence["source_trees"].items():
        actual = run(["git", "rev-parse", f"{evidence['runtime_ref']}:{path}"]).strip()
        require_equal(actual, expected, f"source tree {path}")
    for path, expected in evidence["test_blobs"].items():
        actual = run(["git", "rev-parse", f"{evidence['evidence_ref']}:{path}"]).strip()
        require_equal(actual, expected, f"test blob {path}")
    for path, expected in evidence["test_trees"].items():
        actual = run(["git", "rev-parse", f"{evidence['evidence_ref']}:{path}"]).strip()
        require_equal(actual, expected, f"test tree {path}")


def check_files(evidence: dict[str, Any]) -> None:
    artifact = evidence["artifact"]
    artifact_path = REPO / artifact["path"]
    require_equal(artifact_path.stat().st_size, artifact["bytes"], "ELF bytes")
    require_equal(sha256(artifact_path), artifact["sha256"], "ELF sha256")
    for relative, expected in evidence["evidence_files"].items():
        path = REPO / relative
        if not path.is_file():
            raise CheckError(f"evidence file is absent: {relative}")
        require_equal(path.stat().st_size, expected["bytes"], f"{relative} bytes")
        require_equal(sha256(path), expected["sha256"], f"{relative} sha256")


def check_capture(evidence: dict[str, Any]) -> None:
    path = REPO / evidence["capture"]["path"]
    require_equal(sha256(path), evidence["capture"]["sha256"], "capture sha256")
    with path.open("r", encoding="utf-8") as stream:
        capture = json.load(stream)
    require_equal(capture["schema"], "dragons-clutch/liveness-bank-capture/v2", "capture schema")
    require_equal(capture["runtime_ref"], evidence["runtime_ref"], "capture runtime ref")
    require_equal(capture["evidence_ref"], evidence["evidence_ref"], "capture evidence ref")
    require_equal(capture["artifact"], evidence["artifact"], "capture artifact")
    require_equal(capture["measurements"], evidence["measurements"], "capture measurements")


def check_rent_and_accounts(evidence: dict[str, Any]) -> None:
    rent = evidence["rent"]
    effective = (
        rent["lamports_per_byte_year"] * rent["exemption_threshold_numerator"]
    ) // rent["exemption_threshold_denominator"]
    require_equal(effective, rent["effective_lamports_per_byte"], "effective rent rate")
    for name, row in evidence["accounts"].items():
        expected = max(1, (row["bytes"] + rent["account_storage_overhead_bytes"]) * effective)
        require_equal(row["rent_lamports"], expected, f"rent row {name}")
    terminal_rows = {
        name: {"bytes": bytes_, "rent_lamports": lamports}
        for name, bytes_, lamports, *_ in ACCOUNT_ROWS
    }
    require_equal(evidence["accounts"], terminal_rows, "terminal/probe account inventory")
    output = historical_probe(evidence)
    accounts, metadata = parse_probe(output)
    maximum_stage = accounts.pop("artifact.maximum.stage", None)
    require_equal(
        maximum_stage,
        evidence["accounts"]["artifact.terms.stage"],
        "probe maximum stage alias",
    )
    require_equal(accounts, evidence["accounts"], "historical account probe")
    require_equal(metadata["lamports_per_byte"], effective, "probe rent rate")
    require_equal(
        metadata["account_storage_overhead"],
        rent["account_storage_overhead_bytes"],
        "probe storage overhead",
    )


def check(evidence: dict[str, Any]) -> None:
    check_source_identity(evidence)
    check_files(evidence)
    check_capture(evidence)
    check_rent_and_accounts(evidence)
    require_equal(derive(evidence), evidence["projection"], "policy projection")


def check_current(evidence: dict[str, Any]) -> None:
    """Optional strict gate: runtime source closures must match the frozen ref."""

    drift: list[str] = []
    for path in evidence["source_trees"]:
        tracked = subprocess.run(
            ["git", "diff", "--quiet", evidence["runtime_ref"], "--", path],
            cwd=REPO,
            check=False,
        )
        untracked = run(["git", "ls-files", "--others", "--exclude-standard", "--", path]).strip()
        if tracked.returncode != 0 or untracked:
            drift.append(path)
    if drift:
        raise CheckError("working runtime source drift: " + ", ".join(sorted(drift)))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check-current", action="store_true")
    args = parser.parse_args()
    try:
        evidence = load_evidence()
        check(evidence)
        if args.check_current:
            check_current(evidence)
    except (CheckError, OSError, ValueError, json.JSONDecodeError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print("PASS: exact R1 artifact, bank capture, account probe, rewards, and STOPs agree")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
