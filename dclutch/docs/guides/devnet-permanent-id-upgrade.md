# Update a permanent devnet program

You can update one of the seven permanent devnet program ids without closing or
recycling it. The operator makes every write one role at a time. The tool accepts
only Solana devnet, requires the cluster's exact genesis hash, and requires a
second acknowledgement naming the role and program id you will change.
Mutation also requires an exact acknowledgement that you kept the named fee
payer exclusive from the first finalized wallet snapshot through the final
poststate.

The tool never discovers a wallet. You give it exact authority, fee-payer, and
persistent-Buffer keypair paths. It journals the action and an exact Buffer
writer process lease before checking any key file. The Solana CLI receives the
three keypair paths only for that separately armed Buffer upload. For the final
Loader action, the operator opens the authority and payer signing keys only
after the exact message is durable.

## 1. Build and check the candidate

Run `tools/release/checked-release-candidate.sh` with a new absolute work root.
Do not create or edit Upgrade acceptance JSON yourself. The runner emits
`CHECKED_UPGRADE_GATE.json` only after all thirteen shipped links freshly
compile, report zero SBF overwrite diagnostics, and pass the static frame scan
below the 4,096-byte SBPF v0 bound.

The generated gate binds the exact source commit and tree, link identities,
run stamps, top-package compile markers, diagnostic rows, frame-build logs,
frame-report counts and hashes, every release ELF, and every checked manifest.
Keep the complete work root together. You may move that directory as one unit;
do not move individual files inside it, replace them with symlinks, or edit a
log or report. Record the gate SHA-256 printed by the runner separately. The
Upgrade command requires that exact digest, rehashes every referenced file,
and refuses path escapes, symlinks, missing or unknown roles, changed evidence,
and an ELF path that is not the selected role's exact file inside the gate.

The old handwritten `checked_release_accepted: true` shape has no authority.
Zeroes typed into a JSON file are not build or frame evidence.

## 2. Capture a fresh baseline

First capture the complete permanent substrate with
[`devnet-permanent-substrate-capture-v1`](devnet-release-capture.md). Run it at
an immediate finalized floor before the first Upgrade. Its single account
context must authenticate all seven fixed Program/ProgramData pairs, the
retained authority, the Program residue and parked ProgramData lamport totals,
and the exact fee-payer balance.
This closes the gap that five separate role baselines cannot close: none of
those per-role reads alone proves that the other six permanent deployments and
the payer were one coherent pre-write state. The capture is key-free and has no
caller-supplied Program surface.

Use `devnet-upgrade-baseline-v1` with the candidate raw ELF byte length and a recent
finalized context-slot floor. This command is key-free and read-only. After the
health and genesis checks, it reads Program and ProgramData together in one
finalized snapshot and makes two bounded rent queries for the current and target
sizes. If the current allocation is wider, the baseline retains that live width
and the Upgrade command derives the only admitted padding: zero bytes from the
end of the gate-bound raw ELF to the existing allocation boundary.

The baseline's canonical digest commits:

- the complete role order and the selected role;
- exact devnet genesis and finalized context slot;
- Program and ProgramData addresses, owners, executable flags, lengths,
  lamports, and account digests;
- the Loader deployment slot and retained authority;
- the complete live payload digest;
- the target width, additional bytes, current and target rent minima, and exact
  rent top-up.

The preflight and update commands read Program, ProgramData, and the explicit
fee payer together with one finalized `getMultipleAccounts` call. Every
prestate and poststate call carries a nonzero `minContextSlot`; no result may
mix accounts from different RPC contexts. The update requires every baseline
fact to remain exact.

## 3. Extend only when the candidate does not fit

The original deployments have no unused capacity. If the baseline reports a
nonzero `extension_additional_bytes`, run `devnet-upgrade-extend-v1` as its own
acknowledged act. Your acknowledgement has this exact value:

```text
ROLE:PROGRAM_ID:+ADDITIONAL_BYTES
```

Give Extend its own durable receipt path, for example
`--receipt /absolute/evidence/ROLE-extension-receipt.json`. Never use the
Upgrade receipt path fixed in the deployment-set row. The tool refuses that
collision so an Extension receipt cannot replace the row's Upgrade journal.

