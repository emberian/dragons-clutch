from __future__ import annotations

import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("node_archive_members.py")
SPEC = importlib.util.spec_from_file_location("node_archive_members", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
members = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(members)


class NodeArchiveMembersTests(unittest.TestCase):
    def archive(self, root: Path, names: list[str]) -> Path:
        path = root / "node.tar.xz"
        with tarfile.open(path, "w:xz") as output:
            for name in names:
                info = tarfile.TarInfo(name)
                body = name.encode()
                info.size = len(body)
                output.addfile(info, io.BytesIO(body))
        return path

    def symlink_archive(self, root: Path, name: str) -> Path:
        path = root / "node.tar.xz"
        with tarfile.open(path, "w:xz") as output:
            info = tarfile.TarInfo(name)
            info.type = tarfile.SYMTYPE
            info.linkname = "not-node"
            output.addfile(info)
        return path

    def test_large_valid_archive_materializes_one_bounded_listing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            required = ("node-v26/bin/node", "node-v26/lib/npm-cli.js")
            names = [f"node-v26/lib/file-{index:05d}" for index in range(6_000)]
            archive = self.archive(root, [*required, *names])
            result = members.listing(archive, required)
            self.assertLessEqual(len(result), members.MAX_LISTING_BYTES)
            self.assertEqual(result.count(b"node-v26/bin/node\n"), 1)
            self.assertEqual(len(result.splitlines()), len(names) + len(required))

    def test_missing_required_member_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            archive = self.archive(Path(directory), ["node-v26/bin/not-node"])
            with self.assertRaisesRegex(members.Refusal, "omitted required member"):
                members.listing(archive, ("node-v26/bin/node",))

    def test_duplicate_member_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            archive = self.archive(
                Path(directory), ["node-v26/bin/node", "node-v26/bin/node"]
            )
            with self.assertRaisesRegex(members.Refusal, "repeats member"):
                members.listing(archive, ("node-v26/bin/node",))

    def test_path_substitution_refuses(self) -> None:
        hostile = (
            "/node-v26/bin/node",
            "node-v26/bin/../bin/node",
            "node-v26//bin/node",
            "./node-v26/bin/node",
            "node-v26\\bin\\node",
        )
        for index, name in enumerate(hostile):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                archive = self.archive(Path(directory), [name])
                with self.assertRaises(members.Refusal):
                    members.listing(archive, ())

    def test_required_symlink_substitution_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            archive = self.symlink_archive(Path(directory), "node-v26/bin/node")
            with self.assertRaisesRegex(members.Refusal, "is not a regular file"):
                members.listing(archive, ("node-v26/bin/node",))


if __name__ == "__main__":
    unittest.main()
