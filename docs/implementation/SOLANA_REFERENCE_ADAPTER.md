# Offline Solana reference adapter

Status: implemented offline reference semantics; not an SVM program, deployment
artifact, token adapter, or chain-readiness claim.

The `programs/solana-reference` crate is the narrow executable seam between the
hostile-byte-facing layouts in `programs/solana-layout` and the pure transition
semantics in `crates/clutch-kernel`. It is `no_std`, safe Rust, allocator-free,
and dependency-limited to those two local crates. It has no Solana SDK,
entrypoint, syscall, `AccountInfo`, CPI, Token-2022 implementation, RPC, key,
signing, deployment, or network behavior.

## Implemented reference subset

- Market-init validation decodes the Realm, Profile, Market, Hoard, Position,
  reference kernel, external-balance, and replay accounts; decodes the frozen
  `CreateMarket` intent; checks the canonical market and outcome IDs; checks
  account linkage, versions, bumps, empty initial supplies, and profile limits;
  and runs the kernel invariant checker. It does not authorize or execute
  creation. `CreateMarket` through the transition function fails closed.
- `Split` debits position cash, checks the immutable collateral cap, credits
  hoard collateral, and invokes the complete-set kernel split.
- `Materialize` and `Dematerialize` invoke the kernel and move quantities
  between internal balances and an explicit reference-only external shadow.
  They do not mint or burn an SPL token.
- `Resolve` always refuses. A signer is not evidence of maturity, a sealed
  observation window, feed/source/generation binding, or a terms-to-payout
  relation, and this crate has no typed prevalidated resolution capability.
- `RedeemInternal` also refuses, including against caller-supplied bytes that
  claim the market is already resolved. Without a transition path that can
  produce trusted resolved state, accepting dependent redemption would merely
  move the unsafe boundary one instruction later.
- Every transition consumes the exact next sequence in a replay account bound
  to the position generation. State-account aliases, wrong program owners,
  wrong keys, non-writable accounts, unsigned actors, and wrong bumps refuse.

The frozen layout did not yet contain aggregate supply, payout vectors,
external balances, or replay sequences. This lab therefore defines three
fixed reference-only accounts. They make the missing state explicit without
pretending to freeze a deployment ABI. The Market lifecycle, Hoard collateral,
Position internal balances, and reference accounts each retain one semantic
owner; the adapter reconstructs a kernel state only on the stack.

This reference is intentionally a **closed single-position model**. Before and
after every transition, for every active outcome, it requires:

```text
position.internal[outcome] + external.balance[outcome]
    == kernel.total_supply[outcome]
```

Checked addition is mandatory. Requiring only `<=` would still permit a forged
position to consume aggregate claims attributed to somebody else. Permitting a
larger local position would permit materialization or redemption of claims that
do not exist in aggregate. Equality is therefore the strongest honest linkage
available without enumerating all positions. Multi-position execution remains
refused by this reference representation until an SVM design supplies a checked
aggregate-closure witness. Initialization additionally requires zero internal,
external, and aggregate claims, zero position cash and reserved cash, zero Hoard
collateral, an open position, and a zero replay sequence.

## Exact byte evidence

The tests build full pre-state byte arrays for all six accounts and compare the
full post-state arrays. The split vector independently changes only the named
little-endian fields:

- Hoard collateral at bytes `98..106`;
- Position outcomes 0 and 1 at `74..82` and `82..90`;
- Position cash at `202..210`;
- Kernel aggregate supplies 0 and 1 at `38..46` and `46..54`; and
- Replay sequence at `74..82`.

The materialize/dematerialize test checks aggregate-supply neutrality. An exact
signer-bypass regression proves that an arbitrary signed actor cannot resolve;
it then forges internally coherent “resolved” bytes and proves redemption still
refuses. Adversarial tests cover account aliases, layout versions, runtime
program ownership, stored bumps, stale replay, replay overflow, unsupported
intents, unavailable resolution evidence, and absent signatures.
One regression freezes the exact counterfeit-claim counterexample: internal
outcome balance one, aggregate supply zero, Hoard zero, then `Materialize(0, 1)`.
It must return `AggregateClosureMismatch` without producing post-state. A
bounded trace test checks the equality after splits and materializations across
quantities 1 through 16.

