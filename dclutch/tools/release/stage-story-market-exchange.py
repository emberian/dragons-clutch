#!/usr/bin/env python3
"""Emit one finite, non-submitting plan over the canonical three story markets."""
from __future__ import annotations
import argparse, json, subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = "dclutch-story-market-exchange-plan-v1"
GENESIS = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"

def main() -> int:
    p = argparse.ArgumentParser(); p.add_argument("--work", required=True, type=Path); a = p.parse_args()
    if not a.work.is_absolute() or a.work.exists() or a.work.is_symlink(): raise ValueError("--work must be an absolute fresh non-symlink directory")
    a.work.mkdir(mode=0o700); scenarios = a.work / "scenarios"
    subprocess.run(["cargo", "run", "--locked", "--manifest-path", str(ROOT / "tools/devnet-scenarios/Cargo.toml"), "--", "generate", str(scenarios)], check=True)
    plan = {"schema": SCHEMA, "cluster": {"devnetGenesis": GENESIS, "mainnetObservation": "read-only only; no mainnet signer, transaction, or spend"}, "scenarios": [
      {"id": "flagship-four-outcome", "artifact": str(scenarios / "flagship.json"), "marketCaller": "devnet-sponsored-market + campaign --founding-only", "providerCaller": "devnet-sponsored-push-v1", "terminalCaller": "devnet-terminal-sequence-v1", "status": "caller-backed after runtime bindings"},
      {"id": "graduation-four-outcome", "artifact": str(scenarios / "graduation.json"), "marketCaller": "graduation-market + campaign --founding-only", "providerCaller": "relayed-graduation-resolution", "terminalCaller": "adapter-required", "status": "finite story; resolution/terminal adapter remains required"},
      {"id": "abandoned-graded-failure", "artifact": str(scenarios / "abandoned.json"), "marketCaller": "graded-failure-market-founding", "providerCaller": "funded-failure-resolution", "terminalCaller": "adapter-required", "status": "finite story; founding/provider/terminal adapters remain required"},
    ]}
    (a.work / "exchange-plan.json").write_text(json.dumps(plan, indent=2, sort_keys=True) + "\n")
    print(a.work / "exchange-plan.json"); return 0
if __name__ == "__main__":
    try: raise SystemExit(main())
    except (ValueError, subprocess.CalledProcessError) as error: print(f"story-market-exchange: {error}"); raise SystemExit(2)
