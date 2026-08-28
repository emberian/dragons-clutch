# Update a permanent devnet program

You can update one of the seven permanent devnet program ids without closing or
recycling it. The operator makes every write one role at a time. The tool accepts
only Solana devnet, requires the cluster's exact genesis hash, and requires a
second acknowledgement naming the role and program id you will change.
Mutation also requires an exact acknowledgement that you kept the named fee
payer exclusive for the whole opaque CLI window.

The tool never discovers a wallet. You pass keypair paths to the Solana CLI, and
the operator does not open those files itself. It checks the public addresses
the CLI derives from the paths before any write.

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

The command refuses a zero extension and requires the payer to cover the exact
rent top-up plus a provisional 1,000,000-lamport fee reserve. It invokes one
checked `solana program extend`, then requires all of these postconditions:

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

The Solana CLI's extension message does not contain a transaction signature.
The receipt therefore never extracts one from prose. The operator pages
finalized ProgramData signatures, 1,000 rows at a time, until history crosses
below the new deployment slot. It refuses nonmonotonic pages, repeated rows,
more than one successful same-slot candidate, or a history that does not cross
the target within the explicit sixteen-page operational bound. It then verifies
the selected full transaction and metadata before recording it.

After extension, discard the old baseline and capture a new one. Extension
advances the Loader deployment slot. Upgrade advances it again. You mint and
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

If a receipt already exists, preflight validates its canonical phase digest. A
prepared receipt must still match its complete prestate. A submitted receipt
must already have the exact payload, authority, rent, slot advance, and
finalized Upgrade transaction. A complete receipt must also match the current
Loader accounts, stored transaction, and existing dump. Preflight never resumes
or writes any phase.

## 5. Upgrade the existing id

Run `devnet-upgrade-v1` with the new baseline, generated checked-release gate,
its separately recorded digest, the exact source commit/tree, the gate's role
ELF, output dump, receipt, fee payer, and retained authority. Your second
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

After the preflight passes, repeat the exact command with `--execute`. The
mutation mode additionally verifies both keypair public addresses immediately
before it calls `solana program deploy` for the existing id. Pass these source
admissions exactly as printed by the release runner:

```text
--checked-release-gate ABSOLUTE_WORK_ROOT/CHECKED_UPGRADE_GATE.json
--expected-checked-release-gate-sha256 64_LOWERCASE_HEX
--expected-source-revision 40_LOWERCASE_HEX
--expected-source-tree-sha256 64_LOWERCASE_HEX
```

After the CLI returns a JSON program id and signature, the command rechecks the
cluster and reads Program, ProgramData, and payer in one finalized context. It
requires the deployment slot to advance, the live payload to equal the checked
raw ELF plus zero padding, and parked ProgramData rent to remain unchanged.

The CLI-returned signature is authority for transaction lookup, not prose. The
operator fetches that exact finalized transaction and requires one parsed
Loader-v3 Upgrade instruction for the exact Program, ProgramData, retained
authority, buffer, spill, rent sysvar, and clock sysvar. Its account keys and
balance vectors must prove the explicit payer is signer/writable account zero,
the payer delta equals the finalized transaction fee, and ProgramData lamports
did not move. Separately, the receipt bridges the wallet balance immediately
before `solana program deploy` to the finalized wallet balance after the whole
CLI invocation:

```text
operation_observed_net_spend_lamports = wallet_before - wallet_after
unattributed_cli_net_cost_lamports = operation_observed_net_spend_lamports
  - transaction_fee_lamports
```

The final Upgrade fee is exact transaction evidence. The second value is only
aggregate net cost under your exclusive-payer acknowledgement. It can include
buffer creation/write/close fees and the net effect of buffer-rent funding and
refunds; the receipt does not attribute those lifecycle transactions
individually and never calls the final Upgrade fee the operation's total cost.
Finally, the operator dumps the deployed bytes and verifies the dump against
the raw or exact zero-padded live image.

## 6. Audit the mixed deployment set

Use `devnet-deployment-set-journal-v2` to see which execution role is next.
This command is key-free and read-only. It has no `--execute`, keypair,
signing, loop, or receipt-writing mode. It never replaces
`devnet-upgrade-v1`: you still preflight and update exactly one role with that
command. Registry and Rent are not Upgrade targets in this iteration. They are
explicit `carry-forward` rows authenticated from the existing DEPLOY-1 state.

The deployment-set journal is an immutable reference manifest, not Upgrade
acceptance. You
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
After its one-role receipt is complete, pin the raw receipt and dump SHA-256
values. A later Upgrade may not pin either file before every earlier Upgrade
receipt is complete.

Run the audit with:

```text
dclutch-local-successor-bootstrap devnet-deployment-set-journal-v2 \
  --rpc-url https://api.devnet.solana.com \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --journal /absolute/evidence/deployment-set-journal.json \
  --solana-cli /absolute/pinned/solana
```

Before the set is complete, the report contains exactly one `next_role` and no
`final_set_sha256`. A prepared or submitted receipt names the same role with a
resume action; it never authorizes a replay. If the next baseline says capacity
must grow, the action tells you to extend and capture a fresh baseline. The old
baseline will fail after extension because the ProgramData slot, width,
lamports, and account digest moved. Only the fresh two-role CarryForward
admission plus five freshly valid complete receipts produce the final set
digest. The v2 digest binds every disposition, the one-context infrastructure
snapshot, gate/source/tree/devnet/CLI identity, and every ordered Program,
ProgramData, baseline, receipt, and dump reference. CarryForward can never
masquerade as an Upgrade receipt.

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

Before activation, run the hybrid real-ELF gate with the exact authenticated
live Registry/Rent dumps beside the five new checked ELFs. It must publish and
activate the new five-role set, authenticate the unchanged profile, create
RentCredit, and run DCLTGMF2 through Open. The founding census remains exactly
59 unique keys and one signature; run every shipped-link frame diagnostic and
report M-61 only as 20/20 plus its true 20-seed mean. A host-only plan test is
not a substitute for that gate.

## Resume rules

Receipts move through `prepared`, `submitted`, and `complete` phases and are
written atomically. Every phase has a canonical SHA-256 over its complete
contents. Prepared and submitted receipts must have every later-only field
absent; a complete digest covers the CLI signature/output, finalized
transaction and its digest, poststate/context slot, exact arithmetic, and dump
digest/shape.

- A complete receipt is idempotent with respect to writes, not history. Every
  rerun freshly checks the exact CLI version, devnet genesis, Program,
  ProgramData, payload, authority, deployment slot, finalized transaction, and
  dump. Drift is reported and no write is attempted.
- A submitted receipt resumes only poststate and transaction verification. It
  never submits the write again.
- A prepared receipt resumes only while the complete account and wallet
  prestate remains exact.
- If chain state moved behind a prepared receipt, the previous submission
  outcome is ambiguous. The tool stops and tells you not to replay it.
- A changed role, program id, ProgramData id, authority, payer, genesis,
  baseline, artifact, payload, canonical operation id, receipt digest,
  signature, transaction, poststate, arithmetic, or dump is a substitution and
  is refused.

There is no force flag, multi-role mode, program close, recycle, authority
change, implicit extension, mainnet mode, testnet mode, or unknown-cluster mode.
