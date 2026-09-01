#!/usr/bin/env python3
"""Resume one exact finalized devnet Direct trade through complete retirement.

This supervisor owns ordering and durable child-process dispatch only. Rust
commands remain the semantic owners of the Direct history, resolution,
payouts, Position closes, maker replay closes, and aggregate retirement. The
runner executes from an exact successor campaign pack and reuses the Rust
binary built by the pack-bound public-route campaign.

Keypair paths are passed to those semantic owners but are never opened, hashed,
or copied by this process.
"""

from __future__ import annotations

import argparse
import dataclasses
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile
from typing import Any, Callable, Mapping, NoReturn, Sequence


ROOT = Path(__file__).resolve().parents[2]
PACK_TOOL_PATH = Path(__file__).with_name("successor_campaign_pack.py")
PUBLIC_TOOL_PATH = Path(__file__).with_name("public_route_campaign.py")


def load_module(name: str, path: Path) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


pack_tool = load_module("dclutch_devnet_lifecycle_pack", PACK_TOOL_PATH)
public_tool = load_module("dclutch_devnet_lifecycle_public", PUBLIC_TOOL_PATH)


PLAN_SCHEMA = "dclutch-devnet-direct-complete-life-plan-v1"
JOURNAL_SCHEMA = "dclutch-devnet-direct-complete-life-journal-v1"
REPORT_SCHEMA = "dclutch-devnet-direct-complete-life-suffix-v1"
DEVNET_GENESIS_HASH = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
DIRECT_EVIDENCE_SCHEMA = "dclutch-devnet-direct-trade-finalized-v1"
CHILDREN_SCHEMA = "dclutch-devnet-direct-terminal-children-v1"
PAYOUT_EVIDENCE_SCHEMA = "dclutch-devnet-wallet-terminal-payout-evidence-v1"
POSITION_EVIDENCE_SCHEMA = "dclutch-user-position-close-evidence-v1"
TERMINAL_COMPLETION_SCHEMA = "dclutch-devnet-terminal-sequence-completion-v1"
RESOLUTION_CHECKPOINT_SCHEMA = "dclutch-flagship-resolution-checkpoint-v3"
FEE_EVIDENCE_SCHEMA = "dclutch-direct-fee-settlement-evidence-v1"
MAX_JSON_BYTES = 16 * 1024 * 1024
MAX_CHILD_ATTEMPTS = 96
PUBKEY = re.compile(r"[1-9A-HJ-NP-Za-km-z]{32,44}")
SIGNATURE = re.compile(r"[1-9A-HJ-NP-Za-km-z]{64,88}")
HEX64 = re.compile(r"[0-9a-f]{64}")
CHAOS_ENV_PREFIX = "DCLUTCH_CHAOS_FAULT_"


class Refusal(RuntimeError):
    """The requested suffix is not the exact admitted devnet lifecycle."""


def refuse(message: str) -> NoReturn:
    raise Refusal(message)


def canonical_json(value: Any) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def exact_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        refuse(f"{label} must be an object")
    return value


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, member in pairs:
        if key in value:
            refuse(f"JSON contains duplicate key {key!r}")
        value[key] = member
    return value


def decode_json(payload: bytes, label: str) -> Any:
    try:
        return json.loads(payload, object_pairs_hook=unique_object)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        refuse(f"{label} is not exact UTF-8 JSON: {error}")


def exact_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        refuse(f"{label} must be an array")
    return value


def exact_keys(value: Mapping[str, Any], keys: set[str], label: str) -> None:
    if set(value) != keys:
        refuse(f"{label} fields differ: {sorted(set(value) ^ keys)}")


def text(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value or value.strip() != value:
        refuse(f"{label} must be nonempty canonical text")
    return value


def digest(value: Any, label: str) -> str:
    value = text(value, label)
    if HEX64.fullmatch(value) is None:
        refuse(f"{label} must be 64 lowercase hexadecimal characters")
    return value


def pubkey(value: Any, label: str) -> str:
    value = text(value, label)
    if PUBKEY.fullmatch(value) is None:
        refuse(f"{label} is not a canonical base58 public key")
    return value


def decimal(value: Any, label: str, *, positive: bool = False) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        refuse(f"{label} must be an integer")
    if value < (1 if positive else 0) or value > 2**64 - 1:
        refuse(f"{label} is outside its admitted u64 range")
    return value


def decimal_text(value: Any, label: str) -> int:
    value = text(value, label)
    if not value.isdigit() or (len(value) > 1 and value.startswith("0")):
        refuse(f"{label} must be canonical decimal u64 text")
    parsed = int(value)
    if parsed > 2**64 - 1:
        refuse(f"{label} is outside its admitted u64 range")
    return parsed


def canonical_file(value: str | Path, label: str) -> Path:
    path = Path(value)
    if not path.is_absolute():
        refuse(f"{label} must be absolute")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        refuse(f"{label} is unavailable: {error}")
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_size <= 0
        or metadata.st_size > MAX_JSON_BYTES
        or resolved != path
    ):
        refuse(f"{label} must be one canonical regular file")
    return path


def canonical_log(value: str | Path, label: str) -> Path:
    path = Path(value)
    if not path.is_absolute():
        refuse(f"{label} must be absolute")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        refuse(f"{label} is unavailable: {error}")
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_size > MAX_JSON_BYTES
        or resolved != path
    ):
        refuse(f"{label} must be one bounded canonical regular file")
    return path


def canonical_directory(value: str | Path, label: str) -> Path:
    path = Path(value)
    if not path.is_absolute():
        refuse(f"{label} must be absolute")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        refuse(f"{label} is unavailable: {error}")
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode) or resolved != path:
        refuse(f"{label} must be one canonical directory")
    return path


def keypair_path(value: Any, label: str) -> Path:
    """Authenticate only path geometry; never open or hash secret key bytes."""

    path = Path(text(value, label))
    if not path.is_absolute():
        refuse(f"{label} must be absolute")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        refuse(f"{label} is unavailable: {error}")
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode) or resolved != path:
        refuse(f"{label} must be one canonical non-symlink regular file")
    return path


def read_json(path: Path, label: str) -> dict[str, Any]:
    path = canonical_file(path, label)
    return exact_object(decode_json(path.read_bytes(), label), label)


def atomic_json(path: Path, value: Any, *, new: bool = False) -> None:
    if new and (path.exists() or path.is_symlink()):
        refuse(f"refusing to overwrite {path}")
    payload = json.dumps(value, indent=2, sort_keys=True).encode() + b"\n"
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


@dataclasses.dataclass(frozen=True)
class AcceptedFile:
    path: Path
    sha256: str

    def document(self) -> dict[str, str]:
        return {"path": str(self.path), "sha256": self.sha256}


def accepted_file(value: Any, label: str) -> AcceptedFile:
    source = exact_object(value, label)
    exact_keys(source, {"path", "sha256"}, label)
    path = canonical_file(text(source["path"], f"{label} path"), label)
    expected = digest(source["sha256"], f"{label} SHA-256")
    if sha256_file(path) != expected:
        refuse(f"{label} bytes differ from their accepted SHA-256")
    return AcceptedFile(path, expected)


@dataclasses.dataclass(frozen=True)
class KeyActor:
    address: str
    keypair: Path


@dataclasses.dataclass(frozen=True)
class LifecyclePlan:
    path: Path
    sha256: str
    rpc_url: str
    release_pack: AcceptedFile
    public_campaign: AcceptedFile
    direct_evidence: AcceptedFile
    resolution_input: AcceptedFile
    refreshed_evidence: AcceptedFile | None
    expected_market: str
    terminal_lookup_table: str | None
    fee_payer: KeyActor
    seller: KeyActor
    buyer: KeyActor
    resolution_submitter: Path
    resolution_resolver: Path
    resolution_update_authority: Path


