#!/usr/bin/env python3
"""Unit tests for the seam-audit readers.

These are deliberately not tests that the readers "work" -- the negative
controls in ``negative-controls.sh`` establish that, against defects this
repository actually had.  What is tested here is the reading machinery
underneath, and every case below is one the checker got *wrong* at some point
while it was being written.  A test that could only ever have passed is the
exact failure mode the 2026-08-29 audit was about.

    tools/seam-audit/test-seam-audit.py
"""

from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from seam_audit import (  # noqa: E402
    Finding,
    compare,
    crate_of,
    decode_rust_byte_string,
)
from seam_rules import (  # noqa: E402
    _code_mask,
    _decode_value,
    _identifier,
    _name_tokens,
    _scan_functions,
    _value_tokens,
)


class DecodeLiterals(unittest.TestCase):
    def test_plain_byte_string(self):
        self.assertEqual(
            decode_rust_byte_string('b"dclutch:custody-authority:v1"'),
            b"dclutch:custody-authority:v1",
        )

    def test_length_is_bytes_not_characters(self):
        # The 32-byte bound is on bytes.  Measuring characters would report a
        # multi-byte domain as shorter than the chain sees it.
        self.assertEqual(len(decode_rust_byte_string(r'b"\xff\xfe"')), 2)

    def test_escapes(self):
        self.assertEqual(decode_rust_byte_string(r'b"a\nb\0"'), b"a\nb\x00")

    def test_declines_what_it_cannot_see(self):
        # A domain built by a macro has no statically knowable length, and
        # guessing one would be worse than skipping it.
        self.assertIsNone(decode_rust_byte_string('concat!("a", "b")'))
        self.assertIsNone(decode_rust_byte_string("OTHER_CONSTANT"))

    def test_generated_hex_arrays(self):
        # Lean emits domains as hex arrays with no `b""` anywhere.  A
        # byte-string-only reader is blind to every generated domain --
        # including SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2_GENERATED, which is
        # genuinely unguarded.
        self.assertEqual(_decode_value("[0x64, 0x63, 0x6c]"), b"dcl")
        self.assertEqual(_decode_value("&[0x64, 0x63]"), b"dc")


class SeedSegments(unittest.TestCase):
    def test_path_qualified_segments_resolve_to_one_domain(self):
        for spelling in (
            "RAW_RECORD_PDA_SEED_V1",
            "seeds::RAW_RECORD_PDA_SEED_V1",
            "dclutch_record_contract::RAW_RECORD_PDA_SEED_V1",
            "&crate::seeds::RAW_RECORD_PDA_SEED_V1",
        ):
            self.assertEqual(_identifier(spelling), "RAW_RECORD_PDA_SEED_V1")

    def test_expressions_are_not_identifiers(self):
        self.assertEqual(_identifier("key.expected_digest().as_bytes()"), "")
        self.assertEqual(_identifier("&[raw_bump]"), "")


class CodeMask(unittest.TestCase):
    """Braces inside comments and literals must not move the scanner."""

    def test_line_comment_is_not_code(self):
        text = 'fn a() { // }\n }\n'
        mask = _code_mask(text)
        self.assertEqual(mask[text.index("// }") + 3], 0)

    def test_byte_string_brace_is_not_code(self):
        text = 'const D: &[u8] = b"a{b";\n'
        mask = _code_mask(text)
        self.assertEqual(mask[text.index("{")], 0)

    def test_block_comment_is_not_code(self):
        text = "/* fn hidden() { */ fn real() {}\n"
        mask = _code_mask(text)
        self.assertEqual(mask[text.index("fn hidden")], 0)
        self.assertEqual(mask[text.index("fn real")], 1)

    def test_lifetime_is_not_an_unterminated_literal(self):
        # `AccountInfo<'_>` is everywhere in this tree.  Reading `'` as the
        # start of a character literal would blank the rest of the file.
        text = "fn f(a: &AccountInfo<'_>) { let b = 1; }\n"
        mask = _code_mask(text)
        self.assertEqual(mask[text.index("let b")], 1)


class ScanFunctions(unittest.TestCase):
    """The bug that made the checker miss its own negative control.

    An ast-grep ``fn $NAME(...)`` pattern silently fails on any function
    carrying an attribute or a ``pub(crate)``, because those are children of
    the same node.  On the file holding SEAM_AUDIT #12 it found 25 of 32 --
    and ``process``, the function with the defect in it, was one of the seven.
    """

    def setUp(self):
        self.tree = pathlib.Path(__file__).resolve().parent / "_scan_fixture"
        (self.tree / "crates" / "probe" / "src").mkdir(parents=True, exist_ok=True)
        (self.tree / "Cargo.toml").write_text("[workspace]\n")
        (self.tree / "crates" / "probe" / "src" / "lib.rs").write_text(
            '''
fn plain() -> u8 { 1 }

pub fn public_one(a: &[u8]) -> u8 { 2 }

pub(crate) fn crate_visible() -> u8 { 3 }

#[inline(never)]
pub(crate) fn attributed(accounts: &[u8], count: u8) -> u8 { 4 }

pub async unsafe fn qualified<T: Copy>(value: T) -> u8 { 5 }

trait Shape { fn no_body(&self) -> u8; }

#[cfg(test)]
mod tests {
    #[test]
    fn a_fixture() -> u8 { 6 }
}
'''
        )

    def tearDown(self):
        import shutil

        shutil.rmtree(self.tree, ignore_errors=True)

    def test_every_visibility_and_attribute_is_seen(self):
        found = {f.name: f for f in _scan_functions(self.tree)}
        for name in (
            "plain",
            "public_one",
            "crate_visible",
            "attributed",
            "qualified",
            "a_fixture",
        ):
            self.assertIn(name, found, f"{name} was not scanned")

    def test_a_trait_signature_has_no_body_to_read(self):
        self.assertNotIn("no_body", {f.name for f in _scan_functions(self.tree)})

    def test_parameters_are_captured_for_the_domain_erasure_reader(self):
        found = {f.name: f for f in _scan_functions(self.tree)}
        self.assertIn("accounts", found["attributed"].params)
        self.assertIn("&[u8]", found["public_one"].params)

    def test_fixtures_are_recognised_as_fixtures(self):
        found = {f.name: f for f in _scan_functions(self.tree)}
        self.assertTrue(found["a_fixture"].is_test)
        self.assertFalse(found["attributed"].is_test)


