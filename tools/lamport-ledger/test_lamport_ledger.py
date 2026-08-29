#!/usr/bin/env python3
"""Adversarial tests for the lamport statement.

Every test that matters here is a NEGATIVE CONTROL: it plants a defect this
tool was built to find and asserts the tool finds it. A classifier that has
never caught a real hole is decoration, so each of the four defects below is
one this tool actually hit while it was being written, on real run evidence:

  * the wrapped-SOL native mint counted as a campaign-created collateral mint
    (overstated a founding by exactly 1,000,000,000 lamports);
  * the administration stage's fees booked against the campaign payer, when the
    deployer paid them (misattributed 2,475,000);
  * the validator identity account counted as campaign rent in a whole-cluster
    closure (overstated by six orders of magnitude);
  * a funder poorer than its accounts explain, which is an INCOMPLETE FEE
    RECORD and must never be silently absorbed.

Run:  python3 -m unittest tools.lamport-ledger.test_lamport_ledger   (or)
      python3 tools/lamport-ledger/test_lamport_ledger.py
"""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "lamport_ledger", Path(__file__).resolve().parent / "lamport_ledger.py"
)
assert _SPEC and _SPEC.loader
ll = importlib.util.module_from_spec(_SPEC)
# `@dataclass` resolves annotations through `sys.modules[cls.__module__]`, so a
# dynamically loaded module must be registered BEFORE it is executed.
sys.modules["lamport_ledger"] = ll
_SPEC.loader.exec_module(ll)


PAYER = "4UQ3723biPswxF2iF5BtUpQphg3ymUNswFb6rDBy9U3C"
SOURCE = "6H8Ks96rrUYvxeoFeyrZY6DxNb1iFxhvKoGh5nsw5UMn"
REGISTRY = "3gJW8nFFFqXYV1RtgykmfTNUNq8BcJAaRdnV8W4cM1h5"
CORE = "CtbPLmAcVc8xpzjZMrPZ14QfapnSMbjRdouUZLjUTBPp"
RECORD = "EmjNx1azbxe6UeFuKMdsfpfb7XKoju9wSeKMsVhbvsv4"
WITNESS = "BGey6b6hKxyiLvc5J5uyqxUy68iZarseCoRg6n5oRh8j"
IDENTITY = "HJ1ptQQfNvA4jyxETv6rkJTAE5vzUHsj7C4dkZXmojDE"
WSOL = "So11111111111111111111111111111111111111112"


def evidence(**overrides):
    base = dict(
        run_root=Path("/nonexistent"),
        payer=PAYER,
        roles={"registry": REGISTRY, "core": CORE},
        genesis_accounts={},
        genesis_hash="5FzgBMDzPuXjg2SfcFMJ6zxrRR4o5NkQ7eCgJdr2cZrt",
        opening=ll.Sourced(100_000_000_000, "test#opening"),
        funding_source=SOURCE,
        fees=[],
        named_accounts={},
        journal_lamports={},
        stage_payers={},
        harvested=set(),
        source_opening=None,
    )
    base.update(overrides)
    return ll.Evidence(**base)


def row(address, lamports, owner, **kw):
    return ll.AccountRow(
        address=address,
        lamports=lamports,
        owner=owner,
        data_len=kw.get("data_len", 0),
        executable=kw.get("executable", False),
        slot=100,
    )


