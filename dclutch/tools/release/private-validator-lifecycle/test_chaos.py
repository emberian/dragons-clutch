#!/usr/bin/env python3
"""Adversarial tests for the exact private lifecycle chaos matrix."""

from __future__ import annotations

import copy
import functools
import hashlib
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("chaos.py")
SPEC = importlib.util.spec_from_file_location("dclutch_private_chaos", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
CHAOS = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHAOS
SPEC.loader.exec_module(CHAOS)


# ---------------------------------------------------------------------------
# A SECOND READER OF THE RUST, DELIBERATELY NOT THE ONE THE CONTRACT USES.
#
# `chaos.py` now derives its two schema strings through
# `tools/lib/rust_schema.py`. A test that checked them by calling the same
# function would be asking the reader whether it agrees with itself -- the
# circle `2f2c22246` found in `tools/devnet-reconcile`, where fifty-five green
# tests sat over a reader that refused every real artifact because its fixtures
# were built out of the same constants.
#
# So these read `private_lifecycle.rs` by splitting lines, not by matching the
# shape `rust_schema_constant` matches. They can disagree with it. They also
# reach things it cannot: the three `[&str; N]` vocabularies the Rust checks the
# matrix against, and the serde field sets of the five `deny_unknown_fields`
# structs it deserializes the document into -- which is what makes "an exact key
# set" a claim about the Rust rather than about this file.
RUST_OWNER = (
    Path(__file__).resolve().parents[3]
    / "tools/local-validator/bootstrap/successor/src/private_lifecycle.rs"
)


@functools.lru_cache(maxsize=1)
def rust_lines() -> tuple[str, ...]:
    return tuple(RUST_OWNER.read_text(encoding="utf-8").splitlines())


def rust_str_const(name: str) -> str:
    head = f"const {name}: &str = "
    found = [row.strip() for row in rust_lines() if row.strip().startswith(head)]
    assert len(found) == 1, f"{name} is declared {len(found)} times"
    tail = found[0][len(head) :]
    assert tail.startswith('"') and tail.endswith('";'), tail
    return tail[1:-2]


def rust_str_array(name: str) -> list[str]:
    lines = rust_lines()
    head = f"const {name}: [&str; "
    starts = [index for index, row in enumerate(lines) if row.startswith(head)]
    assert len(starts) == 1, f"{name} is declared {len(starts)} times"
    declared = int(lines[starts[0]][len(head) :].split("]")[0])
    rows = []
    for row in lines[starts[0] + 1 :]:
        stripped = row.strip()
        if stripped == "];":
            break
        assert stripped.startswith('"') and stripped.endswith('",'), stripped
        rows.append(stripped[1:-2])
    assert len(rows) == declared, f"{name} states {declared} and lists {len(rows)}"
    return rows


def camel(field: str) -> str:
    head, *tail = field.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in tail)


def rust_exact_fields(name: str) -> set[str]:
    """The camelCase keys one `deny_unknown_fields` struct accepts, and only those."""

    lines = rust_lines()
    starts = [
        index for index, row in enumerate(lines) if row == f"struct {name} {{"
    ]
    assert len(starts) == 1, f"{name} is declared {len(starts)} times"
    attribute = lines[starts[0] - 1]
    assert 'rename_all = "camelCase"' in attribute and "deny_unknown_fields" in attribute, (
        f"{name} is no longer an exact camelCase document: {attribute}"
    )
    fields = set()
    for row in lines[starts[0] + 1 :]:
        stripped = row.strip()
        if stripped == "}":
            break
        assert stripped.endswith(","), stripped
        fields.add(camel(stripped.split(":")[0].strip()))
    assert fields, f"{name} has no fields"
    return fields


def previous_version(schema: str) -> str:
    """The `-vN` one below a Rust-declared schema string.

    Computed rather than spelled, and that is the whole reason it is a function:
    a test that writes `...-v1` next to an owner that says `-v2` goes red the day
    the owner says `-v3`, at a case whose subject is refusal and not versions.
    The preflight has already paid that bill once, in COHORT-15F/15G, where one
    stale copy read as fifteen failures about other contracts.
    """

    head, marker, version = schema.rpartition("-v")
    assert head and marker and version.isdigit(), schema
    return f"{head}-v{int(version) - 1}"


