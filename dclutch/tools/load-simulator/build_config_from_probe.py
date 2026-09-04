#!/usr/bin/env python3
"""Build a simulator config from a HELD private-validator probe.

The probe is tools/release/private-validator-lifecycle/run.py run with
``--through participant --seeds 1 --hold-after-participant``.  At the hold it
writes ``runs/seed-01/participant-handoff.json`` and SIGSTOPs itself, leaving
the validator alive.  This adapter reads the handoff plus the founding and
participant evidence and emits one config JSON.

Two shapes, and ``--simlife`` chooses the second:

* ``dclutch-load-simulator-config-v1`` -- simulator.py, ONE market that already
  exists, bound by hand from the probe's own evidence.
* ``dclutch-simlife-config-v1`` -- simlife_drive.py's ``lifecycle`` substrate,
  a whole POPULATION that founds its own markets.  It needs no bindings at all
  (a market this run founds is bound from the founding's own evidence) and
  instead needs the substrate's protocol identities: the campaign payer's
  keypair, the two founding identities, and the key directory the Direct trade
  and the resolution read their non-founding roles from.

  Those were hand-typed into a config file before this mode existed, which is
  how a substrate ends up described by a document nobody re-derives.

It never reads key bytes; it only names paths the accepted drivers open
themselves.  Missing facts refuse with the exact field named rather than
guessing.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import simcore  # noqa: E402


class Refusal(RuntimeError):
    pass


def need(mapping: dict, key: str, where: str):
    if key not in mapping:
        raise Refusal(f"{where} lacks required field {key!r}")
    return mapping[key]


def write_config_file(out: Path, config: dict) -> Path:
    """Serialize, prove the bytes carry no credential, then write.

    Both emitters go through here so the value test cannot be true of one
    config shape and forgotten for the other.
    """

    body = json.dumps(config, sort_keys=True, indent=2) + "\n"
    carried = simcore.endpoint_credential((config.get("cluster") or {}).get("rpc_url", ""))
    if carried:
        raise Refusal(
            f"refusing to write a {carried} credential into {out}: store the "
            "credential-free endpoint and let "
            f"${simcore.RPC_URL_ENVIRONMENT} or "
            f"~/{simcore.DEFAULT_PROVIDER_KEY_FILE} supply the key at use time"
        )
    out.write_text(body)
    return out


def load(path: Path, what: str) -> dict:
    if not path.is_file():
        raise Refusal(f"{what} is absent: {path}")
    return json.loads(path.read_text())


def find_first(body, predicate, path=""):
    """Depth-first search for the first value satisfying predicate; returns
    (json-pointer-ish path, value) or None."""
    if predicate(body):
        return path, body
    if isinstance(body, dict):
        for key, value in body.items():
            hit = find_first(value, predicate, f"{path}/{key}")
            if hit:
                return hit
    elif isinstance(body, list):
        for index, value in enumerate(body):
            hit = find_first(value, predicate, f"{path}/{index}")
            if hit:
                return hit
    return None



SIMLIFE_SCHEMA_V1 = "dclutch-simlife-config-v1"

# The two identities the founding campaign authenticates as a PARTITION, read
# from the preparation stage's own report rather than typed. `run.py` refuses
# unless the report carries exactly these two and they differ, so this reads a
# document another gate has already checked.
PREPARE_STAGE_DIRECTORY = "01-prepare-mutable"


def campaign_public_identities(probe: Path) -> dict:
    """`founding-founder` and `substituted-founder`, from the probe's own stage.

    Neither is a keypair in the key directory -- the substituted founder has no
    key at all, because the whole point of the partition is that it is an
    identity the founding refunds to and never signs as -- so they cannot be
    derived from the files on disk and have to come from the report.
    """
    stdout = probe / "runs" / "seed-01" / "stages" / PREPARE_STAGE_DIRECTORY / "stdout.bin"
    if not stdout.is_file():
        raise Refusal(f"the probe's preparation stage wrote no report: {stdout}")
    try:
        body = json.loads(stdout.read_bytes().decode("utf-8"))
    except (UnicodeDecodeError, ValueError) as error:
        raise Refusal(f"the probe's preparation report is not JSON: {error}") from error
    identities = body.get("campaign_public_identities")
    if not isinstance(identities, dict):
        raise Refusal("the preparation report carries no campaign_public_identities")
    for role in ("founding-founder", "substituted-founder"):
        if not identities.get(role):
            raise Refusal(f"campaign_public_identities lacks {role!r}")
    if identities["founding-founder"] == identities["substituted-founder"]:
        raise Refusal("the two public founding identities alias, which the campaign refuses")
    return identities


def substrate_source_revision(args, probe: Path):
    """The revision the programs ON THIS CHAIN were built from.

    A HELD probe has no `SUMMARY.json` -- it is SIGSTOPed at the participant
    boundary and writes that file only when it finishes -- so reading the
    revision from there produced a capture labelled with no substrate at all,
    which is the one thing a published artifact must never be. The checked
    release's own gate names it, and that is the authority: those are the bytes
    the validator loaded.
    """
    root = getattr(args, "release_root", None)
    if root:
        gate = Path(root) / "CHECKED_UPGRADE_GATE.json"
        if not gate.is_file():
            raise Refusal(f"checked release gate absent: {gate}")
        revision = load(gate, "checked release gate").get("source_revision")
        if not isinstance(revision, str) or len(revision) != 40:
            raise Refusal("the checked release gate names no forty-character source revision")
        return revision
    summary = probe / "SUMMARY.json"
    if summary.is_file():
        try:
            found = json.loads(summary.read_text()).get("source_revision")
        except ValueError:
            found = None
        if isinstance(found, str) and len(found) == 40:
            return found
    raise Refusal(
        "no source revision for this substrate: pass --release-root so the capture can say "
        "which programs it was driven against. A held probe has no SUMMARY.json, and a capture "
        "that names no substrate is the one thing a published artifact must not be"
    )


def write_simlife_config(args, probe: Path, boot: str, rpc_url: str, plan: str,
                         key_directory: str) -> int:
    """One `dclutch-simlife-config-v1` for the lifecycle substrate.

    NO BINDINGS, and that is the point of the substrate rather than an omission:
    a market this run founds is bound from the FOUNDING's own evidence, so the
    census observes exactly the accounts the chain gave it rather than anything
    a config typed by hand.
    """
    if not args.seed:
        raise Refusal(
            "--seed is required for --simlife: a world is named by the sentence it was "
            "drawn from, and an unnamed run cannot be re-run by typing its name"
        )
    keys = Path(key_directory)
    payer_keypair = keys / "campaign-payer.json"
    if not payer_keypair.is_file():
        raise Refusal(f"the probe's key directory has no campaign-payer.json: {keys}")
    # Named rather than swept: the lifecycle substrate reads exactly these from
    # the substrate key directory, and each was established by a driver refusing
    # and SAYING which identity it authenticated instead.
    for required in ("core-upgrade-authority.json", "founding-founder.json"):
        if not (keys / required).is_file():
            raise Refusal(f"the probe's key directory has no {required}, which a Direct trade reads")
    identities = campaign_public_identities(probe)
    revision = substrate_source_revision(args, probe)
    config = {
        "schema": SIMLIFE_SCHEMA_V1,
        "cluster": {"label": "local", "rpc_url": rpc_url},
        "bootstrap_bin": boot,
        "work_dir": args.sim_work,
        "substrate": "lifecycle",
        "substrate_label": args.substrate_label or (
            "a fresh loopback validator carrying the seven-role successor release set, "
            f"held at the participant boundary by {probe}"
        ),
        "cadence": {
            "period_seconds": args.period_seconds,
            "jitter_fraction": args.jitter_fraction,
        },
        "world": {
            "seed": args.seed,
            "markets": args.markets,
            "ticks": args.ticks,
            "archetype_mix": args.archetype_mix,
            "slots_per_tick": args.slots_per_tick,
        },
        "lifecycle": {
            "plan": plan,
            "campaign_payer_keypair": str(payer_keypair),
            "founding_founder": identities["founding-founder"],
            "substituted_founder": identities["substituted-founder"],
            "substrate_keys": key_directory,
            "driver_timeout_seconds": args.driver_timeout_seconds,
        },
        "bindings": {},
    }
    config["source_revision"] = revision
    if args.solana_keygen:
        if not Path(args.solana_keygen).is_file():
            raise Refusal(f"solana-keygen absent: {args.solana_keygen}")
        config["lifecycle"]["solana_keygen"] = args.solana_keygen
    if args.pyth_facts:
        # A fact about the CHAIN rather than about a market: one provisioning
        # serves every market on this validator, so it is named once here rather
        # than per market. Required to EXIST, because a resolution driver handed
        # a path to nothing refuses several minutes into a run.
        if not Path(args.pyth_facts).is_file():
            raise Refusal(f"pyth facts document absent: {args.pyth_facts}")
        config["lifecycle"]["pyth_facts"] = args.pyth_facts
    if args.max_lamports_spent is not None:
        if args.max_lamports_spent <= 0:
            raise Refusal("--max-lamports-spent must be positive; omit it for an unbounded run")
        config["budget"] = {"max_lamports_spent": args.max_lamports_spent}
    out = write_config_file(Path(args.output), config)
    print(f"config written: {out}")
    print(f"rpc: {simcore.redact_endpoint(rpc_url)}")
    print(f"world: {args.seed!r}, {args.markets} markets over {args.ticks} ticks "
          f"({args.archetype_mix})")
    print("budget: " + (f"{args.max_lamports_spent} lamports"
                        if args.max_lamports_spent is not None
                        else "UNBOUNDED -- nothing will stop this run for spending"))
    return 0



def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--probe-work", required=True, help="the probe --work dir")
    parser.add_argument("--sim-work", required=True, help="fresh simulator work dir (absolute)")
    parser.add_argument("--bootstrap-bin", default=None,
                        help="successor binary (default: probe host-target build)")
    parser.add_argument("--output", required=True, help="config JSON to write (absolute)")
    parser.add_argument("--period-seconds", type=float, default=8.0)
    parser.add_argument("--no-census", action="store_true",
                        help="omit the census block (NOT for real runs; the "
                             "reconciliation loop is part of the deliverable)")
    parser.add_argument("--simlife", action="store_true",
                        help="emit a dclutch-simlife-config-v1 for the lifecycle "
                             "substrate instead of a one-market simulator config")
    parser.add_argument("--seed", help="the world's seed preimage (--simlife)")
    parser.add_argument("--markets", type=int, default=12)
    parser.add_argument("--ticks", type=int, default=48)
    parser.add_argument("--slots-per-tick", type=int, default=900)
    parser.add_argument("--archetype-mix", default="design-space",
                        choices=("design-space", "foundable-today"))
    parser.add_argument("--jitter-fraction", type=float, default=0.25)
    parser.add_argument("--driver-timeout-seconds", type=float, default=1800.0)
    parser.add_argument("--max-lamports-spent", type=int, default=None,
                        help="the run dies when its fee payers have spent this much; "
                             "omit for a run nothing will stop for spending")
    parser.add_argument("--solana-keygen",
                        help="solana-keygen, which the lifecycle substrate uses to make "
                             "one disposable key per founding role and participant wallet")
    parser.add_argument("--substrate-label")
    parser.add_argument("--release-root",
                        help="the checked release the substrate's programs were built from; its "
                             "gate names the source revision this run was driven against")
    parser.add_argument("--pyth-facts",
                        help="a dclutch-flagship-pyth-update-facts-v1 document; without one "
                             "every resolution refuses, because the producer will not invent "
                             "the Pyth update account it reads")
    args = parser.parse_args(argv)

    probe = Path(args.probe_work)
    seed = probe / "runs" / "seed-01"
    handoff = load(seed / "participant-handoff.json", "participant handoff")
    if handoff.get("schema") != "dclutch-private-validator-participant-handoff-v1":
        raise Refusal(f"unexpected handoff schema {handoff.get('schema')!r}")

    rpc_url = need(handoff, "rpcUrl", "handoff")
    # THE CREDENTIAL DOES NOT ENTER THIS BUILDER, so it cannot leave in a file.
    #
    # This builder has only ever been pointed at a loopback probe, which is why
    # it never redacted anything -- and why cohort-15's devnet fork of it wrote
    # a live Helius key into `sim-config.json` in cleartext. Refusing the keyed
    # endpoint at the one place it arrives is what stops the next fork
    # inheriting that; the endpoint's key is read at use time instead
    # (`simcore.resolve_endpoint`).
    carried = simcore.endpoint_credential(rpc_url)
    if carried:
        raise Refusal(
            f"refusing to write a {carried} credential into a config file: "
            "store the credential-free endpoint and let "
            f"${simcore.RPC_URL_ENVIRONMENT} or "
            f"~/{simcore.DEFAULT_PROVIDER_KEY_FILE} supply the key at use time"
        )
    plan = need(handoff, "plan", "handoff")
    market_input = need(handoff, "marketInput", "handoff")
    founding_evidence = need(handoff, "foundingEvidence", "handoff")
    participant_evidence = need(handoff, "participantEvidence", "handoff")
    key_directory = need(handoff, "keyDirectory", "handoff")

    if args.simlife:
        # BEFORE the one-market evidence is read, and deliberately: a lifecycle
        # world founds its OWN markets, so the probe's founding and participant
        # documents describe a market it will never bind. Requiring them here
        # would refuse a config for a substrate that does not need them.
        boot = args.bootstrap_bin or str(
            probe / "host-target" / "release" / "dclutch-local-successor-bootstrap"
        )
        if not Path(boot).is_file():
            raise Refusal(f"bootstrap binary absent: {boot}")
        return write_simlife_config(args, probe, boot, rpc_url, plan, key_directory)

    founding = load(Path(founding_evidence), "founding evidence")
    targets = need(founding, "founding_targets", "founding evidence")
    market_address = need(targets, "open_market", "founding_targets")
    checkpoint = need(founding, "foundingCheckpoint", "founding evidence")
    accounts = need(checkpoint, "accounts", "foundingCheckpoint")
    payer = need(founding, "payer", "founding evidence")

    def account_address(label: str) -> str:
        entry = need(accounts, label, "foundingCheckpoint.accounts")
        if isinstance(entry, dict):
            return need(entry, "address", f"accounts[{label}]")
        return str(entry)

    market = load(Path(market_input), "market input")
    participant = load(Path(participant_evidence), "participant evidence")

    # Census facts.  Mint and hoard come from founding; the aggregate and the
    # participant's token/position accounts come from the admission report.
    census = None
    if not args.no_census:
        aggregate_hit = find_first(
            participant,
            lambda v: isinstance(v, str) and len(v) in range(32, 45)
            and v not in (market_address,),
            "",
        )
        # The aggregate must be named explicitly, not guessed: look for a key
        # literally containing "aggregate" in either evidence document.
        def keyed(body, needle):
            found = {}
            def walk(node, path=""):
                if isinstance(node, dict):
                    for key, value in node.items():
                        if needle in key.lower() and isinstance(value, str):
                            found[f"{path}/{key}"] = value
                        elif needle in key.lower() and isinstance(value, dict) and "address" in value:
                            found[f"{path}/{key}"] = value["address"]
                        walk(value, f"{path}/{key}")
                elif isinstance(node, list):
                    for index, value in enumerate(node):
                        walk(value, f"{path}/{index}")
            walk(body)
            return found

        aggregates = {**keyed(founding, "aggregate"), **keyed(participant, "aggregate")}
        if len(set(aggregates.values())) != 1:
            raise Refusal(
                "could not resolve exactly one aggregate address from evidence; "
                f"candidates: {aggregates!r}; pass --no-census only for a smoke "
                "run and file the gap"
            )
        aggregate = next(iter(set(aggregates.values())))

        positions = keyed(participant, "position")
        tokens = keyed(participant, "token_account")
        claim_unit = None
        for source, key in ((market, "claim_unit_atoms"), (market, "local_participant_fixture_liquidity_atoms")):
            if key in source:
                claim_unit = int(source[key])
                break
        if claim_unit is None:
            raise Refusal("no claim unit quantity in market input")
        census = {
            "mint": account_address("collateral_mint"),
            "payer": payer,
            "hoard": account_address("founding_hoard_vault"),
            "aggregate": aggregate,
            "claim_unit_atoms": claim_unit,
            "tokens": {
                label.rsplit("/", 1)[-1] or f"t{i}": addr
                for i, (label, addr) in enumerate(sorted(tokens.items()))
            },
            "positions": {
                label.rsplit("/", 1)[-1] or f"p{i}": addr
                for i, (label, addr) in enumerate(sorted(positions.items()))
            },
            "watch": {},
        }

    boot = args.bootstrap_bin or str(probe / "host-target" / "release" / "dclutch-local-successor-bootstrap")
    if not Path(boot).is_file():
        raise Refusal(f"bootstrap binary absent: {boot}")

    config = {
        "schema": "dclutch-load-simulator-config-v1",
        "cluster": {"label": "local", "rpc_url": rpc_url},
        "bootstrap_bin": boot,
        "work_dir": args.sim_work,
        "market_address": market_address,
        "cadence": {"period_seconds": args.period_seconds, "jitter_fraction": 0.25},
        "trade": {
            "mode": "local",
            "max_steps_per_session": 32,
            "step_pause_seconds": 1.0,
            "local": {
                "plan": plan,
                "market_input": market_input,
                "campaign_report": founding_evidence,
                "participant_report": participant_evidence,
                "key_dir": key_directory,
            },
        },
        "census": census,
        "wallets": [],
        "admissions": [],
        "probe": {
            "work": str(probe),
            "validator_pid": handoff.get("validatorPid"),
            "note": "teardown: SIGCONT the (stopped) run.py supervisor; never kill the validator directly",
        },
    }
    out = write_config_file(Path(args.output), config)
    print(f"config written: {out}")
    print(f"market: {market_address}")
    print(f"rpc: {simcore.redact_endpoint(rpc_url)}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Refusal as refusal:
        print(f"REFUSED: {refusal}", file=sys.stderr)
        sys.exit(2)