def parse_actor(value: Any, label: str) -> KeyActor:
    source = exact_object(value, label)
    exact_keys(source, {"address", "keypairPath"}, label)
    return KeyActor(
        pubkey(source["address"], f"{label} address"),
        keypair_path(source["keypairPath"], f"{label} keypair"),
    )


def parse_plan(path: Path) -> LifecyclePlan:
    path = canonical_file(path, "devnet complete-life plan")
    source = read_json(path, "devnet complete-life plan")
    exact_keys(
        source,
        {
            "schema",
            "rpcUrl",
            "genesisHash",
            "releasePack",
            "publicRouteCampaign",
            "directEvidence",
            "resolutionInput",
            "refreshedEvidence",
            "expectedMarket",
            "terminalLookupTable",
            "actors",
            "resolutionAuthorities",
        },
        "devnet complete-life plan",
    )
    if source["schema"] != PLAN_SCHEMA or source["genesisHash"] != DEVNET_GENESIS_HASH:
        refuse("complete-life plan schema or exact devnet genesis changed")
    rpc_url = text(source["rpcUrl"], "devnet RPC URL")
    if not rpc_url.startswith("https://") or "localhost" in rpc_url or "127.0.0.1" in rpc_url:
        refuse("complete-life plan requires one explicit external HTTPS devnet RPC")
    actors = exact_object(source["actors"], "actors")
    exact_keys(actors, {"feePayer", "seller", "buyer"}, "actors")
    fee_payer = parse_actor(actors["feePayer"], "fee payer")
    seller = parse_actor(actors["seller"], "seller")
    buyer = parse_actor(actors["buyer"], "buyer")
    if len({fee_payer.address, seller.address, buyer.address}) != 3:
        refuse("permissionless fee payer must be unrelated to the Direct seller and buyer")
    if len({fee_payer.keypair, seller.keypair, buyer.keypair}) != 3:
        refuse("fee payer, Direct seller, and Direct buyer must name distinct keypair files")
    authorities = exact_object(source["resolutionAuthorities"], "resolution authorities")
    exact_keys(
        authorities,
        {"submitterKeypairPath", "resolverKeypairPath", "updateKeypairPath"},
        "resolution authorities",
    )
    refreshed = source["refreshedEvidence"]
    if refreshed is not None and not isinstance(refreshed, dict):
        refuse("refreshedEvidence must be null or one accepted file")
    lookup = source["terminalLookupTable"]
    if lookup is not None:
        lookup = pubkey(lookup, "terminal lookup table")
    return LifecyclePlan(
        path=path,
        sha256=sha256_file(path),
        rpc_url=rpc_url,
        release_pack=accepted_file(source["releasePack"], "release pack"),
        public_campaign=accepted_file(
            source["publicRouteCampaign"], "public route campaign"
        ),
        direct_evidence=accepted_file(source["directEvidence"], "Direct evidence"),
        resolution_input=accepted_file(source["resolutionInput"], "resolution input"),
        refreshed_evidence=(
            None if refreshed is None else accepted_file(refreshed, "refreshed evidence")
        ),
        expected_market=pubkey(source["expectedMarket"], "expected Market"),
        terminal_lookup_table=lookup,
        fee_payer=fee_payer,
        seller=seller,
        buyer=buyer,
        resolution_submitter=keypair_path(
            authorities["submitterKeypairPath"], "resolution submitter keypair"
        ),
        resolution_resolver=keypair_path(
            authorities["resolverKeypairPath"], "resolution resolver keypair"
        ),
        resolution_update_authority=keypair_path(
            authorities["updateKeypairPath"], "resolution update keypair"
        ),
    )


def verify_absolute_evidence(value: Any, label: str) -> Path:
    try:
        return pack_tool.verify_absolute_evidence(value, label)
    except pack_tool.Refusal as error:
        refuse(str(error))


@dataclasses.dataclass(frozen=True)
class BoundSources:
    pack_root: Path
    pack: Mapping[str, Any]
    public: Mapping[str, Any]
    bootstrap: Path
    successor_plan: Path
    market_input: Path
    campaign_evidence: Path
    public_manifest: Path
    direct_session: Path
    direct_journal: Path
    producer_journal: Path
    direct_evidence_value: Mapping[str, Any]
    payout_rows: tuple[Mapping[str, Any], ...]


def assert_source_pinned_runner(pack_root: Path) -> None:
    expected = (
        pack_root / "source/tools/release/devnet_direct_lifecycle.py"
    ).resolve(strict=True)
    current = Path(__file__).resolve(strict=True)
    if current != expected or current.read_bytes() != expected.read_bytes():
        refuse("execute the devnet complete-life runner from the pack's exact archived source")


def bind_sources(plan: LifecyclePlan) -> BoundSources:
    try:
        pack_root, pack = pack_tool.verify_pack(plan.release_pack.path)
    except pack_tool.Refusal as error:
        refuse(str(error))
    assert_source_pinned_runner(pack_root)
    public = read_json(plan.public_campaign.path, "public route campaign")
    try:
        public_tool.verify_value(plan.public_campaign.path, public)
    except (public_tool.Refusal, pack_tool.Refusal) as error:
        refuse(str(error))
    release_path = verify_absolute_evidence(public["release_pack"], "public campaign pack")
    if release_path != plan.release_pack.path:
        refuse("public route campaign belongs to another release pack")
    inputs = exact_object(public["inputs"], "public campaign inputs")
    successor_plan = verify_absolute_evidence(inputs["plan"], "successor plan")
    direct_session = verify_absolute_evidence(inputs["direct_session"], "Direct session")
    direct_journal = canonical_directory(
        exact_object(inputs["direct_journal"], "Direct journal")["canonical_path"],
        "Direct journal",
    )
    producer_binding = exact_object(inputs["direct_producer"], "Direct producer")
    producer_journal = verify_absolute_evidence(
        producer_binding["journal"], "Direct producer journal"
    )
    producer = read_json(producer_journal, "Direct producer journal")
    sources = exact_object(producer_binding["sources"], "Direct producer sources")
    market_input = verify_absolute_evidence(sources["market_input"], "Market input")
    campaign_evidence = verify_absolute_evidence(
        sources["campaign_report"], "founding campaign evidence"
    )
    public_manifest = verify_absolute_evidence(
        sources["public_manifest"], "Direct public manifest"
    )
    bootstrap = verify_absolute_evidence(
        exact_object(public["build"], "public campaign build")["bootstrap_binary"],
        "source-pinned successor binary",
    )
    expected_direct_evidence = canonical_file(
        text(producer.get("evidenceFile"), "producer Direct evidence path"),
        "producer Direct evidence",
    )
    if (
        expected_direct_evidence != plan.direct_evidence.path
        or sha256_file(expected_direct_evidence) != plan.direct_evidence.sha256
    ):
        refuse("complete-life Direct evidence is not the producer journal's exact output")
    direct = read_json(plan.direct_evidence.path, "Direct finalized evidence")
    if (
        direct.get("schema") != DIRECT_EVIDENCE_SCHEMA
        or direct.get("status") != "finalized"
        or direct.get("cluster") != "devnet"
        or direct.get("market") != plan.expected_market
        or direct.get("publicManifestSha256") != sha256_file(public_manifest)
        or direct.get("privateSessionSha256") != sha256_file(direct_session)
    ):
        refuse("Direct evidence is not the exact finalized devnet producer output")
    seller = pubkey(direct.get("sellerOwner"), "Direct seller")
    buyer = pubkey(direct.get("buyerOwner"), "Direct buyer")
    if seller != plan.seller.address or buyer != plan.buyer.address:
        refuse("complete-life actor keys differ from the authenticated Direct participants")
    rows: list[Mapping[str, Any]] = []
    seen: set[tuple[str, int]] = set()
    for index, raw in enumerate(exact_list(direct.get("claimBalances"), "Direct claim balances")):
        row = exact_object(raw, f"Direct claim balance {index}")
        exact_keys(
            row,
            {"owner", "position", "recipientToken", "claimIndex", "quantityAtoms"},
            f"Direct claim balance {index}",
        )
        owner = pubkey(row["owner"], f"claim balance {index} owner")
        claim_index = decimal(row["claimIndex"], f"claim balance {index} claim index")
        quantity = decimal(
            row["quantityAtoms"], f"claim balance {index} quantity", positive=True
        )
        if owner not in {seller, buyer} or (owner, claim_index) in seen:
            refuse("Direct payout schedule changed owner or duplicated an owner/claim pair")
        seen.add((owner, claim_index))
        rows.append(
            {
                "role": "seller" if owner == seller else "buyer",
                "owner": owner,
                "position": pubkey(row["position"], f"claim balance {index} Position"),
                "recipient": pubkey(
                    row["recipientToken"], f"claim balance {index} recipient"
                ),
                "claimIndex": claim_index,
                "quantityAtoms": quantity,
            }
        )
    if not rows:
        refuse("finalized Direct evidence exposed no claim balances to retire")
    rows.sort(key=lambda row: (0 if row["role"] == "seller" else 1, row["claimIndex"]))
    return BoundSources(
        pack_root=pack_root,
        pack=pack,
        public=public,
        bootstrap=bootstrap,
        successor_plan=successor_plan,
        market_input=market_input,
        campaign_evidence=campaign_evidence,
        public_manifest=public_manifest,
        direct_session=direct_session,
        direct_journal=direct_journal,
        producer_journal=producer_journal,
        direct_evidence_value=direct,
        payout_rows=tuple(rows),
    )