def rust_identity_complaints(document: dict) -> list[str]:
    """Every conjunct `authenticate_chaos` decides from a Rust DECLARATION.

    Not a reimplementation of that function: its index arithmetic, hex widths and
    one-send algebra are logic, and restating logic in a harness produces a
    second interpreter to disagree with. What is restated here is only what the
    Rust file DECLARES -- two schema strings, three vocabularies, five exact
    field sets -- which is exactly the surface a Python writer can drift on
    without anything going red.
    """

    complaints = []
    if document.get("schema") != rust_str_const("CHAOS_SESSION_SCHEMA_V2"):
        complaints.append("session schema is not the Rust-declared session schema")
    if set(document) != rust_exact_fields("ChaosSession"):
        complaints.append("session keys are not the exact ChaosSession field set")
    matrix = document.get("matrix", {})
    if set(matrix) != rust_exact_fields("ChaosMatrix"):
        complaints.append("matrix keys are not the exact ChaosMatrix field set")
    for key, owner in (
        ("stages", "CHAOS_STAGES"),
        ("boundaries", "CHAOS_BOUNDARIES"),
        ("targetMutations", "CHAOS_TARGET_MUTATIONS"),
    ):
        if matrix.get(key) != rust_str_array(owner):
            complaints.append(f"matrix {key} is not the Rust-declared {owner}")
    for index, row in enumerate(document.get("cases", [])):
        if row.get("schema") != rust_str_const("CHAOS_CASE_SCHEMA_V1"):
            complaints.append(f"case {index} schema is not the Rust-declared case schema")
        if set(row) != rust_exact_fields("ChaosCase"):
            complaints.append(f"case {index} keys are not the exact ChaosCase field set")
        if row.get("completedStages") != rust_str_array("CHAOS_STAGES"):
            complaints.append(f"case {index} completedStages is not CHAOS_STAGES")
        for key, owner in (("fault", "ChaosFault"), ("recovery", "ChaosRecovery")):
            value = row.get(key)
            if value is not None and set(value) != rust_exact_fields(owner):
                complaints.append(f"case {index} {key} is not the exact {owner} field set")
    return complaints


def digest(label: str) -> str:
    return hashlib.sha256(label.encode()).hexdigest()


SOURCE_REVISION = hashlib.sha1(b"source").hexdigest()


def signature(index: int) -> str:
    # All-ones is valid base58 spelling for a zero-prefixed byte string.  The
    # chaos contract validates textual closure; finalized RPC authentication
    # in run.py validates the real 64-byte signature.
    return "1" * (64 + index % 8)


def case(spec: CHAOS.FaultSpec, index: int) -> dict:
    row = {
        "schema": CHAOS.CASE_SCHEMA_V1,
        "caseId": spec.case_id,
        "stage": spec.stage,
        "boundary": spec.boundary,
        "targetMutation": CHAOS.TARGET_MUTATIONS[spec.stage],
        "status": "finalized",
        "namedSeed": f"chaos-{index:02d}",
        "genesisHash": "1" * 32,
        "sessionIdentitySha256": digest(f"session-{index}"),
        "sourceRevision": SOURCE_REVISION,
        "checkedReleaseGateSha256": digest("gate"),
        "terminalResultSha256": digest(f"terminal-{index}"),
        "completedStages": list(CHAOS.STAGES),
        "targetIntentSha256": digest(f"intent-{index}"),
        "targetPacketSha256": digest(f"packet-{index}"),
        "targetSignature": signature(index),
        "targetSigningCount": 1,
        "targetDistinctSignatureCount": 1,
        "targetSendCount": 1,
        "fault": None,
        "recovery": None,
        "caseSha256": "0" * 64,
    }
    if spec.interrupted:
        journal = digest(f"journal-{index}")
        row["fault"] = {
            "receiptSha256": digest(f"fault-{index}"),
            "journalBeforeKillSha256": journal,
            "durablePhase": "dispatching",
            "exitCode": -9,
            "signal": 9,
            "sendCountBeforeKill": (
                0 if spec.boundary == CHAOS.PRE_SEND_BOUNDARY else 1
            ),
            "intentSha256": row["targetIntentSha256"],
            "packetSha256": row["targetPacketSha256"],
            "signature": row["targetSignature"],
        }
        row["recovery"] = {
            "sameGenesis": True,
            "sameSessionIdentity": True,
            "journalBeforeRestartSha256": journal,
            "journalAfterFinalizationSha256": digest(f"final-journal-{index}"),
            "intentSha256": row["targetIntentSha256"],
            "packetSha256": row["targetPacketSha256"],
            "signature": row["targetSignature"],
            "pollCount": 1,
            "sendCountAfterRestart": (
                1 if spec.boundary == CHAOS.PRE_SEND_BOUNDARY else 0
            ),
            "signingCountAfterRestart": 0,
            "finalizedSlot": 100 + index,
        }
    row["caseSha256"] = CHAOS._case_digest(row)
    return row