The command refuses a zero extension and requires the payer to cover the exact
rent top-up plus a provisional 1,000,000-lamport fee reserve. It also requires
the exact deployment-set journal. That journal must name this permanent pair as
the first incomplete Upgrade in the fixed Custody, Resolution, Claims, Trading,
Core order. Registry and Rent are CarryForward and cannot enter this mutation
path.

The operator constructs one canonical Loader-v3 checked-extension message. It
fsyncs the unsigned message and last valid block height before reading either
signing key, verifies and fsyncs the signed packet, writes `submitted` before
one `sendTransaction` call with `maxRetries=0`, and then requires all of these
postconditions:

- ProgramData space increased by exactly the acknowledged byte count;
- the old payload is unchanged and only zero capacity was appended;
- ProgramData lamports increased by exactly the quoted rent top-up;
- the retained authority and Program linkage did not move;
- the Loader deployment slot advanced;
- one finalized transaction at that exact slot contains one parsed checked
  Loader extension for the exact Program, ProgramData, authority, payer, and
  byte count;
- that transaction's balance vectors prove payer spend equals ProgramData rent
  delta plus the transaction fee.

The receipt owns the signature of the exact packet it sent. A restart queries
that signature with history enabled and never infers its status from account
state alone. Finalized success must resolve to the exact transaction. Finalized
failure stops for fee attribution, and a still-present pending signature stays
poll-only even after its blockhash lifetime. Only a history-null signature past
its last valid finalized block height, with Program, ProgramData, payer, and the
current deployment-set plan still exact, can be archived before a new
blockhash is prepared.

After a complete extension, retain its separate receipt, discard the old
baseline, and capture a fresh baseline at the new Loader slot. Replace only the
current role's baseline path and SHA-256 in the deployment-set journal, then run
the set audit again. That closes the Extension plan and begins a new immutable
Upgrade plan; do this before creating the Upgrade receipt. Extension advances
the Loader deployment slot, and Upgrade advances it again. You mint and
activate release records only from the final post-Upgrade ProgramData
observation.

## 4. Run the read-only preflight

Run the same `devnet-upgrade-v1` arguments with `--preflight` instead of
`--execute`. Exactly one mode is required.

Preflight is structurally key-free: it does not invoke `solana address`, open a
keypair path, call `solana program`, create a receipt, or create a dump. It does
rehash the complete checked-release gate, authenticate the pinned CLI version
and exact devnet genesis, validate the source and baseline, derive the only
admitted live image, and read Program, ProgramData, and payer in one finalized
context. Its JSON report always carries `mutation_permitted: false`.

If a receipt already exists, preflight validates its canonical phase digest. An
intermediate receipt must carry exactly the fields allowed for its phase. A
complete receipt must also match the current Loader accounts, exact journaled
transaction, arithmetic, and existing dump. Preflight never resumes or writes
any phase.

## 5. Upgrade the existing id

Run `devnet-upgrade-v1` with the new baseline, generated checked-release gate,
its separately recorded digest, the exact source commit/tree, the gate's role
ELF, output dump, receipt, fee payer, retained authority, deployment-set
journal, and a dedicated persistent Buffer keypair plus its explicit public
key. The Buffer public key must be non-default. A Buffer account that already
exists without this operation's receipt is ambiguous and refused. Your second
acknowledgement has this exact value:

```text
ROLE:PROGRAM_ID
```

Your execute-only payer acknowledgement has this exact value:

```text
--i-kept-fee-payer-exclusive ROLE:PROGRAM_ID:FEE_PAYER
```

From the finalized pre-observation until the finalized post-observation, do not
use that payer for any unrelated transaction. This is an operator-stated
exclusive window, not a fact the chain can prove after the fact.

After the preflight passes, repeat the exact command with `--execute`. Include
these durable-action inputs:

```text
--buffer-pubkey PERSISTENT_BUFFER_PUBLIC_KEY
--buffer-keypair /absolute/evidence/persistent-buffer-keypair.json
--deployment-set-journal /absolute/evidence/deployment-set-journal.json
```

Pass these source admissions exactly as printed by the release runner:

```text
--checked-release-gate ABSOLUTE_WORK_ROOT/CHECKED_UPGRADE_GATE.json
--expected-checked-release-gate-sha256 64_LOWERCASE_HEX
--expected-source-revision 40_LOWERCASE_HEX
--expected-source-tree-sha256 64_LOWERCASE_HEX
```

