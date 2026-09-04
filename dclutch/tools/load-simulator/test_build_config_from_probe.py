#!/usr/bin/env python3
"""Tests for the probe-to-config adapter.

The thing being proven is narrow and worth proving: a config this script emits
must be one `simlife_drive.load_config` accepts, and every fact in it must come
from the probe rather than from a default. A hand-typed substrate description is
how a run ends up pointed at a validator that is not the one it thinks it is.

Run: `python3 test_build_config_from_probe.py`
"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

ADAPTER_SPEC = importlib.util.spec_from_file_location(
    "dclutch_build_config_from_probe", HERE / "build_config_from_probe.py"
)
assert ADAPTER_SPEC is not None and ADAPTER_SPEC.loader is not None
adapter = importlib.util.module_from_spec(ADAPTER_SPEC)
sys.modules[ADAPTER_SPEC.name] = adapter
ADAPTER_SPEC.loader.exec_module(adapter)

DRIVE_SPEC = importlib.util.spec_from_file_location(
    "dclutch_simlife_drive_for_adapter", HERE / "simlife_drive.py"
)
assert DRIVE_SPEC is not None and DRIVE_SPEC.loader is not None
drive = importlib.util.module_from_spec(DRIVE_SPEC)
sys.modules[DRIVE_SPEC.name] = drive
DRIVE_SPEC.loader.exec_module(drive)

FOUNDER = "2SVqjPNYveWR2reX11JehENyV65zYbeR88ezapQysuaA"
SUBSTITUTED = "FqsxFRnCmHkzEkWJEYhpqw4CbEPWaeFhpYrCrhA3dXex"
KEY_FILES = (
    "campaign-payer.json",
    "core-upgrade-authority.json",
    "founding-founder.json",
    "resolver.json",
    "pyth-update-account.json",
)


def held_probe(root: Path, *, identities: dict | None = None, keys=KEY_FILES) -> Path:
    """The shape a held `--through participant` probe leaves on disk."""
    probe = root / "probe"
    seed = probe / "runs" / "seed-01"
    stage = seed / "stages" / adapter.PREPARE_STAGE_DIRECTORY
    stage.mkdir(parents=True)
    stage.joinpath("stdout.bin").write_bytes(
        json.dumps({
            "schema": "dclutch-local-mutable-prepare-v1",
            "campaign_public_identities": identities if identities is not None else {
                "founding-founder": FOUNDER,
                "substituted-founder": SUBSTITUTED,
            },
        }).encode()
    )
    key_dir = probe / "keys"
    key_dir.mkdir(parents=True)
    for name in keys:
        key_dir.joinpath(name).write_text("[]")
    plan = seed / "plan.json"
    plan.write_text(json.dumps({"schema": "successor-plan"}))
    binary = probe / "host-target" / "release" / "dclutch-local-successor-bootstrap"
    binary.parent.mkdir(parents=True)
    binary.write_text("#!/bin/sh\nexit 0\n")
    binary.chmod(0o755)
    seed.joinpath("participant-handoff.json").write_text(json.dumps({
        "schema": "dclutch-private-validator-participant-handoff-v1",
        "rpcUrl": "http://127.0.0.1:31432/",
        "plan": str(plan),
        "marketInput": str(seed / "market.json"),
        "foundingEvidence": str(seed / "founding.json"),
        "participantEvidence": str(seed / "participant.json"),
        "keyDirectory": str(key_dir),
    }))
    probe.joinpath("SUMMARY.json").write_text(json.dumps({"source_revision": "a" * 40}))
    release = root / "release"
    release.mkdir(exist_ok=True)
    release.joinpath("CHECKED_UPGRADE_GATE.json").write_text(
        json.dumps({"source_revision": "b" * 40})
    )
    return probe


class SimlifeConfigTests(unittest.TestCase):
    def build(self, root: Path, probe: Path, *extra) -> Path:
        output = root / "config.json"
        code = adapter.main([
            "--probe-work", str(probe),
            "--sim-work", str(root / "run"),
            "--output", str(output),
            "--simlife",
            "--seed", "dclutch/simlife3/test",
            *extra,
        ])
        self.assertEqual(code, 0)
        return output

    def test_the_config_it_writes_is_one_the_drive_accepts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = held_probe(root)
            output = self.build(root, probe, "--markets", "5", "--ticks", "9")
            # The strongest available assertion: the consumer's own loader, with
            # its own refusals, rather than a re-reading of the fields here.
            config = drive.load_config(output)
            self.assertEqual(config["schema"], drive.SCHEMA_CONFIG)
            self.assertEqual(config["substrate"], "lifecycle")
            spec = drive.world_spec_from_config(config)
            self.assertEqual(spec.markets, 5)
            self.assertEqual(spec.ticks, 9)
            # And it builds a substrate rather than merely parsing.
            drive.build_substrate(config, root / "run", execute=False)

    def test_the_emitted_config_can_never_carry_a_provider_key(self):
        """The value test, not the intention: nothing keyed reaches the file.

        This builder has only ever been pointed at a loopback probe, which is
        exactly why it never redacted anything -- and why cohort-15's devnet
        fork of it wrote a live Helius key into `sim-config.json` in cleartext.
        A refusal here is what stops the next fork inheriting that.
        """

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = held_probe(root)
            output = self.build(root, probe)
            self.assertNotIn("api-key", output.read_text())

            handoff = probe / "runs" / "seed-01" / "participant-handoff.json"
            body = json.loads(handoff.read_text())
            secret = "00000000-0000-0000-0000-000000000000"
            body["rpcUrl"] = f"https://devnet.helius-rpc.com/?api-key={secret}"
            handoff.write_text(json.dumps(body))
            keyed = root / "keyed.json"
            with self.assertRaises(adapter.Refusal) as refusal:
                adapter.main([
                    "--probe-work", str(probe),
                    "--sim-work", str(root / "run2"),
                    "--output", str(keyed),
                    "--simlife",
                    "--seed", "dclutch/simlife3/test",
                ])
            self.assertIn("refusing to write a api-key credential", str(refusal.exception))
            self.assertNotIn(secret, str(refusal.exception))
            self.assertFalse(keyed.exists(), "a refused config must not be left on disk")

    def test_every_identity_comes_from_the_probe_and_none_from_a_default(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = held_probe(root)
            config = json.loads(self.build(root, probe).read_text())
            lifecycle = config["lifecycle"]
            self.assertEqual(lifecycle["founding_founder"], FOUNDER)
            self.assertEqual(lifecycle["substituted_founder"], SUBSTITUTED)
            self.assertEqual(
                lifecycle["campaign_payer_keypair"], str(probe / "keys" / "campaign-payer.json")
            )
            self.assertEqual(lifecycle["substrate_keys"], str(probe / "keys"))
            self.assertEqual(config["cluster"]["rpc_url"], "http://127.0.0.1:31432/")
            self.assertEqual(config["source_revision"], "a" * 40)
            # A lifecycle world needs NO bindings: a market this run founds is
            # bound from the founding's own evidence.
            self.assertEqual(config["bindings"], {})

    def test_an_unnamed_world_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = held_probe(root)
            with self.assertRaises(adapter.Refusal) as caught:
                adapter.main([
                    "--probe-work", str(probe), "--sim-work", str(root / "run"),
                    "--output", str(root / "c.json"), "--simlife",
                ])
            self.assertIn("--seed is required", str(caught.exception))

    def test_a_key_directory_missing_a_trade_key_refuses_by_naming_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = held_probe(root, keys=("campaign-payer.json", "founding-founder.json"))
            with self.assertRaises(adapter.Refusal) as caught:
                self.build(root, probe)
            self.assertIn("core-upgrade-authority.json", str(caught.exception))

    def test_aliased_founding_identities_refuse_the_way_the_campaign_does(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = held_probe(root, identities={
                "founding-founder": FOUNDER, "substituted-founder": FOUNDER,
            })
            with self.assertRaises(adapter.Refusal) as caught:
                self.build(root, probe)
            self.assertIn("alias", str(caught.exception))

    def test_the_budget_is_carried_and_a_nonpositive_one_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = held_probe(root)
            config = json.loads(
                self.build(root, probe, "--max-lamports-spent", "12345").read_text()
            )
            self.assertEqual(config["budget"]["max_lamports_spent"], 12345)
            with self.assertRaises(adapter.Refusal):
                self.build(root, probe, "--max-lamports-spent", "0")

    def test_the_pyth_facts_document_is_carried_and_a_missing_one_refuses(self):
        """Without it every resolution refuses, several minutes into a run."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = held_probe(root)
            facts = root / "pyth-update-facts.json"
            facts.write_text(json.dumps({"schema": "dclutch-flagship-pyth-update-facts-v1"}))
            config = json.loads(
                self.build(root, probe, "--pyth-facts", str(facts)).read_text()
            )
            self.assertEqual(config["lifecycle"]["pyth_facts"], str(facts))
            with self.assertRaises(adapter.Refusal) as caught:
                self.build(root, probe, "--pyth-facts", str(root / "nothing.json"))
            self.assertIn("pyth facts document absent", str(caught.exception))

    def test_the_release_gate_names_the_substrate_a_held_probe_cannot(self):
        """A held probe has no SUMMARY.json -- it is stopped, not finished.

        Reading the revision from there produced a capture labelled with no
        substrate at all, which is the one thing a published artifact must not
        be. The checked release's gate names the bytes the validator loaded.
        """
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = held_probe(root)
            (probe / "SUMMARY.json").unlink()
            with self.assertRaises(adapter.Refusal) as caught:
                self.build(root, probe)
            self.assertIn("no source revision", str(caught.exception))
            config = json.loads(
                self.build(root, probe, "--release-root", str(root / "release")).read_text()
            )
            self.assertEqual(config["source_revision"], "b" * 40)

    def test_an_unbounded_run_carries_no_budget_block_at_all(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = held_probe(root)
            config = json.loads(self.build(root, probe).read_text())
            self.assertNotIn("budget", config)


if __name__ == "__main__":
    unittest.main(verbosity=2)
