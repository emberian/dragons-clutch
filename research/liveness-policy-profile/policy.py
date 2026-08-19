#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Audit and derive the sealed, non-promoted liveness policy profile.

The default check reproduces account lengths against the historical source tree,
checks the exact ELF and normalized capture digests, parses every captured CU
number, and recomputes every projected integer.  ``--check-current`` is a
strict drift gate for the live working tree.  It is expected to fail after any
ABI/source change until a new ELF and bank campaign are sealed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Iterable


PROFILE_DIR = Path(__file__).resolve().parent
REPO = PROFILE_DIR.parents[1]
EVIDENCE_PATH = PROFILE_DIR / "evidence.json"
MICRO_LAMPORTS_PER_LAMPORT = 1_000_000
CLUTCH_PROGRAM_ID = "pFKLue7yrMjQKyvJcg3yyR6b9MEm6H8nUt7zPc6tDCT"
ARTIFACT_TRANSACTION_OVERHEAD_CU = 36


class CheckError(RuntimeError):
    """A sealed evidence or derivation check failed."""


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


def load_evidence() -> dict[str, Any]:
    with EVIDENCE_PATH.open("r", encoding="utf-8") as stream:
        evidence = json.load(stream)
    if evidence.get("schema") != "dragons-clutch/liveness-policy-evidence/v1":
        raise CheckError("unexpected evidence schema")
    return evidence


def ceil_div(value: int, denominator: int) -> int:
    if value < 0 or denominator <= 0:
        raise ValueError("ceil_div requires value >= 0 and denominator > 0")
    return (value + denominator - 1) // denominator


def round_up(value: int, quantum: int) -> int:
    return ceil_div(value, quantum) * quantum


def cu_envelope(measured_cu: int, inputs: dict[str, Any]) -> tuple[int, bool]:
    required = ceil_div(
        measured_cu * inputs["cu_headroom_numerator"],
        inputs["cu_headroom_denominator"],
    )
    rounded = round_up(required, inputs["cu_rounding_quantum"])
    envelope = min(rounded, inputs["transaction_cu_ceiling"])
    return envelope, envelope >= required


def work_lamports_for_envelope(envelope_cu: int, inputs: dict[str, Any]) -> int:
    priority = ceil_div(
        envelope_cu * inputs["micro_lamports_per_cu_cap"],
        MICRO_LAMPORTS_PER_LAMPORT,
    )
    return (
        inputs["base_transaction_fee_cap_lamports"]
        + priority
        + inputs["keeper_tip_lamports"]
    )


def measured_work(measured_cu: int, inputs: dict[str, Any]) -> tuple[int, bool]:
    envelope, passes = cu_envelope(measured_cu, inputs)
    return work_lamports_for_envelope(envelope, inputs), passes


def sequence_work(samples: Iterable[int], inputs: dict[str, Any]) -> int:
    return sum(measured_work(sample, inputs)[0] for sample in samples)