def session() -> dict:
    return CHAOS.build_session(
        source_revision=SOURCE_REVISION,
        source_tree_sha256=digest("tree"),
        checked_release_gate_sha256=digest("gate"),
        cases=[case(spec, index) for index, spec in enumerate(CHAOS.MATRIX, start=1)],
    )


class ChaosContractTests(unittest.TestCase):
    def test_matrix_is_exactly_control_plus_two_boundaries_for_eight_stages(self) -> None:
        self.assertEqual(len(CHAOS.MATRIX), 17)
        self.assertEqual(CHAOS.MATRIX[0].case_id, "control")
        self.assertEqual(
            [(row.stage, row.boundary) for row in CHAOS.MATRIX[1:]],
            [
                (stage, boundary)
                for stage in CHAOS.STAGES
                for boundary in CHAOS.BOUNDARIES
            ],
        )

    def test_exact_session_round_trips_and_writes_no_clobber(self) -> None:
        accepted = session()
        self.assertEqual(CHAOS.authenticate_session(accepted), accepted)
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "CHAOS.json"
            CHAOS.write_session_new(path, accepted)
            self.assertEqual(CHAOS.read_session(path), accepted)
            with self.assertRaisesRegex(CHAOS.Refusal, "new absolute path"):
                CHAOS.write_session_new(path, accepted)

    def test_missing_reordered_or_relabelled_case_refuses(self) -> None:
        original = session()
        missing = copy.deepcopy(original)
        missing["cases"].pop()
        missing["sessionSha256"] = CHAOS._session_digest(missing)
        with self.assertRaisesRegex(CHAOS.Refusal, "seventeen"):
            CHAOS.authenticate_session(missing)

        reordered = copy.deepcopy(original)
        reordered["cases"][1], reordered["cases"][2] = (
            reordered["cases"][2],
            reordered["cases"][1],
        )
        reordered["sessionSha256"] = CHAOS._session_digest(reordered)
        with self.assertRaisesRegex(CHAOS.Refusal, "changed identity"):
            CHAOS.authenticate_session(reordered)

    def test_restart_may_not_resign_or_send_after_landed_boundary(self) -> None:
        original = session()
        # Case two for each stage is the lost-response/landed boundary.  Use
        # founding's, matrix index 2.
        hostile = copy.deepcopy(original)
        row = hostile["cases"][2]
        row["recovery"]["signingCountAfterRestart"] = 1
        row["recovery"]["sendCountAfterRestart"] = 1
        row["caseSha256"] = CHAOS._case_digest(row)
        hostile["sessionSha256"] = CHAOS._session_digest(hostile)
        with self.assertRaisesRegex(CHAOS.Refusal, "sends after restart"):
            CHAOS.authenticate_session(hostile)

    def test_packet_signature_or_dead_journal_substitution_refuses(self) -> None:
        original = session()
        for mutation, message in (
            (("recovery", "packetSha256"), "exact packetSha256"),
            (("recovery", "signature"), "exact signature"),
            (("recovery", "journalBeforeRestartSha256"), "while the process was dead"),
        ):
            hostile = copy.deepcopy(original)
            row = hostile["cases"][1]
            row[mutation[0]][mutation[1]] = (
                "1" * 64 if mutation[1] == "signature" else digest("substituted")
            )
            row["caseSha256"] = CHAOS._case_digest(row)
            hostile["sessionSha256"] = CHAOS._session_digest(hostile)
            with self.assertRaisesRegex(CHAOS.Refusal, message):
                CHAOS.authenticate_session(hostile)

    def test_control_cannot_carry_fault_theater(self) -> None:
        original = session()
        hostile = copy.deepcopy(original)
        hostile["cases"][0]["fault"] = copy.deepcopy(hostile["cases"][1]["fault"])
        hostile["cases"][0]["caseSha256"] = CHAOS._case_digest(hostile["cases"][0])
        hostile["sessionSha256"] = CHAOS._session_digest(hostile)
        with self.assertRaisesRegex(CHAOS.Refusal, "fault or recovery theater"):
            CHAOS.authenticate_session(hostile)

    def test_execute_matrix_invokes_every_case_once(self) -> None:
        seen: list[tuple[str, int]] = []

        def execute(spec: CHAOS.FaultSpec, index: int) -> dict:
            seen.append((spec.case_id, index))
            return case(spec, index)

        accepted = CHAOS.execute_matrix(
            execute,
            source_revision=SOURCE_REVISION,
            source_tree_sha256=digest("tree"),
            checked_release_gate_sha256=digest("gate"),
        )
        self.assertEqual(len(seen), 17)
        self.assertEqual(accepted["matrix"]["caseCount"], 17)