def stage_specs(sources: BoundSources) -> list[dict[str, Any]]:
    stages: list[dict[str, Any]] = [
        {"id": "fee-settlement", "kind": "fee-settlement"},
        {"id": "resolution", "kind": "resolution"},
    ]
    stages.extend(
        {
            "id": f"payout-{row['role']}-{row['claimIndex']:03d}",
            "kind": "payout",
            "payout": dict(row),
        }
        for row in sources.payout_rows
    )
    stages.extend(
        [
            {"id": "terminal-children", "kind": "terminal-children"},
            {"id": "position-close-seller", "kind": "position-close", "role": "seller"},
            {"id": "position-close-buyer", "kind": "position-close", "role": "buyer"},
            {"id": "terminal-through-direct-retiring", "kind": "terminal-prefix"},
            {"id": "maker-close-seller", "kind": "maker-close", "role": "seller"},
            {"id": "maker-close-buyer", "kind": "maker-close", "role": "buyer"},
            {"id": "terminal-through-core-retired", "kind": "terminal-finish"},
        ]
    )
    return stages


def journal_digest(value: Mapping[str, Any]) -> str:
    copy = dict(value)
    copy["stateSha256"] = ""
    return sha256_bytes(canonical_json(copy))


def initial_journal(plan: LifecyclePlan, sources: BoundSources) -> dict[str, Any]:
    value: dict[str, Any] = {
        "schema": JOURNAL_SCHEMA,
        "status": "running",
        "genesisHash": DEVNET_GENESIS_HASH,
        "planPath": str(plan.path),
        "planSha256": plan.sha256,
        "releasePackSha256": plan.release_pack.sha256,
        "publicRouteCampaignSha256": plan.public_campaign.sha256,
        "directEvidenceSha256": plan.direct_evidence.sha256,
        "market": plan.expected_market,
        "stages": [
            {
                **spec,
                "phase": "planned",
                "attempts": [],
                "result": None,
            }
            for spec in stage_specs(sources)
        ],
        "stateSha256": "",
    }
    value["stateSha256"] = journal_digest(value)
    return value


def authenticate_journal(
    value: Mapping[str, Any],
    plan: LifecyclePlan,
    sources: BoundSources,
    root: Path,
) -> dict[str, Any]:
    journal = exact_object(dict(value), "lifecycle journal")
    exact_keys(
        journal,
        {
            "schema",
            "status",
            "genesisHash",
            "planPath",
            "planSha256",
            "releasePackSha256",
            "publicRouteCampaignSha256",
            "directEvidenceSha256",
            "market",
            "stages",
            "stateSha256",
        },
        "lifecycle journal",
    )
    expected_specs = stage_specs(sources)
    rows = exact_list(journal["stages"], "lifecycle stages")
    if (
        journal["schema"] != JOURNAL_SCHEMA
        or journal["status"] not in {"running", "finalized"}
        or journal["genesisHash"] != DEVNET_GENESIS_HASH
        or journal["planPath"] != str(plan.path)
        or journal["planSha256"] != plan.sha256
        or journal["releasePackSha256"] != plan.release_pack.sha256
        or journal["publicRouteCampaignSha256"] != plan.public_campaign.sha256
        or journal["directEvidenceSha256"] != plan.direct_evidence.sha256
        or journal["market"] != plan.expected_market
        or len(rows) != len(expected_specs)
        or journal["stateSha256"] != journal_digest(journal)
    ):
        refuse("complete-life journal identity, stage width, or state digest changed")
    reached_open = False
    for index, (row_value, spec) in enumerate(zip(rows, expected_specs)):
        row = exact_object(row_value, f"lifecycle stage {index}")
        expected_keys = set(spec) | {"phase", "attempts", "result"}
        exact_keys(row, expected_keys, f"lifecycle stage {index}")
        if any(row[key] != value for key, value in spec.items()):
            refuse(f"lifecycle stage {index} identity changed")
        if row["phase"] not in {"planned", "dispatching", "finalized"}:
            refuse(f"lifecycle stage {index} phase is not admitted")
        attempts = exact_list(row["attempts"], f"lifecycle stage {index} attempts")
        if len(attempts) > MAX_CHILD_ATTEMPTS:
            refuse(f"lifecycle stage {index} exceeded its bounded attempts")
        for attempt_index, raw_attempt in enumerate(attempts):
            attempt = exact_object(
                raw_attempt, f"lifecycle stage {index} attempt {attempt_index + 1}"
            )
            exact_keys(
                attempt,
                {
                    "ordinal",
                    "commandSha256",
                    "stdout",
                    "stderr",
                    "exitCode",
                    "stdoutSha256",
                    "stderrSha256",
                },
                f"lifecycle stage {index} attempt {attempt_index + 1}",
            )
            ordinal = attempt_index + 1
            if (
                attempt["ordinal"] != ordinal
                or HEX64.fullmatch(str(attempt["commandSha256"])) is None
                or attempt["stdout"]
                != str(root / "logs" / f"{row['id']}-{ordinal:03d}.stdout")
                or attempt["stderr"]
                != str(root / "logs" / f"{row['id']}-{ordinal:03d}.stderr")
            ):
                refuse(f"lifecycle stage {index} attempt identity changed")
            if attempt["exitCode"] is None:
                if attempt["stdoutSha256"] is not None or attempt["stderrSha256"] is not None:
                    refuse("an indeterminate child attempt carried output digests")
            else:
                if isinstance(attempt["exitCode"], bool) or not isinstance(
                    attempt["exitCode"], int
                ):
                    refuse("a completed child attempt carried a noninteger exit code")
                stdout = canonical_log(attempt["stdout"], "child stdout log")
                stderr = canonical_log(attempt["stderr"], "child stderr log")
                if (
                    digest(attempt["stdoutSha256"], "child stdout digest")
                    != sha256_file(stdout)
                    or digest(attempt["stderrSha256"], "child stderr digest")
                    != sha256_file(stderr)
                ):
                    refuse("a completed child attempt log changed")
        if reached_open and row["phase"] == "finalized":
            refuse("a later lifecycle stage finalized before its predecessor")
        if row["phase"] != "finalized":
            reached_open = True
        if row["phase"] == "planned" and attempts:
            refuse("planned lifecycle stage carried child attempts")
        if row["phase"] == "finalized" and not isinstance(row["result"], dict):
            refuse("finalized lifecycle stage omitted its result evidence")
        if row["phase"] != "finalized" and row["result"] is not None:
            refuse("unfinished lifecycle stage carried a final result")
        if row["phase"] == "finalized":
            expected_result = authenticated_stage_result(row, root, plan, sources)
            if row["result"] != expected_result:
                refuse(f"lifecycle stage {row['id']} result evidence changed")
    if journal["status"] == "finalized" and reached_open:
        refuse("lifecycle journal claimed finalized before every stage")
    return journal


