#!/usr/bin/env python3
"""Keep every external local-validator launcher behind the isolation gates."""

from __future__ import annotations

from pathlib import Path
import unittest


REPO = Path(__file__).resolve().parents[2]
SHELL_LAUNCHERS = [
    "programs/clutch-sbf/scripts/run_bringup.sh",
    "programs/clutch-sbf/scripts/run_committed.sh",
    "programs/clutch-sbf/scripts/run_devnet_paces_dryrun.sh",
    "programs/clutch-sbf/scripts/run_general_committed.sh",
    "programs/clutch-sbf/scripts/run_keeper_gate.sh",
    "programs/clutch-sbf/scripts/run_local_real_pyth.sh",
    "programs/clutch-sbf/scripts/run_pyth_devnet_clone.sh",
]


class LauncherContractTests(unittest.TestCase):
    def test_shell_launchers_verify_binary_and_probe_listeners(self) -> None:
        for relative in SHELL_LAUNCHERS:
            with self.subTest(launcher=relative):
                text = (REPO / relative).read_text(encoding="utf-8")
                self.assertIn("verify-runtime.py", text)
                self.assertIn("probe-listeners.sh", text)
                self.assertNotIn(
                    '${SOLANA_TEST_VALIDATOR:-$solana_home/solana-test-validator}', text
                )
                self.assertNotIn("exec solana-test-validator", text)

    def test_operator_uses_the_same_runtime_and_listener_gates(self) -> None:
        text = (REPO / "programs/clutch-sbf/operatord/src/toolchain.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn("verify-runtime.py", text)
        self.assertIn("probe-listeners.sh", text)
        self.assertIn("CLUTCH_LOOPBACK_TEST_VALIDATOR", text)
        self.assertIn(".cache/agave-loopback-validator", text)


if __name__ == "__main__":
    unittest.main()
