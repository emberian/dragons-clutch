from __future__ import annotations

import base64
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


TOOL_PATH = Path(__file__).with_name("public_route_campaign.py")
SPEC = importlib.util.spec_from_file_location("public_route_campaign", TOOL_PATH)
assert SPEC is not None and SPEC.loader is not None
campaign = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(campaign)


class PublicRouteCampaignTests(unittest.TestCase):
    def pack_fixture(self, root: Path) -> dict:
        artifacts = []
        for index, role in enumerate(
            ("core", "claims", "trading", "resolution", "custody", "registry", "rent"),
            start=1,
        ):
            checked = root / f"{role}.checked"
            checked.write_bytes(bytes([index]) * index)
            artifacts.append(
                {
                    "role": role,
                    "checked_manifest": {
                        "canonical_path": checked.name,
                        "bytes": checked.stat().st_size,
                        "sha256": campaign.pack_tool.sha256_file(checked),
                    },
                }
            )
        checked_set = root / "multiprogram.checked"
        checked_set.write_bytes(b"checked-release-set")
        checked_infrastructure = root / "infrastructure.checked"
        checked_infrastructure.write_bytes(b"checked-infrastructure")
        return {
            "artifacts": artifacts,
            "release": {
                "execution_release_set_id": "11" * 32,
                "checked_execution_release_set_id": "22" * 32,
                "checked_execution_release_set": {
                    "canonical_path": checked_set.name,
                    "bytes": checked_set.stat().st_size,
                    "sha256": campaign.pack_tool.sha256_file(checked_set),
                },
                "checked_infrastructure": {
                    "canonical_path": checked_infrastructure.name,
                    "bytes": checked_infrastructure.stat().st_size,
                    "sha256": campaign.pack_tool.sha256_file(checked_infrastructure),
                },
            },
        }

    def producer_fixture(
        self, root: Path, pack: dict, label: str
    ) -> tuple[Path, Path, Path, Path]:
        case = root / label
        case.mkdir()
        journal_root = case / "direct-trade-journal"
        journal_root.mkdir()
        (journal_root / "lookup-freeze.json").write_text(f"{label}-frozen\n")
        plan = case / "plan.json"
        market = case / "market.json"
        campaign_report = case / "campaign.json"
        participant = case / "participant.json"
        seller_ticket = case / "seller-ticket.json"
        buyer_ticket = case / "buyer-ticket.json"
        public_manifest = case / "public.json"
        session = case / "session.json"
        producer = case / "direct-trade-producer.json"
        plan.write_text(json.dumps({"release": "same-checked-release"}) + "\n")
        market.write_text(json.dumps({"market": label}) + "\n")
        campaign_report.write_text(json.dumps({"campaign": label}) + "\n")
        participant.write_text(json.dumps({"participant": label}) + "\n")
        seller_ticket.write_text(json.dumps({"ticket": f"seller-{label}"}) + "\n")
        buyer_ticket.write_text(json.dumps({"ticket": f"buyer-{label}"}) + "\n")
        public_manifest.write_text(json.dumps({"market": label}) + "\n")
        session_value = {
            "schema": campaign.DIRECT_SESSION_SCHEMA,
            "publicManifest": str(public_manifest),
            "publicManifestSha256": campaign.pack_tool.sha256_file(public_manifest),
            "plan": str(plan),
            "marketInput": str(market),
            "payerKeypair": f"/keys/{label}.json",
            "journalDir": str(journal_root),
            "evidenceFile": str(case / "finalized.json"),
            "sessionSha256": "aa" * 32,
        }
        session.write_bytes(campaign.pack_tool.canonical_json(session_value))
        checked_release = root / pack["release"]["checked_execution_release_set"][
            "canonical_path"
        ]
        producer_value = {
            "schema": campaign.DIRECT_PRODUCER_JOURNAL_SCHEMA,
            "phase": "finalized",
            "cluster": "devnet",
            "genesisHash": campaign.DEVNET_GENESIS_HASH,
            "plan": str(plan),
            "planSha256": campaign.pack_tool.sha256_file(plan),
            "marketInput": str(market),
            "marketInputSha256": campaign.pack_tool.sha256_file(market),
            "campaignReport": str(campaign_report),
            "campaignReportSha256": campaign.pack_tool.sha256_file(campaign_report),
            "buyerParticipant": str(participant),
            "buyerParticipantSha256": campaign.pack_tool.sha256_file(participant),
            "checkedExecutionRelease": str(checked_release),
            "checkedExecutionReleaseSha256": campaign.pack_tool.sha256_file(
                checked_release
            ),
            "sellerTicket": str(seller_ticket),
            "sellerTicketSha256": campaign.pack_tool.sha256_file(seller_ticket),
            "buyerTicket": str(buyer_ticket),
            "buyerTicketSha256": campaign.pack_tool.sha256_file(buyer_ticket),
            "payer": f"payer-{label}",
            "payerKeypair": f"/keys/{label}.json",
            "observationSlot": 42,
            "publicManifest": str(public_manifest),
            "publicManifestSha256": campaign.pack_tool.sha256_file(public_manifest),
            "publicManifestBase64": base64.b64encode(public_manifest.read_bytes()).decode(),
            "privateSession": str(session),
            "privateSessionSha256": campaign.pack_tool.sha256_file(session),
            "privateSessionBase64": base64.b64encode(session.read_bytes()).decode(),
            "journalDir": str(journal_root),
            "evidenceFile": str(case / "finalized.json"),
            "previousStateSha256": "bb" * 32,
            "stateSha256": "cc" * 32,
        }
        producer.write_bytes(campaign.pack_tool.canonical_json(producer_value))
        return producer, plan, session, journal_root

    def test_release_report_binds_pack_id_and_byte_exact_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(strict=True)
            pack = self.pack_fixture(root)
            output = root / "release.bin"
            output.write_bytes(b"checked-release-set")
            report = {
                "schema": campaign.RELEASE_REPORT_SCHEMA,
                "output": str(output),
                "bytes": output.stat().st_size,
                "sha256": campaign.pack_tool.sha256_file(output),
                "executionReleaseSetId": pack["release"]["execution_release_set_id"],
                "checkedExecutionReleaseSetId": pack["release"][
                    "checked_execution_release_set_id"
                ],
            }
            campaign.validate_release_report(report, output, pack)
            report["checkedExecutionReleaseSetId"] = "ff" * 32
            with self.assertRaises(campaign.Refusal):
                campaign.validate_release_report(report, output, pack)

    def test_direct_report_binds_route_bytes_and_checked_infrastructure(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(strict=True)
            pack = self.pack_fixture(root)
            output = root / "route.json"
            output.write_text(json.dumps({"format": campaign.DIRECT_MANIFEST_FORMAT}) + "\n")
            report = {
                "schema": campaign.DIRECT_REPORT_SCHEMA,
                "format": campaign.DIRECT_MANIFEST_FORMAT,
                "output": str(output),
                "bytes": output.stat().st_size,
                "sha256": campaign.pack_tool.sha256_file(output),
                "market": "market",
                "payer": "payer",
                "lookupTable": "lookup",
                "lookupTableCreationSlot": "42",
                "checkedInfrastructureSha256": pack["release"]["checked_infrastructure"][
                    "sha256"
                ],
            }
            campaign.validate_direct_report(report, output, pack)
            output.write_text(json.dumps({"format": "substituted"}) + "\n")
            with self.assertRaises(campaign.Refusal):
                campaign.validate_direct_report(report, output, pack)

    def test_journal_manifest_refuses_symlinks_and_rehashes_every_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(strict=True)
            (root / "freeze.json").write_text("frozen\n")
            (root / "setup").mkdir()
            (root / "setup" / "extend.json").write_text("extended\n")
            manifest, count = campaign.journal_manifest_bytes(root)
            self.assertEqual(count, 2)
            self.assertIn(b"freeze.json\t7\t", manifest)
            self.assertIn(b"setup/extend.json\t9\t", manifest)
            (root / "alias.json").symlink_to(root / "freeze.json")
            with self.assertRaises(campaign.Refusal):
                campaign.journal_manifest_bytes(root)

    def test_staged_manifest_ignores_outputs_but_detects_source_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(strict=True)
            source = root / "packages" / "dclutch-cli" / "src"
            source.mkdir(parents=True)
            entry = source / "main.ts"
            entry.write_text("export {};\n")
            before = campaign.staged_source_manifest_bytes(root)
            (root / "packages" / "dclutch-cli" / "dist").mkdir()
            (root / "packages" / "dclutch-cli" / "dist" / "dclutch-terminal.mjs").write_text(
                "bundle\n"
            )
            (root / "packages" / "dclutch-cli" / "node_modules").mkdir()
            (root / "packages" / "dclutch-cli" / "node_modules" / "dependency").write_text(
                "installed\n"
            )
            self.assertEqual(before, campaign.staged_source_manifest_bytes(root))
            entry.write_text("export const changed = true;\n")
            self.assertNotEqual(before, campaign.staged_source_manifest_bytes(root))

    def test_same_release_cross_market_session_substitution_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(strict=True)
            pack = self.pack_fixture(root)
            producer_a, plan_a, session_a, journal_a = self.producer_fixture(
                root, pack, "market-a"
            )
            producer_b, plan_b, session_b, journal_b = self.producer_fixture(
                root, pack, "market-b"
            )
            binding_a = campaign.producer_journal_binding(
                producer_a, plan_a, session_a, journal_a, root, pack
            )
            binding_b = campaign.producer_journal_binding(
                producer_b, plan_b, session_b, journal_b, root, pack
            )
            self.assertNotEqual(
                binding_a["sources"]["campaign_report"]["sha256"],
                binding_b["sources"]["campaign_report"]["sha256"],
            )
            with self.assertRaises(campaign.Refusal):
                campaign.producer_journal_binding(
                    producer_a, plan_b, session_b, journal_b, root, pack
                )

    def test_producer_campaign_receipt_mutation_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(strict=True)
            pack = self.pack_fixture(root)
            producer, plan, session, journal = self.producer_fixture(
                root, pack, "market-a"
            )
            value = json.loads(producer.read_text())
            Path(value["campaignReport"]).write_text('{"campaign":"substituted"}\n')
            with self.assertRaises(campaign.Refusal):
                campaign.producer_journal_binding(
                    producer, plan, session, journal, root, pack
                )

    def test_command_vectors_invoke_only_the_two_public_cli_wrappers(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(strict=True)
            pack = self.pack_fixture(root)
            plan = root / "plan.json"
            session = root / "session.json"
            checked_output = root / "release.bin"
            plan.write_text("{}\n")
            session.write_text("{}\n")
            checked_output.write_bytes(b"checked-release-set")
            release, direct = campaign.command_vectors(
                node="node",
                launcher=root / "dclutch-terminal.mjs",
                bootstrap=root / "bootstrap",
                rpc_url="https://api.devnet.solana.com",
                acknowledgment=campaign.DEVNET_GENESIS_HASH,
                plan=plan,
                session=session,
                artifacts=campaign.artifact_by_role(pack),
                pack_root=root,
                checked_output=checked_output,
                direct_output=root / "route.json",
            )
            self.assertEqual(release[release.index("route") + 1], "release-set")
            self.assertEqual(direct[direct.index("route") + 1], "direct")
            self.assertNotIn("--keypair", release)
            self.assertNotIn("--keypair", direct)
            self.assertEqual(release.count("--core-checked"), 1)
            self.assertEqual(direct.count("--registry-checked"), 1)
            self.assertEqual(direct.count("--rent-checked"), 1)


if __name__ == "__main__":
    unittest.main()