def derive(evidence: dict[str, Any]) -> dict[str, Any]:
    accounts = evidence["accounts"]
    measurements = evidence["measurements"]
    inputs = evidence["policy_inputs"]

    artifact_work = {
        kind: sequence_work(
            measurements[f"artifact.{kind}"]["successful_transactions_cu"], inputs
        )
        for kind in ("policy", "grid", "terms")
    }
    artifact_work["total"] = sum(artifact_work.values())

    final_rent = sum(
        accounts[name]["rent_lamports"]
        for name in ("artifact.policy.final", "artifact.grid.final", "artifact.terms.final")
    )
    stage_rent = sum(
        accounts[name]["rent_lamports"]
        for name in ("artifact.policy.stage", "artifact.grid.stage", "artifact.terms.stage")
    )
    realm_profile_rent = sum(
        accounts[name]["rent_lamports"] for name in ("realm", "profile")
    )
    create_names = (
        "market",
        "hoard",
        "position",
        "kernel",
        "replay",
        "supply_ledger",
        "resolution.v3",
        "token.hoard_immutable_owner",
    )
    create_rent = sum(accounts[name]["rent_lamports"] for name in create_names)
    create_rent += (
        inputs["market_outcome_mints"] * accounts["token.outcome_mint"]["rent_lamports"]
    )

    ceiling_work = work_lamports_for_envelope(inputs["transaction_cu_ceiling"], inputs)
    market_work = artifact_work["total"] + (
        inputs["founding_ceiling_transactions"] * ceiling_work
    )
    market_storage = final_rent + stage_rent + realm_profile_rent + create_rent

    resolve_sample = max(
        measurements["resolution.native.resolve"]["samples_cu"]
        + measurements["native_full_lifecycle"]["resolve_cu"]
    )
    resolution_work, resolution_passes = measured_work(resolve_sample, inputs)

    cancel_sample = max(measurements["order.cancel"]["samples_cu"])
    cancel_work, _ = measured_work(cancel_sample, inputs)
    submit = measurements["candidate.submit_direct_page"]
    submit_work, submit_passes = measured_work(submit["sample_cu"], inputs)
    clear_work = max(cancel_work, ceil_div(submit_work, submit["orders_in_page"]))

    settle = measurements["settlement.narrow_page"]
    settle_work, _ = measured_work(settle["sample_cu"], inputs)
    per_order_settle = ceil_div(settle_work, settle["orders_in_page"])

    policy = {
        "market_work_max_lamports": market_work,
        "market_storage_max_lamports": market_storage,
        "resolution_max_lamports": resolution_work,
        "per_order_clear_max_lamports": clear_work,
        "per_order_settle_max_lamports": per_order_settle,
        "market_total_lamports": market_work + market_storage + resolution_work,
        "order_total_lamports": clear_work + per_order_settle,
    }
    return {
        "artifact_work_lamports": artifact_work,
        "market_create_state_rent_lamports": create_rent,
        "artifact_final_rent_lamports": final_rent,
        "artifact_stage_rent_lamports": stage_rent,
        "realm_profile_rent_lamports": realm_profile_rent,
        "liveness_policy": policy,
        "headroom_gate": {
            "candidate.submit_direct_page": submit_passes,
            "resolution.native.resolve": resolution_passes,
        },
        "shared_feed_pair": "INCOMPLETE_UNMEASURED",
        "per_order_storage": "UNREPRESENTED",
        "neutral_sink": "UNSELECTED",
    }


def require_equal(actual: Any, expected: Any, label: str) -> None:
    if actual != expected:
        raise CheckError(f"{label}: expected {expected!r}, got {actual!r}")


def numbers_after(text: str, label: str) -> list[int]:
    match = re.search(rf"{re.escape(label)}: ([0-9 ]+)", text)
    if match is None:
        raise CheckError(f"capture is missing {label!r}")
    return [int(value) for value in match.group(1).split()]