def output_root(path: Path) -> Path:
    if not path.is_absolute() or path == Path("/") or path.is_symlink():
        refuse("--output-root must be one bounded absolute non-symlink path")
    parent = path.parent.resolve(strict=True)
    if parent != path.parent:
        refuse("--output-root parent must be canonical")
    if not path.exists():
        path.mkdir(mode=0o700)
    return canonical_directory(path, "lifecycle output root")


def artifact(path: Path, label: str) -> dict[str, Any]:
    path = canonical_file(path, label)
    return {"path": str(path), "bytes": path.stat().st_size, "sha256": sha256_file(path)}


def verify_artifact(value: Any, label: str) -> Path:
    source = exact_object(value, label)
    exact_keys(source, {"path", "bytes", "sha256"}, label)
    path = canonical_file(text(source["path"], f"{label} path"), label)
    byte_count = decimal(source["bytes"], f"{label} byte count", positive=True)
    expected = digest(source["sha256"], f"{label} SHA-256")
    if path.stat().st_size != byte_count or sha256_file(path) != expected:
        refuse(f"{label} bytes differ from their report binding")
    return path


def clean_environment() -> dict[str, str]:
    return {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(CHAOS_ENV_PREFIX)
    }


class StageDriver:
    def __init__(
        self,
        root: Path,
        journal_path: Path,
        journal: dict[str, Any],
        executor: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run,
    ) -> None:
        self.root = root
        self.journal_path = journal_path
        self.journal = journal
        self.executor = executor
        self.logs = root / "logs"
        self.logs.mkdir(exist_ok=True)

    def persist(self) -> None:
        self.journal["stateSha256"] = journal_digest(self.journal)
        atomic_json(self.journal_path, self.journal)

    def stage(self, stage_id: str) -> dict[str, Any]:
        matches = [row for row in self.journal["stages"] if row["id"] == stage_id]
        if len(matches) != 1:
            refuse(f"lifecycle journal omitted unique stage {stage_id}")
        return matches[0]

    def invoke(self, row: dict[str, Any], argv: Sequence[str]) -> subprocess.CompletedProcess[bytes]:
        if row["phase"] == "finalized":
            refuse(f"finalized stage {row['id']} cannot invoke another child")
        if len(row["attempts"]) >= MAX_CHILD_ATTEMPTS:
            refuse(f"stage {row['id']} exhausted its bounded child attempts")
        ordinal = len(row["attempts"]) + 1
        stdout_path = self.logs / f"{row['id']}-{ordinal:03d}.stdout"
        stderr_path = self.logs / f"{row['id']}-{ordinal:03d}.stderr"
        command_sha256 = sha256_bytes(canonical_json(list(argv)))
        attempt = {
            "ordinal": ordinal,
            "commandSha256": command_sha256,
            "stdout": str(stdout_path),
            "stderr": str(stderr_path),
            "exitCode": None,
            "stdoutSha256": None,
            "stderrSha256": None,
        }
        row["phase"] = "dispatching"
        row["attempts"].append(attempt)
        self.persist()
        result = self.executor(
            list(argv),
            cwd=self.root,
            env=clean_environment(),
            capture_output=True,
            check=False,
        )
        stdout_path.write_bytes(result.stdout)
        stderr_path.write_bytes(result.stderr)
        attempt["exitCode"] = result.returncode
        attempt["stdoutSha256"] = sha256_file(stdout_path)
        attempt["stderrSha256"] = sha256_file(stderr_path)
        self.persist()
        if result.returncode != 0:
            detail = result.stderr.decode(errors="replace").strip()[:4096]
            refuse(f"stage {row['id']} child exited {result.returncode}: {detail}")
        return result

    def finalize(self, row: dict[str, Any], result: Mapping[str, Any]) -> None:
        row["phase"] = "finalized"
        row["result"] = dict(result)
        self.persist()


def child_base(sources: BoundSources) -> list[str]:
    return [str(sources.bootstrap)]


def devnet_args(plan: LifecyclePlan) -> list[str]:
    return [
        "--rpc-url",
        plan.rpc_url,
        "--i-mean-devnet",
        DEVNET_GENESIS_HASH,
    ]


def finalized_fee_evidence(path: Path, plan: LifecyclePlan) -> dict[str, Any] | None:
    if not path.exists():
        return None
    value = read_json(path, "fee-settlement evidence")
    if (
        value.get("schema") != FEE_EVIDENCE_SCHEMA
        or value.get("cluster") != "devnet"
        or value.get("market") != plan.expected_market
        or value.get("maker") != plan.buyer.address
        or value.get("feePayer") != plan.fee_payer.address
        or not isinstance(value.get("landed"), dict)
    ):
        refuse(
            "fee-settlement evidence is not one finalized devnet buyer obligation paid by the unrelated plan payer"
        )
    return artifact(path, "fee-settlement evidence")


def run_fee(driver: StageDriver, plan: LifecyclePlan, sources: BoundSources) -> None:
    row = driver.stage("fee-settlement")
    if row["phase"] == "finalized":
        return
    evidence = driver.root / "fee-settlement.json"
    ready = finalized_fee_evidence(evidence, plan)
    if ready is None:
        driver.invoke(
            row,
            child_base(sources)
            + ["devnet-direct-fee-settlement-v1"]
            + devnet_args(plan)
            + [
                "--public-manifest",
                str(sources.public_manifest),
                "--maker",
                plan.buyer.address,
                "--evidence",
                str(evidence),
                "--execute",
                "--fee-payer-keypair",
                str(plan.fee_payer.keypair),
            ],
        )
        ready = finalized_fee_evidence(evidence, plan)
    if ready is None:
        refuse("fee settlement returned success without finalized evidence")
    driver.finalize(row, ready)


def resolution_complete(path: Path, plan: LifecyclePlan) -> dict[str, Any] | None:
    if not path.exists():
        return None
    value = read_json(path, "resolution checkpoint")
    receipts = exact_list(value.get("receipts"), "resolution checkpoint receipts")
    if (
        value.get("format") != RESOLUTION_CHECKPOINT_SCHEMA
        or value.get("inputSha256") != plan.resolution_input.sha256
        or value.get("stagePlan") is not None
        or value.get("verifiedTerminal") is not True
        or [receipt.get("stage") for receipt in receipts]
        != [
            "submit",
            "resolution-provider-execute-v1",
            "core-terminal-accept-v1",
            "reclaim",
        ]
    ):
        return None
    return artifact(path, "resolution checkpoint")


def run_resolution(driver: StageDriver, plan: LifecyclePlan, sources: BoundSources) -> None:
    row = driver.stage("resolution")
    if row["phase"] == "finalized":
        return
    checkpoint = driver.root / "resolution-checkpoint.json"
    ready = resolution_complete(checkpoint, plan)
    if ready is None:
        driver.invoke(
            row,
            child_base(sources)
            + ["flagship-resolution-v1"]
            + devnet_args(plan)
            + [
                "--input",
                str(plan.resolution_input.path),
                "--checkpoint",
                str(checkpoint),
                "--through",
                "complete",
                "--execute",
                "--submitter-keypair",
                str(plan.resolution_submitter),
                "--resolver-keypair",
                str(plan.resolution_resolver),
                "--update-keypair",
                str(plan.resolution_update_authority),
            ],
        )
        ready = resolution_complete(checkpoint, plan)
    if ready is None:
        refuse("resolution command did not authenticate one terminal checkpoint")
    driver.finalize(row, ready)


