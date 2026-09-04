#!/usr/bin/env python3

from __future__ import annotations

import argparse
import contextlib
import io
import json
from pathlib import Path
import shutil
import tempfile
import unittest

from tools.release import artifact_provenance as MODULE


RUN = "ab" * 32
SOURCE = "0123456789abcdef0123456789abcdef01234567"
TREE = "cd" * 32
BUILD_INVOCATION = (
    "CARGO_TERM_COLOR=never CARGO_TARGET_DIR=build-target cargo build-sbf "
    "--manifest-path programs/dclutch-trading-sbf/Cargo.toml -- --locked"
)
FRAME_INVOCATION = (
    "RUSTC_BOOTSTRAP=1 RUSTFLAGS='-Zemit-stack-sizes --emit=obj,link' "
    "CARGO_TERM_COLOR=never CARGO_TARGET_DIR=frame-target-trading cargo build-sbf "
    "--manifest-path programs/dclutch-trading-sbf/Cargo.toml -- --locked"
)
MARKER = (
    "   Compiling dclutch-trading-sbf v0.1.0 (/source/programs/dclutch-trading-sbf)"
)


def write(path: Path, value: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(value)


def create_fixture(
    root: Path,
    label: str = "trading",
    package: str = "dclutch-trading-sbf",
    produces_artifact: bool = True,
    *,
    source: str = SOURCE,
    tree: str = TREE,
    run: str = RUN,
    marker_root: str = "/source",
    elf_salt: bytes = b"",
) -> Path:
    build_invocation = BUILD_INVOCATION.replace("trading", label).replace(
        "dclutch-trading-sbf", package
    )
    frame_invocation = FRAME_INVOCATION.replace("trading", label).replace(
        "dclutch-trading-sbf", package
    )
    marker = MARKER.replace("dclutch-trading-sbf", package).replace(
        "(/source/", f"({marker_root}/"
    )
    write(
        root / f"build-{label}.log",
        (
            f"dclutch-sbf-build-run-v1={run}\n"
            f"dclutch-sbf-build-invocation-v1={build_invocation}\n"
            f"{marker}\nFinished\n"
        ).encode(),
    )
    write(
        root / f"frame-build-{label}.log",
        (
            f"dclutch-sbf-frame-run-v1={run}\n"
            f"dclutch-sbf-frame-invocation-v1={frame_invocation}\n"
            f"{marker}\nFinished\n"
        ).encode(),
    )
    frame_object = (
        root
        / f"frame-target-{label}/sbpf-solana-solana/release/deps/{package.replace('-', '_')}.o"
    )
    write(frame_object, b"measured-object")
    write(
        root / f"frame/{label}.txt",
        (
            "dclutch-sbf-frame-report-v1\n"
            f"label={label}\n"
            f"package={package}\n"
            f"source_tree_sha256={tree}\n"
            f"build_run_id={run}\n"
            "frame_count=3\n"
            "frame_bound_bytes=4096\n"
            "frames_at_or_over_bound=0\n"
            "deepest_frame_bytes=2048\n"
            f"object_sha256={MODULE.sha256_file(frame_object)}\n"
            "measurement_output:\n"
            "  3 measured frames, bound 4096; deepest:\n"
        ).encode(),
    )
    if produces_artifact:
        write(root / f"elf/{label}.so", b"\x7fELF" + label.encode() * 8 + elf_salt)
    (root / "provenance").mkdir(exist_ok=True)
    output = root / f"provenance/{label}.json"
    MODULE.emit(
        argparse.Namespace(
            root=str(root),
            output=str(output),
            label=label,
            package=package,
            artifact_stem=package.replace("-", "_") if produces_artifact else None,
            source_revision=source,
            source_tree_sha256=tree,
            build_run_id=run,
            build_invocation=build_invocation,
            build_log=f"build-{label}.log",
            build_compile_marker=marker,
            diagnostics_count=0,
            frame_invocation=frame_invocation,
            frame_build_log=f"frame-build-{label}.log",
            frame_compile_marker=marker,
            frame_object=str(frame_object.relative_to(root)),
            frame_report=f"frame/{label}.txt",
            elf=f"elf/{label}.so" if produces_artifact else None,
        )
    )
    return output


def file_evidence(path: Path, root: Path) -> dict[str, object]:
    return {
        "canonical_path": str(path.relative_to(root)),
        "bytes": path.stat().st_size,
        "sha256": MODULE.sha256_file(path),
    }


def gate(root: Path, provenance: Path) -> tuple[Path, str]:
    links: list[dict[str, object]] = []
    for label, package, produces_artifact in MODULE.SHIPPED_LINKS:
        canonical = root / f"provenance/{label}.json"
        if label == "trading":
            descriptor_path = provenance
        elif canonical.exists():
            descriptor_path = canonical
        else:
            descriptor_path = create_fixture(root, label, package, produces_artifact)
        descriptor = json.loads(descriptor_path.read_text())
        links.append(
            {
                "label": label,
                "package": package,
                "sbf_diagnostics_count": 0,
                "frame_bound_bytes": 4096,
                "frames_at_or_over_bound": 0,
                "elf": descriptor["shipped_elf"],
                "artifact_provenance": file_evidence(descriptor_path, root),
            }
        )
    value = {
        "schema": MODULE.GATE_SCHEMA,
        "source_revision": SOURCE,
        "source_tree_sha256": TREE,
        "build_run_id": RUN,
        "link_count": len(MODULE.SHIPPED_LINKS),
        "links": links,
    }
    path = root / "CHECKED_UPGRADE_GATE.json"
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    return path, MODULE.sha256_file(path)


def candidate_root(
    root: Path,
    *,
    run: str = RUN,
    marker_root: str = "/source",
    elf_salt: bytes = b"",
) -> tuple[str, str]:
    """One complete candidate work root, emitted exactly as the runner does."""

    source_tree_bytes = b"canonical source tree manifest\n"
    tree = MODULE.sha256_bytes(source_tree_bytes)
    write(root / "source-tree.txt", source_tree_bytes)
    write(
        root / "build-links.tsv",
        "".join(
            f"{label}\t{package}\n" for label, package, _ in MODULE.SHIPPED_LINKS
        ).encode(),
    )
    write(root / "build-run.txt", f"dclutch-sbf-build-run-v1={run}\n".encode())
    write(
        root / "build-diagnostics.txt",
        "".join(f"{label}=0\n" for label, _, _ in MODULE.SHIPPED_LINKS).encode(),
    )
    for label, package, produces_artifact in MODULE.SHIPPED_LINKS:
        create_fixture(
            root, label, package, produces_artifact, tree=tree, run=run,
            marker_root=marker_root,
            elf_salt=elf_salt if label == "trading" else b"",
        )
        if produces_artifact:
            write(root / f"evidence/{label}/checked.bin", label.encode())
    MODULE.emit_gate(
        argparse.Namespace(
            root=str(root),
            source_revision=SOURCE,
            source_tree_sha256=tree,
            solana_cli_version="solana-cli 4.0.2 (fixture)",
            build_run_id=run,
        )
    )
    return (
        MODULE.sha256_file(root / MODULE.REPRODUCIBLE_GATE_NAME),
        MODULE.sha256_file(root / "CHECKED_UPGRADE_GATE.json"),
    )


class ReproducibleGateTests(unittest.TestCase):
    def test_two_roots_at_one_commit_produce_one_reproducible_gate(self) -> None:
        """The whole defect: the certified bytes reproduced, the evidence did not.

        Two runs of one commit differ in their build-run nonce, in every log
        and report that stamps it, and in the absolute work root their compile
        markers name.  The reproducible gate must not see any of that; the
        run-bound gate must, or it would not be recording the run.
        """

        with tempfile.TemporaryDirectory() as first_text, tempfile.TemporaryDirectory() as second_text:
            first = Path(first_text).resolve()
            second = Path(second_text).resolve()
            first_gate, first_run_gate = candidate_root(first)
            second_gate, second_run_gate = candidate_root(
                second, run="cd" * 32, marker_root="/another/work/root/source"
            )
            self.assertEqual(first_gate, second_gate)
            self.assertEqual(
                (first / MODULE.REPRODUCIBLE_GATE_NAME).read_bytes(),
                (second / MODULE.REPRODUCIBLE_GATE_NAME).read_bytes(),
            )
            self.assertEqual(
                (first / MODULE.GATE_DIGEST_NAME).read_bytes(),
                (second / MODULE.GATE_DIGEST_NAME).read_bytes(),
            )
            self.assertNotEqual(first_run_gate, second_run_gate)
            for root, gate_digest in ((first, first_gate), (second, second_gate)):
                selected = MODULE.select_reproducible_role(root, gate_digest, "trading")
                self.assertEqual(selected["elf_path"], str(root / "elf/trading.so"))

    def test_the_gate_names_no_per_run_file_and_the_record_names_them_all(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            gate_digest, _ = candidate_root(root)
            gate = json.loads((root / MODULE.REPRODUCIBLE_GATE_NAME).read_text())
            named = json.dumps(gate)
            for per_run in ("build-run.txt", "build-core.log", "frame-build-core.log",
                            "frame/core.txt", "provenance/core.json", "build_run_id"):
                self.assertNotIn(per_run, named)
            for reproducible in ("source-tree.txt", "build-links.tsv",
                                 "build-diagnostics.txt", "elf/core.so",
                                 "evidence/core/checked.bin"):
                self.assertIn(reproducible, named)
            record = json.loads((root / MODULE.RUN_RECORD_NAME).read_text())
            self.assertEqual(record["schema"], MODULE.RUN_RECORD_SCHEMA)
            self.assertEqual(record["reproducible_gate"]["sha256"], gate_digest)
            self.assertEqual(
                record["build_run_manifest"]["canonical_path"], "build-run.txt"
            )
            self.assertEqual(
                {link["label"] for link in record["links"]},
                {label for label, _, _ in MODULE.SHIPPED_LINKS},
            )
            self.assertEqual(
                (root / MODULE.GATE_DIGEST_NAME).read_bytes(),
                f"{gate_digest}  {MODULE.REPRODUCIBLE_GATE_NAME}\n".encode(),
            )

    def test_one_byte_of_one_shipped_artifact_moves_the_gate(self) -> None:
        for target in ("elf/trading.so", "evidence/trading/checked.bin",
                       "source-tree.txt", "build-diagnostics.txt"):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as root_text:
                root = Path(root_text).resolve()
                gate_digest, _ = candidate_root(root)
                MODULE.verify_reproducible_gate(root, gate_digest)
                with (root / target).open("ab") as output:
                    output.write(b"\x00")
                with self.assertRaisesRegex(
                    MODULE.Refusal, "SHA-256 differs|byte count differs|malformed"
                ):
                    MODULE.verify_reproducible_gate(root, gate_digest)

    def test_a_candidate_from_another_commit_refuses_by_the_same_name(self) -> None:
        """The hostile: a rebuild whose ELF is not the admitted one.

        This is what a cohort re-admitting from a fresh candidate must fail,
        and it must fail on the artifact digest -- not on a nonce, and not on
        whether the original scratch still exists.
        """

        with tempfile.TemporaryDirectory() as first_text, tempfile.TemporaryDirectory() as second_text:
            first = Path(first_text).resolve()
            second = Path(second_text).resolve()
            admitted, _ = candidate_root(first)
            rebuilt, _ = candidate_root(second, run="ef" * 32, elf_salt=b"!")
            self.assertNotEqual(admitted, rebuilt)
            with self.assertRaisesRegex(
                MODULE.Refusal, "reproducible gate SHA-256 differs"
            ):
                MODULE.verify_reproducible_gate(second, admitted)
            # And the after-the-fact tamper, which keeps the gate file intact
            # and changes the artifact underneath it.
            (first / "elf/trading.so").write_bytes(b"\x7fELF" + b"trading" * 8 + b"!")
            with self.assertRaisesRegex(
                MODULE.Refusal, "trading ELF (byte count|SHA-256) differs"
            ):
                MODULE.verify_reproducible_gate(first, admitted)

    def test_the_reproducible_gate_survives_losing_every_per_run_file(self) -> None:
        """A cohort whose build scratch is gone re-admits from the bytes alone."""

        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            gate_digest, _ = candidate_root(root)
            shutil.rmtree(root / "provenance")
            shutil.rmtree(root / "frame")
            shutil.rmtree(root / "run")
            (root / "build-run.txt").unlink()
            (root / "CHECKED_UPGRADE_GATE.json").unlink()
            for log in list(root.glob("build-*.log")) + list(
                root.glob("frame-build-*.log")
            ):
                log.unlink()
            selected = MODULE.select_reproducible_role(root, gate_digest, "custody")
            self.assertEqual(selected["elf_path"], str(root / "elf/custody.so"))
            self.assertEqual(
                selected["checked_manifest_path"],
                str(root / "evidence/custody/checked.bin"),
            )


class ArtifactProvenanceTests(unittest.TestCase):
    def test_reusable_gate_emitter_preserves_the_canonical_all_link_schema(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            source_tree_bytes = b"canonical source tree manifest\n"
            tree = MODULE.sha256_bytes(source_tree_bytes)
            write(root / "source-tree.txt", source_tree_bytes)
            write(
                root / "build-links.tsv",
                "".join(
                    f"{label}\t{package}\n"
                    for label, package, _produces in MODULE.SHIPPED_LINKS
                ).encode(),
            )
            write(root / "build-run.txt", f"dclutch-sbf-build-run-v1={RUN}\n".encode())
            write(
                root / "build-diagnostics.txt",
                "".join(
                    f"{label}=0\n" for label, _package, _produces in MODULE.SHIPPED_LINKS
                ).encode(),
            )
            for label, package, produces_artifact in MODULE.SHIPPED_LINKS:
                create_fixture(
                    root,
                    label,
                    package,
                    produces_artifact,
                    tree=tree,
                )
                if produces_artifact:
                    write(root / f"evidence/{label}/checked.bin", label.encode())

            MODULE.emit_gate(
                argparse.Namespace(
                    root=str(root),
                    source_revision=SOURCE,
                    source_tree_sha256=tree,
                    solana_cli_version="solana-cli 4.0.2 (fixture)",
                    build_run_id=RUN,
                )
            )
            gate_path = root / "CHECKED_UPGRADE_GATE.json"
            value = json.loads(gate_path.read_text())
            self.assertEqual(gate_path.read_bytes(), MODULE.canonical_json(value))
            self.assertEqual(
                set(value),
                {
                    "schema",
                    "source_revision",
                    "source_tree_sha256",
                    "solana_cli_version",
                    "build_run_id",
                    "link_count",
                    "source_tree_manifest",
                    "build_links_manifest",
                    "build_run_manifest",
                    "diagnostics_manifest",
                    "links",
                },
            )
            self.assertEqual(
                [(link["label"], link["package"]) for link in value["links"]],
                [(label, package) for label, package, _ in MODULE.SHIPPED_LINKS],
            )
            selected = MODULE.select_gate_role(
                gate_path, MODULE.sha256_file(gate_path), "trading"
            )
            self.assertEqual(selected["elf_path"], str(root / "elf/trading.so"))
            with self.assertRaisesRegex(MODULE.Refusal, "already exists"):
                MODULE.emit_gate(
                    argparse.Namespace(
                        root=str(root),
                        source_revision=SOURCE,
                        source_tree_sha256=tree,
                        solana_cli_version="solana-cli 4.0.2 (fixture)",
                        build_run_id=RUN,
                    )
                )

    def test_exact_descriptor_and_gate_role_select(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            descriptor = create_fixture(root)
            verified = MODULE.verify_descriptor(
                root,
                descriptor,
                expected_label="trading",
                expected_package="dclutch-trading-sbf",
                expected_source_revision=SOURCE,
                expected_source_tree_sha256=TREE,
                expected_build_run_id=RUN,
            )
            self.assertEqual(verified["elf_path"], root / "elf/trading.so")
            gate_path, gate_sha = gate(root, descriptor)
            selected = MODULE.select_gate_role(gate_path, gate_sha, "trading")
            self.assertEqual(selected["elf_path"], str(root / "elf/trading.so"))
            self.assertEqual(
                selected["elf_sha256"], MODULE.sha256_file(root / "elf/trading.so")
            )

    def test_stale_log_frame_object_and_elf_refuse(self) -> None:
        targets = (
            "build-trading.log",
            "frame-target-trading/sbpf-solana-solana/release/deps/dclutch_trading_sbf.o",
            "elf/trading.so",
        )
        for target in targets:
            with self.subTest(
                target=target
            ), tempfile.TemporaryDirectory() as root_text:
                root = Path(root_text).resolve()
                descriptor = create_fixture(root)
                with (root / target).open("ab") as output:
                    output.write(b"stale")
                with self.assertRaisesRegex(MODULE.Refusal, "SHA-256|byte count"):
                    MODULE.verify_descriptor(root, descriptor)

    def test_renamed_adjacent_and_symlink_elf_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            descriptor = create_fixture(root)
            adjacent = root / "elf/renamed-trading.so"
            shutil.copyfile(root / "elf/trading.so", adjacent)
            verified = MODULE.verify_descriptor(root, descriptor)
            self.assertNotEqual(adjacent, verified["elf_path"])
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(
                io.StringIO()
            ):
                status = MODULE.main(
                    [
                        "verify",
                        "--root",
                        str(root),
                        "--descriptor",
                        str(descriptor),
                        "--label",
                        "trading",
                        "--elf",
                        str(adjacent),
                    ]
                )
            self.assertEqual(status, 1)
            original = root / "elf/trading.so"
            moved = root / "moved.so"
            original.rename(moved)
            original.symlink_to(moved)
            with self.assertRaisesRegex(MODULE.Refusal, "non-symlink"):
                MODULE.verify_descriptor(root, descriptor)

    def test_wrong_role_descriptor_and_source_digest_refuse_at_gate(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            trading = create_fixture(root)
            resolution = create_fixture(
                root, "resolution", "dclutch-resolution-proof-sbf"
            )
            gate_path, _ = gate(root, trading)
            value = json.loads(gate_path.read_text())
            value["links"][0]["artifact_provenance"] = file_evidence(resolution, root)
            gate_path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
            with self.assertRaisesRegex(MODULE.Refusal, "label differs"):
                MODULE.select_gate_role(
                    gate_path, MODULE.sha256_file(gate_path), "trading"
                )

            gate_path.unlink()
            gate_path, _ = gate(root, trading)
            value = json.loads(gate_path.read_text())
            value["source_tree_sha256"] = "ef" * 32
            gate_path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
            with self.assertRaisesRegex(MODULE.Refusal, "source tree SHA-256 differs"):
                MODULE.select_gate_role(
                    gate_path, MODULE.sha256_file(gate_path), "trading"
                )

    def test_gate_must_retain_exact_canonical_shipped_link_order(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            descriptor = create_fixture(root)
            gate_path, _ = gate(root, descriptor)
            value = json.loads(gate_path.read_text())
            value["links"][0], value["links"][1] = value["links"][1], value["links"][0]
            gate_path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
            with self.assertRaisesRegex(MODULE.Refusal, "link order/identity"):
                MODULE.select_gate_role(
                    gate_path, MODULE.sha256_file(gate_path), "trading"
                )

    def test_invocation_stamp_and_frame_source_are_load_bearing(self) -> None:
        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            descriptor = create_fixture(root)
            log = root / "build-trading.log"
            body = log.read_text().replace(BUILD_INVOCATION, "adjacent build")
            log.write_text(body)
            value = json.loads(descriptor.read_text())
            value["plain_build"]["log"] = file_evidence(log, root)
            descriptor.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
            with self.assertRaisesRegex(MODULE.Refusal, "invocation stamp differs"):
                MODULE.verify_descriptor(root, descriptor)

        with tempfile.TemporaryDirectory() as root_text:
            root = Path(root_text).resolve()
            descriptor = create_fixture(root)
            report = root / "frame/trading.txt"
            report.write_text(report.read_text().replace(TREE, "ef" * 32))
            value = json.loads(descriptor.read_text())
            value["frame_measurement"]["report"] = file_evidence(report, root)
            descriptor.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
            with self.assertRaisesRegex(MODULE.Refusal, "source/object"):
                MODULE.verify_descriptor(root, descriptor)


if __name__ == "__main__":
    unittest.main()
