#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Adversarial tests for the offline provisional-source snapshot gate."""

from __future__ import annotations

import copy
import json
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import check_provisional_snapshot as checker


ROOT = pathlib.Path(__file__).resolve().parents[3]
MANIFEST = ROOT / checker.DEFAULT_MANIFEST


class ProvisionalSnapshotTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.snapshot = json.loads(MANIFEST.read_text(encoding="utf-8"))

    def test_repository_record_agrees_with_review_and_clone_script(self) -> None:
        checker.check(ROOT)

    def test_promotion_status_is_refused(self) -> None:
        promoted = copy.deepcopy(self.snapshot)
        promoted["status"] = "RELEASE"
        promoted["classification"]["compiled_registry_row"] = True
        with self.assertRaisesRegex(checker.SnapshotError, "promotion"):
            checker.validate_manifest(promoted)

    def test_duplicate_account_address_is_refused(self) -> None:
        duplicated = copy.deepcopy(self.snapshot)
        duplicated["accounts"]["router_program"]["address"] = duplicated["accounts"][
            "receiver_program"
        ]["address"]
        with self.assertRaisesRegex(checker.SnapshotError, "distinct"):
            checker.validate_manifest(duplicated)

    def test_malformed_body_digest_is_refused(self) -> None:
        malformed = copy.deepcopy(self.snapshot)
        malformed["accounts"]["receiver_config"]["account_body_sha256"] = "00"
        with self.assertRaisesRegex(checker.SnapshotError, "malformed lowercase hex"):
            checker.validate_manifest(malformed)

    def test_valid_width_but_wrong_discriminator_is_refused(self) -> None:
        malformed = copy.deepcopy(self.snapshot)
        malformed["observed_post_update"]["discriminator_hex"] = "0000000000000000"
        with self.assertRaisesRegex(checker.SnapshotError, "derived"):
            checker.validate_manifest(malformed)

    def test_clone_script_identity_drift_is_refused(self) -> None:
        script = (ROOT / checker.CLONE_SCRIPT).read_text(encoding="utf-8")
        changed = script.replace(
            self.snapshot["accounts"]["receiver_config"]["address"],
            self.snapshot["accounts"]["receiver_program"]["address"],
        )
        with self.assertRaisesRegex(checker.SnapshotError, "clone script identity drift"):
            checker.validate_script_agreement(self.snapshot, changed)


if __name__ == "__main__":
    unittest.main()