def actor_for_role(plan: LifecyclePlan, role: str) -> KeyActor:
    if role == "seller":
        return plan.seller
    if role == "buyer":
        return plan.buyer
    refuse(f"unknown Direct actor role {role}")


def payout_evidence_digest(value: Mapping[str, Any]) -> str:
    poststates: list[dict[str, Any]] = []
    for index, raw in enumerate(exact_list(value["poststates"], "wallet payout poststates")):
        poststate = exact_object(raw, f"wallet payout poststate {index}")
        exact_keys(
            poststate,
            {"address", "owner", "lamports", "executable", "dataLen", "dataSha256"},
            f"wallet payout poststate {index}",
        )
        poststates.append(
            {
                "address": poststate["address"],
                "owner": poststate["owner"],
                "lamports": poststate["lamports"],
                "executable": poststate["executable"],
                "dataLen": poststate["dataLen"],
                "dataSha256": poststate["dataSha256"],
            }
        )
    projected = {
        "schema": value["schema"],
        "cluster": value["cluster"],
        "inputSha256": value["inputSha256"],
        "payoutIntentSha256": value["payoutIntentSha256"],
        "journalStateSha256": value["journalStateSha256"],
        "signature": value["signature"],
        "finalizedSlot": value["finalizedSlot"],
        "feeLamports": value["feeLamports"],
        "computeUnitsConsumed": value["computeUnitsConsumed"],
        "feePayer": value["feePayer"],
        "owner": value["owner"],
        "market": value["market"],
        "recipient": value["recipient"],
        "payout": value["payout"],
        "lookupTable": value["lookupTable"],
        "lookupAddressesSha256": value["lookupAddressesSha256"],
        "payoutInstructionSha256": value["payoutInstructionSha256"],
        "custodyRequestSha256": value["custodyRequestSha256"],
        "returnDataProducer": value["returnDataProducer"],
        "returnDataBase64": value["returnDataBase64"],
        "poststates": poststates,
        "evidenceSha256": "",
    }
    return sha256_bytes(
        json.dumps(projected, separators=(",", ":"), ensure_ascii=False).encode()
    )


def payout_complete(path: Path, payout: Mapping[str, Any], plan: LifecyclePlan) -> dict[str, Any] | None:
    if not path.exists():
        return None
    value = read_json(path, "wallet payout evidence")
    expected_keys = {
        "schema",
        "cluster",
        "inputSha256",
        "payoutIntentSha256",
        "journalStateSha256",
        "signature",
        "finalizedSlot",
        "feeLamports",
        "computeUnitsConsumed",
        "feePayer",
        "owner",
        "market",
        "recipient",
        "payout",
        "lookupTable",
        "lookupAddressesSha256",
        "payoutInstructionSha256",
        "custodyRequestSha256",
        "returnDataProducer",
        "returnDataBase64",
        "poststates",
        "evidenceSha256",
    }
    exact_keys(value, expected_keys, "wallet payout evidence")
    evidence_sha256 = payout_evidence_digest(value)
    decimal_text(value.get("payout"), "wallet payout atoms")
    payout_input = path.with_name("input.json")
    if (
        value.get("schema") != PAYOUT_EVIDENCE_SCHEMA
        or value.get("cluster") != "devnet"
        or value.get("market") != plan.expected_market
        or value.get("feePayer") != plan.fee_payer.address
        or value.get("owner") != payout["owner"]
        or value.get("recipient") != payout["recipient"]
        or SIGNATURE.fullmatch(str(value.get("signature", ""))) is None
        or decimal(value.get("finalizedSlot"), "wallet payout finalized slot", positive=True) <= 0
        or value.get("evidenceSha256") != evidence_sha256
        or not payout_input.exists()
        or value.get("inputSha256") != sha256_file(canonical_file(payout_input, "wallet payout input"))
    ):
        refuse("wallet payout evidence substituted its exact devnet intent or self-digest")
    return artifact(path, "wallet payout evidence")


def run_payout(
    driver: StageDriver,
    plan: LifecyclePlan,
    sources: BoundSources,
    row: dict[str, Any],
) -> None:
    if row["phase"] == "finalized":
        return
    payout = exact_object(row["payout"], f"{row['id']} payout")
    actor = actor_for_role(plan, payout["role"])
    root = driver.root / "payouts" / row["id"]
    root.mkdir(parents=True, exist_ok=True)
    journal_dir = root / "journal"
    journal_dir.mkdir(exist_ok=True)
    payout_input = root / "input.json"
    evidence = root / "evidence.json"
    ready = payout_complete(evidence, payout, plan)
    if ready is None and not payout_input.exists():
        produced = driver.invoke(
            row,
            child_base(sources)
            + ["wallet-terminal-payout-input"]
            + devnet_args(plan)
            + [
                "--plan",
                str(sources.successor_plan),
                "--evidence",
                str(sources.campaign_evidence),
                "--market",
                plan.expected_market,
                "--owner",
                payout["owner"],
                "--recipient",
                payout["recipient"],
                "--claim-index",
                str(payout["claimIndex"]),
                "--quantity",
                str(payout["quantityAtoms"]),
            ],
        )
        try:
            value = exact_object(json.loads(produced.stdout), "wallet payout input")
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            refuse(f"wallet payout input producer did not emit exact JSON: {error}")
        atomic_json(payout_input, value, new=True)
    if ready is not None:
        # A preexisting semantic receipt may have been left after the Rust
        # owner returned but before this outer journal finalized. Reopen it
        # through the Rust/RPC authenticator in read-only mode before trusting
        # it; the Python self-digest is a substitution detector, not authority.
        driver.invoke(
            row,
            payout_command(plan, sources, actor, payout_input, journal_dir, evidence, False),
        )
        ready = payout_complete(evidence, payout, plan)
    else:
        canonical_file(payout_input, "wallet payout input")
        for _ in range(MAX_CHILD_ATTEMPTS):
            driver.invoke(
                row,
                payout_command(
                    plan, sources, actor, payout_input, journal_dir, evidence, True
                ),
            )
            ready = payout_complete(evidence, payout, plan)
            if ready is not None:
                break
    if ready is None:
        refuse(f"{row['id']} did not reach its finalized payout evidence")
    ready["input"] = artifact(payout_input, "wallet payout input")
    driver.finalize(row, ready)


def payout_command(
    plan: LifecyclePlan,
    sources: BoundSources,
    actor: KeyActor,
    payout_input: Path,
    journal_dir: Path,
    evidence: Path,
    execute: bool,
) -> list[str]:
    command = (
        child_base(sources)
        + ["devnet-wallet-terminal-payout-v1"]
        + devnet_args(plan)
        + [
            "--input",
            str(payout_input),
            "--fee-payer",
            plan.fee_payer.address,
            "--fee-payer-keypair",
            str(plan.fee_payer.keypair),
            "--owner-keypair",
            str(actor.keypair),
            "--journal-dir",
            str(journal_dir),
            "--evidence",
            str(evidence),
        ]
    )
    if execute:
        command.append("--execute")
    return command


def children_complete(path: Path, plan: LifecyclePlan, sources: BoundSources) -> dict[str, Any] | None:
    if not path.exists():
        return None
    value = read_json(path, "Direct terminal children")
    positions = exact_list(value.get("positionChildren"), "Direct Position children")
    makers = exact_list(value.get("makerReplayChildren"), "Direct maker children")
    direct_digest = sources.direct_evidence_value.get("evidenceSha256")
    if (
        value.get("schema") != CHILDREN_SCHEMA
        or value.get("cluster") != "devnet"
        or value.get("market") != plan.expected_market
        or value.get("directEvidenceSha256") != plan.direct_evidence.sha256
        or value.get("directSemanticEvidenceSha256") != direct_digest
        or value.get("openMakerRootCount") != len(makers)
        or len(positions) != 2
        or {item.get("role") for item in positions} != {"seller", "buyer"}
        or {item.get("owner") for item in positions}
        != {plan.seller.address, plan.buyer.address}
        or {item.get("maker") for item in makers}
        != {plan.seller.address, plan.buyer.address}
    ):
        refuse("Direct terminal child projection changed its authenticated devnet identities")
    return artifact(path, "Direct terminal children")


