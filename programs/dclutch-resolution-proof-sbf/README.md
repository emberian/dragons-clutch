# dClutch successor Resolution controller

This is a physical specialization of the Lean-owned Source Resolution
relation. It handles a primary, terminal, already-posted Pyth `PriceUpdateV2`
whose Receiver verification level is `Full`, the first exactly funded ordered
recovery advancement, and explicit Product-owned failure after exhaustion.

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
funding ID, while failure uses the occurrence Source-material ID as its
manifest config/allocation identity. Funding ledger, custody lamports, worker
lamports, Source state, and the typed 312-byte certificate commit together.

Certificates use Lean's exact success/recovery/exhaustion/failure tags and a
typed ordered PDA namespace, so recovery receipts cannot overwrite a prior
receipt. All writable borrows and validation complete before any copy or
lamport assignment. A refusal therefore leaves state, certificate, funding,
and worker balances unchanged even in direct host invocation; SVM transaction
rollback remains an additional runtime boundary.

This slice does **not** yet execute the Product-resolution effect against the
Core/claims owner, perform recovery-provider CPI, physically certify the
separate exhaustion step, create accounts, or retire them. The certificate is
the compact handoff to the shared executor. Registry/Core account mutation,
Loader behavior, Pyth program correctness, Clock correctness, SHA-256 syscall
correctness, and SVM rollback are not Lean proofs.

## Optimized SBF checkpoint

The funded controller was built locally with:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-resolution-proof-sbf/Cargo.toml \
  --lto --optimize-size \
  --sbf-out-dir target/resolution-funded-deploy
```

`cargo-build-sbf 4.0.0`, platform-tools v1.53, and SBF rustc 1.89.0 produced
a verifier-clean 166,176-byte ELF. SHA-256 was
`a8e11578d2fdd0418d1a52baa87cd3f0660a57ec40ba4951069cc38449f1b2f4`.
The section audit was `.text` 160,880, `.rodata` 1,171, `.data.rel.ro` 464,
`.dynamic` 176, `.dynsym` 288, `.dynstr` 154, and `.rel.dyn` 2,096 bytes.

This is a local build checkpoint, not a checked release, deployed artifact,
or mainnet claim. A clean committed rebuild must pin its own digest before use.