def capture_measurements(text: str) -> dict[str, Any]:
    terms_all = numbers_after(text, "terms transaction CU sequence")
    successful_terms_indices = (0, 1, 2, 3, 6, 7, 8, 9, 10, 11, 12)

    def pair(pattern: str) -> list[int]:
        match = re.search(pattern, text)
        if match is None:
            raise CheckError(f"capture is missing pattern {pattern!r}")
        return [int(match.group(1)), int(match.group(2))]

    market_rows = re.findall(
        r"degree ([01]); resolution bytes ([0-9]+); CreateMarket transaction CU sample ([0-9]+)",
        text,
    )
    if len(market_rows) != 2:
        raise CheckError("capture must contain exactly two CreateMarket samples")

    native = re.findall(
        r"d([123]) resolve=([0-9]+) retry=([0-9]+) redeem_internal=([0-9]+)", text
    )
    external = re.findall(r"d([123]) redeem_external_exact=([0-9]+)", text)
    if len(native) != 3 or len(external) != 3:
        raise CheckError("capture must contain native degree 1 through 3 rows")
    full = re.findall(
        r"d([123]) create=([0-9]+) endow=([0-9]+) split=([0-9]+) "
        r"materialize=([0-9]+) resolve=([0-9]+) redeem_external=([0-9]+) "
        r"redeem_internal=([0-9]+) withdraw=([0-9]+)",
        text,
    )
    if len(full) != 3:
        raise CheckError("capture must contain joined native lifecycle degree 1 through 3 rows")

    def single(pattern: str) -> int:
        match = re.search(pattern, text)
        if match is None:
            raise CheckError(f"capture is missing pattern {pattern!r}")
        return int(match.group(1))

    return {
        "artifact.policy": {
            "successful_transactions_cu": numbers_after(text, "policy transaction CU sequence"),
            "attempted_transactions_cu": numbers_after(text, "policy transaction CU sequence"),
        },
        "artifact.grid": {
            "successful_transactions_cu": numbers_after(text, "grid transaction CU sequence"),
            "attempted_transactions_cu": numbers_after(text, "grid transaction CU sequence"),
        },
        "artifact.terms": {
            "successful_transactions_cu": [terms_all[index] for index in successful_terms_indices],
            "attempted_transactions_cu": terms_all,
        },
        "market.create.v2": {
            "sample_cu": int(market_rows[0][2]),
            "account_bytes": int(market_rows[0][1]),
            "maximum_claim": "UNMEASURED",
        },
        "market.create.v3": {
            "sample_cu": int(market_rows[1][2]),
            "account_bytes": int(market_rows[1][1]),
            "maximum_claim": "UNMEASURED",
        },
        "order.place": {
            "samples_cu": pair(r"PlaceOrder transaction CU: buy=([0-9]+) sell=([0-9]+)")
        },
        "order.cancel": {
            "samples_cu": pair(r"CancelOrder transaction CU: buy=([0-9]+) sell=([0-9]+)")
        },
        "candidate.submit_direct_page": {
            "sample_cu": single(r"SubmitDirectPage prefunded transaction CU: ([0-9]+)"),
            "orders_in_page": 2,
        },
        "settlement.narrow_page": {
            "sample_cu": single(r"SettlePage direct full-slice transaction CU: ([0-9]+)"),
            "orders_in_page": 2,
        },
        "resolution.native.resolve": {"samples_cu": [int(row[1]) for row in native]},
        "resolution.native.retry": {"samples_cu": [int(row[2]) for row in native]},
        "redemption.native.internal": {"samples_cu": [int(row[3]) for row in native]},
        "redemption.native.external": {"samples_cu": [int(row[1]) for row in external]},
        "withdraw_cash": {
            "sample_cu": single(r"WithdrawCash transaction CU: ([0-9]+)")
        },
        "native_full_lifecycle": {
            "create_market_cu": [int(row[1]) for row in full],
            "endow_cu": [int(row[2]) for row in full],
            "split_cu": [int(row[3]) for row in full],
            "materialize_cu": [int(row[4]) for row in full],
            "resolve_cu": [int(row[5]) for row in full],
            "redeem_external_cu": [int(row[6]) for row in full],
            "redeem_internal_cu": [int(row[7]) for row in full],
            "withdraw_cu": [int(row[8]) for row in full],
            "genesis_assisted_source_archive": True,
        },
        "source_archive": {
            "host_tests_passed": single(r"host tests: ([0-9]+) passed"),
            "host_tests_failed": single(r"host tests: [0-9]+ passed; ([0-9]+) failed"),
            "standalone_create_append_seal_cu": "UNMEASURED",
            "resolve_authentication_cost": "INCLUDED_IN_NATIVE_RESOLVE_SAMPLES",
        },
    }