def run_children(driver: StageDriver, plan: LifecyclePlan, sources: BoundSources) -> None:
    row = driver.stage("terminal-children")
    if row["phase"] == "finalized":
        return
    output = driver.root / "terminal-children.json"
    ready = children_complete(output, plan, sources)
    if ready is None:
        driver.invoke(
            row,
            child_base(sources)
            + ["devnet-direct-terminal-children-v1"]
            + devnet_args(plan)
            + [
                "--plan",
                str(sources.successor_plan),
                "--market-input",
                str(sources.market_input),
                "--campaign-evidence",
                str(sources.campaign_evidence),
                "--direct-evidence",
                str(plan.direct_evidence.path),
                "--output",
                str(output),
            ],
        )
        ready = children_complete(output, plan, sources)
    if ready is None:
        refuse("Direct child projection returned success without its exact receipt")
    driver.finalize(row, ready)


def position_complete(path: Path, role: str, plan: LifecyclePlan) -> dict[str, Any] | None:
    if not path.exists():
        return None
    value = read_json(path, f"{role} Position close evidence")
    actor = actor_for_role(plan, role)
    phase = value.get("phase")
    identity = value.get("plan") if phase == "finalized" else value
    identity = exact_object(identity, f"{role} Position close identity")
    finalized = value.get("finalized")
    if (
        value.get("schema") != POSITION_EVIDENCE_SCHEMA
        or value.get("cluster") != "devnet"
        or identity.get("owner") != actor.address
        or identity.get("market") != plan.expected_market
        or identity.get("sourceKind") != "direct-terminal"
        or identity.get("sourceSha256") != plan.direct_evidence.sha256
        or phase not in {"finalized", "already-closed"}
        or (
            phase == "finalized"
            and (
                not isinstance(finalized, dict)
                or SIGNATURE.fullmatch(str(finalized.get("signature", ""))) is None
                or decimal(
                    finalized.get("slot"),
                    f"{role} Position close finalized slot",
                    positive=True,
                )
                <= 0
            )
        )
    ):
        refuse(f"{role} Position close evidence changed its exact devnet identity")
    return {
        **artifact(path, f"{role} Position close evidence"),
        "phase": phase,
        "recoveryWithoutOriginalSignature": phase == "already-closed",
    }


def run_position_close(
    driver: StageDriver,
    plan: LifecyclePlan,
    sources: BoundSources,
    role: str,
) -> None:
    row = driver.stage(f"position-close-{role}")
    if row["phase"] == "finalized":
        return
    actor = actor_for_role(plan, role)
    output = driver.root / f"position-close-{role}.json"
    ready = position_complete(output, role, plan)
    if ready is None:
        driver.invoke(
            row,
            child_base(sources)
            + ["devnet-user-position-close-v1"]
            + devnet_args(plan)
            + [
                "--direct-evidence",
                str(plan.direct_evidence.path),
                "--plan",
                str(sources.successor_plan),
                "--market-input",
                str(sources.market_input),
                "--campaign-evidence",
                str(sources.campaign_evidence),
                "--position-owner",
                actor.address,
                "--fee-payer",
                plan.fee_payer.address,
                "--evidence",
                str(output),
                "--execute",
                "--position-owner-keypair",
                str(actor.keypair),
                "--fee-payer-keypair",
                str(plan.fee_payer.keypair),
            ],
        )
        ready = position_complete(output, role, plan)
    if ready is None:
        refuse(f"{role} Position close returned success without evidence")
    driver.finalize(row, ready)


def terminal_paths(root: Path) -> tuple[Path, Path, Path]:
    journal = root / "terminal-journal"
    return root / "terminal-session.json", journal, root / "terminal-completion.json"


def terminal_command(plan: LifecyclePlan, sources: BoundSources, root: Path) -> list[str]:
    session, journal, completion = terminal_paths(root)
    journal.mkdir(exist_ok=True)
    command = (
        child_base(sources)
        + ["devnet-terminal-sequence-v1"]
        + devnet_args(plan)
        + [
            "--plan",
            str(sources.successor_plan),
            "--market-input",
            str(sources.market_input),
            "--evidence",
            str(sources.campaign_evidence),
            "--market",
            plan.expected_market,
            "--fee-payer",
            plan.fee_payer.address,
            "--fee-payer-keypair",
            str(plan.fee_payer.keypair),
            "--session",
            str(session),
            "--journal-dir",
            str(journal),
            "--completion",
            str(completion),
            "--execute",
        ]
    )
    if plan.refreshed_evidence is not None:
        command.extend(["--refreshed-evidence", str(plan.refreshed_evidence.path)])
    if plan.terminal_lookup_table is not None:
        command.extend(["--lookup-table", plan.terminal_lookup_table])
    return command


def terminal_journal_complete(root: Path, name: str) -> dict[str, Any] | None:
    path = root / "terminal-journal" / name
    if not path.exists():
        return None
    value = read_json(path, f"terminal journal {name}")
    if value.get("schema") != "dclutch-devnet-terminal-sequence-journal-v1" or value.get(
        "phase"
    ) != "finalized":
        return None
    return artifact(path, f"terminal journal {name}")


def run_terminal_prefix(driver: StageDriver, plan: LifecyclePlan, sources: BoundSources) -> None:
    row = driver.stage("terminal-through-direct-retiring")
    if row["phase"] == "finalized":
        return
    ready = terminal_journal_complete(driver.root, "11-direct-begin-retiring.json")
    for _ in range(MAX_CHILD_ATTEMPTS):
        if ready is not None:
            break
        driver.invoke(row, terminal_command(plan, sources, driver.root))
        ready = terminal_journal_complete(driver.root, "11-direct-begin-retiring.json")
    if ready is None:
        refuse("terminal sequence did not reach authenticated Direct Retiring")
    driver.finalize(row, ready)


def direct_generation(sources: BoundSources) -> int:
    manifest = read_json(sources.public_manifest, "Direct public manifest")
    context = exact_object(manifest.get("context"), "Direct public manifest context")
    return decimal(context.get("generation"), "Direct generation")


def maker_complete(
    path: Path,
    role: str,
    plan: LifecyclePlan,
    sources: BoundSources,
) -> dict[str, Any] | None:
    if not path.exists():
        return None
    value = read_json(path, f"{role} maker-close evidence")
    landed = value.get("landed")
    already_closed = value.get("alreadyClosed") is True
    maker_plan = value.get("plan")
    children = read_json(path.with_name("terminal-children.json"), "Direct terminal children")
    maker_children = exact_list(children.get("makerReplayChildren"), "Direct maker children")
    expected = [row for row in maker_children if row.get("maker") == actor_for_role(plan, role).address]
    if (
        value.get("schema") != "dclutch-direct-close-maker-evidence-v1"
        or value.get("cluster") != "devnet"
        or value.get("market") != plan.expected_market
        or value.get("directEvidenceSha256") != plan.direct_evidence.sha256
        or value.get("directRoot") != children.get("directRoot")
        or value.get("generation") != direct_generation(sources)
        or len(expected) != 1
        or value.get("makerReplay") != expected[0].get("replay")
        or (already_closed and (maker_plan is not None or landed is not None))
        or (
            not already_closed
            and (
                not isinstance(maker_plan, dict)
                or maker_plan.get("maker") != actor_for_role(plan, role).address
                or not isinstance(landed, dict)
                or SIGNATURE.fullmatch(str(landed.get("signature", ""))) is None
            )
        )
    ):
        refuse(f"{role} maker-close evidence changed its devnet identity")
    return {
        **artifact(path, f"{role} maker-close evidence"),
        "alreadyClosed": already_closed,
    }


