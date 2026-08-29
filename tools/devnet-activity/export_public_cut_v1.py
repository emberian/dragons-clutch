#!/usr/bin/env python3
"""Derive the web's public devnet cut from reconciled Activity evidence."""
from __future__ import annotations
import argparse, importlib.util, json, os, sys, tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
ACTIVITY_PATH = Path(__file__).with_name("activity.py")
OUT = ROOT / "apps/dclutch-web/fixtures/public-cut.devnet.json"

spec = importlib.util.spec_from_file_location("public_cut_activity", ACTIVITY_PATH)
assert spec and spec.loader
activity = importlib.util.module_from_spec(spec); sys.modules[spec.name] = activity; spec.loader.exec_module(activity)

class Refusal(RuntimeError): pass

def state(path: Path, label: str) -> dict[str, Any]:
    try: return activity.authenticated_state(activity.canonical_existing_file(path, label), label)
    except activity.Refusal as error: raise Refusal(str(error)) from error

def sig(value: Any, label: str) -> str:
    try: return activity.signature_text(value, label)
    except activity.Refusal as error: raise Refusal(str(error)) from error

def export(manifest_path: Path, work: Path, reconciliation_path: Path, output: Path) -> None:
    try: manifest = activity.parse_manifest(manifest_path)
    except activity.Refusal as error: raise Refusal(str(error)) from error
    reconciliation = state(reconciliation_path, "activity reconciliation")
    if (reconciliation.get("schema") != activity.RECONCILIATION_SCHEMA or reconciliation.get("clusterTarget") != "devnet" or reconciliation.get("genesisHash") != activity.DEVNET_GENESIS_HASH or reconciliation.get("manifestSha256") != manifest.sha256 or reconciliation.get("untrustedProjectionUsed") is not False):
        raise Refusal("reconciliation is not the exact finalized public-devnet Activity result")
    reconciled = {row.get("adapterId"): set(row.get("signatures", [])) for row in reconciliation.get("activity", []) if isinstance(row, dict)}
    if set(reconciled) != {adapter.adapter_id for adapter in manifest.adapters}:
        raise Refusal("reconciliation omits or invents an Activity adapter")
    private, public = activity.unverified_wallet_indexes(manifest, work)
    private_rows = {row["id"]: row for row in private["wallets"]}; public_rows = {row["id"]: row for row in public["wallets"]}
    market: str | None = None; steps: dict[str, str | None] = {"found": None, "trade": None, "resolve": None, "redeem": None}
    for adapter in manifest.adapters:
        argv, completion_path = activity.expanded_adapter(adapter, manifest, work, private_rows, public_rows)
        journal = state(activity.adapter_journal_path(work, adapter.adapter_id), f"adapter journal {adapter.adapter_id}")
        if journal.get("phase") != "finalized": raise Refusal(f"adapter {adapter.adapter_id} is not finalized")
        signatures = [sig(item, f"adapter {adapter.adapter_id} signature") for item in journal.get("signatures", [])]
        if not signatures or set(signatures) != set(reconciled[adapter.adapter_id]): raise Refusal(f"adapter {adapter.adapter_id} signatures do not equal reconciled finalized history")
        completion = state(completion_path, f"adapter {adapter.adapter_id} completion")
        kinds = {next(operation.kind for operation in manifest.scenario.operations if operation.operation_id == covered) for covered in adapter.covers}
        if "found" in kinds:
            execution = completion.get("execution")
            if not isinstance(execution, dict) or not isinstance(execution.get("market"), str): raise Refusal("founding completion omitted its Market")
            market = activity.pubkey_text(execution["market"], "founding Market"); steps["found"] = signatures[-1]
        if "direct" in kinds:
            if completion.get("schema") != "dclutch-devnet-direct-trade-finalized-v1": raise Refusal("Direct completion schema changed")
            candidate = activity.pubkey_text(completion.get("market"), "Direct Market")
            market = candidate if market is None else market
            if market != candidate: raise Refusal("Direct completion belongs to another Market")
            steps["trade"] = sig(completion.get("signature"), "Direct signature")
        # A terminal completion has no payout mutation; a redeem link must be
        # owned by a distinct redeem adapter/receipt, never guessed from retire.
        if "resolve" in kinds or "redeem" in kinds:
            if len(kinds) != 1: raise Refusal("combined terminal adapter cannot mint resolve/redeem public links")
            step = next(iter(kinds)); steps[step] = signatures[-1]
    if market is None or steps["found"] is None: raise Refusal("finalized public cut lacks Market/founding")
    result = {"schema": "dclutch-public-cut-v1", "cluster": "devnet", "market": market, "activity": steps}
    if output != OUT or output.is_symlink(): raise Refusal("public cut output is the sole committed fixture")
    descriptor, temporary = tempfile.mkstemp(prefix=".public-cut.", dir=output.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as written:
            json.dump(result, written, indent=2); written.write("\n"); written.flush(); os.fsync(written.fileno())
        os.replace(temporary, output)
        activity.fsync_directory(output.parent)
    finally:
        if os.path.exists(temporary): os.unlink(temporary)

def main() -> int:
    parser = argparse.ArgumentParser(); parser.add_argument("--manifest", required=True); parser.add_argument("--work", required=True); parser.add_argument("--reconciliation", required=True); parser.add_argument("--output", default=str(OUT)); args = parser.parse_args()
    try: export(Path(args.manifest), Path(args.work), Path(args.reconciliation), Path(args.output)); return 0
    except Refusal as error: print(f"public cut export refused: {error}", file=sys.stderr); return 2
if __name__ == "__main__": raise SystemExit(main())
