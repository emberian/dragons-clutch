#!/usr/bin/env python3
"""Exact executable contract for the private lifecycle's 17 chaos cases.

This module does not simulate a protocol transition and it does not bless a
caller-authored checklist.  The lifecycle supervisor supplies one callback
that runs a fresh owned-loopback lifecycle for each :class:`FaultSpec` and
returns facts acquired from the durable journal, the killed process, the
restarted exterior, and the terminal result.  This module owns the finite
matrix and refuses any result that does not prove the crash was at the named
boundary and that recovery reused one exact packet and signature.

There are seventeen cases by construction: one uninterrupted control, then
two interruption boundaries for each of the eight accepted chaos stages.  The
order is protocol evidence.  Adding a stage or a boundary changes the session
schema rather than silently changing what ``--through full`` means.
"""

from __future__ import annotations

import dataclasses
import hashlib
import json
import os
from pathlib import Path
import re
from typing import Any, Callable, Mapping, Sequence


SESSION_SCHEMA_V2 = "dclutch-owned-loopback-private-lifecycle-chaos-session-v2"
CASE_SCHEMA_V1 = "dclutch-owned-loopback-private-lifecycle-chaos-case-v1"
CONTROL_CASE_ID = "control"
CONTROL_STAGE = "control"
CONTROL_BOUNDARY = "uninterrupted"
PRE_SEND_BOUNDARY = "dispatching-before-send"
LOST_RESPONSE_BOUNDARY = "landed-before-finalization-fsync"
EXPECTED_KILL_SIGNAL = 9
EXPECTED_FAULT_EXIT_CODE = -EXPECTED_KILL_SIGNAL
MAX_SESSION_BYTES = 16 * 1024 * 1024

STAGES: tuple[str, ...] = (
    "founding",
    "participant",
    "alt",
    "seal",
    "hot",
    "resolution",
    "payout",
    "retire",
)
BOUNDARIES: tuple[str, ...] = (PRE_SEND_BOUNDARY, LOST_RESPONSE_BOUNDARY)
TARGET_MUTATIONS: Mapping[str, str] = {
    "control": "complete-life",
    "founding": "dcltgmf3",
    "participant": "position-admission",
    "alt": "lookup-freeze",
    "seal": "capability-seal",
    "hot": "hot",
    "resolution": "core-terminal-accept",
    "payout": "wallet-terminal-payout",
    "retire": "aggregate-retirement-finish",
}

_HEX64 = re.compile(r"[0-9a-f]{64}\Z")
_HEX40 = re.compile(r"[0-9a-f]{40}\Z")
_BASE58 = re.compile(r"[1-9A-HJ-NP-Za-km-z]+\Z")


class Refusal(RuntimeError):
    """One fail-closed chaos evidence refusal."""


@dataclasses.dataclass(frozen=True)
class FaultSpec:
    """One exact lifecycle execution requested from the supervisor."""

    case_id: str
    stage: str
    boundary: str

    @property
    def interrupted(self) -> bool:
        """Whether this case must terminate one exterior process."""

        return self.case_id != CONTROL_CASE_ID


def matrix() -> tuple[FaultSpec, ...]:
    """Return the sole admitted 17-case order."""

    rows = [FaultSpec(CONTROL_CASE_ID, CONTROL_STAGE, CONTROL_BOUNDARY)]
    rows.extend(
        FaultSpec(f"{stage}:{boundary}", stage, boundary)
        for stage in STAGES
        for boundary in BOUNDARIES
    )
    result = tuple(rows)
    if len(result) != 17:  # This is a code-owner assertion, not input validation.
        raise AssertionError("the lifecycle chaos matrix is not exactly seventeen cases")
    return result


MATRIX: tuple[FaultSpec, ...] = matrix()


def sha256_bytes(value: bytes) -> str:
    """Lowercase SHA-256 text."""

    return hashlib.sha256(value).hexdigest()


def canonical_json_bytes(value: Any) -> bytes:
    """Canonical JSON used by the session's own digest."""

    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode("ascii")