def maker_command(
    plan: LifecyclePlan,
    sources: BoundSources,
    role: str,
    output: Path,
) -> list[str]:
    actor = actor_for_role(plan, role)
    return (
        child_base(sources)
        + ["devnet-direct-close-maker-v1"]
        + devnet_args(plan)
        + [
            "--plan",
            str(sources.successor_plan),
            "--market-input",
            str(sources.market_input),
            "--campaign-evidence",
            str(sources.campaign_evidence),
            "--direct-evidence",
            str(plan.direct_evidence.path),
            "--market",
            plan.expected_market,
            "--maker",
            actor.address,
            "--evidence",
            str(output),
            "--execute",
            "--fee-payer-keypair",
            str(plan.fee_payer.keypair),
        ]
    )


def run_maker_close(
    driver: StageDriver,
    plan: LifecyclePlan,
    sources: BoundSources,
    role: str,
) -> None:
    row = driver.stage(f"maker-close-{role}")
    if row["phase"] == "finalized":
        return
    output = driver.root / f"maker-close-{role}.json"
    ready = maker_complete(output, role, plan, sources)
    if ready is None:
        driver.invoke(row, maker_command(plan, sources, role, output))
        ready = maker_complete(output, role, plan, sources)
    if ready is None:
        refuse(f"{role} maker replay close returned success without evidence")
    driver.finalize(row, ready)


def terminal_completion(root: Path, plan: LifecyclePlan) -> dict[str, Any] | None:
    _, _, path = terminal_paths(root)
    if not path.exists():
        return None
    value = read_json(path, "terminal completion")
    journals = exact_list(value.get("journals"), "terminal completion journals")
    if (
        value.get("schema") != TERMINAL_COMPLETION_SCHEMA
        or value.get("status") != "finalized"
        or value.get("cluster") != "devnet"
        or value.get("genesisHash") != DEVNET_GENESIS_HASH
        or value.get("market") != plan.expected_market
        or not journals
        or exact_object(
            journals[-1].get("mutation"), "terminal completion final mutation"
        ).get("kind")
        != "aggregate-retirement"
        or journals[-1].get("phase") != "finalized"
    ):
        return None
    return artifact(path, "terminal completion")


def authenticated_stage_result(
    row: Mapping[str, Any],
    root: Path,
    plan: LifecyclePlan,
    sources: BoundSources,
) -> dict[str, Any]:
    stage_id = text(row.get("id"), "lifecycle stage id")
    kind = text(row.get("kind"), f"{stage_id} kind")
    result: dict[str, Any] | None
    if kind == "fee-settlement":
        result = finalized_fee_evidence(root / "fee-settlement.json", plan)
    elif kind == "resolution":
        result = resolution_complete(root / "resolution-checkpoint.json", plan)
    elif kind == "payout":
        payout = exact_object(row.get("payout"), f"{stage_id} payout")
        payout_root = root / "payouts" / stage_id
        result = payout_complete(payout_root / "evidence.json", payout, plan)
        if result is not None:
            result["input"] = artifact(payout_root / "input.json", "wallet payout input")
    elif kind == "terminal-children":
        result = children_complete(root / "terminal-children.json", plan, sources)
    elif kind == "position-close":
        role = text(row.get("role"), f"{stage_id} role")
        result = position_complete(root / f"position-close-{role}.json", role, plan)
    elif kind == "terminal-prefix":
        result = terminal_journal_complete(root, "11-direct-begin-retiring.json")
    elif kind == "maker-close":
        role = text(row.get("role"), f"{stage_id} role")
        result = maker_complete(root / f"maker-close-{role}.json", role, plan, sources)
    elif kind == "terminal-finish":
        result = terminal_completion(root, plan)
    else:
        refuse(f"lifecycle stage {stage_id} has unknown kind {kind}")
    if result is None:
        refuse(f"finalized lifecycle stage {stage_id} lost its semantic evidence")
    return result


def run_terminal_finish(driver: StageDriver, plan: LifecyclePlan, sources: BoundSources) -> None:
    row = driver.stage("terminal-through-core-retired")
    if row["phase"] == "finalized":
        return
    ready = terminal_completion(driver.root, plan)
    for _ in range(MAX_CHILD_ATTEMPTS):
        if ready is not None:
            break
        driver.invoke(row, terminal_command(plan, sources, driver.root))
        ready = terminal_completion(driver.root, plan)
    if ready is None:
        refuse("terminal sequence did not reach aggregate retirement/Core Retired")
    driver.finalize(row, ready)