class RustAuthenticatorHandoffTests(unittest.TestCase):
    """The half of the handoff this suite could not see before.

    Every other test here builds its fixtures out of `chaos.py`'s own constants
    and asks `chaos.py` whether it likes them. That cannot fail on the fact that
    matters -- whether the Rust that reads this session back agrees about what it
    is called -- and it was the shape under which `tools/devnet-reconcile` held a
    `-v1` chaos schema against the crate's `-v2` for as long as anyone had looked.
    """

    def test_a_session_this_contract_writes_carries_the_rust_declared_identity(
        self,
    ) -> None:
        self.assertEqual(rust_identity_complaints(session()), [])
        # And the two derived constants are the Rust's, read the other way.
        self.assertEqual(CHAOS.SESSION_SCHEMA_V2, rust_str_const("CHAOS_SESSION_SCHEMA_V2"))
        self.assertEqual(CHAOS.CASE_SCHEMA_V1, rust_str_const("CHAOS_CASE_SCHEMA_V1"))

    def test_the_contract_states_neither_schema_string_in_its_own_words(self) -> None:
        # The literal that was here until this change, and the one in the
        # runner's `finalize_lifecycle_receipt`, are the two second authors. A
        # grep is the whole check: a value with one author appears in this file
        # zero times.
        for owner in ("CHAOS_SESSION_SCHEMA_V2", "CHAOS_CASE_SCHEMA_V1"):
            self.assertNotIn(rust_str_const(owner), MODULE_PATH.read_text())

    def test_a_superseded_session_schema_refuses_on_both_sides(self) -> None:
        # The one below current is the string `tools/devnet-reconcile` actually
        # held while the crate had moved on, and it refused every session the
        # driver wrote for as long as nothing ran it.
        hostile = copy.deepcopy(session())
        hostile["schema"] = previous_version(rust_str_const("CHAOS_SESSION_SCHEMA_V2"))
        hostile["sessionSha256"] = CHAOS._session_digest(hostile)
        # The Rust refuses it: `authenticate_chaos`'s first conjunct is
        # `chaos.schema != CHAOS_SESSION_SCHEMA_V2`.
        self.assertEqual(
            rust_identity_complaints(hostile),
            ["session schema is not the Rust-declared session schema"],
        )
        # And so does this contract, which is what makes the pair a handoff
        # rather than two independent opinions.
        with self.assertRaisesRegex(CHAOS.Refusal, "changed schema"):
            CHAOS.authenticate_session(hostile)

    def test_a_case_schema_that_is_not_the_current_one_refuses_on_both_sides(
        self,
    ) -> None:
        hostile = copy.deepcopy(session())
        row = hostile["cases"][0]
        row["schema"] = previous_version(rust_str_const("CHAOS_CASE_SCHEMA_V1"))
        row["caseSha256"] = CHAOS._case_digest(row)
        hostile["sessionSha256"] = CHAOS._session_digest(hostile)
        self.assertEqual(
            rust_identity_complaints(hostile),
            ["case 0 schema is not the Rust-declared case schema"],
        )
        with self.assertRaisesRegex(CHAOS.Refusal, "changed identity"):
            CHAOS.authenticate_session(hostile)

    def test_an_extra_document_key_refuses_under_deny_unknown_fields(self) -> None:
        # `chaos.py` and the Rust both hold the document to an EXACT key set, and
        # this proves the two sets are the same one: a key `chaos.py` would
        # reject is a key serde would reject, named by the struct it belongs to.
        hostile = copy.deepcopy(session())
        hostile["chaosSessionNote"] = "added by a later author"
        self.assertEqual(
            rust_identity_complaints(hostile),
            ["session keys are not the exact ChaosSession field set"],
        )
        with self.assertRaisesRegex(CHAOS.Refusal, "changed its exact fields"):
            CHAOS.authenticate_session(hostile)


if __name__ == "__main__":
    unittest.main()
