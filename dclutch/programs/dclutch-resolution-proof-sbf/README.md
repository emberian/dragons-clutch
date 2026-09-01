# dClutch successor Resolution controller

This is a physical specialization of the Lean-owned Source Resolution
relation. It handles a primary, terminal, already-posted Pyth `PriceUpdateV2`
whose Receiver verification level is `Full`, exactly funded ordered recovery,
the final funded exhaustion transition, and explicit Product-owned failure.

**The program does not verify a provider signature, and must never be described
as doing so.** There is no ed25519, secp256k1 or keccak in this program or in
`dclutch-pyth-svm`; `FullPriceUpdateV2::parse` reads a `VerificationLevel` tag
byte and nothing more. Soundness is DELEGATED: the update account must be owned
by the Receiver program named in a Registry-finalized `PythReleaseV1` reached
from the Market's own `identity.resolution_policy`, pinned by ProgramData,
deployment slot, upgrade policy and whole-body config digest, and the Receiver
verified the Wormhole VAA's guardian signatures before it ever wrote those
bytes. That is a real root of trust and a checkable one —
`crates/dclutch-svm-harness/tests/support/pyth_provider.rs` runs the captured
router ELF's `verify_encoded_vaa_v1` on a genuinely signed 13-of-19 VAA — but
it is delegation, not first-party cryptography, and the difference is the whole
security argument.

The *execution* route performs no provider CPI and reads the Pyth update
read-only. The *transport* route (`provider_transport_v3.rs`) does CPI the
Receiver, in exactly three places: `PostUpdate` at submission, and
`ReclaimRent` on each of the two reclaim routes below. Before producing any
mutation the execution route authenticates:

- the exact rent-exempt 352-byte Core Market PDA in `Open`/`Consumed` state
  and the Market-bound Source state;
- the persisted Core `registry_program` and `selected_release_set`, then the
  Registry-owned activated release-set PDA and its Resolution role;
- its own current Loader V3 Program, ProgramData, complete ELF digest,
  deployment slot, and upgrade-authority policy against that role;
- Registry-owned finalized Source-material, Product-instance, and Pyth-release
  records, each with its exact vacant staging PDA;
- the Product instance ID selected by Core and Source material, plus its
  result-domain ID and partition width against Source material's embedded
  `FiniteResultDomainV1`;
- every embedded Source component content identity used by the hot path;
- Pyth Receiver/router ProgramData links and deployment slots;
- the Receiver configuration digest/router binding and fully verified update;
- the canonical Clock and Rent sysvars; and
- exact generation, Product-domain, provider-release, PDA, owner, privilege,
  account-order, and non-alias coordinates.

The direct primary account frame is exactly 21 accounts: Source, certificate,
Core Market, Registry activation, Resolution Program and ProgramData, Source
material raw/staging, Product instance raw/staging, Pyth release raw/staging,
posted price update, Receiver Program/ProgramData/config, Router
Program/ProgramData, Clock, Rent, and System. The direct funded frame is exactly
17 accounts: Source, certificate, action FundingState, worker, Core Market,
Registry activation, Resolution Program/ProgramData, Source material
raw/staging, Product instance raw/staging, capability manifest raw/staging,
Clock, Rent, and System. There is no unattached execution-authority-manifest
account and no standalone result-domain record. Those would duplicate the
CoreState/Registry/Product authority join and are rejected by the exact frame.

It delegates normalization, timing, confidence, exact Product mapping, and
Source lifecycle mutation to `dclutch-source-contract`. Funded transitions
also authenticate the Market-selected finalized capability manifest and its
program-owned canonical `FundingStateV1` PDA. The immutable entry's complete
positive native-lamport Bounty quote is the work charge; no instruction amount
is accepted. The recovery allocation is the exact next Source attempt's
funding ID, exhaustion uses the immutable recovery-policy ID, and failure uses
the occurrence Source-material ID as its manifest config/allocation identity.
Funding ledger, custody lamports, worker lamports, Source state, and the typed
312-byte certificate commit together. Only after those writes succeed, the
direct funded route returns a Lean-owned 376-byte
`FundedTransitionReceiptV1`. It binds the exact executing Resolution program,
Registry-selected V4 semantic release, SHA-256 of the complete 96-byte request,
Source/Funding/worker/certificate keys, Source pre/post digests, exact
FundingState bytes plus post-custody lamports, generation, certificate
kind/sequence, bounty payout, remaining bounty funding, Product selector, and
the projected terminal/refund phase. Recovery, exhaustion, and explicit
failure form an exact receipt partition; malformed cross-phase receipts refuse.

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
The direct funded receipt is not returned on this Core route and cannot replace
the Core acknowledgment or any persisted Source/funding fact.