def transaction_rows(root: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []

    def add(label: str, value: Mapping[str, Any]) -> None:
        signature = value.get("signature")
        slot = value.get("slot", value.get("finalizedSlot"))
        if not isinstance(signature, str) or SIGNATURE.fullmatch(signature) is None:
            refuse(f"{label} omitted a canonical finalized signature")
        if isinstance(slot, str) and slot.isdigit():
            slot = int(slot)
        if isinstance(slot, bool) or not isinstance(slot, int) or slot <= 0:
            refuse(f"{label} omitted a positive finalized slot")
        rows.append({"label": label, "signature": signature, "finalizedSlot": slot})

    fee = read_json(root / "fee-settlement.json", "fee settlement")
    add("direct-fee-settlement", exact_object(fee["landed"], "fee landed"))
    resolution = read_json(root / "resolution-checkpoint.json", "resolution checkpoint")
    for receipt in exact_list(resolution["receipts"], "resolution receipts"):
        add(f"resolution/{receipt['stage']}", exact_object(receipt, "resolution receipt"))
    for evidence in sorted((root / "payouts").glob("*/evidence.json")):
        add(f"payout/{evidence.parent.name}", read_json(evidence, "payout evidence"))
    for role in ("seller", "buyer"):
        value = read_json(root / f"position-close-{role}.json", f"{role} close")
        if value.get("phase") == "finalized":
            add(
                f"position-close/{role}",
                exact_object(value["finalized"], f"{role} close finalized"),
            )
        maker = read_json(root / f"maker-close-{role}.json", f"{role} maker close")
        if isinstance(maker.get("landed"), dict):
            add(f"maker-close/{role}", maker["landed"])
    completion = read_json(root / "terminal-completion.json", "terminal completion")
    for entry in exact_list(completion["journals"], "terminal journals"):
        signature = entry.get("signature")
        if isinstance(signature, str) and signature:
            mutation = exact_object(entry["mutation"], "terminal mutation")
            add(f"terminal/{text(mutation.get('kind'), 'terminal mutation kind')}", entry)
    signatures = [row["signature"] for row in rows]
    if len(signatures) != len(set(signatures)):
        refuse("complete-life transaction ledger repeated a finalized signature")
    return rows


def report_digest(report: Mapping[str, Any]) -> str:
    value = dict(report)
    value["reportSha256"] = ""
    return sha256_bytes(canonical_json(value))


def publish_exact_report(path: Path, report: Mapping[str, Any]) -> None:
    if path.exists() or path.is_symlink():
        existing = read_json(path, "persisted Direct complete-life report")
        if existing != report:
            refuse("persisted Direct complete-life report differs from reauthenticated evidence")
        return
    atomic_json(path, report, new=True)


def write_report(
    root: Path,
    plan: LifecyclePlan,
    sources: BoundSources,
    journal: Mapping[str, Any],
) -> Path:
    report_path = root / "DIRECT_COMPLETE_LIFE.json"
    report: dict[str, Any] = {
        "schema": REPORT_SCHEMA,
        "status": "finalized",
        "evidenceLevel": "exact-source-public-devnet-execution",
        "notMainnetEvidence": True,
        "genesisHash": DEVNET_GENESIS_HASH,
        "rpcUrl": plan.rpc_url,
        "market": plan.expected_market,
        "sourceRevision": sources.pack["source"]["revision"],
        "sourceTreeSha256": sources.pack["source"]["tree_sha256"],
        "inputs": {
            "plan": artifact(plan.path, "complete-life plan"),
            "releasePack": artifact(plan.release_pack.path, "release pack"),
            "publicRouteCampaign": artifact(plan.public_campaign.path, "public route campaign"),
            "producerJournal": artifact(sources.producer_journal, "producer journal"),
            "directEvidence": artifact(plan.direct_evidence.path, "Direct evidence"),
            "resolutionInput": artifact(plan.resolution_input.path, "resolution input"),
            "refreshedEvidence": (
                None
                if plan.refreshed_evidence is None
                else artifact(plan.refreshed_evidence.path, "refreshed evidence")
            ),
        },
        "actors": {
            "permissionlessFeePayer": plan.fee_payer.address,
            "seller": plan.seller.address,
            "buyer": plan.buyer.address,
        },
        "payoutSchedule": [dict(row) for row in sources.payout_rows],
        "journal": artifact(root / "RUN.json", "complete-life journal"),
        "stages": journal["stages"],
        "transactions": transaction_rows(root),
        "terminalCompletion": artifact(
            root / "terminal-completion.json", "terminal completion"
        ),
        "reportSha256": "",
    }
    report["reportSha256"] = report_digest(report)
    publish_exact_report(report_path, report)
    return report_path


def execute(plan: LifecyclePlan, root: Path, sources: BoundSources) -> Path:
    journal_path = root / "RUN.json"
    if journal_path.exists():
        journal = authenticate_journal(
            read_json(journal_path, "complete-life journal"), plan, sources, root
        )
    else:
        journal = initial_journal(plan, sources)
        atomic_json(journal_path, journal, new=True)
    driver = StageDriver(root, journal_path, journal)
    run_fee(driver, plan, sources)
    run_resolution(driver, plan, sources)
    for row in driver.journal["stages"]:
        if row["kind"] == "payout":
            run_payout(driver, plan, sources, row)
    run_children(driver, plan, sources)
    run_position_close(driver, plan, sources, "seller")
    run_position_close(driver, plan, sources, "buyer")
    run_terminal_prefix(driver, plan, sources)
    run_maker_close(driver, plan, sources, "seller")
    run_maker_close(driver, plan, sources, "buyer")
    run_terminal_finish(driver, plan, sources)
    driver.journal["status"] = "finalized"
    driver.persist()
    authenticated = authenticate_journal(driver.journal, plan, sources, root)
    return write_report(root, plan, sources, authenticated)


def verify_report(path: Path) -> None:
    if path.name != "DIRECT_COMPLETE_LIFE.json":
        refuse("Direct complete-life report must retain its canonical filename")
    report = read_json(path, "Direct complete-life report")
    expected = {
        "schema",
        "status",
        "evidenceLevel",
        "notMainnetEvidence",
        "genesisHash",
        "rpcUrl",
        "market",
        "sourceRevision",
        "sourceTreeSha256",
        "inputs",
        "actors",
        "payoutSchedule",
        "journal",
        "stages",
        "transactions",
        "terminalCompletion",
        "reportSha256",
    }
    exact_keys(report, expected, "Direct complete-life report")
    if (
        report["schema"] != REPORT_SCHEMA
        or report["status"] != "finalized"
        or report["evidenceLevel"] != "exact-source-public-devnet-execution"
        or report["notMainnetEvidence"] is not True
        or report["genesisHash"] != DEVNET_GENESIS_HASH
        or report["reportSha256"] != report_digest(report)
    ):
        refuse("Direct complete-life report header or digest changed")
    inputs = exact_object(report["inputs"], "report inputs")
    exact_keys(
        inputs,
        {
            "plan",
            "releasePack",
            "publicRouteCampaign",
            "producerJournal",
            "directEvidence",
            "resolutionInput",
            "refreshedEvidence",
        },
        "report inputs",
    )
    plan_path = verify_artifact(inputs["plan"], "complete-life plan")
    plan = parse_plan(plan_path)
    sources = bind_sources(plan)
    actors = exact_object(report["actors"], "report actors")
    exact_keys(actors, {"permissionlessFeePayer", "seller", "buyer"}, "report actors")
    if (
        report["rpcUrl"] != plan.rpc_url
        or report["market"] != plan.expected_market
        or report["sourceRevision"] != sources.pack["source"]["revision"]
        or report["sourceTreeSha256"] != sources.pack["source"]["tree_sha256"]
        or actors
        != {
            "permissionlessFeePayer": plan.fee_payer.address,
            "seller": plan.seller.address,
            "buyer": plan.buyer.address,
        }
    ):
        refuse("complete-life report source, RPC, Market, or actor binding changed")
    expected_inputs = {
        "releasePack": plan.release_pack.path,
        "publicRouteCampaign": plan.public_campaign.path,
        "producerJournal": sources.producer_journal,
        "directEvidence": plan.direct_evidence.path,
        "resolutionInput": plan.resolution_input.path,
    }
    for name, expected_path in expected_inputs.items():
        if verify_artifact(inputs[name], f"report {name}") != expected_path:
            refuse(f"report {name} path differs from its source-bound input")
    refreshed = inputs["refreshedEvidence"]
    if plan.refreshed_evidence is None:
        if refreshed is not None:
            refuse("report introduced refreshed evidence absent from the plan")
    elif verify_artifact(refreshed, "report refreshed evidence") != plan.refreshed_evidence.path:
        refuse("report refreshed evidence path differs from the plan")
    root = path.parent
    journal_path = verify_artifact(report["journal"], "complete-life journal")
    if journal_path != root / "RUN.json":
        refuse("complete-life report names a journal outside its output root")
    journal = authenticate_journal(
        read_json(journal_path, "complete-life journal"), plan, sources, root
    )
    if journal["status"] != "finalized" or report["stages"] != journal["stages"]:
        refuse("complete-life report and finalized journal diverged")
    if report["payoutSchedule"] != [dict(row) for row in sources.payout_rows]:
        refuse("complete-life report Market or payout schedule changed")
    if report["transactions"] != transaction_rows(root):
        refuse("complete-life transaction ledger changed from child evidence")
    completion = verify_artifact(
        report["terminalCompletion"], "terminal completion"
    )
    if completion != root / "terminal-completion.json" or terminal_completion(root, plan) is None:
        refuse("complete-life report omitted exact aggregate retirement completion")


def parser() -> argparse.ArgumentParser:
    top = argparse.ArgumentParser(description=__doc__)
    commands = top.add_subparsers(dest="command", required=True)
    run = commands.add_parser("run", help="resume the exact devnet Direct suffix")
    run.add_argument("--plan", required=True)
    run.add_argument("--output-root", required=True)
    verify = commands.add_parser("verify", help="rehash one completed suffix report")
    verify.add_argument("--report", required=True)
    return top


def main(argv: Sequence[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
        if arguments.command == "run":
            plan = parse_plan(Path(arguments.plan))
            sources = bind_sources(plan)
            report = execute(plan, output_root(Path(arguments.output_root)), sources)
            verify_report(report)
            print(f"Direct complete-life report={report}")
            print(f"Direct complete-life sha256={sha256_file(report)}")
        else:
            report = canonical_file(arguments.report, "Direct complete-life report")
            verify_report(report)
            print(f"Direct complete-life verified sha256={sha256_file(report)}")
        return 0
    except (OSError, Refusal, pack_tool.Refusal, public_tool.Refusal, ValueError) as error:
        print(f"DEVNET DIRECT COMPLETE-LIFE REFUSED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