def parse_probe(output: str) -> tuple[dict[str, Any], dict[str, int]]:
    accounts: dict[str, Any] = {}
    candidate: dict[str, int] = {}
    for line in output.splitlines():
        fields = line.split("\t")
        if len(fields) == 3 and fields[1].isdigit() and fields[2].isdigit():
            accounts[fields[0]] = {
                "bytes": int(fields[1]),
                "rent_lamports": int(fields[2]),
            }
        elif len(fields) == 2 and fields[0].startswith("candidate.") and fields[1].isdigit():
            candidate[fields[0]] = int(fields[1])
    return accounts, candidate


def historical_probe(evidence: dict[str, Any]) -> str:
    with tempfile.TemporaryDirectory(prefix="clutch-liveness-profile-") as temp_name:
        temp = Path(temp_name)
        archive = subprocess.Popen(
            ["git", "archive", "--format=tar", evidence["evidence_ref"]],
            cwd=REPO,
            stdout=subprocess.PIPE,
        )
        assert archive.stdout is not None
        extract = subprocess.run(
            ["tar", "-xf", "-"], cwd=temp, stdin=archive.stdout, check=False
        )
        archive.stdout.close()
        archive_return = archive.wait()
        if archive_return != 0 or extract.returncode != 0:
            raise CheckError("could not materialize historical evidence tree")

        destination = temp / "research" / "liveness-policy-profile"
        (destination / "src").mkdir(parents=True)
        for name in ("Cargo.toml", "Cargo.lock"):
            shutil.copy2(PROFILE_DIR / name, destination / name)
        shutil.copy2(PROFILE_DIR / "src" / "main.rs", destination / "src" / "main.rs")
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(PROFILE_DIR / "target" / evidence["evidence_ref"])
        return run(
            ["cargo", "run", "--offline", "--locked", "--quiet", "--manifest-path", str(destination / "Cargo.toml")],
            cwd=temp,
            env=env,
        )


def check_source_blobs(evidence: dict[str, Any]) -> None:
    for path, expected in evidence["source_blobs"].items():
        actual = run(["git", "rev-parse", f"{evidence['evidence_ref']}:{path}"]).strip()
        require_equal(actual, expected, f"historical source blob {path}")


def check_rent(evidence: dict[str, Any]) -> None:
    rent = evidence["rent"]
    effective = (
        rent["lamports_per_byte_year"] * rent["exemption_threshold_numerator"]
    ) // rent["exemption_threshold_denominator"]
    require_equal(effective, rent["effective_lamports_per_byte"], "effective rent rate")
    for name, row in evidence["accounts"].items():
        expected = max(
            1,
            (row["bytes"] + rent["account_storage_overhead_bytes"]) * effective,
        )
        require_equal(row["rent_lamports"], expected, f"rent row {name}")


def check_capture(evidence: dict[str, Any]) -> None:
    capture_path = REPO / evidence["capture"]["path"]
    require_equal(sha256(capture_path), evidence["capture"]["sha256"], "capture sha256")
    text = capture_path.read_text(encoding="utf-8")
    require_equal(
        single_from_text(text, r"ELF sha256: ([0-9a-f]{64})"),
        evidence["artifact"]["sha256"],
        "captured ELF sha256",
    )
    require_equal(
        int(single_from_text(text, r"ELF bytes: ([0-9]+)")),
        evidence["artifact"]["bytes"],
        "captured ELF bytes",
    )
    require_equal(capture_measurements(text), evidence["measurements"], "captured measurements")


def single_from_text(text: str, pattern: str) -> str:
    match = re.search(pattern, text)
    if match is None:
        raise CheckError(f"capture is missing pattern {pattern!r}")
    return match.group(1)


