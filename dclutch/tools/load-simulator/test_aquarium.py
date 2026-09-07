#!/usr/bin/env python3
"""Hermetic checks for the bounded aquarium supervisor."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import stat
import sys
import tempfile
import types
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("aquarium", HERE / "aquarium.py")
assert SPEC and SPEC.loader
aquarium = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = aquarium
SPEC.loader.exec_module(aquarium)


def key(number: int) -> str:
    return aquarium.base58_encode(number.to_bytes(32, "big"))


class AquariumTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.release_gate_verifier = aquarium.verify_release_gate
        self.release_pack_verifier = aquarium.verify_release_pack
        aquarium.verify_release_gate = lambda _path, _digest: None
        self.current = {role: key(index + 1) for index, role in enumerate(("registry", "rent", "custody", "resolution", "claims", "trading", "core"))}
        self.prior = {role: key(index + 20) for index, role in enumerate(self.current)}
        self.manifest = self.root / "18.json"
        self.manifest.write_text(json.dumps({"schema": aquarium.COHORT_SCHEMA, "cohort": 18,
            "deploy_commit": "a" * 40, "roles": list(self.current), "programs": self.current,
            "general_accelerator": {"program_id": key(90), "deployment_slot": "1000",
                "elf_sha256": "f" * 64, "semantic_release_id": "e" * 64},
            "markets": [{"label": "1", "kind": "direct", "address": key(100)}]}))
        self.old = self.root / "17.json"
        self.old.write_text(json.dumps({"schema": aquarium.COHORT_SCHEMA, "cohort": 17,
            "programs": self.prior}))
        self.boot = self.root / "bootstrap"
        self.boot.write_text("#!/usr/bin/env bash\nset -eu\nargs=\"$*\"\nout=''\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = --out ]; then out=\"$2\"; shift 2; else shift; fi\ndone\nprintf '{\"schema\":\"dclutch-direct-intent-ticket-author-receipt-v1\"}\\n' > /dev/stdout\nprintf '%s\\n' \"$args\" > \"$out\"\n")
        self.boot.chmod(self.boot.stat().st_mode | stat.S_IEXEC)
        self.sim = self.root / "sim.json"
        self.sim.write_text(json.dumps({"schema": "dclutch-load-simulator-config-v1",
            "cluster": {"label": "devnet", "rpc_url": "https://api.devnet.solana.com/", "devnet_genesis": aquarium.DEVNET_GENESIS},
            "bootstrap_bin": str(self.boot), "work_dir": str(self.root / "child"), "market_address": key(100),
            "trade": {"mode": "devnet", "devnet": {"pairs": [
                {"seller_ticket_sha256": "b" * 64, "buyer_ticket_sha256": "c" * 64},
                {"seller_ticket_sha256": "d" * 64, "buyer_ticket_sha256": "e" * 64}] }},
            "budget": {"max_lamports_spent": 40}}))
        self.gate = self.root / "RELEASE_GATE.json"
        self.gate.write_text(json.dumps({"schema": aquarium.REPRODUCIBLE_GATE_SCHEMA,
            "source_revision": "a" * 40, "source_tree_sha256": "1" * 64}))
        self.pack = self.root / aquarium.RELEASE_PACK_BASENAME
        self.pack.write_text(json.dumps({"fixture": "release pack is reauthenticated by a stub"}))
        self.checked_bootstrap = {"path": self.boot, "sha256": aquarium.sha256_file(self.boot),
                                  "bytes": self.boot.stat().st_size}
        aquarium.verify_release_pack = lambda _path, _digest, _revision, _tree: self.checked_bootstrap

    def tearDown(self):
        aquarium.verify_release_gate = self.release_gate_verifier
        aquarium.verify_release_pack = self.release_pack_verifier
        self.temp.cleanup()

    def body(self):
        return {"schema": aquarium.SCHEMA_CONFIG, "work_dir": str(self.root / "work"),
            "public_status": str(self.root / "public" / "aquarium-status-v1.json"),
            "limits": {"max_active_markets": 2, "min_active_markets": 1, "max_wallets": 3,
                       "max_lamports_spent": 80, "heartbeat_seconds": 2},
            "cohort": {"manifest": str(self.manifest), "manifest_sha256": aquarium.sha256_file(self.manifest),
                       "deploy_commit": "a" * 40, "checked_at": "2026-09-07T00:00:00+00:00",
                       "program_ids": self.current, "prior_manifest": str(self.old),
                       "general_accelerator": {"program_id": key(90), "deployment_slot": 1000,
                           "elf_sha256": "f" * 64, "semantic_release_id": "e" * 64},
                       "release_gate": {"path": str(self.gate), "sha256": aquarium.sha256_file(self.gate)},
                       "release_pack": {"path": str(self.pack), "sha256": aquarium.sha256_file(self.pack)}},
            "synthetic_actors": [key(300), key(301)],
            "markets": [{"market_id": "direct-1", "address": key(100), "source_market_label": "1",
                         "join_open": False, "epochs": [{"epoch_id": "opening", "cycles": 2,
                         "simulator_config": str(self.sim)}]}]}

    def write(self, body):
        path = self.root / "aquarium.json"; path.write_text(json.dumps(body)); return path

    def test_checked_manifest_and_nonreused_ticket_epochs_are_accepted(self):
        calls = []
        aquarium.verify_release_gate = lambda path, digest: calls.append((path, digest))
        parsed = aquarium.validate_config(self.write(self.body()))
        self.assertEqual(parsed["cohort"]["number"], 18)
        self.assertEqual(calls, [(self.gate, aquarium.sha256_file(self.gate))])
        instance = aquarium.Aquarium(parsed, execute=False)
        instance.publish("preflight")
        public = json.loads((self.root / "public" / "aquarium-status-v1.json").read_text())
        self.assertEqual(public["schema"], aquarium.SCHEMA_STATUS)
        self.assertTrue(public["activity"]["synthetic_actors"])
        self.assertFalse(public["activity"]["active_markets"][0]["join_open"])
        self.assertNotIn(str(self.root), json.dumps(public))

    def test_refuses_reused_cohort_identity(self):
        old = json.loads(self.old.read_text()); old["programs"]["core"] = self.current["core"]
        self.old.write_text(json.dumps(old))
        with self.assertRaisesRegex(aquarium.Refusal, "reused prior core"):
            aquarium.validate_config(self.write(self.body()))

    def test_refuses_a_partial_role_set_or_gate_for_another_commit(self):
        manifest = json.loads(self.manifest.read_text()); manifest["roles"].pop()
        self.manifest.write_text(json.dumps(manifest))
        with self.assertRaisesRegex(aquarium.Refusal, "exact current ordered set"):
            aquarium.validate_config(self.write(self.body()))
        self.manifest.write_text(json.dumps({"schema": aquarium.COHORT_SCHEMA, "cohort": 18,
            "deploy_commit": "a" * 40, "roles": list(self.current), "programs": self.current,
            "general_accelerator": {"program_id": key(90), "deployment_slot": "1000", "elf_sha256": "f" * 64, "semantic_release_id": "e" * 64},
            "markets": [{"label": "1", "kind": "direct", "address": key(100)}]}))
        self.gate.write_text(json.dumps({"schema": aquarium.REPRODUCIBLE_GATE_SCHEMA,
            "source_revision": "b" * 40, "source_tree_sha256": "1" * 64}))
        with self.assertRaisesRegex(aquarium.Refusal, "release_gate does not authenticate"):
            aquarium.validate_config(self.write(self.body()))

    def test_refuses_ticket_cycle_reuse_and_unearned_public_join(self):
        body = self.body(); body["markets"][0]["epochs"][0]["cycles"] = 3
        with self.assertRaisesRegex(aquarium.Refusal, "only 2 pinned ticket pairs"):
            aquarium.validate_config(self.write(body))
        body = self.body(); body["markets"][0]["join_open"] = True
        with self.assertRaisesRegex(aquarium.Refusal, "checked release binding"):
            aquarium.validate_config(self.write(body))

    def test_refuses_one_ticket_digest_reused_on_another_side(self):
        sim = json.loads(self.sim.read_text())
        sim["trade"]["devnet"]["pairs"][1]["buyer_ticket_sha256"] = "b" * 64
        self.sim.write_text(json.dumps(sim))
        with self.assertRaisesRegex(aquarium.Refusal, "ticket digest appears"):
            aquarium.validate_config(self.write(self.body()))

    def test_refuses_noncanonical_accelerator_slot(self):
        manifest = json.loads(self.manifest.read_text())
        manifest["general_accelerator"]["deployment_slot"] = "+1000"
        self.manifest.write_text(json.dumps(manifest))
        with self.assertRaisesRegex(aquarium.Refusal, "canonical non-negative decimal"):
            aquarium.validate_config(self.write(self.body()))

    def test_release_pack_refuses_a_host_provenance_from_another_source(self):
        pack = self.root / aquarium.RELEASE_PACK_BASENAME
        def write_pack(revision):
            pack.write_text(json.dumps({
                "schema": aquarium.RELEASE_PACK_SCHEMA,
                "source": {"revision": revision, "tree_sha256": "1" * 64},
                "product_handoff": {"build": {"successor": {
                    "canonical_path": str(self.boot), "bytes": self.boot.stat().st_size,
                    "sha256": aquarium.sha256_file(self.boot)}}},
            }))
        write_pack("a" * 40)
        verified = types.SimpleNamespace(returncode=0, stdout=b"verified")
        with mock.patch.object(aquarium.subprocess, "run", return_value=verified):
            accepted = self.release_pack_verifier(pack, aquarium.sha256_file(pack), "a" * 40, "1" * 64)
            self.assertEqual(accepted["path"], self.boot)
            write_pack("b" * 40)
            with self.assertRaisesRegex(aquarium.Refusal, "source identity differs"):
                self.release_pack_verifier(pack, aquarium.sha256_file(pack), "a" * 40, "1" * 64)

    def test_preparation_accepts_only_the_release_pack_host_binary(self):
        aquarium_path = self.write(self.body())
        def ticket(maker, collateral, nonce):
            return {"keypair_env": "DCLUTCH_TEST_TICKET_KEY", "maker": maker,
                    "collateral_account": collateral, "lifecycle": "fok", "outcome": "1",
                    "generation": "1", "nonce": str(nonce), "valid_from": "10", "valid_through": "100",
                    "maximum_fill": "100", "limit_price": "1000000", "fee_basis_points": "50"}
        spec = {"schema": aquarium.SCHEMA_PREPARE, "aquarium_config": str(aquarium_path),
                "market_id": "direct-1", "epoch_id": "host-bound", "cycles": 1,
                "output_dir": str(self.root / "host-bound"),
                "simulator_work_dir": str(self.root / "child-host-bound"),
                "simulator_template": str(self.sim),
                "ticket_pairs": [{"seller": ticket(key(601), key(602), 1),
                                  "buyer": ticket(key(603), key(604), 2)}]}
        path = self.root / "host-bound.json"; path.write_text(json.dumps(spec))
        self.assertEqual(aquarium.prepare_spec(path)["bootstrap"], self.boot)

    def test_preparation_refuses_a_stale_template_host_before_ticket_authoring(self):
        aquarium_path = self.write(self.body())
        stale = self.root / "stale-bootstrap"; stale.write_text("#!/bin/sh\nexit 99\n")
        stale.chmod(stale.stat().st_mode | stat.S_IEXEC)
        template = json.loads(self.sim.read_text()); template["bootstrap_bin"] = str(stale)
        template_path = self.root / "stale-template.json"; template_path.write_text(json.dumps(template))
        def ticket(maker, collateral, nonce):
            return {"keypair_env": "DCLUTCH_TEST_TICKET_KEY", "maker": maker,
                    "collateral_account": collateral, "lifecycle": "fok", "outcome": "1",
                    "generation": "1", "nonce": str(nonce), "valid_from": "10", "valid_through": "100",
                    "maximum_fill": "100", "limit_price": "1000000", "fee_basis_points": "50"}
        spec = {"schema": aquarium.SCHEMA_PREPARE, "aquarium_config": str(aquarium_path),
                "market_id": "direct-1", "epoch_id": "stale-host", "cycles": 1,
                "output_dir": str(self.root / "stale-host"),
                "simulator_work_dir": str(self.root / "child-stale-host"),
                "simulator_template": str(template_path),
                "ticket_pairs": [{"seller": ticket(key(611), key(612), 1),
                                  "buyer": ticket(key(613), key(614), 2)}]}
        path = self.root / "stale-host.json"; path.write_text(json.dumps(spec))
        with self.assertRaisesRegex(aquarium.Refusal, "differs from the checked release host binary"):
            aquarium.prepare_spec(path)
        self.assertFalse((self.root / "stale-host").exists())

    def test_refuses_a_credential_stored_in_an_epoch_config(self):
        sim = json.loads(self.sim.read_text())
        sim["cluster"]["rpc_url"] = "https://devnet.helius-rpc.com/?api-key=not-a-secret"
        self.sim.write_text(json.dumps(sim))
        with self.assertRaisesRegex(aquarium.Refusal, "credentials never enter"):
            aquarium.validate_config(self.write(self.body()))

    def test_resume_refuses_a_changed_epoch_plan(self):
        parsed = aquarium.validate_config(self.write(self.body())); instance = aquarium.Aquarium(parsed, execute=False)
        market, epoch = parsed["markets"][0], parsed["markets"][0]["epochs"][0]
        instance.record(market, epoch, "planned")
        self.sim.write_text(self.sim.read_text() + "\n")
        with self.assertRaises(aquarium.simcore.JournalConflict):
            instance.record(market, epoch, "executing")

    def test_supervisor_runs_one_finite_child_and_adopts_its_spend(self):
        """The supervisor's process edge is real, without touching a cluster.

        The fake is named ``simulator.py`` in a temporary module directory, so
        the command path remains the production fixed argv shape rather than a
        configurable shell fragment.
        """
        fake = self.root / "simulator.py"
        fake.write_text("""#!/usr/bin/env python3