class Ownership(unittest.TestCase):
    def test_a_crate_owns_every_module_under_it(self):
        self.assertEqual(
            crate_of("crates/dclutch-record-contract/src/lib.rs"),
            "crates/dclutch-record-contract",
        )
        self.assertEqual(
            crate_of("programs/dclutch-core-sbf/src/capability.rs"),
            "programs/dclutch-core-sbf",
        )


class RoleTokens(unittest.TestCase):
    def test_version_and_boilerplate_are_not_role(self):
        self.assertEqual(
            _name_tokens("CUSTODY_AUTHORITY_PDA_DOMAIN_V1"), ["custody", "authority"]
        )
        self.assertEqual(
            _value_tokens(b"dclutch:custody-authority:v1"), ["custody", "authority"]
        )

    def test_the_confirmed_collision_shares_nothing(self):
        # CLAIMS_FOUNDING_AGGREGATE_SEED_V4 carries `dclutch:lbv2:market`.
        wanted = _name_tokens("CLAIMS_FOUNDING_AGGREGATE_SEED_V4")
        carried = _value_tokens(b"dclutch:lbv2:market")
        self.assertFalse(set(wanted) & set(carried))


class GateSemantics(unittest.TestCase):
    """The ratchet turns both ways, and the reasons file is load-bearing."""

    def baseline(self, entries):
        return {"schema": "dclutch-seam-audit-v1", "findings": entries}

    def test_a_new_finding_fails(self):
        failures, _ = compare(
            [Finding(code="SEED_LEN_OVER_MAX", key="a\tB")],
            self.baseline({}),
            set(),
        )
        self.assertTrue(any(line.startswith("NEW") for line in failures))

    def test_a_recorded_finding_passes(self):
        failures, _ = compare(
            [Finding(code="SEED_LEN_OVER_MAX", key="a\tB")],
            self.baseline({"SEED_LEN_OVER_MAX": {"a\tB": "benign-x"}}),
            {"benign-x"},
        )
        self.assertEqual(failures, [])

    def test_a_fixed_finding_must_leave_the_register(self):
        # Without this the baseline becomes a place defects go to be forgotten:
        # a fix would leave its entry standing as cover for the next one.
        failures, _ = compare(
            [],
            self.baseline({"SEED_LEN_OVER_MAX": {"a\tB": "benign-x"}}),
            {"benign-x"},
        )
        self.assertTrue(any(line.startswith("GONE") for line in failures))

    def test_a_verdict_with_no_written_reason_fails(self):
        failures, _ = compare(
            [Finding(code="SEED_LEN_OVER_MAX", key="a\tB")],
            self.baseline({"SEED_LEN_OVER_MAX": {"a\tB": "quietly-accepted"}}),
            set(),
        )
        self.assertTrue(any(line.startswith("UNREASONED") for line in failures))


class RegisterIsHonest(unittest.TestCase):
    """The committed register must say what it holds, in both files."""

    def setUp(self):
        import json

        here = pathlib.Path(__file__).resolve().parent
        self.baseline = json.loads((here / "baseline.json").read_text())
        self.reasons = (here / "EXCEPTIONS.md").read_text()

    def test_every_verdict_in_use_has_a_section(self):
        used = {
            verdict
            for entries in self.baseline["findings"].values()
            for verdict in entries.values()
        }
        for verdict in used:
            self.assertIn(f"### {verdict}", self.reasons, f"{verdict} has no reason")

    def test_nothing_is_left_untriaged(self):
        untriaged = [
            f"{code} {key}"
            for code, entries in self.baseline["findings"].items()
            for key, verdict in entries.items()
            if verdict == "untriaged"
        ]
        self.assertEqual(untriaged, [], "every finding must carry a verdict")

    def test_the_known_open_defect_is_recorded_as_a_defect(self):
        # Not as an accepted exception.  SEAM_AUDIT #13b is unfixed, and a
        # register that filed it under "accepted" would read as clean.
        signer = self.baseline["findings"]["TRANSACTION_LEVEL_SIGNER_CENSUS"]
        thirteen_b = [
            verdict
            for key, verdict in signer.items()
            if "authenticate_expired_checkpoint_v1" in key
        ]
        self.assertEqual(thirteen_b, ["confirmed-defect"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