def check_artifact(evidence: dict[str, Any]) -> None:
    path = REPO / evidence["artifact"]["path"]
    if not path.is_file():
        raise CheckError(f"sealed ELF is absent: {path}")
    require_equal(path.stat().st_size, evidence["artifact"]["bytes"], "ELF bytes")
    require_equal(sha256(path), evidence["artifact"]["sha256"], "ELF sha256")
    for relative, expected in evidence["build_evidence"].items():
        log_path = REPO / relative
        if not log_path.is_file():
            raise CheckError(f"build evidence is absent: {log_path}")
        require_equal(log_path.stat().st_size, expected["bytes"], f"{relative} bytes")
        require_equal(sha256(log_path), expected["sha256"], f"{relative} sha256")


def check_historical_probe(evidence: dict[str, Any]) -> None:
    output = historical_probe(evidence)
    accounts, candidate = parse_probe(output)
    require_equal(accounts, evidence["accounts"], "historical account probe")
    expected_policy = evidence["projection"]["liveness_policy"]
    expected_candidate = {
        "candidate.market.work_lamports": expected_policy["market_work_max_lamports"],
        "candidate.market.storage_lamports": expected_policy["market_storage_max_lamports"],
        "candidate.market.resolution_lamports": expected_policy["resolution_max_lamports"],
        "candidate.market.total_lamports": expected_policy["market_total_lamports"],
        "candidate.order.clear_lamports": expected_policy["per_order_clear_max_lamports"],
        "candidate.order.settle_lamports": expected_policy["per_order_settle_max_lamports"],
        "candidate.order.total_lamports": expected_policy["order_total_lamports"],
    }
    require_equal(candidate, expected_candidate, "clutch-liveness kernel projection")


def check_current(evidence: dict[str, Any]) -> None:
    drift: list[str] = []
    for path, expected in evidence["source_blobs"].items():
        current_path = REPO / path
        if not current_path.is_file():
            drift.append(f"missing {path}")
            continue
        actual = run(["git", "hash-object", path]).strip()
        if actual != expected:
            drift.append(f"{path}: expected {expected}, got {actual}")
    if drift:
        raise CheckError("working source has drifted from the measured tree:\n" + "\n".join(drift))
    output = run(
        ["cargo", "run", "--offline", "--locked", "--quiet", "--manifest-path", str(PROFILE_DIR / "Cargo.toml")]
    )
    accounts, _ = parse_probe(output)
    require_equal(accounts, evidence["accounts"], "current account probe")


def transaction_units_from_program_logs(output: str) -> list[int]:
    """Recover artifact transaction metadata from its one-instruction logs.

    The artifact tests submit one Clutch instruction and no compute-budget
    instruction. Their captured bank metadata is exactly the top-level program
    consumption plus the pinned runtime's 36-CU transaction overhead. The
    equality is itself replay-checked; it must be revised rather than assumed
    for a different runtime/test shape.
    """

    consumed = [
        int(value)
        for value in re.findall(
            rf"Program {CLUTCH_PROGRAM_ID} consumed ([0-9]+) of [0-9]+ compute units",
            output,
        )
    ]
    if not consumed:
        raise CheckError("artifact replay emitted no Clutch program CU logs")
    return [value + ARTIFACT_TRANSACTION_OVERHEAD_CU for value in consumed]


def replay_pair(output: str, pattern: str) -> list[int]:
    match = re.search(pattern, output)
    if match is None:
        raise CheckError(f"replay is missing pattern {pattern!r}")
    return [int(match.group(1)), int(match.group(2))]


def replay_single(output: str, pattern: str) -> int:
    match = re.search(pattern, output)
    if match is None:
        raise CheckError(f"replay is missing pattern {pattern!r}")
    return int(match.group(1))