class Classification(unittest.TestCase):
    def test_wrapped_sol_is_not_campaign_collateral(self):
        """The defect: wSOL is Token-owned and 82 bytes, exactly like a campaign
        collateral mint. Counting it inflated a founding by a round 1e9."""
        ev = evidence()
        account = row(WSOL, 1_000_000_000, ll.TOKEN_PROGRAM, data_len=82)
        ll.classify(account, ev, {REGISTRY: "registry", CORE: "core"})
        self.assertEqual(account.flow_class, "cluster.fixture")

        statement = ll.Statement(
            slot=100, genesis_hash=ev.genesis_hash, evidence=ev, rows=[account]
        )
        self.assertEqual(
            statement.campaign_created_rent,
            0,
            "a cluster fixture must never count as campaign-placed rent",
        )

    def test_validator_identity_is_not_campaign_rent(self):
        """The defect: in a whole-cluster closure the validator identity is a
        System-owned wallet holding a genesis-scale balance."""
        ev = evidence()
        account = row(IDENTITY, 500_000_000_000_000, ll.SYSTEM_PROGRAM)
        ll.classify(account, ev, {})
        self.assertEqual(account.flow_class, "cluster.unnamed-wallet")
        statement = ll.Statement(
            slot=100, genesis_hash=ev.genesis_hash, evidence=ev, rows=[account]
        )
        self.assertEqual(statement.campaign_created_rent, 0)

    def test_a_wallet_the_run_named_IS_campaign_rent(self):
        """The converse, and the reason naming is the discriminator: the
        founding-projection witness is also a bare System wallet, and it really
        does hold 3,396,480 lamports of refunded founding rent."""
        ev = evidence(harvested={WITNESS})
        account = row(WITNESS, 3_396_480, ll.SYSTEM_PROGRAM)
        ll.classify(account, ev, {})
        self.assertEqual(account.flow_class, "wallet.campaign-named")
        statement = ll.Statement(
            slot=100, genesis_hash=ev.genesis_hash, evidence=ev, rows=[account]
        )
        self.assertEqual(statement.campaign_created_rent, 3_396_480)

    def test_role_programs_classify_by_the_runs_own_role_map(self):
        """A redeployed cohort must not be classified against stale ids: the map
        comes from the run, so an unknown owner falls through to unclassified."""
        ev = evidence()
        known = row(RECORD, 15_701_760, REGISTRY, data_len=2128)
        ll.classify(known, ev, {REGISTRY: "registry"})
        self.assertEqual(known.flow_class, "market.rent.registry-record")

        stranger = row(RECORD, 15_701_760, "9zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz")
        ll.classify(stranger, ev, {REGISTRY: "registry"})
        self.assertEqual(
            stranger.flow_class,
            "unclassified",
            "an account owned by a program the run never named must be reported, "
            "not guessed into a class",
        )


class FeeAttribution(unittest.TestCase):
    def test_fees_are_summed_per_payer_not_in_aggregate(self):
        """The defect: administration is paid by the deployer, founding by the
        campaign payer. One total misattributes 2,475,000 lamports."""
        ev = evidence(
            fees=[
                ll.FeeEvent("a", 1, 75_000, "l", "administration", False, "s", SOURCE),
                ll.FeeEvent("b", 2, 75_000, "l", "administration", False, "s", SOURCE),
                ll.FeeEvent("c", 3, 90_000, "l", "founding", False, "s", PAYER),
            ]
        )
        self.assertEqual(ev.total_fees, 240_000)
        self.assertEqual(ev.fees_paid_by(PAYER), 90_000)
        self.assertEqual(ev.fees_paid_by(SOURCE), 150_000)
        self.assertEqual(ev.fees_by_payer(), {SOURCE: 150_000, PAYER: 90_000})

    def test_a_refused_transaction_still_pays(self):
        ev = evidence(
            fees=[ll.FeeEvent("a", 1, 75_000, "hostile", "founding", True, "s", PAYER)]
        )
        self.assertEqual(ev.fees_paid_by(PAYER), 75_000)