Before any key access, the new receipt binds the immutable deployment-set plan
digest, Buffer public key and keypair path, raw ELF digest and length, authority,
expected rent, finalized Program/ProgramData/payer prestate, and conservative
Buffer-upload expiry height. The plan digest includes every fixed row, path,
baseline, CarryForward pin, gate, source, authority, and payer, while excluding
only the five completion receipt/dump digests that you fill in later. It remains
stable as the set advances.

For each Buffer attempt, a private-process-group supervisor first publishes and
fsyncs a no-clobber lease containing its PID, process-group ID, C/UTC
operating-system start token, random process nonce, exact command digest,
operation ID, and attempt number. The receipt binds and fsyncs that lease before
the supervisor receives its exact-content, regular-file permit and before any
key check or CLI mutation. A stale or symlink permit refuses. The supervisor
then `exec`s only this CLI mutation so the leased PID remains the exact writer,
once per leased window:

```text
solana program write-buffer EXACT_ELF \
  --buffer EXACT_PERSISTENT_BUFFER_KEYPAIR \
  --buffer-authority EXACT_AUTHORITY_KEYPAIR \
  --fee-payer EXACT_PAYER_KEYPAIR \
  --max-len EXACT_RAW_ELF_LENGTH \
  --max-sign-attempts 1 --output json --url EXACT_DEVNET_URL
```

That phase cannot invoke Upgrade or Extend. If the operator process crashes,
recovery checks the exact PID, start token, random process identity, and private
process group. It never signals a reused PID, attaches a Buffer, or starts a
second writer while the leased process is alive. If CLI output is lost,
recovery waits for the exact leased process to exit and uses the same Buffer
identity. A complete Buffer can attach only after its
Loader owner, metadata authority, byte width, rent, and full payload digest are
exact. The operator walks bounded finalized signature history back across the
prestate and authenticates every transaction: exact payer and signatures, one
exact create plus initialize, only idempotent writes whose offsets and bytes
slice the checked ELF, no unrelated program, exact rent, and every successful
or failed transaction fee. The payer delta must equal Buffer rent plus all
those fees. Every expired attempt remains in the receipt with its lease and exit
boundary. A missing or partial Buffer cannot start a second writer until the
conservative finalized-height window expires and the exact previous process is
proven gone.

After the Buffer is authenticated, the operator constructs the canonical
Loader-v3 Upgrade message consuming that exact Buffer and spilling its rent to
the explicit payer. It fsyncs the message before reading authority or payer
keys, verifies and fsyncs the signed transaction packet, writes `submitted`,
and calls `sendTransaction` once with `maxRetries=0`. A restart queries the
exact journaled signature with transaction history enabled. Finalized failure
or payer movement stops for attribution; a pending signature remains poll-only.
Only an absent signature past its last-valid height can be archived, and only
while the full Loader, payer, authenticated Buffer, and current immutable set-plan
prestate remain exact.

After finalization, the command rechecks the cluster and reads Program,
ProgramData, and payer in one finalized context. It requires the deployment
slot to advance, the live payload to equal the checked raw ELF plus zero
padding, and parked ProgramData rent to remain unchanged. The exact finalized
transaction must contain one Loader-v3 Upgrade for the journaled Program,
ProgramData, authority, Buffer, payer spill, rent sysvar, and clock sysvar. Its
balances must prove:

```text
transaction_payer_post = transaction_payer_pre + buffer_rent - final_upgrade_fee
operation_observed_net_spend = buffer_upload_fees + final_upgrade_fee
unattributed_cli_net_cost_lamports = buffer_upload_fees
```

`unattributed_cli_net_cost_lamports` is a legacy field name. Its value is now
exactly attributed by the authenticated Buffer history; Buffer rent is funded
during upload and refunded by the final Upgrade. ProgramData lamports do not
move. The receipt bridges the payer's post-upload balance to the final
transaction prebalance and the final transaction postbalance to the finalized
wallet observation.

Finally, the operator dumps to an operation-owned temporary file, validates the
raw or exact zero-padded bytes, fsyncs it, and publishes the final dump without
clobbering an existing path. A partial temporary is never final evidence and is
safely recreated on resume.

## 6. Audit the mixed deployment set

