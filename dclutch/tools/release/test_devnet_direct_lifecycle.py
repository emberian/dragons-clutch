from __future__ import annotations

import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("devnet_direct_lifecycle.py")
SPEC = importlib.util.spec_from_file_location("devnet_direct_lifecycle", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
lifecycle = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = lifecycle
SPEC.loader.exec_module(lifecycle)


MARKET = "1" * 32
FEE_PAYER = "2" * 32
SELLER = "3" * 32
BUYER = "4" * 32
RECIPIENT = "5" * 32
POSITION = "6" * 32
SIGNATURE = "7" * 88
GENERATION = 9


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


class DevnetDirectLifecycleTests(unittest.TestCase):
    def artifact_file(self, root: Path, name: str, payload: bytes = b"{}\n") -> Path:
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        return path

    def plan_and_sources(
        self, root: Path
    ) -> tuple[lifecycle.LifecyclePlan, lifecycle.BoundSources]:
        plan_path = self.artifact_file(root, "plan.json")
        pack = self.artifact_file(root, "pack.json")
        public = self.artifact_file(root, "public.json")
        direct = self.artifact_file(root, "direct.json")
        resolution = self.artifact_file(root, "resolution.json")
        fee_key = self.artifact_file(root, "keys/fee.json", b"")
        seller_key = self.artifact_file(root, "keys/seller.json", b"")
        buyer_key = self.artifact_file(root, "keys/buyer.json", b"")
        submitter = self.artifact_file(root, "keys/submitter.json", b"")
        resolver = self.artifact_file(root, "keys/resolver.json", b"")
        update = self.artifact_file(root, "keys/update.json", b"")
        plan = lifecycle.LifecyclePlan(
            path=plan_path,
            sha256=lifecycle.sha256_file(plan_path),
            rpc_url="https://api.devnet.solana.com/",
            release_pack=lifecycle.AcceptedFile(pack, lifecycle.sha256_file(pack)),
            public_campaign=lifecycle.AcceptedFile(public, lifecycle.sha256_file(public)),
            direct_evidence=lifecycle.AcceptedFile(direct, lifecycle.sha256_file(direct)),
            resolution_input=lifecycle.AcceptedFile(
                resolution, lifecycle.sha256_file(resolution)
            ),
            refreshed_evidence=None,
            expected_market=MARKET,
            terminal_lookup_table=None,
            fee_payer=lifecycle.KeyActor(FEE_PAYER, fee_key),
            seller=lifecycle.KeyActor(SELLER, seller_key),
            buyer=lifecycle.KeyActor(BUYER, buyer_key),
            resolution_submitter=submitter,
            resolution_resolver=resolver,
            resolution_update_authority=update,
        )
        source = self.artifact_file(root, "source.json")
        sources = lifecycle.BoundSources(
            pack_root=root,
            pack={"source": {"revision": "a" * 40, "tree_sha256": "b" * 64}},
            public={},
            bootstrap=source,
            successor_plan=source,
            market_input=source,
            campaign_evidence=source,
            public_manifest=source,
            direct_session=source,
            direct_journal=root,
            producer_journal=source,
            direct_evidence_value={"evidenceSha256": "c" * 64},
            payout_rows=(
                {
                    "role": "seller",
                    "owner": SELLER,
                    "position": POSITION,
                    "recipient": RECIPIENT,
                    "claimIndex": 0,
                    "quantityAtoms": 7,
                },
            ),
        )
        return plan, sources

    def plan_document(self, root: Path) -> tuple[Path, dict[str, object]]:
        accepted: dict[str, dict[str, str]] = {}
        for name in ("pack", "public", "direct", "resolution"):
            path = self.artifact_file(root, f"{name}.json")
            accepted[name] = {
                "path": str(path),
                "sha256": lifecycle.sha256_file(path),
            }
        keys = {}
        for name in ("fee", "seller", "buyer", "submitter", "resolver", "update"):
            keys[name] = str(self.artifact_file(root, f"keys/{name}.json", b""))
        value: dict[str, object] = {
            "schema": lifecycle.PLAN_SCHEMA,
            "rpcUrl": "https://api.devnet.solana.com/",
            "genesisHash": lifecycle.DEVNET_GENESIS_HASH,
            "releasePack": accepted["pack"],
            "publicRouteCampaign": accepted["public"],
            "directEvidence": accepted["direct"],
            "resolutionInput": accepted["resolution"],
            "refreshedEvidence": None,
            "expectedMarket": MARKET,
            "terminalLookupTable": None,
            "actors": {
                "feePayer": {"address": FEE_PAYER, "keypairPath": keys["fee"]},
                "seller": {"address": SELLER, "keypairPath": keys["seller"]},
                "buyer": {"address": BUYER, "keypairPath": keys["buyer"]},
            },
            "resolutionAuthorities": {
                "submitterKeypairPath": keys["submitter"],
                "resolverKeypairPath": keys["resolver"],
                "updateKeypairPath": keys["update"],
            },
        }
        path = root / "complete-life-plan.json"
        write_json(path, value)
        return path, value

    def payout_evidence(
        self, root: Path, plan: lifecycle.LifecyclePlan
    ) -> tuple[Path, dict[str, object], dict[str, object]]:
        payout_root = root / "payouts/payout-seller-000"
        payout_root.mkdir(parents=True)
        input_path = payout_root / "input.json"
        write_json(input_path, {"intent": "fixture"})
        payout = {
            "role": "seller",
            "owner": SELLER,
            "position": POSITION,
            "recipient": RECIPIENT,
            "claimIndex": 0,
            "quantityAtoms": 7,
        }
        value: dict[str, object] = {
            "schema": lifecycle.PAYOUT_EVIDENCE_SCHEMA,
            "cluster": "devnet",
            "inputSha256": lifecycle.sha256_file(input_path),
            "payoutIntentSha256": "1" * 64,
            "journalStateSha256": "2" * 64,
            "signature": SIGNATURE,
            "finalizedSlot": 9,
            "feeLamports": 5000,
            "computeUnitsConsumed": 400,
            "feePayer": plan.fee_payer.address,
            "owner": SELLER,
            "market": MARKET,
            "recipient": RECIPIENT,
            "payout": "7",
            "lookupTable": "8" * 32,
            "lookupAddressesSha256": "3" * 64,
            "payoutInstructionSha256": "4" * 64,
            "custodyRequestSha256": None,
            "returnDataProducer": "9" * 32,
            "returnDataBase64": "AA==",
            "poststates": [
                {
                    "address": "A" * 32,
                    "owner": "B" * 32,
                    "lamports": 1,
                    "executable": False,
                    "dataLen": 0,
                    "dataSha256": "5" * 64,
                }
            ],
            "evidenceSha256": "",
        }
        value["evidenceSha256"] = lifecycle.payout_evidence_digest(value)
        path = payout_root / "evidence.json"
        write_json(path, value)
        return path, payout, value

    def test_duplicate_json_keys_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory).resolve() / "duplicate.json"
            path.write_text('{"same":1,"same":2}\n')
            with self.assertRaises(lifecycle.Refusal):
                lifecycle.read_json(path, "duplicate fixture")

    def test_signature_domain_is_distinct_from_public_keys(self) -> None:
        self.assertIsNotNone(lifecycle.PUBKEY.fullmatch(MARKET))
        self.assertIsNone(lifecycle.SIGNATURE.fullmatch(MARKET))
        self.assertIsNotNone(lifecycle.SIGNATURE.fullmatch(SIGNATURE))

    def test_plan_requires_unrelated_actors_and_exact_devnet(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            path, value = self.plan_document(root)
            parsed = lifecycle.parse_plan(path)
            self.assertEqual(parsed.expected_market, MARKET)
            hostile = copy.deepcopy(value)
            hostile["actors"]["feePayer"]["address"] = SELLER  # type: ignore[index]
            write_json(path, hostile)
            with self.assertRaises(lifecycle.Refusal):
                lifecycle.parse_plan(path)
            hostile = copy.deepcopy(value)
            hostile["actors"]["feePayer"]["keypairPath"] = hostile["actors"]["seller"]["keypairPath"]  # type: ignore[index]
            write_json(path, hostile)
            with self.assertRaises(lifecycle.Refusal):
                lifecycle.parse_plan(path)
            hostile = copy.deepcopy(value)
            hostile["genesisHash"] = "not-devnet"
            write_json(path, hostile)
            with self.assertRaises(lifecycle.Refusal):
                lifecycle.parse_plan(path)

    def test_source_pinned_runner_refuses_an_adjacent_copy(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            archived = root / "source/tools/release/devnet_direct_lifecycle.py"
            archived.parent.mkdir(parents=True)
            archived.write_text("# substituted runner\n")
            with self.assertRaises(lifecycle.Refusal):
                lifecycle.assert_source_pinned_runner(root)

    def test_stage_order_keeps_maker_close_between_terminal_halves(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            _, sources = self.plan_and_sources(root)
            ids = [row["id"] for row in lifecycle.stage_specs(sources)]
            self.assertLess(ids.index("terminal-through-direct-retiring"), ids.index("maker-close-seller"))
            self.assertLess(ids.index("maker-close-buyer"), ids.index("terminal-through-core-retired"))

    def test_dispatch_is_durable_before_child_and_retry_is_appended(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            row = {
                "id": "fee-settlement",
                "kind": "fee-settlement",
                "phase": "planned",
                "attempts": [],
                "result": None,
            }
            journal = {"stages": [row], "stateSha256": ""}
            journal_path = root / "RUN.json"
            seen: list[dict[str, object]] = []

            def fail_after_observing(*args: object, **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                persisted = lifecycle.read_json(journal_path, "dispatch journal")
                attempt = persisted["stages"][0]["attempts"][0]
                seen.append(attempt)
                self.assertEqual(persisted["stages"][0]["phase"], "dispatching")
                self.assertIsNone(attempt["exitCode"])
                self.assertNotIn("DCLUTCH_CHAOS_FAULT_TEST", kwargs["env"])
                return subprocess.CompletedProcess(args[0], 9, b"", b"first failure")

            os.environ["DCLUTCH_CHAOS_FAULT_TEST"] = "armed"
            try:
                driver = lifecycle.StageDriver(root, journal_path, journal, fail_after_observing)
                with self.assertRaises(lifecycle.Refusal):
                    driver.invoke(row, ["fixture", "first"])
            finally:
                os.environ.pop("DCLUTCH_CHAOS_FAULT_TEST", None)
            restarted = lifecycle.read_json(journal_path, "restart journal")

            def succeed(*args: object, **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                persisted = lifecycle.read_json(journal_path, "second dispatch journal")
                self.assertEqual(len(persisted["stages"][0]["attempts"]), 2)
                self.assertIsNone(persisted["stages"][0]["attempts"][1]["exitCode"])
                return subprocess.CompletedProcess(args[0], 0, b"", b"")

            retry = lifecycle.StageDriver(root, journal_path, restarted, succeed)
            retry.invoke(retry.stage("fee-settlement"), ["fixture", "second"])
            final = lifecycle.read_json(journal_path, "retried journal")
            self.assertEqual([row["ordinal"] for row in final["stages"][0]["attempts"]], [1, 2])
            self.assertEqual(final["stages"][0]["attempts"][0]["exitCode"], 9)
            self.assertEqual(final["stages"][0]["attempts"][1]["exitCode"], 0)
            self.assertEqual(len(seen), 1)

    def test_journal_refuses_a_later_finalized_stage(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            plan, sources = self.plan_and_sources(root)
            journal = lifecycle.initial_journal(plan, sources)
            journal["stages"][1]["phase"] = "finalized"
            journal["stages"][1]["result"] = {}
            journal["stateSha256"] = lifecycle.journal_digest(journal)
            with self.assertRaises(lifecycle.Refusal):
                lifecycle.authenticate_journal(journal, plan, sources, root)

    def test_payout_input_lineage_refuses_even_with_recomputed_self_digest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            plan, _ = self.plan_and_sources(root)
            path, payout, value = self.payout_evidence(root, plan)
            self.assertIsNotNone(lifecycle.payout_complete(path, payout, plan))
            value["inputSha256"] = "f" * 64
            value["evidenceSha256"] = lifecycle.payout_evidence_digest(value)
            write_json(path, value)
            with self.assertRaises(lifecycle.Refusal):
                lifecycle.payout_complete(path, payout, plan)

    def test_losing_claim_zero_payout_is_valid(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            plan, _ = self.plan_and_sources(root)
            path, payout, value = self.payout_evidence(root, plan)
            self.assertGreater(payout["quantityAtoms"], 0)
            value["payout"] = "0"
            value["evidenceSha256"] = lifecycle.payout_evidence_digest(value)
            write_json(path, value)
            self.assertIsNotNone(lifecycle.payout_complete(path, payout, plan))

    def test_preexisting_payout_evidence_is_reauthenticated_by_rust(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            plan, sources = self.plan_and_sources(root)
            path, payout, value = self.payout_evidence(root, plan)
            # This substitution can carry a recomputed structural self-digest;
            # finalized RPC history, owned by Rust, is what must reject it.
            value["returnDataBase64"] = "AQ=="
            value["evidenceSha256"] = lifecycle.payout_evidence_digest(value)
            write_json(path, value)
            self.assertIsNotNone(lifecycle.payout_complete(path, payout, plan))
            row = {
                "id": "payout-seller-000",
                "kind": "payout",
                "payout": payout,
                "phase": "planned",
                "attempts": [],
                "result": None,
            }
            journal = {"stages": [row], "stateSha256": ""}
            calls: list[list[str]] = []

            def rust_refuses(*args: object, **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                command = list(args[0])
                calls.append(command)
                self.assertNotIn("--execute", command)
                return subprocess.CompletedProcess(
                    command, 9, b"", b"semantic evidence/history mismatch"
                )

            driver = lifecycle.StageDriver(
                root, root / "RUN.json", journal, rust_refuses
            )
            with self.assertRaises(lifecycle.Refusal):
                lifecycle.run_payout(driver, plan, sources, row)
            self.assertEqual(len(calls), 1)

    def test_fee_evidence_requires_the_unrelated_plan_payer(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            plan, _ = self.plan_and_sources(root)
            path = root / "fee-settlement.json"
            value = {
                "schema": lifecycle.FEE_EVIDENCE_SCHEMA,
                "cluster": "devnet",
                "market": MARKET,
                "maker": BUYER,
                "feePayer": FEE_PAYER,
                "landed": {"signature": SIGNATURE, "slot": 10},
            }
            write_json(path, value)
            self.assertIsNotNone(lifecycle.finalized_fee_evidence(path, plan))
            value["feePayer"] = SELLER
            write_json(path, value)
            with self.assertRaises(lifecycle.Refusal):
                lifecycle.finalized_fee_evidence(path, plan)

    def test_terminal_completion_substitution_refuses_on_resume(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            plan, sources = self.plan_and_sources(root)
            completion = {
                "schema": lifecycle.TERMINAL_COMPLETION_SCHEMA,
                "status": "finalized",
                "cluster": "devnet",
                "genesisHash": lifecycle.DEVNET_GENESIS_HASH,
                "market": MARKET,
                "journals": [
                    {
                        "mutation": {"kind": "aggregate-retirement"},
                        "phase": "finalized",
                    }
                ],
            }
            path = root / "terminal-completion.json"
            write_json(path, completion)
            row = {"id": "terminal-through-core-retired", "kind": "terminal-finish"}
            self.assertIsNotNone(
                lifecycle.authenticated_stage_result(row, root, plan, sources)
            )
            completion["journals"][0]["mutation"]["kind"] = "direct-close-capability"
            write_json(path, completion)
            with self.assertRaises(lifecycle.Refusal):
                lifecycle.authenticated_stage_result(row, root, plan, sources)

    def test_maker_close_command_and_evidence_bind_all_direct_sources(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            plan, sources = self.plan_and_sources(root)
            write_json(sources.public_manifest, {"context": {"generation": GENERATION}})
            replay = "C" * 32
            direct_root = "D" * 32
            write_json(
                root / "terminal-children.json",
                {
                    "directRoot": direct_root,
                    "makerReplayChildren": [
                        {"maker": SELLER, "replay": replay},
                        {"maker": BUYER, "replay": "E" * 32},
                    ],
                },
            )
            output = root / "maker-close-seller.json"
            value = {
                "schema": "dclutch-direct-close-maker-evidence-v1",
                "cluster": "devnet",
                "market": MARKET,
                "generation": GENERATION,
                "directRoot": direct_root,
                "directEvidenceSha256": plan.direct_evidence.sha256,
                "makerReplay": replay,
                "plan": {"maker": SELLER},
                "alreadyClosed": False,
                "landed": {"signature": SIGNATURE, "slot": 12},
            }
            write_json(output, value)
            self.assertIsNotNone(
                lifecycle.maker_complete(output, "seller", plan, sources)
            )

            command = lifecycle.maker_command(plan, sources, "seller", output)
            exact_pairs = {
                "--plan": str(sources.successor_plan),
                "--market-input": str(sources.market_input),
                "--campaign-evidence": str(sources.campaign_evidence),
                "--direct-evidence": str(plan.direct_evidence.path),
            }
            for flag, expected in exact_pairs.items():
                self.assertEqual(command[command.index(flag) + 1], expected)
            self.assertNotIn("--market-plan", command)

            for field, hostile in (
                ("directEvidenceSha256", "f" * 64),
                ("directRoot", "F" * 32),
                ("generation", GENERATION + 1),
            ):
                substituted = {**value, field: hostile}
                write_json(output, substituted)
                with self.assertRaises(lifecycle.Refusal):
                    lifecycle.maker_complete(output, "seller", plan, sources)

    def test_final_report_is_reused_exactly_and_never_overwritten(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory).resolve() / "DIRECT_COMPLETE_LIFE.json"
            report = {"schema": lifecycle.REPORT_SCHEMA, "value": 1}
            lifecycle.publish_exact_report(path, report)
            before = path.read_bytes()
            lifecycle.publish_exact_report(path, report)
            with self.assertRaises(lifecycle.Refusal):
                lifecycle.publish_exact_report(path, {**report, "value": 2})
            self.assertEqual(path.read_bytes(), before)

    def test_report_digest_detects_one_field_mutation(self) -> None:
        report = {"schema": lifecycle.REPORT_SCHEMA, "reportSha256": "", "value": 1}
        digest = lifecycle.report_digest(report)
        report["value"] = 2
        self.assertNotEqual(lifecycle.report_digest(report), digest)


if __name__ == "__main__":
    unittest.main()
