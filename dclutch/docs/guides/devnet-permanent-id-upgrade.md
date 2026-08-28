# Update a permanent devnet program

You can update one of the seven permanent devnet program ids without closing or
recycling it. The operator makes every write one role at a time. The tool accepts
only Solana devnet, requires the cluster's exact genesis hash, and requires a
second acknowledgement naming the role and program id you will change.

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

The update command reads the accounts again immediately before the write and
requires every baseline fact to remain exact.

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
The receipt therefore never extracts one from prose. It finds one unambiguous
finalized ProgramData signature at the new deployment slot and verifies its
full transaction and metadata before recording it.

After extension, discard the old baseline and capture a new one. Extension
advances the Loader deployment slot. Upgrade advances it again. You mint and
activate release records only from the final post-Upgrade ProgramData
observation.

## 4. Upgrade the existing id

Run `devnet-upgrade-v1` with the new baseline, generated checked-release gate,
its separately recorded digest, the exact source commit/tree, the gate's role
ELF, output dump, receipt, fee payer, and retained authority. Your second
acknowledgement has this exact value:

```text
ROLE:PROGRAM_ID
```

The command verifies the pinned Solana CLI version, exact devnet genesis,
keypair public addresses, Program-to-ProgramData link, Loader owners and
privileges, retained authority, baseline, complete gate, selected raw ELF,
derived live digest, and zero padding before it calls `solana program deploy`
for that existing program id. Pass these source admissions exactly as printed
by the release runner:

```text
--checked-release-gate ABSOLUTE_WORK_ROOT/CHECKED_UPGRADE_GATE.json
--expected-checked-release-gate-sha256 64_LOWERCASE_HEX
--expected-source-revision 40_LOWERCASE_HEX
--expected-source-tree-sha256 64_LOWERCASE_HEX
```

After the CLI returns a JSON program id and signature, the command rechecks the
cluster and accounts. It requires the deployment slot to advance, requires the
live payload to equal the checked raw ELF plus zero padding, requires parked
ProgramData rent to remain unchanged, dumps the deployed bytes, and verifies
that dump against the raw or exact live image. The complete receipt carries the
before and after wallet and ProgramData lamports with checked arithmetic.

## Resume rules

Receipts move through `prepared`, `submitted`, and `complete` phases and are
written atomically.

- A complete receipt is idempotent. Re-running with the same exact inputs makes
  no CLI call.
- A submitted receipt resumes only poststate and transaction verification. It
  never submits the write again.
- A prepared receipt resumes only while the complete account and wallet
  prestate remains exact.
- If chain state moved behind a prepared receipt, the previous submission
  outcome is ambiguous. The tool stops and tells you not to replay it.
- A changed role, program id, ProgramData id, authority, payer, genesis,
  baseline, artifact, payload, or canonical operation id is a substitution and
  is refused.

There is no force flag, multi-role mode, program close, recycle, authority
change, implicit extension, mainnet mode, testnet mode, or unknown-cluster mode.
