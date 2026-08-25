# dClutch successor Resolution controller

This is a physical specialization of the Lean-owned Source Resolution
relation. It handles a primary, terminal, already-posted Pyth `PriceUpdateV2`
whose Receiver verification level is `Full`, exactly funded ordered recovery,
the final funded exhaustion transition, and explicit Product-owned failure.

The program does not post or reclaim provider accounts and performs no
provider CPI. The Pyth update is read-only. Before producing any mutation it
authenticates:

- the Core-owned open Market and Market-bound Source state;
- the finalized execution-authority manifest selected by the Market;
- the Registry-owned activated release-set PDA and its Resolution role;
- its own current Loader V3 Program, ProgramData, complete ELF digest,
  deployment slot, and upgrade-authority policy against that role;
- finalized Source-material, Product result-domain, and Pyth-release records;
- every embedded Source component content identity used by the hot path;
- Pyth Receiver/router ProgramData links and deployment slots;
- the Receiver configuration digest/router binding and fully verified update;
- the canonical Clock and Rent sysvars; and
- exact generation, Product-domain, provider-release, PDA, owner, privilege,
  account-order, and non-alias coordinates.

It delegates normalization, timing, confidence, exact Product mapping, and
Source lifecycle mutation to `dclutch-source-contract`. Funded transitions
also authenticate the Market-selected finalized capability manifest and its
program-owned canonical `FundingStateV1` PDA. The immutable entry's complete
positive native-lamport Bounty quote is the work charge; no instruction amount
is accepted. The recovery allocation is the exact next Source attempt's
funding ID, exhaustion uses the immutable recovery-policy ID, and failure uses
the occurrence Source-material ID as its manifest config/allocation identity.
Funding ledger, custody lamports, worker lamports, Source state, and the typed
312-byte certificate commit together.

The V4 controller also consumes the sole canonical Market-Core effect wire.
Each instruction is exactly a 280-byte `CoreEffectEnvelopeV1` followed by 304
role-owned bytes: a compact 16-byte funding-count header and the Lean-owned
288-byte `ResolutionRoleRequestV1`. The Core action fixes the Resolution role,
so this route has no Trading capability selector. The three real FundingState
accounts carry their canonical entry indices; the request independently binds
their exact PDAs and recovery/exhaustion/failure order. No inline key list
duplicates those account facts. The envelope's request length, request digest,
caller-authority PDA, and full effect digest bind the complete 304-byte role
wire, not merely the Resolution tail.

The common Core-effect account prefix is fixed at sixteen accounts: caller
authority, Core Market, Registry activation, Registry program, Core Program and
ProgramData, Resolution Program and ProgramData, finalized Source material and
staging vacancy, finalized capability manifest and staging vacancy, Source
state, and the recovery/exhaustion/failure funding states. Action tails are:

- CreateFund: Rent, System;
- VerifyFundReady: immutable RentCredit beneficiary, Clock, Rent;
- AdmitTerminal: terminal certificate, Rent; and
- CloseFund: terminal certificate, closure receipt, immutable RentCredit
  beneficiary, Clock, Rent, System.

CreateFund allocates the deterministic Source and three funding PDAs from exact
prepaid system accounts. VerifyFundReady consumes only each entry's immutable
Rent+Creation compartments and credits the persisted RentCredit. AdmitTerminal
authenticates the terminal Source, all three active funding states, and the
state-derived certificate PDA. CloseFund creates a deterministic 384-byte
closure receipt, classifies every remaining native lamport and donation through
the funding contract, and atomically discharges Source plus all three funding
accounts to the same persisted RentCredit. Resolution returns only the canonical
240-byte `CoreEffectAckV1`; it does not define a private effect receipt.

Certificates use Lean's exact success/recovery/exhaustion/failure tags and a
typed ordered V3 PDA namespace. Primary success is the state-derived first
sequence, so a client can construct its one exact PDA before execution.
Resolution accepts a system-owned, zero-data PDA prepaid with at least exact
rent, tolerates surplus dust, and allocates/assigns it only at the final output
gate. A refusal or replay therefore rolls back certificate creation together
with Source, funding, and worker mutation under SVM transaction semantics.

This slice does **not** perform recovery-provider CPI or make the external
provider runtime a semantic authority. Primary Pyth remains an authenticated,
read-only adapter observation. Core owns its sparse lifecycle transition and
immediate acknowledgment check; Resolution owns Source, the three action-specific
funding ledgers, certificates, and closure receipt. Registry/Core account
mutation, Loader/System behavior, Pyth program correctness, Clock correctness,
SHA-256 syscall correctness, and SVM rollback are not Lean proofs.

## Optimized SBF checkpoint

The funded controller was built locally with:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-resolution-proof-sbf/Cargo.toml \
  --lto --optimize-size \
  --sbf-out-dir target/resolution-funded-deploy
```

An exact `git archive b0e515f` build with `cargo-build-sbf 4.0.0`,
platform-tools v1.53, and SBF rustc 1.89.0 produced a verifier-clean
210,528-byte V3 ELF. SHA-256 was
`f684b845a60a25e661dee334e2866895d830956aedba74c8e1bf705d5abee2e7`.
The section audit was `.text` 196,688, `.rodata` 5,240, `.data.rel.ro` 1,424,
`.dynamic` 176, `.dynsym` 312, `.dynstr` 177, and `.rel.dyn` 5,568 bytes. The
prior V2 artifact is not valid for this ABI.

This is a local build checkpoint, not a checked release, deployed artifact,
or mainnet claim. A clean committed rebuild must pin its own digest before use.
