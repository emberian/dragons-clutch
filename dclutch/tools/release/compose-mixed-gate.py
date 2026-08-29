#!/usr/bin/env python3
"""Compose one checked gate from an authenticated two-revision SBF batch plan."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import sys

from artifact_provenance import (
    SHIPPED_LINKS,
    Refusal,
    atomic_new,
    canonical_json,
    evidence,
    frame_fields,
    read_json,
    sha256_file,
    verify_evidence,
    verify_descriptor,
)


SCHEMA = "dclutch-checked-upgrade-gate-v2"
PLAN_SCHEMA = "dclutch-sbf-release-batch-plan-v1"


def refuse(message: str) -> None:
    raise Refusal(message)


def copy_exact(source_root: Path, output_root: Path, relative: str) -> None:
    source = source_root / relative
    if source.is_symlink() or not source.is_file() or source.resolve() != source:
        refuse(f"source evidence is not one canonical regular file: {source}")
    target = output_root / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists() or target.is_symlink():
        if target.is_symlink() or not target.is_file() or target.read_bytes() != source.read_bytes():
            refuse(f"mixed gate evidence collision differs: {relative}")
        return
    shutil.copyfile(source, target)


def copy_descriptor_closure(
    source_root: Path, output_root: Path, descriptor_source: Path, label: str
) -> None:
    descriptor = read_json(descriptor_source, f"{label} descriptor")
    relative_descriptor = f"provenance/{label}.json"
    target = output_root / relative_descriptor
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists() or target.is_symlink():
        refuse(f"descriptor target already exists: {target}")
    shutil.copyfile(descriptor_source, target)
    refs = [
        descriptor["plain_build"]["log"],
        descriptor["frame_measurement"]["build_log"],
        descriptor["frame_measurement"]["object"],
        descriptor["frame_measurement"]["report"],
    ]
    if descriptor["shipped_elf"] is not None:
        refs.append(descriptor["shipped_elf"])
    for ref in refs:
        copy_exact(source_root, output_root, ref["canonical_path"])


def load_plan(path: Path, rebuilt: str) -> dict:
    plan = read_json(path, "carry-forward plan")
    expected = {
        "schema", "base_revision", "base_source_tree_sha256",
        "candidate_revision", "candidate_source_tree_sha256", "link_count",
        "changed_link_count", "links", "qualification",
    }
    if set(plan) != expected or plan["schema"] != PLAN_SCHEMA:
        refuse("carry-forward plan schema or fields differ")
    if plan["link_count"] != len(SHIPPED_LINKS) or plan["changed_link_count"] != 1:
        refuse("carry-forward plan is not exact all-13 with one rebuilt link")
    identities = [(row.get("label"), row.get("package")) for row in plan["links"]]
    if identities != [(label, package) for label, package, _ in SHIPPED_LINKS]:
        refuse("carry-forward plan link order/identity differs")
    changed = []
    for row in plan["links"]:
        required = {
            "label", "package", "artifact_stem", "base_input_digest",
            "candidate_input_digest", "requires_new_artifact", "changed_inputs",
            "consumers",
        }
        if set(row) != required:
            refuse(f"carry-forward plan row fields differ for {row.get('label')}")
        if row["requires_new_artifact"]:
            changed.append(row["label"])
            if not row["changed_inputs"] or row["base_input_digest"] == row["candidate_input_digest"]:
                refuse("rebuilt row lacks an exact changed closure")
        elif row["changed_inputs"] or row["base_input_digest"] != row["candidate_input_digest"]:
            refuse(f"carry-forward closure differs for {row['label']}")
    if changed != [rebuilt]:
        refuse(f"carry-forward plan rebuilt set is {changed}, expected [{rebuilt!r}]")
    return plan


def compose(args: argparse.Namespace) -> Path:
    source_root = Path(args.source_root).resolve(strict=True)
    manifest_root = Path(args.manifest_root).resolve(strict=True)
    plan_source = Path(args.carry_forward_plan).resolve(strict=True)
    output_root = Path(args.output_root)
    if output_root.exists() or output_root.is_symlink() or not output_root.is_absolute():
        refuse("output root must be one new absolute path")
    plan = load_plan(plan_source, args.rebuilt_link)
    output_root.mkdir(mode=0o700, parents=False)
    (output_root / "provenance").mkdir()

    base_descriptors = Path(args.base_descriptor_dir).resolve(strict=True)
    candidate_descriptors = Path(args.candidate_descriptor_dir).resolve(strict=True)
    for label, _, _ in SHIPPED_LINKS:
        directory = candidate_descriptors if label == args.rebuilt_link else base_descriptors
        copy_descriptor_closure(source_root, output_root, directory / f"{label}.json", label)

    shutil.copyfile(plan_source, output_root / "carry-forward-plan.json")
    copy_exact(source_root, output_root, args.base_source_tree_manifest)
    copy_exact(source_root, output_root, args.candidate_source_tree_manifest)
    for label, _, produces_artifact in SHIPPED_LINKS:
        if produces_artifact:
            source = manifest_root / "evidence" / label / "checked.bin"
            target = output_root / "evidence" / label / "checked.bin"
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)

    build_links = "".join(f"{label}\t{package}\n" for label, package, _ in SHIPPED_LINKS)
    diagnostics = "".join(f"{label}=0\n" for label, _, _ in SHIPPED_LINKS)
    (output_root / "build-links.tsv").write_text(build_links)
    (output_root / "build-diagnostics.txt").write_text(diagnostics)

    cohort_values = {}
    links = []
    for label, package, produces_artifact in SHIPPED_LINKS:
        descriptor_path = output_root / "provenance" / f"{label}.json"
        descriptor = read_json(descriptor_path, f"{label} descriptor")
        disposition = "rebuilt" if label == args.rebuilt_link else "carry-forward"
        cohort = "candidate" if disposition == "rebuilt" else "base"
        expected_revision = plan[f"{cohort}_revision"]
        expected_tree = plan[f"{cohort}_source_tree_sha256"]
        verified = verify_descriptor(
            output_root,
            descriptor_path,
            expected_label=label,
            expected_package=package,
            expected_source_revision=expected_revision,
            expected_source_tree_sha256=expected_tree,
            expected_build_run_id=descriptor["build_run_id"],
        )
        cohort_values.setdefault(cohort, descriptor["build_run_id"])
        if cohort_values[cohort] != descriptor["build_run_id"]:
            refuse(f"{cohort} cohort contains more than one build run")
        report = frame_fields(output_root / descriptor["frame_measurement"]["report"]["canonical_path"])
        link = {
            "label": label,
            "package": package,
            "disposition": disposition,
            "cohort": cohort,
            "build_log": descriptor["plain_build"]["log"],
            "compile_marker": descriptor["plain_build"]["compile_marker"],
            "sbf_diagnostics_count": 0,
            "frame_build_log": descriptor["frame_measurement"]["build_log"],
            "frame_compile_marker": descriptor["frame_measurement"]["compile_marker"],
            "frame_report": descriptor["frame_measurement"]["report"],
            "artifact_provenance": evidence(output_root, f"provenance/{label}.json", f"{label} provenance"),
            "frame_count": int(report["frame_count"]),
            "frame_bound_bytes": int(report["frame_bound_bytes"]),
            "frames_at_or_over_bound": int(report["frames_at_or_over_bound"]),
            "deepest_frame_bytes": int(report["deepest_frame_bytes"]),
            "elf": descriptor["shipped_elf"],
            "checked_manifest": evidence(output_root, f"evidence/{label}/checked.bin", f"{label} manifest") if produces_artifact else None,
        }
        if verified["elf_path"] is None and produces_artifact:
            refuse(f"{label} omitted its shipped ELF")
        links.append(link)

    cohorts = []
    for name in ("base", "candidate"):
        manifest_relative = getattr(args, f"{name}_source_tree_manifest")
        cohorts.append({
            "name": name,
            "source_revision": plan[f"{name}_revision"],
            "source_tree_sha256": plan[f"{name}_source_tree_sha256"],
            "build_run_id": cohort_values[name],
            "source_tree_manifest": evidence(output_root, manifest_relative, f"{name} source tree"),
        })
    gate = {
        "schema": SCHEMA,
        "source_revision": plan["candidate_revision"],
        "source_tree_sha256": plan["candidate_source_tree_sha256"],
        "solana_cli_version": args.solana_cli_version,
        "link_count": len(links),
        "source_tree_manifest": cohorts[1]["source_tree_manifest"],
        "build_links_manifest": evidence(output_root, "build-links.tsv", "build links"),
        "diagnostics_manifest": evidence(output_root, "build-diagnostics.txt", "diagnostics"),
        "carry_forward_plan": evidence(output_root, "carry-forward-plan.json", "carry-forward plan"),
        "cohorts": cohorts,
        "links": links,
    }
    target = output_root / "CHECKED_UPGRADE_GATE.json"
    atomic_new(target, gate)
    print(f"checked mixed Upgrade gate sha256={sha256_file(target)}")
    return target


def authenticate_existing_gate(
    gate_path: Path,
    expected_gate_sha256: str,
    expected_source_revision: str,
    expected_source_tree_sha256: str,
    selected_link: str,
) -> dict:
    """Reauthenticate a mixed gate and return one canonical selected row.

    This is the sole Python admission API for consumers such as the private
    lifecycle driver.  It replays the carry-forward closure proof and every
    artifact/manfiest reference; callers consume the returned projection and
    do not parse v2 independently.
    """
    gate_path = gate_path.resolve(strict=True)
    root = gate_path.parent
    if gate_path.is_symlink() or gate_path.name != "CHECKED_UPGRADE_GATE.json":
        refuse("mixed gate path is not one canonical regular gate")
    if sha256_file(gate_path) != expected_gate_sha256:
        refuse("mixed gate SHA-256 differs from the expected release pin")
    gate = read_json(gate_path, "mixed checked gate")
    exact_gate_fields = {
        "schema", "source_revision", "source_tree_sha256", "solana_cli_version",
        "link_count", "source_tree_manifest", "build_links_manifest",
        "diagnostics_manifest", "carry_forward_plan", "cohorts", "links",
    }
    if set(gate) != exact_gate_fields or gate.get("schema") != SCHEMA:
        refuse("mixed gate schema or fields differ")
    if (
        gate["source_revision"] != expected_source_revision
        or gate["source_tree_sha256"] != expected_source_tree_sha256
    ):
        refuse("mixed gate source revision/tree differs from the expected release pin")
    for field in (
        "source_tree_manifest", "build_links_manifest", "diagnostics_manifest",
        "carry_forward_plan",
    ):
        verify_evidence(root, gate[field], f"mixed gate {field}")
    plan = read_json(
        verify_evidence(root, gate["carry_forward_plan"], "mixed carry-forward plan"),
        "mixed carry-forward plan",
    )
    rebuilt = [row.get("label") for row in plan.get("links", []) if row.get("requires_new_artifact")]
    if len(rebuilt) != 1:
        refuse("mixed carry-forward plan does not select exactly one rebuilt link")
    plan = load_plan(root / gate["carry_forward_plan"]["canonical_path"], rebuilt[0])
    if (
        plan["candidate_revision"] != expected_source_revision
        or plan["candidate_source_tree_sha256"] != expected_source_tree_sha256
    ):
        refuse("carry-forward plan candidate differs from the mixed gate")
    cohorts = gate["cohorts"]
    if not isinstance(cohorts, list) or [row.get("name") for row in cohorts] != ["base", "candidate"]:
        refuse("mixed gate cohorts are not canonical base then candidate")
    cohort_map = {}
    for cohort in cohorts:
        if set(cohort) != {
            "name", "source_revision", "source_tree_sha256", "build_run_id",
            "source_tree_manifest",
        }:
            refuse("mixed gate cohort fields differ")
        name = cohort["name"]
        if (
            cohort["source_revision"] != plan[f"{name}_revision"]
            or cohort["source_tree_sha256"] != plan[f"{name}_source_tree_sha256"]
        ):
            refuse(f"mixed gate {name} cohort differs from the carry-forward plan")
        verify_evidence(root, cohort["source_tree_manifest"], f"{name} source tree")
        cohort_map[name] = cohort
    if gate["source_tree_manifest"] != cohort_map["candidate"]["source_tree_manifest"]:
        refuse("mixed gate top-level source tree does not equal the candidate cohort source tree")

    links = gate["links"]
    identities = [(row.get("label"), row.get("package")) for row in links]
    if gate["link_count"] != len(SHIPPED_LINKS) or identities != [
        (label, package) for label, package, _ in SHIPPED_LINKS
    ]:
        refuse("mixed gate link order/identity is not canonical all-13")
    selected = None
    for row, (label, package, produces_artifact), plan_row in zip(links, SHIPPED_LINKS, plan["links"]):
        disposition = "rebuilt" if plan_row["requires_new_artifact"] else "carry-forward"
        cohort_name = "candidate" if disposition == "rebuilt" else "base"
        cohort = cohort_map[cohort_name]
        provenance_path = verify_evidence(root, row.get("artifact_provenance"), f"{label} provenance")
        verified = verify_descriptor(
            root,
            provenance_path,
            expected_label=label,
            expected_package=package,
            expected_source_revision=cohort["source_revision"],
            expected_source_tree_sha256=cohort["source_tree_sha256"],
            expected_build_run_id=cohort["build_run_id"],
        )
        descriptor = verified["descriptor"]
        report = frame_fields(root / descriptor["frame_measurement"]["report"]["canonical_path"])
        expected_row = {
            "label": label,
            "package": package,
            "disposition": disposition,
            "cohort": cohort_name,
            "build_log": descriptor["plain_build"]["log"],
            "compile_marker": descriptor["plain_build"]["compile_marker"],
            "sbf_diagnostics_count": 0,
            "frame_build_log": descriptor["frame_measurement"]["build_log"],
            "frame_compile_marker": descriptor["frame_measurement"]["compile_marker"],
            "frame_report": descriptor["frame_measurement"]["report"],
            "artifact_provenance": row["artifact_provenance"],
            "frame_count": int(report["frame_count"]),
            "frame_bound_bytes": int(report["frame_bound_bytes"]),
            "frames_at_or_over_bound": int(report["frames_at_or_over_bound"]),
            "deepest_frame_bytes": int(report["deepest_frame_bytes"]),
            "elf": descriptor["shipped_elf"],
            "checked_manifest": row["checked_manifest"] if produces_artifact else None,
        }
        if row != expected_row:
            refuse(f"mixed gate {label} row differs from its authenticated descriptor/frame projection")
        if produces_artifact:
            verify_evidence(root, row["elf"], f"{label} ELF")
            verify_evidence(root, row["checked_manifest"], f"{label} checked manifest")
        elif row["elf"] is not None or row["checked_manifest"] is not None:
            refuse(f"frame-only {label} claims a deployable artifact")
        if label == selected_link:
            selected = {
                "schema": "dclutch-checked-mixed-gate-link-selection-v1",
                "gate_path": str(gate_path),
                "gate_sha256": expected_gate_sha256,
                "source_revision": expected_source_revision,
                "source_tree_sha256": expected_source_tree_sha256,
                "solana_cli_version": gate["solana_cli_version"],
                "label": label,
                "package": package,
                "disposition": disposition,
                "artifact_source_revision": cohort["source_revision"],
                "artifact_source_tree_sha256": cohort["source_tree_sha256"],
                "artifact_build_run_id": cohort["build_run_id"],
                "artifact_provenance": row["artifact_provenance"],
                "elf": row["elf"],
                "checked_manifest": row["checked_manifest"],
                "carry_forward_plan": gate["carry_forward_plan"],
            }
    if selected is None:
        refuse(f"mixed gate omitted selected link {selected_link!r}")
    return selected


def verify_main(arguments: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Reauthenticate and select one mixed-gate link")
    parser.add_argument("--gate", required=True)
    parser.add_argument("--expected-gate-sha256", required=True)
    parser.add_argument("--expected-source-revision", required=True)
    parser.add_argument("--expected-source-tree-sha256", required=True)
    parser.add_argument("--selected-link", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args(arguments)
    projection = authenticate_existing_gate(
        Path(args.gate), args.expected_gate_sha256, args.expected_source_revision,
        args.expected_source_tree_sha256, args.selected_link,
    )
    atomic_new(Path(args.output), projection)
    print(f"checked mixed selection sha256={sha256_file(Path(args.output))}")
    return 0


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "verify":
        try:
            return verify_main(sys.argv[2:])
        except (OSError, KeyError, TypeError, ValueError, Refusal) as error:
            print(f"MIXED CHECKED GATE REFUSED: {error}", file=sys.stderr)
            return 1
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", required=True)
    parser.add_argument("--manifest-root", required=True)
    parser.add_argument("--carry-forward-plan", required=True)
    parser.add_argument("--base-descriptor-dir", required=True)
    parser.add_argument("--candidate-descriptor-dir", required=True)
    parser.add_argument("--base-source-tree-manifest", required=True)
    parser.add_argument("--candidate-source-tree-manifest", required=True)
    parser.add_argument("--rebuilt-link", required=True)
    parser.add_argument("--solana-cli-version", required=True)
    parser.add_argument("--output-root", required=True)
    try:
        compose(parser.parse_args())
        return 0
    except (OSError, KeyError, TypeError, ValueError, Refusal) as error:
        print(f"MIXED CHECKED GATE REFUSED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
