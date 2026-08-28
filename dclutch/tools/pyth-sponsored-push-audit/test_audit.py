#!/usr/bin/env python3
"""Adversarial tests for the offline sponsored-push safety matrix."""

from __future__ import annotations

import dataclasses
import struct
import unittest
from pathlib import Path

import audit


class SponsoredPushAuditTests(unittest.TestCase):
    def setUp(self) -> None:
        self.policy = audit.canonical_policy()
        self.snapshot = audit.canonical_snapshot()

    def mutate_data(self, offset: int, value: bytes) -> audit.AccountSnapshot:
        data = bytearray(self.snapshot.data)
        data[offset : offset + len(value)] = value
        return dataclasses.replace(self.snapshot, data=bytes(data))

    def refuse(self, snapshot: audit.AccountSnapshot, text: str) -> None:
        with self.assertRaisesRegex(audit.Refusal, text):
            audit.authenticate(self.policy, snapshot)

    def test_canonical_candidate_is_exact_and_identity_normalized(self) -> None:
        update = audit.authenticate(self.policy, self.snapshot)
        self.assertEqual(update.feed_id, audit.SOL_USD_FEED_ID)
        self.assertEqual(update.price, 10_450_253_500)
        self.assertEqual(update.exponent, -8)
        self.assertEqual(
            audit.derive_with_pinned_bump(self.policy),
            audit._base58_decode(audit.LEGACY_SOL_USD_ACCOUNT),
        )

    def test_account_owner_address_authority_and_privilege_substitutions_refuse(self) -> None:
        self.refuse(dataclasses.replace(self.snapshot, address="1" * 32), "account substitution")
        self.refuse(dataclasses.replace(self.snapshot, owner="1" * 32), "receiver ownership")
        self.refuse(dataclasses.replace(self.snapshot, executable=True), "receiver ownership")
        self.refuse(dataclasses.replace(self.snapshot, writable=True), "privilege")
        self.refuse(dataclasses.replace(self.snapshot, rent_exempt=False), "rent")
        self.refuse(self.mutate_data(8, bytes(32)), "write authority")

    def test_exact_shape_full_verification_feed_and_tail_refuse_hostiles(self) -> None:
        self.refuse(dataclasses.replace(self.snapshot, data=self.snapshot.data[:-1]), "length")
        self.refuse(self.mutate_data(0, bytes(8)), "discriminator")
        self.refuse(self.mutate_data(40, bytes([0])), "not Full")
        self.refuse(self.mutate_data(40, bytes([2])), "not Full")
        self.refuse(self.mutate_data(41, bytes(32)), "feed substitution")
        self.refuse(self.mutate_data(133, bytes([1])), "allocation tail")

    def test_posted_slot_and_publication_order_refuse_replay_shapes(self) -> None:
        self.refuse(self.mutate_data(125, struct.pack("<Q", 0)), "posted slot")
        self.refuse(
            self.mutate_data(125, struct.pack("<Q", self.snapshot.current_slot + 1)),
            "posted slot",
        )
        self.refuse(self.mutate_data(93, struct.pack("<q", 0)), "publication ordering")
        publish = struct.unpack_from("<q", self.snapshot.data, 93)[0]
        self.refuse(
            self.mutate_data(101, struct.pack("<q", publish + 1)),
            "publication ordering",
        )

    def test_window_staleness_future_skew_and_latest_value_overwrite_refuse(self) -> None:
        before_window = bytearray(self.snapshot.data)
        struct.pack_into("<q", before_window, 93, self.policy.window_start - 1)
        struct.pack_into("<q", before_window, 101, self.policy.window_start - 2)
        self.refuse(
            dataclasses.replace(self.snapshot, data=bytes(before_window)),
            "publication window",
        )
        # A later sponsored push can overwrite the last in-window value. That
        # is a liveness loss, never permission to consume the later value.
        self.refuse(
            self.mutate_data(93, struct.pack("<q", self.policy.window_end + 1)),
            "publication window",
        )
        publish = struct.unpack_from("<q", self.snapshot.data, 93)[0]
        stale = dataclasses.replace(
            self.snapshot,
            current_unix_seconds=publish + self.policy.maximum_age + 1,
        )
        self.refuse(stale, "freshness")
        future = dataclasses.replace(
            self.snapshot,
            current_unix_seconds=publish - self.policy.maximum_future_skew - 1,
        )
        self.refuse(future, "freshness")

    def test_exponent_and_independent_confidence_boundary_are_exact(self) -> None:
        self.refuse(self.mutate_data(89, struct.pack("<i", -7)), "exponent")
        price = struct.unpack_from("<q", self.snapshot.data, 73)[0]
        boundary = abs(price) * self.policy.maximum_confidence_bps // 10_000
        at_boundary = self.mutate_data(81, struct.pack("<Q", boundary))
        self.assertEqual(audit.authenticate(self.policy, at_boundary).confidence, boundary)
        self.refuse(
            self.mutate_data(81, struct.pack("<Q", boundary + 1)),
            "confidence",
        )

    def test_admissible_mutation_changes_observed_digest_without_preflight_truth(self) -> None:
        first = audit.authenticate(self.policy, self.snapshot)
        changed = self.mutate_data(73, struct.pack("<q", first.price + 1))
        second = audit.authenticate(self.policy, changed)
        self.assertNotEqual(first.body_sha256, second.body_sha256)
        self.assertEqual(second.price, first.price + 1)

    def test_head_is_monotone_under_exact_three_field_order(self) -> None:
        update = audit.authenticate(self.policy, self.snapshot)
        original = audit.candidate_from_update("candidate-a", update)
        older = dataclasses.replace(original, address="candidate-old", publish_time=original.publish_time - 1)
        later_time = dataclasses.replace(original, address="candidate-time", publish_time=original.publish_time + 1)
        later_slot = dataclasses.replace(original, address="candidate-slot", posted_slot=original.posted_slot + 1)
        later_digest = dataclasses.replace(original, address="candidate-digest", body_sha256="ff" * 32)
        self.assertEqual(audit.advance_head(None, original), original)
        self.assertEqual(audit.advance_head(original, older), original)
        self.assertEqual(audit.advance_head(original, later_time), later_time)
        self.assertEqual(audit.advance_head(original, later_slot), later_slot)
        self.assertEqual(audit.advance_head(original, later_digest), later_digest)

    def test_deadline_closes_set_without_upstream_monotonicity(self) -> None:
        deadline = audit.primary_deadline(self.policy)
        self.assertFalse(audit.candidate_set_closed(self.policy, deadline))
        self.assertTrue(audit.candidate_set_closed(self.policy, deadline + 1))
        update = audit.authenticate(self.policy, self.snapshot)
        head = audit.candidate_from_update("candidate-a", update)
        self.assertEqual(
            audit.terminal_selection(self.policy, deadline + 1, head),
            "best-valid-submitted-candidate",
        )
        self.assertEqual(
            audit.terminal_selection(self.policy, deadline + 1, None),
            "funded-failure",
        )
        with self.assertRaisesRegex(audit.Refusal, "candidate set is open"):
            audit.terminal_selection(self.policy, deadline, None)

    def test_upstream_advancement_is_not_closure_without_exact_push_proof(self) -> None:
        update = audit.authenticate(self.policy, self.snapshot)
        advanced = dataclasses.replace(update, publish_time=self.policy.window_end + 1)
        before_deadline = self.policy.window_end + 1
        self.assertFalse(
            audit.candidate_set_closed(self.policy, before_deadline, advanced, False)
        )
        self.assertTrue(
            audit.candidate_set_closed(self.policy, before_deadline, advanced, True)
        )

    def test_late_candidate_after_primary_deadline_refuses(self) -> None:
        late = dataclasses.replace(
            self.snapshot,
            current_unix_seconds=audit.primary_deadline(self.policy) + 1,
        )
        self.refuse(late, "freshness|candidate admission deadline")

    def test_matrix_is_complete_and_tree_citations_exist(self) -> None:
        tool = Path(__file__).resolve().parent
        audit.validate_matrix(tool.parents[1], tool / "matrix.json")


if __name__ == "__main__":
    unittest.main()