class Identity(unittest.TestCase):
    def _statement(self, rows, ev):
        return ll.Statement(slot=100, genesis_hash=ev.genesis_hash, evidence=ev, rows=rows)

    def test_both_funders_are_counted(self):
        ev = evidence(
            source_opening=ll.Sourced(1_000_000_000, "test#source"),
            fees=[
                ll.FeeEvent("a", 1, 10_000, "l", "administration", False, "s", SOURCE),
                ll.FeeEvent("b", 2, 20_000, "l", "founding", False, "s", PAYER),
            ],
        )
        rows = [
            row(PAYER, 100_000_000_000 - 20_000 - 500_000, ll.SYSTEM_PROGRAM),
            row(SOURCE, 1_000_000_000 - 10_000 - 300_000, ll.SYSTEM_PROGRAM),
        ]
        statement = self._statement(rows, ev)
        implied, lines = statement.rent_implied_by_all_funders()
        self.assertEqual(implied, 800_000, "both funders' placements must be summed")
        self.assertEqual(len(lines), 2)

    def test_a_leak_is_reported_and_never_absorbed(self):
        """The core negative control. The funder is poorer than its accounts
        explain: lamports left and arrived nowhere the closure can see."""
        ev = evidence(
            fees=[ll.FeeEvent("a", 1, 20_000, "l", "founding", False, "s", PAYER)],
            harvested={RECORD},
        )
        rows = [
            # payer spent 20,000 fees + 500,000 elsewhere ...
            row(PAYER, 100_000_000_000 - 20_000 - 500_000, ll.SYSTEM_PROGRAM),
            # ... but only 300,000 of it landed in a named account.
            row(RECORD, 300_000, REGISTRY),
        ]
        for account in rows:
            ll.classify(account, ev, {REGISTRY: "registry"})
        statement = self._statement(rows, ev)
        ll.detect_divergences(statement)
        kinds = [d.kind for d in statement.divergences]
        self.assertIn("spend-exceeds-observed-holdings", kinds)
        leak = next(d for d in statement.divergences if d.kind == "spend-exceeds-observed-holdings")
        self.assertEqual(leak.lamports, -200_000)

    def test_rent_from_an_unnamed_funder_is_reported(self):
        """The opposite sign: accounts hold more than the known funders spent,
        so some other key paid. Also a finding, also never absorbed."""
        ev = evidence(
            fees=[ll.FeeEvent("a", 1, 20_000, "l", "founding", False, "s", PAYER)],
            harvested={RECORD},
        )
        rows = [
            row(PAYER, 100_000_000_000 - 20_000 - 100_000, ll.SYSTEM_PROGRAM),
            row(RECORD, 900_000, REGISTRY),
        ]
        for account in rows:
            ll.classify(account, ev, {REGISTRY: "registry"})
        statement = self._statement(rows, ev)
        ll.detect_divergences(statement)
        kinds = [d.kind for d in statement.divergences]
        self.assertIn("rent-from-an-unnamed-funder", kinds)

    def test_an_unclassified_account_is_a_divergence_with_its_address(self):
        ev = evidence(fees=[], harvested=set())
        rows = [
            row(PAYER, 100_000_000_000, ll.SYSTEM_PROGRAM),
            row(RECORD, 4_242, "9zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"),
        ]
        for account in rows:
            ll.classify(account, ev, {REGISTRY: "registry"})
        self.assertEqual(rows[1].flow_class, "unclassified")
        statement = self._statement(rows, ev)
        ll.detect_divergences(statement)
        found = next(d for d in statement.divergences if d.kind == "unclassified-accounts")
        self.assertEqual(found.lamports, 4_242)
        self.assertTrue(
            any(RECORD in entry for entry in found.accounts),
            "a divergence must name the ACCOUNT, never only the number",
        )

    def test_journal_disagreeing_with_chain_is_reported(self):
        ev = evidence(journal_lamports={RECORD: (999, "journal#somewhere")})
        rows = [row(PAYER, 100_000_000_000, ll.SYSTEM_PROGRAM), row(RECORD, 1_000, REGISTRY)]
        for account in rows:
            ll.classify(account, ev, {REGISTRY: "registry"})
        statement = self._statement(rows, ev)
        ll.detect_divergences(statement)
        kinds = [d.kind for d in statement.divergences]
        self.assertIn("journal-vs-chain", kinds)