Run the evidence gates independently because this crate is intentionally not in
a root workspace:

```sh
cargo test --manifest-path programs/solana-reference/Cargo.toml
cargo clippy --manifest-path programs/solana-reference/Cargo.toml --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path programs/solana-reference/Cargo.toml --no-deps
```

## Obligations before an SVM adapter or token CPI

An eventual SVM adapter must establish all of the following before a token CPI
is added. The reference crate does not establish them.

1. Derive every Realm, Profile, Market, Hoard, Position, outcome-mint,
   external-token, replay, feed, and authority PDA from frozen domain-separated
   seeds and compare both address and canonical bump. Caller-supplied expected
   keys are not sufficient.
2. Authenticate each `AccountInfo` owner, executable bit, signer bit, writable
   bit, data length, discriminator, version, rent/lifecycle state, and uniqueness
   by role before borrowing mutable data.
3. Prove that no writable alias, duplicate account, remaining-account shuffle,
   owner substitution, close/reopen generation reuse, or instruction replay can
   cause one logical debit or credit to be applied twice.
4. Freeze the transaction-atomic ordering: validate and compute first, perform
   token effects, then commit program state, with every CPI failure rolling back
   all effects under actual SVM semantics.
5. Select and pin the exact SPL Token or Token-2022 program and extensions.
   Validate mint authority, freeze authority, decimals, transfer fees, hooks,
   delegates, withheld fees, interest-bearing/confidential extensions, account
   owner, and token-account mint. Unsupported extensions must refuse.
6. Prove one-to-one conservation between internal-to-external materialization
   and actual mint/burn or escrow movements. The external shadow in this lab is
   not token evidence and must not survive as a second balance truth.
7. Prove collateral conservation across user token accounts and the Hoard,
   including checked decimals, transfer-fee behavior, exact observed balance
   deltas, collateral caps, and the rule that principal is never a fee, bounty,
   rent source, reserve, or treasury source.
8. Define a non-discretionary authenticated resolution path carrying typed and
   checked maturity, sealed `WindowResult`, feed/source/generation identity,
   market-terms binding, and payout-set membership. No signer or caller-supplied
   binding substitutes for this evidence; resolution and redemption remain
   unconditionally refused here.
9. Freeze replay semantics across Solana transaction replay, durable nonces,
   instruction duplication, batch retries, position close/reopen, and program
   upgrades. The local sequence account is only a model.
10. Bound stack, heap, compute units, account count, serialization cost, CPI
    count, and transaction size on the pinned SBF toolchain. The large offline
    post-state witness is not an onchain mutation strategy.
11. Reconcile concurrent multi-position aggregate supply. Each owner position
    must change the single market aggregate exactly once; closure must prove
    that all internal and external balances are represented without scanning an
    unbounded set. The offline adapter's single-position equality is a refusal
    boundary, not a multi-position solution.
12. Establish upgrade authority, program-data identity, initialization race
    handling, account closing destinations, migration/version rules, emergency
    posture, and immutable-profile behavior without introducing discretionary
    seizure or undercollateralization.
13. Add differential fixtures connecting pure-kernel results, host adapter
    results, SBF execution, and token-program post-balances. Simulations and
    fixtures remain evidence of those exact cases, not mainnet correctness.
14. Name the exact verification claims and trust boundary. Any Verus/Rocq proof
    must pin source digest and toolchain and must state that SVM runtime,
    serialization, PDA derivation, CPI, token programs, and deployment remain
    outside the pure-kernel theorem unless separately proved.

Until those obligations have checked artifacts, the correct description is
“offline reference transition adapter.”
