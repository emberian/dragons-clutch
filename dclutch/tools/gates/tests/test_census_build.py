"""A shared observation ledger must not share Cargo metadata across checkouts."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from gates import census  # noqa: E402


class CensusBuildIsolationTests(unittest.TestCase):
    def test_two_checkouts_sharing_a_ledger_build_in_distinct_workspace_targets(self):
        with tempfile.TemporaryDirectory(prefix="dclutch-census-build.") as directory:
            base = Path(directory)
            shared_ledger = base / "observations"
            launches = []

            def command(args, *, cwd, env):
                launches.append((args, cwd, Path(env["CARGO_TARGET_DIR"])))
                return subprocess.CompletedProcess(args, 0)

            for name in ("source-a", "source-b"):
                root = base / name
                crate = root / "tools" / "gauntlet" / "census"
                crate.mkdir(parents=True)
                (crate / "Cargo.toml").write_text("")
                with patch.object(census, "REPO", root), patch.object(census, "CRATE", crate), \
                        patch.object(census, "have", return_value=True), \
                        patch.object(census, "sh", side_effect=command), \
                        patch.dict(os.environ, {"CARGO_TARGET_DIR": str(base / "foreign-target")}, clear=True):
                    result = census.binary(shared_ledger, run_tests=False)
                self.assertEqual(result, root / "target" / "release" / "dclutch-route-census")

            self.assertEqual(len(launches), 2)
            self.assertNotEqual(launches[0][2], launches[1][2])
            for args, cwd, target in launches:
                self.assertEqual(target, cwd / "target")
                self.assertIn("--locked", args)
                self.assertEqual(args[args.index("-p") + 1], "dclutch-route-census")


if __name__ == "__main__":
    unittest.main()
