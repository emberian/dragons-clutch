#!/usr/bin/env python3
"""Build a simulator config from the existing journey's held validator.

Run tools/gauntlet/journey/run-journey.sh with --hold-after-participant PATH,
then pass that exact participant handoff with --handoff PATH. The journey
exports the actual admitted accounts and the native basis payout scale. This
adapter does not derive economic quantities or search JSON for likely keys.

The default output drives one already-founded market. --simlife drives a
population through the existing lifecycle substrate; it additionally requires
the checked preparation's public founding identities and campaign payer path.
No key bytes are read here: accepted drivers open their own named key files.
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


DIRECT_EXECUTION_PRICE_V1 = 500_000
DIRECT_PRICE_SCALE_V1 = 1_000_000
DIRECT_FEE_BASIS_POINTS_V1 = 50
DIRECT_FEE_DENOMINATOR_V1 = 10_000


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


def planned_local_fill_atoms(market_input: dict, participant: dict, cycles: int) -> int:
    """Split the fixture's finite one-fill capacity across the requested run."""
    if cycles < 1:
        raise Refusal("--cycles must be positive")
    capacity = need(
        market_input, "local_participant_fixture_liquidity_atoms", "market input"
    )
    collateral = (((participant.get("collateral") or {}).get("intent") or {}).get(
        "quantityAtoms"
    ))
    if type(capacity) is not int or capacity < 1:
        raise Refusal("market input fixture liquidity must be a positive integer")
    if type(collateral) is not int or collateral < 1:
        raise Refusal("participant evidence has no positive collateral quantityAtoms")
    fill = capacity // cycles
    # At the accepted owned-loopback price, every even fill has an integral
    # quote. The producer repeats this check from the authenticated live config.
    fill -= fill % 2
    if fill < 1:
        raise Refusal("fixture liquidity cannot fund one positive fill per requested cycle")
    gross = fill * DIRECT_EXECUTION_PRICE_V1 // DIRECT_PRICE_SCALE_V1
    fee = gross * DIRECT_FEE_BASIS_POINTS_V1 // DIRECT_FEE_DENOMINATOR_V1
    required = (gross + fee) * cycles
    if required > collateral:
        raise Refusal(
            f"planned {cycles} fills require {required} collateral atoms but admission funded {collateral}"
        )
    return fill


def census_from_handoff(handoff: dict) -> dict:
    """Preserve the native journey's complete account set and payout scale."""
    census = need(handoff, "census", "participant handoff")
    if not isinstance(census, dict):
        raise Refusal("participant handoff census must be an object")
    for key in ("mint", "payer", "hoard", "aggregate"):
        value = need(census, key, "census")
        if not isinstance(value, str) or not value:
            raise Refusal(f"census.{key} must name an account")
    unit = need(census, "claim_unit_atoms", "census")
    if type(unit) is not int or not 0 < unit <= 2**64 - 1:
        raise Refusal("census.claim_unit_atoms must be a positive native u64 payout scale")
    for key in ("tokens", "positions", "watch"):
        mapping = need(census, key, "census")
        if not isinstance(mapping, dict) or any(
            not isinstance(label, str) or not label
            or not isinstance(address, str) or not address
            for label, address in mapping.items()
        ):
            raise Refusal(f"census.{key} must map labels to account addresses")
    if not census["tokens"] or not census["positions"]:
        raise Refusal("a participant handoff must census its collateral and claim holders")
    return census


SIMLIFE_SCHEMA_V1 = "dclutch-simlife-config-v1"

# The two identities the founding campaign authenticates as a PARTITION, read
# from the preparation stage's own report rather than typed. `run.py` refuses
# unless the report carries exactly these two and they differ, so this reads a
# document another gate has already checked.
PREPARE_STAGE_DIRECTORY = "01-prepare-mutable"


def campaign_public_identities(probe: Path, handoff: dict) -> dict:
    """Read the preparation's explicit public founding identity partition."""
    identities = handoff.get("campaignPublicIdentities")
    if identities is None:
        # Historical captures retain the exact preparation report. This is a
        # single known document, never a recursive search or an identity guess.
        stdout = probe / "runs" / "seed-01" / "stages" / PREPARE_STAGE_DIRECTORY / "stdout.bin"
        body = load(stdout, "checked preparation report")
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
                         key_directory: str, handoff: dict) -> int:
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
    payer_keypair = Path(handoff.get("campaignPayerKeypair") or keys / "campaign-payer.json")
    if not payer_keypair.is_file():
        raise Refusal(f"campaign payer keypair path is absent: {payer_keypair}")
    # Named rather than swept: the lifecycle substrate reads exactly these from
    # the substrate key directory, and each was established by a driver refusing
    # and SAYING which identity it authenticated instead.
    for required in ("core-upgrade-authority.json", "founding-founder.json"):
        if not (keys / required).is_file():
            raise Refusal(f"the probe's key directory has no {required}, which a Direct trade reads")
    identities = campaign_public_identities(probe, handoff)
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
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--handoff", help="participant handoff JSON written by the journey")
    source.add_argument("--probe-work", help="historical capture root containing runs/seed-01/participant-handoff.json")
    parser.add_argument("--sim-work", required=True, help="fresh simulator work dir (absolute)")
    parser.add_argument("--bootstrap-bin", default=None,
                        help="successor binary (default: handoff bootstrapBin or historical host-target build)")
    parser.add_argument("--output", required=True, help="config JSON to write (absolute)")
    parser.add_argument("--period-seconds", type=float, default=8.0)
    parser.add_argument("--cycles", type=int, default=3,
                        help="finite Direct cycles the generated local config must fund")
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

    if args.handoff:
        handoff_path = Path(args.handoff).resolve()
        probe = handoff_path.parent
    else:
        probe = Path(args.probe_work).resolve()
        handoff_path = probe / "runs" / "seed-01" / "participant-handoff.json"
    handoff = load(handoff_path, "participant handoff")
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

    boot = args.bootstrap_bin or handoff.get("bootstrapBin") or str(
        probe / "host-target" / "release" / "dclutch-local-successor-bootstrap"
    )
    if not Path(boot).is_file():
        raise Refusal(f"bootstrap binary absent: {boot}; pass --bootstrap-bin")
    if args.simlife:
        return write_simlife_config(args, probe, boot, rpc_url, plan, key_directory, handoff)

    founding = load(Path(founding_evidence), "founding evidence")
    if founding.get("schema") != "dclutch-successor-campaign-report-v1":
        raise Refusal("founding evidence must be the checked campaign's report")
    execution = need(founding, "execution", "founding evidence")
    if execution.get("completed") is not True:
        raise Refusal("founding execution is not completed")
    accounts = need(need(execution, "market", "founding execution"), "accounts", "founding market")
    market_address = need(need(accounts, "founding_market", "founding accounts"), "address", "founding_market")
    # Authentication and fresh account reads remain in the existing native
    # trade producer. This adapter only forwards its supported input paths.
    market_input_body = load(Path(market_input), "market input")
    participant_body = load(Path(participant_evidence), "participant evidence")
    fill_atoms = planned_local_fill_atoms(market_input_body, participant_body, args.cycles)
    census = None if args.no_census else census_from_handoff(handoff)

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
                "fill_atoms": fill_atoms,
            },
        },
        "census": census,
        "wallets": [],
        "admissions": [],
        "probe": {
            "work": str(probe),
            "handoff": str(handoff_path),
            "validator_pid": handoff.get("validatorPid"),
            "supervisor_pid": handoff.get("supervisorPid"),
            "note": "cleanup: after all drivers finish, SIGCONT the stopped journey supervisor to release its owned validator",
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
