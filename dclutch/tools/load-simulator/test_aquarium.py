#!/usr/bin/env python3
"""Hermetic checks for the bounded aquarium supervisor."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import stat
import sys
import tempfile
import unittest

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
        self.root = Path(self.temp.name)
        self.current = {role: key(index + 1) for index, role in enumerate(("registry", "rent", "custody", "resolution", "claims", "trading", "core"))}
        self.prior = {role: key(index + 20) for index, role in enumerate(self.current)}
        self.manifest = self.root / "18.json"
        self.manifest.write_text(json.dumps({"schema": aquarium.COHORT_SCHEMA, "cohort": 18,
            "deploy_commit": "a" * 40, "roles": list(self.current), "programs": self.current,
            "markets": [{"label": "1", "kind": "direct", "address": key(100)}]}))
        self.old = self.root / "17.json"
        self.old.write_text(json.dumps({"schema": aquarium.COHORT_SCHEMA, "cohort": 17,
            "programs": self.prior}))
        self.sim = self.root / "sim.json"
        self.sim.write_text(json.dumps({"schema": "dclutch-load-simulator-config-v1",
            "cluster": {"label": "devnet", "rpc_url": "https://api.devnet.solana.com/", "devnet_genesis": aquarium.DEVNET_GENESIS},
            "work_dir": str(self.root / "child"), "market_address": key(100),
            "trade": {"mode": "devnet", "devnet": {"pairs": [
                {"seller_ticket_sha256": "b" * 64, "buyer_ticket_sha256": "c" * 64},
                {"seller_ticket_sha256": "d" * 64, "buyer_ticket_sha256": "e" * 64}] }},
            "budget": {"max_lamports_spent": 40}}))

    def tearDown(self): self.temp.cleanup()

    def body(self):
        return {"schema": aquarium.SCHEMA_CONFIG, "work_dir": str(self.root / "work"),
            "public_status": str(self.root / "public" / "aquarium-status-v1.json"),
            "limits": {"max_active_markets": 2, "min_active_markets": 1, "max_wallets": 3,
                       "max_lamports_spent": 80, "heartbeat_seconds": 2},
            "cohort": {"manifest": str(self.manifest), "manifest_sha256": aquarium.sha256_file(self.manifest),
                       "deploy_commit": "a" * 40, "checked_at": "2026-09-07T00:00:00+00:00",
                       "program_ids": self.current, "prior_manifest": str(self.old)},
            "synthetic_actors": [key(300), key(301)],
            "markets": [{"market_id": "direct-1", "address": key(100), "source_market_label": "1",
                         "join_open": False, "epochs": [{"epoch_id": "opening", "cycles": 2,
                         "simulator_config": str(self.sim)}]}]}

    def write(self, body):
        path = self.root / "aquarium.json"; path.write_text(json.dumps(body)); return path

    def test_checked_manifest_and_nonreused_ticket_epochs_are_accepted(self):
        parsed = aquarium.validate_config(self.write(self.body()))
        self.assertEqual(parsed["cohort"]["number"], 18)
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

    def test_refuses_ticket_cycle_reuse_and_unearned_public_join(self):
        body = self.body(); body["markets"][0]["epochs"][0]["cycles"] = 3
        with self.assertRaisesRegex(aquarium.Refusal, "only 2 pinned ticket pairs"):
            aquarium.validate_config(self.write(body))
        body = self.body(); body["markets"][0]["join_open"] = True
        with self.assertRaisesRegex(aquarium.Refusal, "noncustodial admission"):
            aquarium.validate_config(self.write(body))

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


if __name__ == "__main__": unittest.main()