Use `devnet-deployment-set-journal-v2` to see which execution role is next.
This command is key-free and read-only. It has no `--execute`, keypair,
signing, loop, or receipt-writing mode. It never replaces
`devnet-upgrade-v1`: you still preflight and update exactly one role with that
command. Registry and Rent are not Upgrade targets in this iteration. They are
explicit `carry-forward` rows authenticated from the existing DEPLOY-1 state.

The deployment-set journal is a phase-scoped reference manifest, not Upgrade
acceptance. Once an Extend or Upgrade receipt exists, every plan field it binds
is immutable for that in-flight mutation. You
may write it yourself because none of its fields can make a role complete. It
contains no copied Loader poststate and no success boolean. A completed role is
recognized only when its pinned one-role receipt and dump both rehash exactly,
the receipt passes the normal checked-gate binding, and a fresh finalized read
matches its exact Program, ProgramData, payload, authority, slot, transaction,
arithmetic, and dump. Registry and Rent are recognized only when one pinned
`getMultipleAccounts` snapshot rehashes and a fresh one-request finalized read
matches all nine accounts: both Loader pairs, both artifact raw records, both
absent staging accounts, and the singleton infrastructure profile. A staging
account must be RPC `null`; a fabricated System-owned empty account is not
absence.

Capture that bounded nine-account snapshot with
[`devnet-carry-forward-capture-v1`](devnet-release-capture.md). After the five
Upgrades, use the same guide's ProgramData capture step for the five changed
roles before preparing the release plan. Do not hand-assemble either body.

The manifest has this shape:

```json
{
  "schema": "dclutch-devnet-deployment-set-journal-v2",
  "checked_release_gate": {
    "canonical_path": "/absolute/release/CHECKED_UPGRADE_GATE.json",
    "sha256": "64_lowercase_hex"
  },
  "source_revision": "40_lowercase_hex",
  "source_tree_sha256": "64_lowercase_hex",
  "devnet_genesis_hash": "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG",
  "solana_cli_version": "exact output of solana --version",
  "retained_upgrade_authority": "PUBLIC_KEY",
  "fee_payer": "PUBLIC_KEY",
  "infrastructure_carry_forward": {
    "canonical_path": "/absolute/evidence/carry-forward-snapshot.json",
    "sha256": "64_lowercase_hex"
  },
  "roles": [
    {
      "role": "registry",
      "disposition": "carry-forward",
      "program_id": "Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj",
      "programdata_id": "ENRSwrUEymWaXyrNtyD4QXXXk3tsTmcTGPTUFvnpsRVz",
      "baseline": null,
      "receipt": {
        "canonical_path": "/absolute/evidence/registry-receipt.json",
        "sha256": null
      },
      "dump": {
        "canonical_path": "/absolute/evidence/live-registry.so",
        "sha256": "e1f4a20f0fefb60ad8f809f153c4403363d298d5eb11b88e29abe404048ac6e1"
      }
    }
  ]
}
```

The example shows one CarryForward role only. Your manifest must contain all
seven entries in this exact order and with these permanent pairs and tags:

| order | role | tag | Program | ProgramData |
|---:|---|---|---|---|
| 0 | registry | `carry-forward` | `Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj` | `ENRSwrUEymWaXyrNtyD4QXXXk3tsTmcTGPTUFvnpsRVz` |
| 1 | rent | `carry-forward` | `DgfYeuorJUmnktxgCmUXy65f6MFBGcc1aMQoauxoJCY3` | `78MW6W4iPzBVLceAwTL51CtyLcpcFM2iGVMDbzZtUFmy` |
| 2 | custody | `upgrade` | `34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH` | `EhB7hHJ7vsCW3nCeqbxbJrn5Jsi6gbqwpVhoLMPZ8ENf` |
| 3 | resolution | `upgrade` | `2GHmxBawHTmwDRzqXuqdeC9A9Gj2HzucRd29wGpfgzmd` | `2QFBQJdLBXAnJWTVK8KeeUtWZEFhQqqN2CbkrWjMjY6f` |
| 4 | claims | `upgrade` | `85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN` | `4La2511ddSxUcAQfdhKvEeGEasih3TStbQWVFEQKd34j` |
| 5 | trading | `upgrade` | `5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk` | `AE1cWbCvXedE23XH3otSxvDQ7xVx7WLNMYDc8y8rqkrn` |
| 6 | core | `upgrade` | `HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N` | `AD6mb5SP6yqc5GFexf3xhpr1wKaZQhS7Hrt41iZhKxaN` |