class Parsing(unittest.TestCase):
    def test_lamports_must_be_exact(self):
        self.assertEqual(ll.integer(5, "x"), 5)
        self.assertEqual(ll.integer("100000000000", "x"), 100_000_000_000)
        for bad in (1.5, "0x10", "1_000", True, None, "007"):
            with self.subTest(bad=bad):
                with self.assertRaises(SystemExit):
                    ll.integer(bad, "x")

    def test_pubkey_recognition(self):
        self.assertTrue(ll.looks_like_pubkey(PAYER))
        self.assertTrue(ll.looks_like_pubkey(ll.SYSTEM_PROGRAM))
        for bad in ("hello", "", "0OIl" * 8, "short"):
            self.assertFalse(ll.looks_like_pubkey(bad), bad)

    def test_harvest_finds_addresses_at_any_depth(self):
        found: set[str] = set()
        ll.harvest_addresses(
            {"a": [{"b": {"address": WITNESS}}, "noise"], "c": PAYER}, found
        )
        self.assertEqual(found, {WITNESS, PAYER})

    def test_stage_log_fees_are_parsed_exactly(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            stage = root / "stages" / "06-founding"
            stage.mkdir(parents=True)
            (stage / "stderr.bin").write_bytes(
                b"noise\n"
                b"campaign transaction: slot=1119 fee=90000 compute_units=8281 create mint\n"
                b"campaign transaction: slot=1151 fee=75000 compute_units=27171 publish record: Begin\n"
                b"unrelated line\n"
            )
            events = ll.stage_log_fees(root, "founding", PAYER)
        self.assertEqual([e.lamports for e in events], [90_000, 75_000])
        self.assertEqual([e.slot for e in events], [1119, 1151])
        self.assertTrue(all(e.payer == PAYER for e in events))
        self.assertEqual(events[1].label, "publish record: Begin")


class TraceEmission(unittest.TestCase):
    def test_trace_matches_the_existing_oracles_schema(self):
        """The union test: what this tool emits must be what
        `tools/economic-lifecycle-ledger/ledger.py check-lamports` accepts."""
        ev = evidence(
            fees=[ll.FeeEvent("a", 1, 20_000, "l", "founding", False, "s", PAYER)],
            harvested={RECORD},
        )
        rows = [row(PAYER, 99_000_000_000, ll.SYSTEM_PROGRAM), row(RECORD, 300_000, REGISTRY)]
        for account in rows:
            ll.classify(account, ev, {REGISTRY: "registry"})
        statement = ll.Statement(
            slot=100, genesis_hash=ev.genesis_hash, evidence=ev, rows=rows
        )
        trace = ll.render_trace(statement)
        self.assertEqual(trace["schema"], "dclutch-exact-lamport-trace-v1")
        self.assertEqual(set(trace.keys()), {"schema", "events"})
        kinds = {event["kind"] for event in trace["events"]}
        self.assertEqual(kinds, {"transfer", "network-fee", "rent-lock"})
        for event in trace["events"]:
            self.assertIsInstance(event["lamports"], str)
            self.assertRegex(event["lamports"], r"^(0|[1-9][0-9]*)$")
            if event["kind"] == "transfer":
                self.assertEqual(
                    set(event), {"kind", "stage", "source", "destination", "lamports"}
                )
            elif event["kind"] == "network-fee":
                self.assertEqual(set(event), {"kind", "stage", "payer", "lamports"})
            else:
                self.assertEqual(
                    set(event), {"kind", "stage", "payer", "account", "class", "lamports"}
                )
        accounts = [e["account"] for e in trace["events"] if e["kind"] == "rent-lock"]
        self.assertEqual(
            len(accounts), len(set(accounts)), "check-lamports requires unique rent accounts"
        )


def run_root(tmp: str, journals: list, transactions: list | None = None) -> Path:
    """A minimal run directory: enough evidence for load_evidence, no more."""
    root = Path(tmp)
    (root / "mutable").mkdir(parents=True)
    (root / "mutable" / "plan.json").write_text(json.dumps({"genesis_accounts": {}}))
    founding: dict = {
        "roles": [{"role": "registry", "program_id": REGISTRY}],
        "payer": PAYER,
        "foundingSubmissionJournals": journals,
    }
    if transactions is not None:
        founding["execution"] = {"transactions": transactions}
    (root / "founding.json").write_text(json.dumps(founding))
    return root


class JournalFees(unittest.TestCase):
    """The defect these controls pin: the -385,000 residual on selseam-hold-01.

    310,000 of it was four FINALIZED submission journals whose fees sat in
    founding.json and were counted by NOTHING (the funding-readiness ops are
    driven over a path that never prints the stage's `campaign transaction:`
    line); the last 75,000 was a submitted-never-observed journal whose
    deterministic fee the record stated and the statement ignored.
    """

    def test_a_finalized_journal_fee_is_counted(self):
        with tempfile.TemporaryDirectory() as tmp:
            ev = ll.load_evidence(run_root(tmp, [{
                "operation": "dcltcfq1",
                "phase": "finalized",
                "feeLamports": 80_000,
                "exactFeeLamports": 80_000,
                "expectedSignature": "sig-cfq1",
                "finalizedSlot": 10_633,
                "payer": PAYER,
            }]))
        self.assertEqual(ev.total_fees, 80_000)
        self.assertEqual(ev.fees[0].payer, PAYER)
        self.assertIn("foundingSubmissionJournals[0]", ev.fees[0].source)
        self.assertEqual(ev.unresolved, [])

    def test_a_journal_joined_to_an_execution_row_is_not_double_counted(self):
        """run.py's founding gate asserts journals JOIN execution.transactions
        by signature; when both records exist the fee must count ONCE."""
        with tempfile.TemporaryDirectory() as tmp:
            ev = ll.load_evidence(run_root(
                tmp,
                [{
                    "operation": "dcltcfq1",
                    "phase": "finalized",
                    "feeLamports": 80_000,
                    "expectedSignature": "sig-cfq1",
                    "finalizedSlot": 10_633,
                    "payer": PAYER,
                }],
                transactions=[{
                    "signature": "sig-cfq1",
                    "slot": 10_633,
                    "fee_lamports": 80_000,
                    "label": "founding-core-funding-create",
                }],
            ))
        self.assertEqual(ev.total_fees, 80_000, "one transaction, one fee")
        self.assertEqual(len(ev.fees), 1)

    def test_a_submitted_journal_is_a_bound_never_zero_never_dropped(self):
        with tempfile.TemporaryDirectory() as tmp:
            ev = ll.load_evidence(run_root(tmp, [{
                "operation": "resolution-funding-activate-v1",
                "phase": "submitted",
                "feeLamports": None,
                "exactFeeLamports": 75_000,
                "expectedSignature": "sig-activate",
                "payer": PAYER,
            }]))
        self.assertEqual(ev.total_fees, 0, "an unresolved fee is never guessed")
        self.assertEqual(len(ev.unresolved), 1)
        sub = ev.unresolved[0]
        self.assertEqual(sub.bound_lamports, 75_000)
        self.assertEqual(sub.signature, "sig-activate")
        self.assertEqual(sub.resolution, "unresolved")

    def test_a_driver_witnessed_landing_marker_is_a_named_fee(self):
        """The driver's sealing pass can witness a landing while the chain
        still serves the status; by ledger time the marker may be the ONLY
        surviving witness, and a witnessed landing at a deterministic fee is a
        named fee, not a bound."""
        with tempfile.TemporaryDirectory() as tmp:
            ev = ll.load_evidence(run_root(tmp, [{
                "operation": "resolution-funding-activate-v1",
                "phase": "submitted",
                "feeLamports": None,
                "exactFeeLamports": 75_000,
                "expectedSignature": "sig-activate",
                "payer": PAYER,
                "unresolvedFee": {
                    "resolution": "chain-status-only",
                    "statusSlot": 11_700,
                    "unresolvedFeeBoundLamports": 75_000,
                    "checkedAtSlot": 15_900,
                },
            }]))
        self.assertEqual(ev.total_fees, 75_000)
        self.assertEqual(ev.unresolved, [])
        self.assertIn("driver-witnessed", ev.fees[0].source)

    def test_a_chain_unserved_marker_stays_a_bound(self):
        with tempfile.TemporaryDirectory() as tmp:
            ev = ll.load_evidence(run_root(tmp, [{
                "operation": "resolution-funding-activate-v1",
                "phase": "submitted",
                "feeLamports": None,
                "exactFeeLamports": 75_000,
                "expectedSignature": "sig-activate",
                "payer": PAYER,
                "unresolvedFee": {
                    "resolution": "chain-unserved",
                    "statusSlot": None,
                    "unresolvedFeeBoundLamports": 75_000,
                    "checkedAtSlot": 15_900,
                },
            }]))
        self.assertEqual(ev.total_fees, 0)
        self.assertEqual(len(ev.unresolved), 1)
        self.assertEqual(ev.unresolved[0].bound_lamports, 75_000)

    def test_a_finalized_journal_without_a_fee_is_the_same_unknown(self):
        with tempfile.TemporaryDirectory() as tmp:
            ev = ll.load_evidence(run_root(tmp, [{
                "operation": "broken",
                "phase": "finalized",
                "feeLamports": None,
                "exactFeeLamports": 75_000,
                "expectedSignature": "sig-broken",
            }]))
        self.assertEqual(ev.total_fees, 0)
        self.assertEqual(len(ev.unresolved), 1)


class ConservationVerdict(unittest.TestCase):
    """The bar: a statement on a founding closes EXACT or names its bound
    with a reason. It never reports a residual and shrugs."""

    def _statement(self, ev, rows):
        for account in rows:
            ll.classify(account, ev, {REGISTRY: "registry"})
        statement = ll.Statement(
            slot=100, genesis_hash=ev.genesis_hash, evidence=ev, rows=rows
        )
        ll.detect_divergences(statement)
        return statement

    def _unresolved(self, bound, payer=PAYER, resolved_fee=None):
        return ll.UnresolvedSubmission(
            operation="resolution-funding-activate-v1",
            signature="sig-activate",
            bound_lamports=bound,
            payer=payer,
            stage="founding",
            source="founding.json#foundingSubmissionJournals[4]",
            resolved_fee=resolved_fee,
        )

    def test_exact_when_everything_is_named(self):
        ev = evidence(
            fees=[ll.FeeEvent("a", 1, 20_000, "l", "founding", False, "s", PAYER)],
            harvested={RECORD},
        )
        rows = [
            row(PAYER, 100_000_000_000 - 20_000 - 300_000, ll.SYSTEM_PROGRAM),
            row(RECORD, 300_000, REGISTRY),
        ]
        statement = self._statement(ev, rows)
        self.assertEqual(statement.conservation["verdict"], "exact")
        self.assertEqual(statement.divergences, [])

    def test_a_miss_equal_to_the_bound_closes_bounded_with_the_suspect_named(self):
        """The selseam-hold-01 shape: the funders are poorer by EXACTLY the
        deterministic fee of the one submission the run never observed."""
        ev = evidence(
            fees=[ll.FeeEvent("a", 1, 20_000, "l", "founding", False, "s", PAYER)],
            harvested={RECORD},
            unresolved=[self._unresolved(75_000)],
        )
        rows = [
            row(PAYER, 100_000_000_000 - 20_000 - 300_000 - 75_000, ll.SYSTEM_PROGRAM),
            row(RECORD, 300_000, REGISTRY),
        ]
        statement = self._statement(ev, rows)
        conservation = statement.conservation
        self.assertEqual(conservation["verdict"], "bounded")
        self.assertIn("resolution-funding-activate-v1", conservation["reason"])
        self.assertIn("sig-activate", conservation["reason"])
        self.assertIn("EXACTLY", conservation["reason"])
        self.assertNotIn(
            "spend-exceeds-observed-holdings",
            [d.kind for d in statement.divergences],
            "a miss the bound covers is CLOSED WITH A BOUND, not a divergence",
        )

    def test_a_miss_beyond_the_bound_is_still_a_divergence(self):
        ev = evidence(
            fees=[ll.FeeEvent("a", 1, 20_000, "l", "founding", False, "s", PAYER)],
            harvested={RECORD},
            unresolved=[self._unresolved(75_000)],
        )
        rows = [
            row(PAYER, 100_000_000_000 - 20_000 - 300_000 - 200_000, ll.SYSTEM_PROGRAM),
            row(RECORD, 300_000, REGISTRY),
        ]
        statement = self._statement(ev, rows)
        self.assertEqual(statement.conservation["verdict"], "divergent")
        self.assertIn(
            "spend-exceeds-observed-holdings",
            [d.kind for d in statement.divergences],
        )

    def test_zero_residual_proves_an_identity_funders_submission_never_landed(self):
        ev = evidence(
            fees=[ll.FeeEvent("a", 1, 20_000, "l", "founding", False, "s", PAYER)],
            harvested={RECORD},
            unresolved=[self._unresolved(75_000)],
        )
        rows = [
            row(PAYER, 100_000_000_000 - 20_000 - 300_000, ll.SYSTEM_PROGRAM),
            row(RECORD, 300_000, REGISTRY),
        ]
        statement = self._statement(ev, rows)
        conservation = statement.conservation
        self.assertEqual(conservation["verdict"], "exact")
        self.assertIn("never", conservation["reason"])

    def test_a_bound_the_journal_cannot_state_is_unbounded_not_bounded(self):
        ev = evidence(
            fees=[ll.FeeEvent("a", 1, 20_000, "l", "founding", False, "s", PAYER)],
            harvested={RECORD},
            unresolved=[self._unresolved(None)],
        )
        rows = [
            row(PAYER, 100_000_000_000 - 20_000 - 300_000 - 75_000, ll.SYSTEM_PROGRAM),
            row(RECORD, 300_000, REGISTRY),
        ]
        statement = self._statement(ev, rows)
        self.assertEqual(statement.conservation["verdict"], "unbounded-unknown")
        self.assertIn(
            "spend-exceeds-observed-holdings",
            [d.kind for d in statement.divergences],
        )

    def test_a_journal_fee_sharing_slot_and_fee_with_a_stage_log_line_is_flagged(self):
        ev = evidence(
            fees=[
                ll.FeeEvent(
                    "<unsigned:06-founding:0>", 10_633, 80_000, "l", "founding",
                    False, "stages/06-founding/stderr.bin#line-match[0]", PAYER,
                ),
                ll.FeeEvent(
                    "sig-cfq1", 10_633, 80_000, "dcltcfq1", "founding", False,
                    "founding.json#foundingSubmissionJournals[0].feeLamports", PAYER,
                ),
            ],
        )
        rows = [row(PAYER, 100_000_000_000 - 160_000, ll.SYSTEM_PROGRAM)]
        statement = self._statement(ev, rows)
        self.assertIn(
            "journal-fee-possible-double-count",
            [d.kind for d in statement.divergences],
        )


class FakeRpc:
    """Scripted try_call answers, in the shape the real endpoint returns."""

    def __init__(self, script: dict):
        self.script = script

    def try_call(self, method, params):
        answer = self.script.get(method, (None, f"unscripted method {method}"))
        return answer


class Resolution(unittest.TestCase):
    def _sub(self, bound=75_000):
        return ll.UnresolvedSubmission(
            operation="resolution-funding-activate-v1",
            signature="sig-activate",
            bound_lamports=bound,
            payer=PAYER,
            stage="founding",
            source="founding.json#foundingSubmissionJournals[4]",
        )

    def test_a_chain_served_lookup_promotes_the_bound_to_a_named_fee(self):
        ev = evidence(unresolved=[self._sub()])
        rpc = FakeRpc({
            "getSignatureStatuses": ({"value": [{"slot": 11_700, "err": None}]}, None),
            "getTransaction": ({"slot": 11_700, "meta": {"fee": 75_000, "err": None}}, None),
        })
        ll.resolve_unobserved_submissions(ev, rpc)
        sub = ev.unresolved[0]
        self.assertEqual(sub.resolution, "chain-served")
        self.assertEqual(sub.resolved_fee, 75_000)
        self.assertEqual(ev.total_fees, 75_000)
        self.assertIn("chain:getTransaction", ev.fees[0].source)

    def test_a_status_without_metadata_uses_the_deterministic_fee(self):
        """The chain confirms it LANDED but no longer serves the transaction:
        the deterministic exactFeeLamports IS the fee, and the source says
        exactly which two facts were joined to conclude that."""
        ev = evidence(unresolved=[self._sub()])
        rpc = FakeRpc({
            "getSignatureStatuses": ({"value": [{"slot": 11_700, "err": None}]}, None),
            "getTransaction": (None, "RPC getTransaction refused: history off"),
        })
        ll.resolve_unobserved_submissions(ev, rpc)
        sub = ev.unresolved[0]
        self.assertEqual(sub.resolution, "chain-status-only")
        self.assertEqual(sub.resolved_fee, 75_000)
        self.assertEqual(ev.total_fees, 75_000)
        self.assertIn("deterministic fee", ev.fees[0].source)

    def test_an_unserved_signature_keeps_its_bound(self):
        ev = evidence(unresolved=[self._sub()])
        rpc = FakeRpc({
            "getSignatureStatuses": ({"value": [None]}, None),
        })
        ll.resolve_unobserved_submissions(ev, rpc)
        self.assertEqual(ev.unresolved[0].resolution, "chain-unserved")
        self.assertIsNone(ev.unresolved[0].resolved_fee)
        self.assertEqual(ev.total_fees, 0)

    def test_a_refused_endpoint_degrades_to_the_bound_never_kills(self):
        ev = evidence(unresolved=[self._sub()])
        rpc = FakeRpc({
            "getSignatureStatuses": (None, "RPC getSignatureStatuses failed: down"),
        })
        ll.resolve_unobserved_submissions(ev, rpc)
        self.assertTrue(ev.unresolved[0].resolution.startswith("rpc-refused"))
        self.assertIsNone(ev.unresolved[0].resolved_fee)
        self.assertEqual(ev.total_fees, 0)


class Safety(unittest.TestCase):
    def test_remote_rpc_is_refused_without_acknowledgement(self):
        with self.assertRaises(SystemExit):
            ll.Rpc("https://api.devnet.solana.com", allow_remote=False)
        ll.Rpc("http://127.0.0.1:39100", allow_remote=False)
        ll.Rpc("https://api.devnet.solana.com", allow_remote=True)


if __name__ == "__main__":
    unittest.main(verbosity=2)