Certificates use Lean's exact success/recovery/exhaustion/failure tags and a
typed ordered V3 PDA namespace. Primary success is the state-derived first
sequence, so a client can construct its one exact PDA before execution.
Resolution accepts a system-owned, zero-data PDA prepaid with at least exact
rent, tolerates surplus dust, and allocates/assigns it only at the final output
gate. A refusal or replay therefore rolls back certificate creation together
with Source, funding, worker mutation, and return data under SVM transaction
semantics. Callers must read return data immediately, require the Resolution
program as producer, decode the exact receipt, and compare its bound request
and state observations before lending it authority.

This slice does **not** perform recovery-provider CPI or make the external
provider runtime a semantic authority. Primary Pyth remains an authenticated,
read-only adapter observation. Core owns its sparse lifecycle transition and
immediate acknowledgment check; Resolution owns Source, the three action-specific
funding ledgers, certificates, and closure receipt. Registry/Core account
mutation, Loader/System behavior, Pyth program correctness, Clock correctness,
SHA-256 syscall correctness, and SVM rollback are not Lean proofs.

## Two reclaim routes, and why there are two

A provider submission owns two rent-bearing accounts: its program-owned
lifecycle PDA and the Receiver-owned `PriceUpdateV2` it posted. Every one of
them has a route home, and which route depends on what the submission became.

`process_reclaim` (`DCLTPRL3`, 18 accounts) serves the submission that WON. It
requires `ProviderUpdateStatusV3::Consumed` and a program-owned certificate
carrying that lifecycle's own `provider_evidence`.

`process_abandon` (`DCLTPAB3`, 18 accounts) serves every other outcome: the
loser of a first-valid race, and every submission on a market that ended on its
funded failure walk. Their lifecycles stay `Submitted` with `terminal_sequence`,
`certificate` and `provider_evidence` all zero — the wire's own statement that
they never became truth — so they cannot be expressed in the consumed wire at
all. `ProviderReclaimRequestV3::decode` refuses a zero terminal sequence and any
zero identity, `certificate` among them. **That is why this is a second route
and not a relaxed conjunct in the first: the consumed gate did not move.**

Abandonment requires BOTH that the submitter's own `reclaim_after_unix_seconds`
has passed AND that the Source can no longer consume the update — it has left
`Primary`, or its account has already been discharged by `CloseFund`. Both,
never either. The deadline alone would let a stranger delete a live market's
answer for a transaction fee, which is the failure `RecordStillConsumable`
(`0x8016`) was allocated for one transport over; the mirrored refusal here is
`SubmissionStillConsumable` (`0x8017`). The frame is the same eighteen
coordinates in the same order with the same privileges; index 5 carries the
Source resolution state instead of the terminal certificate, because the Source
is what proves consumption can never happen.

`process_reclaim` returns a typed receipt and `process_abandon` deliberately
returns nothing. That asymmetry is the point rather than an oversight: the
consumed route has an actual reader — `provider_finalized_projection_v3.rs`
decodes `ProviderReclaimReceiptV3` out of return data — and the abandon route
has none. Everything an abandon receipt would have carried is already in the
poststate: both accounts closed, the exact total on the persisted refund
recipient. The receipt existed briefly, for symmetry, and was cut; do not
restore it without a caller that decodes it.

The gap this closes was named in
`docs/evidence/LIVENESS_CENSUS_2026_08_29.md` row Q8a, which reached the same
construction — new magic, new request type, new terminality conjunct — and
stopped at the analysis.

## Optimized SBF checkpoint

The funded controller was built locally with:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-resolution-proof-sbf/Cargo.toml \
  --lto --optimize-size \
  --sbf-out-dir target/resolution-funded-deploy
```

The historical `b0e515f` V3 artifact predates the 352-byte CoreState,
Registry-owned Product-instance record, 21/17-account direct frames, V4 funded
receipt, and Core-effect routes. It is not valid for this ABI. A clean committed
archive rebuild must supply the next artifact digest; dirty-overlay hashes are
not release evidence.

This is a local build checkpoint, not a checked release, deployed artifact,
or mainnet claim. A clean committed rebuild must pin its own digest before use.
