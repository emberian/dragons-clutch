#!/usr/bin/env python3
"""Advance one durable sponsored-Pyth/terminal step on devnet.

This is a process coordinator only: `devnet-sponsored-push-v1` and
`devnet-terminal-sequence-v1` remain the semantic and receipt owners.
"""
from __future__ import annotations

import argparse, json, subprocess, sys
from pathlib import Path

GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
PRICE = "7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE"
SCHEMA = "dclutch-devnet-sponsored-keeper-v1"
ROOT = Path(__file__).resolve().parents[2]
BOOT = ROOT / "tools/local-validator/bootstrap/successor/Cargo.toml"

def die(message: str) -> None: raise ValueError(message)
def obj(path: Path) -> dict:
    if not path.is_absolute() or path.is_symlink() or not path.is_file(): die(f"{path} must be an absolute regular file")
    value = json.loads(path.read_text())
    if not isinstance(value, dict): die(f"{path} must contain one JSON object")
    return value
def phase(path: Path) -> str | None:
    if not path.exists(): return None
    value = obj(path); result = value.get("phase")
    if result not in {"Planned", "Prepared", "Submitted", "Finalized"}: die(f"{path} has no supported durable phase")
    return result
def require_string(value: object, name: str) -> str:
    if not isinstance(value, str) or not value: die(f"{name} must be nonempty string")
    return value
def absolute(value: object, name: str) -> str:
    path = Path(require_string(value, name))
    if not path.is_absolute(): die(f"{name} must be absolute")
    return str(path)
def command(*args: str) -> list[str]:
    return ["cargo", "run", "--locked", "--manifest-path", str(BOOT), "--", *args]

def load(path: Path) -> dict:
    value = obj(path)
    if set(value) != {"schema", "rpcUrl", "sponsoredInput", "signer", "signerKeypair", "terminal"}: die("keeper spec has exact fields")
    if value["schema"] != SCHEMA: die("unsupported keeper schema")
    sponsored = obj(Path(absolute(value["sponsoredInput"], "sponsoredInput")))
    accounts = sponsored.get("accounts")
    if sponsored.get("format") != "dclutch-sponsored-push-exterior-input-v1" or not isinstance(accounts, dict) or accounts.get("price_account") != PRICE:
        die("sponsoredInput is not the fixed credential-free SOL/USD exterior input")
    terminal = value["terminal"]
    if not isinstance(terminal, dict) or set(terminal) != {"plan", "marketInput", "campaignEvidence", "market", "feePayer", "feePayerKeypair", "session", "journalDir", "completion", "lookupTable"}: die("terminal has exact fields")
    for key in ("plan", "marketInput", "campaignEvidence", "feePayerKeypair"):
        absolute(terminal[key], f"terminal.{key}")
    for key in ("session", "journalDir", "completion"):
        absolute(terminal[key], f"terminal.{key}")
    for key in ("market", "feePayer"):
        require_string(terminal[key], f"terminal.{key}")
    require_string(value["rpcUrl"], "rpcUrl"); require_string(value["signer"], "signer")
    absolute(value["signerKeypair"], "signerKeypair")
    if terminal["lookupTable"] is not None: require_string(terminal["lookupTable"], "terminal.lookupTable")
    return value

def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", required=True, type=Path); parser.add_argument("--work", required=True, type=Path)
    parser.add_argument("--execute", action="store_true", help="allow exactly the selected existing driver to read its named key and submit")
    args = parser.parse_args(argv)
    spec = load(args.spec); work = args.work
    if not work.is_absolute() or work.is_symlink(): die("--work must be an absolute non-symlink directory")
    work.mkdir(mode=0o700, parents=True, exist_ok=True)
    capture, settle = work / "capture.json", work / "settle.json"
    current = phase(capture)
    action, report, candidate = ("capture", capture, None) if current != "Finalized" else ("settle", settle, None)
    if current == "Finalized":
        capture_doc = obj(capture); candidate = require_string(capture_doc.get("candidate"), "capture candidate")
        if phase(settle) == "Finalized":
            terminal = spec["terminal"]
            argsv = command("devnet-terminal-sequence-v1", "--rpc-url", spec["rpcUrl"], "--i-mean-devnet", GENESIS,
                "--plan", terminal["plan"], "--market-input", terminal["marketInput"], "--evidence", terminal["campaignEvidence"],
                "--market", terminal["market"], "--fee-payer", terminal["feePayer"], "--fee-payer-keypair", terminal["feePayerKeypair"],
                "--session", terminal["session"], "--journal-dir", terminal["journalDir"], "--completion", terminal["completion"])
            if terminal["lookupTable"] is not None: argsv += ["--lookup-table", terminal["lookupTable"]]
            if args.execute: argsv.append("--execute")
            return subprocess.run(argsv).returncode
    argsv = command("devnet-sponsored-push-v1", "--rpc-url", spec["rpcUrl"], "--i-mean-devnet", GENESIS,
        "--input", spec["sponsoredInput"], "--output", str(report), "--action", action, "--signer", spec["signer"])
    if candidate is not None: argsv += ["--candidate", candidate]
    if args.execute: argsv += ["--execute", "--signer-keypair", spec["signerKeypair"]]
    return subprocess.run(argsv).returncode

if __name__ == "__main__":
    try: raise SystemExit(main(sys.argv[1:]))
    except (ValueError, OSError, json.JSONDecodeError) as error:
        print(f"devnet-sponsored-keeper: {error}", file=sys.stderr); raise SystemExit(2)