Use canonical absolute paths, as printed by `realpath`; a symlink in any path is
refused. Registry and Rent have no baseline or receipt and pin their exact live
DEPLOY-1 ELF dumps. Each Upgrade role pins a baseline. For an unstarted Upgrade,
keep the receipt and dump digests `null` and make sure neither target exists.
While that one role is in progress, keep both digests `null`; its receipt may
exist at the fixed canonical path and the audit validates its self-digest and
reports the resume action without treating it as completion. After the receipt
is complete, pin the raw receipt and dump SHA-256 values before advancing. A
later Upgrade may not pin either file before every earlier Upgrade receipt is
complete. Editing only those completion digests does not change the immutable
plan digest already bound by an in-flight receipt. A completed Extension is the
one exception between mutations: retain its separate receipt, capture the new
baseline, replace the current row's baseline pin, and re-audit before creating
the Upgrade receipt. That deliberate baseline replacement starts a new Upgrade
plan; it is never made behind an in-flight receipt.

Run the audit with:

```text
dclutch-local-successor-bootstrap devnet-deployment-set-journal-v2 \
  --rpc-url https://api.devnet.solana.com \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --journal /absolute/evidence/deployment-set-journal.json \
  --solana-cli /absolute/pinned/solana
```

Before the set is complete, the report contains exactly one `next_role` and no
`final_set_sha256`. Any unpinned incomplete receipt at the fixed path names the
same role with a resume action; it never authorizes a replay. If the next
baseline says capacity must grow, the action tells you to extend and capture a
fresh baseline. The old baseline will fail after extension because the
ProgramData slot, width, lamports, and account digest moved. Only the fresh
two-role CarryForward admission plus five freshly valid complete receipts
produce the final set digest. The v2 digest binds every disposition, the
one-context infrastructure snapshot, gate/source/tree/devnet/CLI identity, and
every ordered Program, ProgramData, baseline, receipt, and dump reference.
CarryForward can never masquerade as an Upgrade receipt.

## 7. Prepare the mixed release plan

Do not upgrade Registry or Rent. Even a byte-identical Upgrade advances the
Loader slot and supersedes the ArtifactRelease binding stored in the immutable
singleton `ProtocolInfrastructureProfileV1`. Core cannot overwrite that
account, and Found reauthenticates its stored bindings against the current
deployments.

Checked `prepare --deployment-set-journal` rehashes the journal, snapshot, gate,
five baselines and receipts, all seven dumps, and five canonical candidate ELFs
again. It forbids every raw program, ELF, semantic, live-hash, authority, slot,
and publication override. You supply current ProgramData bodies only for the
five receipt-backed Upgrade roles. Registry and Rent ProgramData bytes come
only from the authenticated CarryForward snapshot; their public observed-body
flags are refused.

Prepare derives Registry and Rent semantic IDs only by hostile-decoding their
existing finalized `ArtifactReleaseV1` bodies. It must reproduce both artifact
bodies, IDs, raw/staging PDAs, and the singleton profile address and body
byte-for-byte. The execution release set contains only the five newly upgraded
roles. Any Registry/Rent receipt, moved slot, substituted semantic, non-null
staging account, or changed profile stops before plan emission.

Treat the saved plan as an untrusted projection when you start the external
campaign. Before it reads any keypair file, the campaign rehashes the complete
mixed deployment-set evidence and binds every saved mutable program pin and
artifact body back to it. It then reads all seven Program/ProgramData pairs in
one finalized `getMultipleAccounts` context and requires exact Loader owner and
privilege shape, Program-to-ProgramData linkage, full ProgramData digest,
deployment slot, retained authority, live payload digest, and checked-candidate
plus zero-padding geometry. A mutable saved plan with no checked set, or any
edited pin, body, link, slot, authority, or payload, stops before key loading.

The campaign's wallet budget is state-aware. Each record is priced by the same
chain-derived publication planner the executor uses: an exact finalized record
or valid in-flight cursor is not charged twice, a vacant coordinate is charged
its exact remaining sponsor debit, and a conflict refuses. The carried
Registry/Rent records and exact singleton profile therefore cost zero. A
missing activation cache is charged at
`ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1`; an exact rent-exempt partial or
complete cache costs zero, while an underfunded or substituted cache refuses.