import json, pathlib, sys
cfg = json.loads(pathlib.Path(sys.argv[sys.argv.index('--config') + 1]).read_text())
work = pathlib.Path(cfg['work_dir']); work.mkdir(parents=True, exist_ok=True)
(work / 'status.json').write_text(json.dumps({'trades': {'landed': 1}, 'spend': {'spent_lamports': 3}}))
print('fake simulator completed')
""")
        fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
        parsed = aquarium.validate_config(self.write(self.body()))
        previous = aquarium.HERE
        aquarium.HERE = self.root
        try:
            instance = aquarium.Aquarium(parsed, execute=True)
            self.assertEqual(instance.run(), 0)
        finally:
            aquarium.HERE = previous
        public = json.loads((self.root / "public" / "aquarium-status-v1.json").read_text())
        self.assertEqual(public["state"], "stopped")
        self.assertEqual(public["run"]["lamports_spent_observed"], 3)
        journal = self.root / "work" / "epochs" / "direct-1" / "opening" / "journal.json"
        self.assertEqual(json.loads(journal.read_text())["phase"], "finalized")

    def test_checked_snapshot_reauthenticates_status_before_static_copy(self):
        config_path = self.write(self.body())
        parsed = aquarium.validate_config(config_path)
        aquarium.Aquarium(parsed, execute=False).publish("stopped")
        destination = self.root / "site" / "aquarium-status-v1.json"
        self.assertEqual(aquarium.cmd_publish_status(types.SimpleNamespace(
            config=str(config_path), destination=str(destination))), 0)
        published = json.loads(destination.read_text())
        self.assertEqual(published["cohort"]["checked_at"], "2026-09-07T00:00:00Z")
        self.assertNotIn(str(self.root), destination.read_text())
        source = json.loads(parsed["public_status"].read_text())
        source["cohort"]["deployment_commit"] = "b" * 40
        parsed["public_status"].write_text(json.dumps(source))
        with self.assertRaisesRegex(aquarium.Refusal, "does not match the checked config"):
            aquarium.cmd_publish_status(types.SimpleNamespace(
                config=str(config_path), destination=str(self.root / "other" / "aquarium-status-v1.json")))

    def test_supervisor_requires_owned_run_id_to_stop_and_allows_explicit_resume(self):
        config_path = self.write(self.body())
        launched = types.SimpleNamespace(pid=4242)
        with mock.patch.object(aquarium.subprocess, "Popen", return_value=launched) as popen:
            self.assertEqual(aquarium.cmd_start(types.SimpleNamespace(config=str(config_path), execute=False)), 0)
        record_path = aquarium.supervisor_path(self.root / "work")
        record = json.loads(record_path.read_text())
        argv = popen.call_args.args[0]
        self.assertIn("--supervisor-run-id", argv)
        self.assertIn(record["run_id"], argv)
        with mock.patch.object(aquarium, "process_command", return_value="unrelated --supervisor-run-id other"):
            with self.assertRaisesRegex(aquarium.Refusal, "no signal was sent"):
                aquarium.cmd_stop(types.SimpleNamespace(config=str(config_path)))
        self.assertEqual(json.loads(record_path.read_text())["phase"], "lost")
        resumed = types.SimpleNamespace(pid=4343)
        with mock.patch.object(aquarium, "process_command", return_value=None), \
             mock.patch.object(aquarium.subprocess, "Popen", return_value=resumed):
            self.assertEqual(aquarium.cmd_resume(types.SimpleNamespace(config=str(config_path), execute=False)), 0)
        self.assertNotEqual(json.loads(record_path.read_text())["run_id"], record["run_id"])

    def test_epoch_preparation_authors_distinct_ticket_files_without_a_transaction(self):
        aquarium_path = self.write(self.body())
        def ticket(maker, collateral, nonce):
            return {
                "keypair_env": "DCLUTCH_TEST_TICKET_KEY", "maker": maker,
                "collateral_account": collateral, "lifecycle": "fok", "outcome": "1",
                "generation": "1", "nonce": str(nonce), "valid_from": "10",
                "valid_through": "100", "maximum_fill": "100", "limit_price": "1000000",
                "fee_basis_points": "50",
            }
        spec = {
            "schema": aquarium.SCHEMA_PREPARE, "aquarium_config": str(aquarium_path),
            "market_id": "direct-1", "epoch_id": "replenish-1", "cycles": 1,
            "output_dir": str(self.root / "prepared"),
            "simulator_work_dir": str(self.root / "child-replenished"),
            "simulator_template": str(self.sim),
            "ticket_pairs": [{"seller": ticket(key(501), key(502), 1),
                              "buyer": ticket(key(503), key(504), 2)}],
        }
        path = self.root / "prepare.json"; path.write_text(json.dumps(spec))
        parsed = aquarium.prepare_spec(path)
        self.assertEqual(parsed["market"]["market_id"], "direct-1")
        self.assertEqual(aquarium.cmd_prepare_epoch(types.SimpleNamespace(spec=str(path), author=True)), 0)
        produced = json.loads((self.root / "prepared" / "simulator-config.json").read_text())
        self.assertEqual(len(produced["trade"]["devnet"]["pairs"]), 1)
        self.assertTrue((self.root / "prepared" / "pair-000" / "seller.json").is_file())
        prepared = json.loads((self.root / "prepared" / "prepared-epoch.json").read_text())
        self.assertNotIn("DCLUTCH_TEST_TICKET_KEY", json.dumps(prepared))
        self.assertEqual(prepared["plan"]["reserved_lamports_before"], 40)
        self.assertEqual(prepared["plan"]["reserved_lamports_after"], 80)

    def test_preparation_refuses_cumulative_budget_and_existing_work_root(self):
        aquarium_path = self.write(self.body())
        def ticket(maker, collateral, nonce):
            return {"keypair_env": "DCLUTCH_TEST_TICKET_KEY", "maker": maker,
                    "collateral_account": collateral, "lifecycle": "fok", "outcome": "1",
                    "generation": "1", "nonce": str(nonce), "valid_from": "10", "valid_through": "100",
                    "maximum_fill": "100", "limit_price": "1000000", "fee_basis_points": "50"}
        spec = {"schema": aquarium.SCHEMA_PREPARE, "aquarium_config": str(aquarium_path),
                "market_id": "direct-1", "epoch_id": "replenish-2", "cycles": 1,
                "output_dir": str(self.root / "prepared-2"),
                "simulator_work_dir": str(self.root / "child-replenished-2"),
                "simulator_template": str(self.sim),
                "ticket_pairs": [{"seller": ticket(key(511), key(512), 1),
                                  "buyer": ticket(key(513), key(514), 2)}]}
        sim = json.loads(self.sim.read_text()); sim["budget"]["max_lamports_spent"] = 41
        self.sim.write_text(json.dumps(sim))
        path = self.root / "prepare-2.json"; path.write_text(json.dumps(spec))
        with self.assertRaisesRegex(aquarium.Refusal, "would exceed limits.max_lamports_spent"):
            aquarium.prepare_spec(path)
        sim["budget"]["max_lamports_spent"] = 40; self.sim.write_text(json.dumps(sim))
        (self.root / "child-replenished-2").mkdir()
        with self.assertRaisesRegex(aquarium.Refusal, "may not bypass an old driver journal"):
            aquarium.prepare_spec(path)


if __name__ == "__main__": unittest.main()