def replay(evidence: dict[str, Any]) -> None:
    check_artifact(evidence)
    with tempfile.TemporaryDirectory(prefix="clutch-liveness-replay-") as temp_name:
        temp = Path(temp_name)
        archive = subprocess.Popen(
            ["git", "archive", "--format=tar", evidence["evidence_ref"]],
            cwd=REPO,
            stdout=subprocess.PIPE,
        )
        assert archive.stdout is not None
        extracted = subprocess.run(["tar", "-xf", "-"], cwd=temp, stdin=archive.stdout, check=False)
        archive.stdout.close()
        if archive.wait() != 0 or extracted.returncode != 0:
            raise CheckError("could not materialize historical replay tree")
        env = os.environ.copy()
        env["SBF_OUT_DIR"] = str((REPO / evidence["artifact"]["path"]).parent)
        env["CARGO_TARGET_DIR"] = str(
            PROFILE_DIR / "target" / f"replay-{evidence['evidence_ref'][:8]}"
        )
        manifest = temp / "programs" / "clutch-sbf" / "svm-tests" / "Cargo.toml"
        def one(test_name: str, test_filter: str = "") -> str:
            argv = [
                "cargo", "test", "--offline", "--locked", "--manifest-path", str(manifest),
                "--test", test_name,
            ]
            if test_filter:
                argv.append(test_filter)
            argv.extend(["--", "--nocapture"])
            print(f"replay {test_name}{'::' + test_filter if test_filter else ''}", flush=True)
            return run(argv, cwd=temp, env=env)

        artifact_output = one(
            "artifact_transport", "every_admitted_artifact_kind_lands_as_its_exact_raw_codec"
        )
        artifact_units = transaction_units_from_program_logs(artifact_output)
        require_equal(
            artifact_units[:4],
            evidence["measurements"]["artifact.policy"]["attempted_transactions_cu"],
            "replayed policy artifact CU",
        )
        require_equal(
            artifact_units[4:],
            evidence["measurements"]["artifact.grid"]["attempted_transactions_cu"],
            "replayed grid artifact CU",
        )

        terms_output = one(
            "artifact_transport", "terms_upload_resumes_after_bank_rehydration_and_seals_atomically"
        )
        require_equal(
            transaction_units_from_program_logs(terms_output),
            evidence["measurements"]["artifact.terms"]["attempted_transactions_cu"],
            "replayed Terms artifact CU",
        )

        # CreateMarket CU is payer/PDA-iteration dependent, so replay checks
        # success and the exact v2/v3 account widths but never equates a new
        # sample with the historical observation.
        blank_output = one(
            "blank_bank_lifecycle",
            "categorical_and_native_markets_construct_from_only_sealed_artifacts",
        )
        widths = {
            int(degree): int(width)
            for degree, width in re.findall(
                r"blank-bank degree=([01]) resolution_bytes=([0-9]+)", blank_output
            )
        }
        require_equal(widths, {0: 165, 1: 319}, "replayed blank-bank ABI widths")

        order_output = one(
            "order_reservation", "funded_orders_reserve_release_and_isolate_owners"
        )
        require_equal(
            replay_pair(order_output, r"PlaceOrder CU: buy=([0-9]+), sell=([0-9]+)"),
            evidence["measurements"]["order.place"]["samples_cu"],
            "replayed PlaceOrder CU",
        )
        require_equal(
            replay_pair(order_output, r"CancelOrder CU: buy=([0-9]+), sell=([0-9]+)"),
            evidence["measurements"]["order.cancel"]["samples_cu"],
            "replayed CancelOrder CU",
        )

        submit_output = one(
            "coupled_authority", "prefunded_submission_is_exact_once_and_leaves_authority_frozen"
        )
        require_equal(
            replay_single(submit_output, r"SubmitDirectPage prefunded CU: ([0-9]+)"),
            evidence["measurements"]["candidate.submit_direct_page"]["sample_cu"],
            "replayed SubmitDirectPage CU",
        )
        settle_output = one(
            "coupled_settlement", "direct_full_slice_settles_once_and_substitution_rolls_back"
        )
        require_equal(
            replay_single(settle_output, r"SettlePage direct full-slice CU: ([0-9]+)"),
            evidence["measurements"]["settlement.narrow_page"]["sample_cu"],
            "replayed SettlePage CU",
        )
        withdraw_output = one(
            "collateral_leg", "withdraw_pays_only_unreserved_cash_and_preserves_locked_backing"
        )
        require_equal(
            replay_single(withdraw_output, r"SVM WithdrawCash: paid [0-9]+ unreserved atoms, ([0-9]+) CU"),
            evidence["measurements"]["withdraw_cash"]["sample_cu"],
            "replayed WithdrawCash CU",
        )

        native_output = one("native_resolution")
        native_rows = sorted(
            (
                int(degree), int(resolve), int(retry), int(redeem)
            )
            for degree, resolve, retry, redeem in re.findall(
                r"native d([123]): resolve ([0-9]+) CU, retry ([0-9]+) CU, redeem ([0-9]+) CU",
                native_output,
            )
        )
        external_rows = sorted(
            (int(degree), int(units))
            for degree, units in re.findall(
                r"native d([123]) external exact lot [0-9]+: ([0-9]+) CU", native_output
            )
        )
        require_equal(
            [row[1] for row in native_rows],
            evidence["measurements"]["resolution.native.resolve"]["samples_cu"],
            "replayed native Resolve CU",
        )
        require_equal(
            [row[2] for row in native_rows],
            evidence["measurements"]["resolution.native.retry"]["samples_cu"],
            "replayed native retry CU",
        )
        require_equal(
            [row[3] for row in native_rows],
            evidence["measurements"]["redemption.native.internal"]["samples_cu"],
            "replayed native internal redemption CU",
        )
        require_equal(
            [row[1] for row in external_rows],
            evidence["measurements"]["redemption.native.external"]["samples_cu"],
            "replayed native external redemption CU",
        )
        full_output = one("native_full_lifecycle")
        full_rows = sorted(
            (
                int(degree),
                int(create),
                int(endow),
                int(split),
                int(materialize),
                int(resolve),
                int(external_redeem),
                int(internal_redeem),
                int(withdraw),
            )
            for (
                degree,
                create,
                endow,
                split,
                materialize,
                resolve,
                external_redeem,
                internal_redeem,
                withdraw,
            ) in re.findall(
                r"native full d([123]): CreateMarket ([0-9]+) CU.*?"
                r"native full d\1 point=[0-9]+: Endow ([0-9]+), Split ([0-9]+), "
                r"Materialize ([0-9]+), Resolve ([0-9]+), ExternalRedeem ([0-9]+), "
                r"InternalRedeem \[([0-9]+), [0-9]+, [0-9]+, [0-9]+\], "
                r"Withdraw ([0-9]+) CU",
                full_output,
                flags=re.DOTALL,
            )
        )
        expected_full = evidence["measurements"]["native_full_lifecycle"]
        full_keys = (
            "create_market_cu",
            "endow_cu",
            "split_cu",
            "materialize_cu",
            "resolve_cu",
            "redeem_external_cu",
            "redeem_internal_cu",
            "withdraw_cu",
        )
        for column, key in enumerate(full_keys, start=1):
            require_equal(
                [row[column] for row in full_rows],
                expected_full[key],
                f"replayed joined native {key}",
            )
        one("source_archive")


def check(evidence: dict[str, Any]) -> None:
    check_source_blobs(evidence)
    check_artifact(evidence)
    check_capture(evidence)
    check_rent(evidence)
    require_equal(derive(evidence), evidence["projection"], "policy projection")
    check_historical_probe(evidence)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check-current",
        action="store_true",
        help="also require the working source tree and account probe to match the measured tree",
    )
    parser.add_argument(
        "--replay",
        action="store_true",
        help="rerun the pinned historical bank tests against the exact local ELF",
    )
    args = parser.parse_args()
    try:
        evidence = load_evidence()
        check(evidence)
        if args.check_current:
            check_current(evidence)
        if args.replay:
            replay(evidence)
    except (CheckError, OSError, ValueError, json.JSONDecodeError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print("PASS: pinned liveness evidence and arithmetic candidate are internally consistent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
