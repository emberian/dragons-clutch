#!/usr/bin/env python3

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import dryplan


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def pin(path: Path) -> dict[str, object]:
    raw = path.read_bytes()
    return {"canonicalPath": str(path.resolve()), "sha256": hashlib.sha256(raw).hexdigest()}


def gate_pin(root: Path, relative: str, raw: bytes) -> dict[str, object]:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(raw)
    return {"canonical_path": relative, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}


def base58(raw: bytes) -> str:
    number = int.from_bytes(raw, "big")
    output = ""
    while number:
        number, digit = divmod(number, 58)
        output = dryplan.BASE58_ALPHABET[digit] + output
    leading = len(raw) - len(raw.lstrip(b"\0"))
    return "1" * leading + (output or "1")


class Fixture:
    def __init__(self, root: Path):
        self.root = root
        self.gate_root = root / "checked"
        self.gate_root.mkdir()
        self.capture_path = root / "capture.json"
        self.baseline_paths: list[Path] = []
        self.fee_payer = "11111111111111111111111111111111"
        self.buffer_keys = [base58(bytes([200 + ordinal]) * 32) for ordinal in range(5)]
        self.elf_lengths: dict[str, int] = {}
        self._make_gate()
        self.capture = self._make_capture()
        self._make_baselines()
        self.inputs_path = root / "inputs.json"
        self.inputs = {
            "schema": dryplan.INPUTS_SCHEMA,
            "checkedReleaseGate": pin(self.gate_root / "CHECKED_UPGRADE_GATE.json"),
            "permanentSubstrateCapture": pin(self.capture_path),
            "baselines": [
                {"role": role, **pin(path)}
                for role, path in zip(dryplan.UPGRADE_ROLES, self.baseline_paths)
            ],
            "feePayer": self.fee_payer,
            "buffers": [
                {"role": role, "publicKey": key}
                for role, key in zip(dryplan.UPGRADE_ROLES, self.buffer_keys)
            ],
        }
        write_json(self.inputs_path, self.inputs)

    def _make_gate(self) -> None:
        links = []
        shared: dict[str, dict[str, object]] = {}
        for name in ("plain-invocation", "plain-log", "frame-invocation", "frame-log", "frame-object", "frame-report"):
            shared[name] = gate_pin(self.gate_root, f"evidence/{name}.txt", name.encode())
        for ordinal, role in enumerate(dryplan.UPGRADE_ROLES):
            elf_raw = b"\x7fELF" + bytes([ordinal + 1]) * (60 + ordinal)
            self.elf_lengths[role] = len(elf_raw)
            elf = gate_pin(self.gate_root, f"elf/{role}.so", elf_raw)
            provenance = {
                "schema": dryplan.PROVENANCE_SCHEMA,
                "label": role,
                "package": dryplan.PACKAGES[role],
                "artifact_stem": dryplan.ARTIFACT_STEMS[role],
                "source_revision": "1" * 40,
                "source_tree_sha256": "2" * 64,
                "build_run_id": "fixture",
                "plain_build": {
                    "invocation": shared["plain-invocation"],
                    "log": shared["plain-log"],
                    "compile_marker": f"Compiling {dryplan.PACKAGES[role]}",
                    "sbf_diagnostics_count": 0,
                },
                "shipped_elf": elf,
                "frame_measurement": {
                    "invocation": shared["frame-invocation"],
                    "build_log": shared["frame-log"],
                    "compile_marker": f"Compiling {dryplan.PACKAGES[role]}",
                    "object": shared["frame-object"],
                    "report": shared["frame-report"],
                },
            }
            provenance_raw = (json.dumps(provenance, indent=2) + "\n").encode()
            provenance_pin = gate_pin(self.gate_root, f"provenance/{role}.json", provenance_raw)
            links.append({
                "label": role,
                "package": dryplan.PACKAGES[role],
                "elf": elf,
                "artifact_provenance": provenance_pin,
            })
        for ordinal in range(8):
            links.append({"label": f"other-{ordinal}", "package": f"other-{ordinal}", "elf": None})
        gate = {
            "schema": dryplan.GATE_SCHEMA,
            "source_revision": "1" * 40,
            "source_tree_sha256": "2" * 64,
            "solana_cli_version": "solana-cli 3.0.0",
            "build_run_id": "fixture",
            "link_count": len(links),
            "links": links,
        }
        write_json(self.gate_root / "CHECKED_UPGRADE_GATE.json", gate)

    def _make_capture(self) -> dict[str, object]:
        roles = []
        for ordinal, (role, _, program, programdata, slot) in enumerate(dryplan.ROLES):
            live, programdata_lamports, live_sha256, programdata_sha256 = dryplan.DEPLOY1_FACTS[role]
            roles.append({
                "ordinal": ordinal,
                "role": role,
                "program_id": program,
                "programdata_id": programdata,
                "program_lamports": 1_000 + ordinal,
                "program_data_sha256": f"{ordinal + 1:064x}",
                "programdata_lamports": programdata_lamports,
                "programdata_account_bytes": live + dryplan.PROGRAMDATA_METADATA_BYTES,
                "programdata_account_sha256": programdata_sha256,
                "deployment_slot": slot,
                "live_elf_bytes": live,
                "live_elf_sha256": live_sha256,
            })
        capture = {
            "schema": dryplan.CAPTURE_SCHEMA,
            "endpoint": dryplan.CAPTURE_ENDPOINT,
            "commitment": "finalized",
            "rpc_method": "getMultipleAccounts",
            "context_slot": 500_000_000,
            "expected_upgrade_authority": dryplan.AUTHORITY,
            "fee_payer": self.fee_payer,
            "fee_payer_lamports": 32_000_000_000,
            "canonical_role_order": [row[0] for row in dryplan.ROLES],
            "roles": roles,
            "program_lamports_total": sum(row["program_lamports"] for row in roles),
            "programdata_lamports_total": sum(row["programdata_lamports"] for row in roles),
            "snapshot_sha256": "",
        }
        body = json.dumps(capture, separators=(",", ":")).encode()
        hasher = hashlib.sha256()
        hasher.update(b"dclutch/devnet-permanent-substrate-snapshot/v1\n")
        hasher.update(len(body).to_bytes(8, "little"))
        hasher.update(body)
        capture["snapshot_sha256"] = hasher.hexdigest()
        write_json(self.capture_path, capture)
        return capture

    def _make_baselines(self) -> None:
        capture_by_role = {row["role"]: row for row in self.capture["roles"]}
        for role in dryplan.UPGRADE_ROLES:
            role_row = next(row for row in dryplan.ROLES if row[0] == role)
            ordinal = [row[0] for row in dryplan.ROLES].index(role)
            captured = capture_by_role[role]
            current = captured["programdata_account_bytes"]
            live = captured["live_elf_bytes"]
            target_live = max(self.elf_lengths[role], live)
            target_space = max(current, target_live + dryplan.PROGRAMDATA_METADATA_BYTES)
            target_minimum = 20_000 + target_space
            baseline = {
                "schema": dryplan.BASELINE_SCHEMA,
                "canonical_role_order": [row[0] for row in dryplan.ROLES],
                "role_ordinal": ordinal,
                "role": role,
                "program_id": role_row[2],
                "programdata_id": role_row[3],
                "expected_upgrade_authority": dryplan.AUTHORITY,
                "rpc_origin_redacted": dryplan.DEVNET_ENDPOINT,
                "genesis_hash": dryplan.DEVNET_GENESIS,
                "context_slot": self.capture["context_slot"],
                "observation": {
                    "program_lamports": captured["program_lamports"],
                    "program_owner": dryplan.LOADER,
                    "program_executable": True,
                    "program_data_bytes": 36,
                    "program_account_sha256": captured["program_data_sha256"],
                    "programdata_lamports": captured["programdata_lamports"],
                    "programdata_owner": dryplan.LOADER,
                    "programdata_executable": False,
                    "programdata_data_bytes": current,
                    "deployment_slot": captured["deployment_slot"],
                    "upgrade_authority": dryplan.AUTHORITY,
                    "live_elf_bytes": live,
                    "live_elf_sha256": captured["live_elf_sha256"],
                    "programdata_account_sha256": captured["programdata_account_sha256"],
                },
                "target_live_elf_bytes": target_live,
                "extension_additional_bytes": target_space - current,
                "current_rent_exempt_minimum_lamports": 10_000,
                "target_rent_exempt_minimum_lamports": target_minimum,
                "extension_lamport_top_up": max(target_minimum - captured["programdata_lamports"], 0),
                "baseline_sha256": "",
            }
            baseline["baseline_sha256"] = dryplan.baseline_digest(baseline, role)
            path = self.root / f"{role}-baseline.json"
            write_json(path, baseline)
            self.baseline_paths.append(path)

    def rewrite_gate_pin(self) -> None:
        self.inputs["checkedReleaseGate"] = pin(self.gate_root / "CHECKED_UPGRADE_GATE.json")
        write_json(self.inputs_path, self.inputs)

    def rewrite_capture_pin(self) -> None:
        self.inputs["permanentSubstrateCapture"] = pin(self.capture_path)
        write_json(self.inputs_path, self.inputs)

    def rewrite_baseline_pin(self, role: str) -> None:
        index = list(dryplan.UPGRADE_ROLES).index(role)
        self.inputs["baselines"][index].update(pin(self.baseline_paths[index]))
        write_json(self.inputs_path, self.inputs)