def _object(value: Any, keys: set[str], label: str) -> Mapping[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        observed = sorted(value) if isinstance(value, dict) else type(value).__name__
        raise Refusal(f"{label} changed its exact fields: {observed}")
    return value


def _text(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value or not value.isascii():
        raise Refusal(f"{label} must be nonempty ASCII text")
    return value


def _sha(value: Any, label: str) -> str:
    value = _text(value, label)
    if _HEX64.fullmatch(value) is None:
        raise Refusal(f"{label} must be one lowercase SHA-256")
    return value


def _git_commit(value: Any, label: str) -> str:
    value = _text(value, label)
    if _HEX40.fullmatch(value) is None:
        raise Refusal(f"{label} must be one full lowercase Git commit")
    return value


def _pubkey(value: Any, label: str) -> str:
    value = _text(value, label)
    if not 32 <= len(value) <= 44 or _BASE58.fullmatch(value) is None:
        raise Refusal(f"{label} must be one canonical-looking base58 identity")
    return value


def _signature(value: Any, label: str) -> str:
    value = _text(value, label)
    if not 64 <= len(value) <= 88 or _BASE58.fullmatch(value) is None:
        raise Refusal(f"{label} must be one canonical-looking base58 signature")
    return value


def _count(value: Any, expected: int, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value != expected:
        raise Refusal(f"{label} must equal {expected}")
    return value


def _positive(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise Refusal(f"{label} must be a positive integer")
    return value


def _final_stages(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or value != list(STAGES):
        raise Refusal(f"{label} must be the exact ordered eight-stage terminal prefix")
    return value


def _case_digest(document: Mapping[str, Any]) -> str:
    copy = dict(document)
    supplied = copy.pop("caseSha256", None)
    _sha(supplied, "chaos case digest")
    return sha256_bytes(canonical_json_bytes(copy))


def authenticate_case(value: Any, spec: FaultSpec) -> dict[str, Any]:
    """Authenticate one control or interrupted run against its requested case."""

    common = {
        "schema",
        "caseId",
        "stage",
        "boundary",
        "targetMutation",
        "status",
        "namedSeed",
        "genesisHash",
        "sessionIdentitySha256",
        "sourceRevision",
        "checkedReleaseGateSha256",
        "terminalResultSha256",
        "completedStages",
        "targetIntentSha256",
        "targetPacketSha256",
        "targetSignature",
        "targetSigningCount",
        "targetDistinctSignatureCount",
        "targetSendCount",
        "fault",
        "recovery",
        "caseSha256",
    }
    row = dict(_object(value, common, f"chaos case {spec.case_id}"))
    if (
        row["schema"] != CASE_SCHEMA_V1
        or row["caseId"] != spec.case_id
        or row["stage"] != spec.stage
        or row["boundary"] != spec.boundary
        or row["targetMutation"] != TARGET_MUTATIONS[spec.stage]
        or row["status"] != "finalized"
    ):
        raise Refusal(f"chaos case {spec.case_id} changed identity or status")
    seed = _text(row["namedSeed"], f"{spec.case_id} named seed")
    if re.fullmatch(r"chaos-[0-9]{2}", seed) is None:
        raise Refusal(f"{spec.case_id} named seed is not chaos-NN")
    _pubkey(row["genesisHash"], f"{spec.case_id} genesis hash")
    _git_commit(row["sourceRevision"], f"{spec.case_id} sourceRevision")
    for field in (
        "sessionIdentitySha256",
        "checkedReleaseGateSha256",
        "terminalResultSha256",
        "targetIntentSha256",
        "targetPacketSha256",
    ):
        _sha(row[field], f"{spec.case_id} {field}")
    _signature(row["targetSignature"], f"{spec.case_id} target signature")
    _final_stages(row["completedStages"], f"{spec.case_id} completed stages")
    _count(row["targetSigningCount"], 1, f"{spec.case_id} target signing count")
    _count(
        row["targetDistinctSignatureCount"],
        1,
        f"{spec.case_id} distinct target signature count",
    )
    _count(row["targetSendCount"], 1, f"{spec.case_id} target send count")

    if not spec.interrupted:
        if row["fault"] is not None or row["recovery"] is not None:
            raise Refusal("the uninterrupted control carried fault or recovery theater")
    else:
        fault = _object(
            row["fault"],
            {
                "receiptSha256",
                "journalBeforeKillSha256",
                "durablePhase",
                "exitCode",
                "signal",
                "sendCountBeforeKill",
                "intentSha256",
                "packetSha256",
                "signature",
            },
            f"{spec.case_id} fault",
        )
        recovery = _object(
            row["recovery"],
            {
                "sameGenesis",
                "sameSessionIdentity",
                "journalBeforeRestartSha256",
                "journalAfterFinalizationSha256",
                "intentSha256",
                "packetSha256",
                "signature",
                "pollCount",
                "sendCountAfterRestart",
                "signingCountAfterRestart",
                "finalizedSlot",
            },
            f"{spec.case_id} recovery",
        )
        for field in (
            "receiptSha256",
            "journalBeforeKillSha256",
            "intentSha256",
            "packetSha256",
        ):
            _sha(fault[field], f"{spec.case_id} fault {field}")
        for field in (
            "journalBeforeRestartSha256",
            "journalAfterFinalizationSha256",
            "intentSha256",
            "packetSha256",
        ):
            _sha(recovery[field], f"{spec.case_id} recovery {field}")
        _signature(fault["signature"], f"{spec.case_id} fault signature")
        _signature(recovery["signature"], f"{spec.case_id} recovery signature")
        if fault["exitCode"] != EXPECTED_FAULT_EXIT_CODE or fault["signal"] != 9:
            raise Refusal(f"{spec.case_id} did not record an actual SIGKILL boundary")
        if fault["durablePhase"] != "dispatching":
            raise Refusal(f"{spec.case_id} kill did not follow a durable Dispatching journal")
        expected_before = 0 if spec.boundary == PRE_SEND_BOUNDARY else 1
        _count(
            fault["sendCountBeforeKill"],
            expected_before,
            f"{spec.case_id} sends before kill",
        )
        expected_after = 1 if spec.boundary == PRE_SEND_BOUNDARY else 0
        _count(
            recovery["sendCountAfterRestart"],
            expected_after,
            f"{spec.case_id} sends after restart",
        )
        _count(
            recovery["signingCountAfterRestart"],
            0,
            f"{spec.case_id} signatures after restart",
        )
        _positive(recovery["pollCount"], f"{spec.case_id} recovery poll count")
        _positive(recovery["finalizedSlot"], f"{spec.case_id} finalized slot")
        if recovery["sameGenesis"] is not True or recovery["sameSessionIdentity"] is not True:
            raise Refusal(f"{spec.case_id} restarted on another validator or session")
        if fault["journalBeforeKillSha256"] != recovery["journalBeforeRestartSha256"]:
            raise Refusal(f"{spec.case_id} changed its durable journal while the process was dead")
        if recovery["journalAfterFinalizationSha256"] == fault["journalBeforeKillSha256"]:
            raise Refusal(f"{spec.case_id} recovery never finalized its durable journal")
        for field in ("intentSha256", "packetSha256", "signature"):
            target = {
                "intentSha256": row["targetIntentSha256"],
                "packetSha256": row["targetPacketSha256"],
                "signature": row["targetSignature"],
            }[field]
            if fault[field] != target or recovery[field] != target:
                raise Refusal(f"{spec.case_id} changed its exact {field} across restart")

    if _case_digest(row) != row["caseSha256"]:
        raise Refusal(f"{spec.case_id} case digest changed")
    return row


def _session_digest(document: Mapping[str, Any]) -> str:
    copy = dict(document)
    supplied = copy.pop("sessionSha256", None)
    _sha(supplied, "chaos session digest")
    return sha256_bytes(canonical_json_bytes(copy))


def build_session(
    *,
    source_revision: str,
    source_tree_sha256: str,
    checked_release_gate_sha256: str,
    cases: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    """Authenticate all results and build the exact no-gap session."""

    if len(cases) != len(MATRIX):
        raise Refusal("chaos session did not contain exactly seventeen cases")
    accepted = []
    for index, (case, spec) in enumerate(zip(cases, MATRIX, strict=True), start=1):
        row = authenticate_case(case, spec)
        if row["namedSeed"] != f"chaos-{index:02d}":
            raise Refusal("chaos cases changed their exact ordered named seeds")
        accepted.append(row)
    _git_commit(source_revision, "chaos source revision")
    _sha(source_tree_sha256, "chaos source tree")
    _sha(checked_release_gate_sha256, "chaos checked release gate")
    for row in accepted:
        if (
            row["sourceRevision"] != source_revision
            or row["checkedReleaseGateSha256"] != checked_release_gate_sha256
        ):
            raise Refusal("chaos case escaped the session's source or checked release")
    result: dict[str, Any] = {
        "schema": SESSION_SCHEMA_V2,
        "status": "finalized",
        "sourceRevision": source_revision,
        "sourceTreeSha256": source_tree_sha256,
        "checkedReleaseGateSha256": checked_release_gate_sha256,
        "matrix": {
            "caseCount": len(MATRIX),
            "stages": list(STAGES),
            "boundaries": list(BOUNDARIES),
            "targetMutations": [TARGET_MUTATIONS[stage] for stage in STAGES],
        },
        "cases": accepted,
        "sessionSha256": "0" * 64,
    }
    result["sessionSha256"] = _session_digest(result)
    return result


def authenticate_session(value: Any) -> dict[str, Any]:
    """Reopen a serialized V2 session without trusting its conclusions."""

    document = dict(
        _object(
            value,
            {
                "schema",
                "status",
                "sourceRevision",
                "sourceTreeSha256",
                "checkedReleaseGateSha256",
                "matrix",
                "cases",
                "sessionSha256",
            },
            "chaos session",
        )
    )
    if document["schema"] != SESSION_SCHEMA_V2 or document["status"] != "finalized":
        raise Refusal("chaos session changed schema or terminal status")
    matrix_value = _object(
        document["matrix"],
        {"caseCount", "stages", "boundaries", "targetMutations"},
        "chaos matrix",
    )
    if (
        matrix_value["caseCount"] != 17
        or matrix_value["stages"] != list(STAGES)
        or matrix_value["boundaries"] != list(BOUNDARIES)
        or matrix_value["targetMutations"]
        != [TARGET_MUTATIONS[stage] for stage in STAGES]
    ):
        raise Refusal("chaos session changed its exact 1 + 8x2 matrix")
    rebuilt = build_session(
        source_revision=document["sourceRevision"],
        source_tree_sha256=document["sourceTreeSha256"],
        checked_release_gate_sha256=document["checkedReleaseGateSha256"],
        cases=document["cases"],
    )
    if rebuilt != document or _session_digest(document) != document["sessionSha256"]:
        raise Refusal("chaos session digest or canonical reconstruction changed")
    return document


CaseExecutor = Callable[[FaultSpec, int], Mapping[str, Any]]


def execute_matrix(
    executor: CaseExecutor,
    *,
    source_revision: str,
    source_tree_sha256: str,
    checked_release_gate_sha256: str,
) -> dict[str, Any]:
    """Run all cases through one supervisor-owned execution callback.

    ``case_index`` is one-based and is the sole source of the ``chaos-NN``
    seed name.  The callback must launch a fresh validator for each case and
    return only after the resumed run reaches the exact terminal result.
    """

    cases = [executor(spec, index) for index, spec in enumerate(MATRIX, start=1)]
    return build_session(
        source_revision=source_revision,
        source_tree_sha256=source_tree_sha256,
        checked_release_gate_sha256=checked_release_gate_sha256,
        cases=cases,
    )


def read_session(path: Path) -> dict[str, Any]:
    """Read one bounded, duplicate-key-free V2 session."""

    if not path.is_absolute() or not path.is_file() or path.is_symlink():
        raise Refusal("chaos session path must be one absolute regular file")
    data = path.read_bytes()
    if not data or len(data) > MAX_SESSION_BYTES:
        raise Refusal("chaos session is outside its bounded byte width")

    def no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise Refusal(f"chaos session repeats JSON key {key}")
            result[key] = value
        return result

    try:
        value = json.loads(data, object_pairs_hook=no_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise Refusal("chaos session is not one exact UTF-8 JSON value") from error
    return authenticate_session(value)


def write_session_new(path: Path, session: Mapping[str, Any]) -> None:
    """Publish one authenticated session without clobber or symlink follow."""

    authenticate_session(session)
    if not path.is_absolute() or not path.parent.is_dir() or path.exists():
        raise Refusal("chaos output must be one new absolute path in an existing directory")
    body = json.dumps(session, indent=2, sort_keys=True).encode("ascii") + b"\n"
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as target:
            target.write(body)
            target.flush()
            os.fsync(target.fileno())
    finally:
        os.close(descriptor)
    directory = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def main(argv: Sequence[str]) -> int:
    """Verify one completed session; execution is owned by run.py."""

    if len(argv) != 2 or argv[0] != "verify":
        raise Refusal("usage: chaos.py verify /ABSOLUTE/CHAOS_SESSION.json")
    session = read_session(Path(argv[1]))
    print(
        json.dumps(
            {
                "schema": session["schema"],
                "status": session["status"],
                "caseCount": session["matrix"]["caseCount"],
                "sessionSha256": session["sessionSha256"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    import sys

    try:
        raise SystemExit(main(sys.argv[1:]))
    except Refusal as error:
        print(f"private-lifecycle-chaos: REFUSED: {error}", file=sys.stderr)
        raise SystemExit(1) from error
