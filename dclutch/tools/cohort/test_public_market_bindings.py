#!/usr/bin/env python3
"""Focused red and green proofs for the public first-admission binding writer."""
import base64
import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
WRITER = HERE / "public-market-bindings.py"


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode() + b"\n"


class PublicMarketBindingsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.cohort = self.root / "cohort.json"
        self.cohort.write_bytes(canonical({
            "schema": "dclutch-cohort-manifest-v1", "cohort": 18,
            "markets": [{"address": "market-a"}, {"address": "market-b"}],
        }))

    def tearDown(self):
        self.temp.cleanup()

    def report(self, name, market, body):
        path = self.root / name
        path.write_bytes(canonical({
            "schema": "dclutch-market-founding-report-v1", "market": market,
            "linked_liability_basis_record": {
                "data_base64": base64.b64encode(body).decode(),
                "sha256": hashlib.sha256(body).hexdigest(),
            },
        }))
        return path

    def invoke(self, reports, output):
        command = ["python3", str(WRITER), "--cohort", str(self.cohort)]
        for report in reports:
            command.extend(["--founding-report", str(report)])
        command.extend(["--output", str(output)])
        return subprocess.run(command, text=True, capture_output=True)

    def test_emits_every_checked_market_in_one_manifest(self):
        first = self.report("first.json", "market-a", b"first-basis")
        second = self.report("second.json", "market-b", b"second-basis")
        output = self.root / "bindings.json"
        result = self.invoke([second, first], output)
        self.assertEqual(result.returncode, 0, result.stderr)
        binding = json.loads(output.read_text())
        self.assertEqual(binding["cohort"]["manifest_sha256"], hashlib.sha256(self.cohort.read_bytes()).hexdigest())
        self.assertEqual(binding["markets"]["market-a"]["linked_basis_record_digest"], hashlib.sha256(b"first-basis").hexdigest())
        self.assertEqual(binding["markets"]["market-b"]["linked_basis_record_digest"], hashlib.sha256(b"second-basis").hexdigest())

    def test_refuses_duplicate_market_without_replacing_accepted_output(self):
        accepted = self.root / "bindings.json"
        first = self.report("first.json", "market-a", b"first-basis")
        self.assertEqual(self.invoke([first], accepted).returncode, 0)
        before = accepted.read_bytes()
        duplicate = self.report("duplicate.json", "market-a", b"other-basis")
        result = self.invoke([first, duplicate], accepted)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("repeat market market-a", result.stderr)
        self.assertEqual(accepted.read_bytes(), before)

    def test_refuses_a_report_for_a_market_the_manifest_does_not_name(self):
        report = self.report("other.json", "market-outside", b"basis")
        result = self.invoke([report], self.root / "bindings.json")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not in this cohort manifest", result.stderr)


if __name__ == "__main__":
    unittest.main()