class DryplanTests(unittest.TestCase):
    def test_duplicate_json_key_refuses(self) -> None:
        with self.assertRaises(dryplan.Refusal):
            dryplan.load_json_bytes(b'{"schema":"first","schema":"second"}', "hostile")

    def test_template_is_key_free_non_executable_and_self_authenticating(self) -> None:
        value = dryplan.template()
        dryplan.validate_plan(value, "template")
        self.assertFalse(value["mutationPermitted"])
        self.assertEqual([row["disposition"] for row in value["roles"]], ["carry-forward", "carry-forward", "upgrade", "upgrade", "upgrade", "upgrade", "upgrade"])
        self.assertNotIn("keypair", json.dumps(value).lower())
        activity = value["activityV3Plan"]
        self.assertEqual(activity["semanticAuthority"]["sha256"], dryplan.ACTIVITY_AUTHORITY_SHA256)
        self.assertEqual(activity["derived"]["walletCount"], 10)
        self.assertEqual(activity["derived"]["postInitTransferLamports"], 200_000_000)
        self.assertEqual(activity["derived"]["initialFundingLamports"], 360_000_000)
        self.assertEqual(activity["derived"]["maxSpendLamports"], 210_000_000)
        self.assertEqual(activity["derived"]["maxPostInitFeeLamports"], 10_000_000)
        self.assertEqual(activity["derived"]["maxActivityFeeLamports"], 10_000_000)
        self.assertEqual(sum(activity["derived"]["operationCounts"].values()), 25)

    def test_template_refuses_mainnet_identity_and_mutation_claim(self) -> None:
        for mutation in (
            lambda value: value["cluster"].update(genesisHash="mainnet"),
            lambda value: value.update(mutationPermitted=True),
            lambda value: value["roles"][0].update(programId=value["roles"][1]["programId"]),
            lambda value: value["activityV3Plan"]["derived"].update(maxSpendLamports=200_000_000),
        ):
            value = dryplan.template()
            mutation(value)
            value["stateSha256"] = dryplan.state_digest(value)
            with self.assertRaises(dryplan.Refusal):
                dryplan.validate_plan(value)

    def test_assemble_joins_exact_gate_capture_baselines_and_buffers(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory).resolve())
            value = dryplan.assemble(fixture.inputs_path)
            dryplan.validate_plan(value, "captured")
            self.assertEqual([row["role"] for row in value["upgradeOperations"]], list(dryplan.UPGRADE_ROLES))
            self.assertEqual(value["authorities"]["captureContextSlot"], 500_000_000)
            self.assertFalse(value["mutationPermitted"])
            expected_top_up = sum(row["extensionRentTopUpLamports"] for row in value["upgradeOperations"])
            self.assertEqual(value["accounting"]["extensionRentTopUpLamports"], expected_top_up)

    def test_gate_role_substitution_and_provenance_substitution_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory).resolve())
            gate_path = fixture.gate_root / "CHECKED_UPGRADE_GATE.json"
            gate = json.loads(gate_path.read_text())
            gate["links"][0]["package"] = "wrong-package"
            write_json(gate_path, gate)
            fixture.rewrite_gate_pin()
            with self.assertRaises(dryplan.Refusal):
                dryplan.assemble(fixture.inputs_path)

        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory).resolve())
            provenance_path = fixture.gate_root / "provenance/custody.json"
            provenance = json.loads(provenance_path.read_text())
            provenance["plain_build"]["sbf_diagnostics_count"] = 1
            write_json(provenance_path, provenance)
            gate_path = fixture.gate_root / "CHECKED_UPGRADE_GATE.json"
            gate = json.loads(gate_path.read_text())
            link = next(row for row in gate["links"] if row["label"] == "custody")
            link["artifact_provenance"] = {
                "canonical_path": "provenance/custody.json",
                "bytes": provenance_path.stat().st_size,
                "sha256": hashlib.sha256(provenance_path.read_bytes()).hexdigest(),
            }
            write_json(gate_path, gate)
            fixture.rewrite_gate_pin()
            with self.assertRaises(dryplan.Refusal):
                dryplan.assemble(fixture.inputs_path)

    def test_stale_capture_slot_authority_and_lamport_totals_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory).resolve())
            capture = json.loads(fixture.capture_path.read_text())
            capture["expected_upgrade_authority"] = fixture.fee_payer
            capture["snapshot_sha256"] = "0" * 64
            write_json(fixture.capture_path, capture)
            fixture.rewrite_capture_pin()
            with self.assertRaises(dryplan.Refusal):
                dryplan.assemble(fixture.inputs_path)

        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory).resolve())
            capture = json.loads(fixture.capture_path.read_text())
            capture["programdata_lamports_total"] += 1
            capture["snapshot_sha256"] = "0" * 64
            write_json(fixture.capture_path, capture)
            fixture.rewrite_capture_pin()
            with self.assertRaises(dryplan.Refusal):
                dryplan.assemble(fixture.inputs_path)

    def test_baseline_slot_digest_and_rent_arithmetic_substitution_refuse(self) -> None:
        for mutate in (
            lambda baseline: baseline["observation"].update(deployment_slot=baseline["observation"]["deployment_slot"] + 1),
            lambda baseline: baseline.update(extension_lamport_top_up=baseline["extension_lamport_top_up"] + 1),
            lambda baseline: baseline.update(baseline_sha256="0" * 64),
        ):
            with tempfile.TemporaryDirectory() as directory:
                fixture = Fixture(Path(directory).resolve())
                path = fixture.baseline_paths[0]
                baseline = json.loads(path.read_text())
                mutate(baseline)
                write_json(path, baseline)
                fixture.rewrite_baseline_pin("custody")
                with self.assertRaises(dryplan.Refusal):
                    dryplan.assemble(fixture.inputs_path)

    def test_duplicate_buffer_and_tampered_manifest_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(Path(directory).resolve())
            fixture.inputs["buffers"][1]["publicKey"] = fixture.inputs["buffers"][0]["publicKey"]
            write_json(fixture.inputs_path, fixture.inputs)
            with self.assertRaises(dryplan.Refusal):
                dryplan.assemble(fixture.inputs_path)

        value = dryplan.template()
        value["downstreamSequence"].pop()
        with self.assertRaises(dryplan.Refusal):
            dryplan.validate_plan(value)


if __name__ == "__main__":
    unittest.main()
