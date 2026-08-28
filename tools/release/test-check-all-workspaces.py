#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import pathlib
import sys
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).with_name("check-all-workspaces.py")
SPEC = importlib.util.spec_from_file_location("check_all_workspaces", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class WorkspaceDiscoveryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="dclutch-all-workspaces-test.")
        self.root = pathlib.Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write(self, relative: str, contents: str) -> None:
        target = self.root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(contents)

    def test_discovers_every_workspace_root_in_bytewise_path_order(self) -> None:
        self.write("Cargo.toml", "[workspace]\nmembers = []\n")
        self.write("Cargo.lock", "version = 4\n")
        self.write("z/Cargo.toml", "[workspace]\nmembers = []\n")
        self.write("z/Cargo.lock", "version = 4\n")
        self.write("a/Cargo.toml", "[workspace]\nmembers = []\n")
        self.write("a/Cargo.lock", "version = 4\n")
        self.write("member/Cargo.toml", "[package]\nname = 'member'\nversion = '0.1.0'\n")
        self.assertEqual(
            MODULE.discover_workspaces(self.root),
            [
                MODULE.Workspace("Cargo.toml", "Cargo.lock"),
                MODULE.Workspace("a/Cargo.toml", "a/Cargo.lock"),
                MODULE.Workspace("z/Cargo.toml", "z/Cargo.lock"),
            ],
        )

    def test_workspace_without_adjacent_lock_refuses(self) -> None:
        self.write("Cargo.toml", "[workspace]\nmembers = []\n")
        with self.assertRaisesRegex(ValueError, "workspace lock"):
            MODULE.discover_workspaces(self.root)

    def test_lock_manifest_covers_member_strays_as_well_as_workspace_locks(self) -> None:
        self.write("Cargo.lock", "root\n")
        self.write("member/Cargo.lock", "stray\n")
        rows = MODULE.lock_rows(self.root)
        self.assertEqual(
            [path for path, _digest in rows], ["Cargo.lock", "member/Cargo.lock"]
        )

    def test_symlink_lock_refuses(self) -> None:
        self.write("real.lock", "version = 4\n")
        (self.root / "Cargo.lock").symlink_to(self.root / "real.lock")
        with self.assertRaisesRegex(ValueError, "not one regular file"):
            MODULE.lock_rows(self.root)


if __name__ == "__main__":
    unittest.main()