Activation also has a key-free size reachability gate. Agave 4.0.2 charges the
single full-ELF SHA-256 at `85 + max(10, live_elf_bytes / 2)` CU. The campaign
reserves 150,000 CU for every other Registry operation and refuses a pending
role above 2,499,831 live bytes, the last width that can fit under the
1,400,000-CU transaction maximum under that conservative projection. This is
an impossibility filter, not a replacement for measured CU evidence. The
campaign report prints each role's authenticated live width, conservative
projection, and headroom before it opens a keypair file.

The finalized permanent-substrate observation at slot 489,506,366 and the
current M-61 budget rows give this frontier:

| role | live ELF bytes | measured CU | measured + tolerance | ceiling headroom |
|---|---:|---:|---:|---:|
| Core | 934,088 | 546,984 | 566,984 | 833,016 |
| Claims | 1,010,496 | 573,441 | 593,441 | 806,559 |
| Trading | 1,325,848 | 721,945 | 741,945 | 658,055 |
| Resolution | 588,336 | 330,385 | 350,385 | 1,049,615 |
| Custody | 360,328 | 235,103 | 255,103 | 1,144,897 |

These are the currently deployed widths and the checked budget measurements,
not a prediction for a new candidate. A fresh candidate still needs the
20-seed M-61 measurement and its checked-release role binding. In particular,
the generic 9,034,536-byte `dclutch_sbf.so` Source artifact is not Resolution:
the checked gate refuses that role/path substitution, and the size gate would
independently project it beyond the chain ceiling before any send.

Before activation, run the hybrid real-ELF gate with the exact authenticated
live Registry/Rent dumps beside the five new checked ELFs. It must publish and
activate the new five-role set, authenticate the unchanged profile, create
RentCredit, and run DCLTGMF3 through Open. The founding census remains exactly
59 unique keys and one signature; run every shipped-link frame diagnostic and
report M-61 only as 20/20 plus its true 20-seed mean. A host-only plan test is
not a substitute for that gate.

## Resume rules

Extension receipts move through `Prepared`, `MessagePrepared`,
`SignedNotSubmitted`, `Submitted`, and `Complete`. Upgrade receipts add
`BufferWriteArmed` and `BufferReady` before message preparation. Every phase is
published with no-clobber creation or exact-content compare-and-swap
replacement, parent directory fsync, and a canonical SHA-256 over all fields.
A durable transition lock binds its owner PID/start token plus exact prior and
target receipt digests. After a crash, the next invocation deterministically
finishes an exact pending publish or cleans an already-published target; it
never overwrites a different valid receipt. Every later-only field
must be absent until its phase. A complete Upgrade digest covers the Buffer
upload history, expired packet archive, exact message and packet, submitted
signature, finalized transaction and digest, poststate/context slot, arithmetic,
and dump digest/shape.

- A complete receipt is idempotent with respect to writes, not history. Every
  rerun freshly checks the exact CLI version, devnet genesis, Program,
  ProgramData, payload, authority, deployment slot, finalized transaction, and
  dump. Drift is reported and no write is attempted.
- `BufferWriteArmed` first reconciles the durable process lease. It may attach
  an exact completed Buffer only after the leased writer is proven exited. A
  live exact PID/start-token/process-group remains the sole writer. A reused PID
  is never signaled. A missing or partial Buffer stays poll-only through its
  conservative expiry window; after expiry and exact writer exit it can re-arm
  only the same Buffer identity, retaining the earlier attempt history.
- `MessagePrepared` can read keys and sign only the already journaled message.
  `SignedNotSubmitted` can submit only the already verified packet and cannot
  sign again.
- `Submitted` always queries the exact signature with history enabled.
  Finalized success resolves that transaction; finalized failure stops for fee
  attribution; pending stays poll-only. Exact expiry archives only a null
  signature with unchanged Loader, payer, and (for Upgrade) Buffer prestate.
- `Prepared` resumes only while the complete account, wallet, journal, and
  Buffer prestate remain exact.
- A changed role, program id, ProgramData id, authority, payer, genesis,
  deployment-set journal, Buffer identity, baseline, artifact, payload,
  canonical operation id, receipt digest, message, packet, signature,
  transaction, poststate, arithmetic, or dump is a substitution and is refused.

There is no force flag, multi-role mode, program close, recycle, authority
change, implicit extension, mainnet mode, testnet mode, or unknown-cluster mode.
