# Collateral adapter conformance model

Status: host-only research model and adversarial tests. It is not an SBF
adapter, persisted ABI, deployment profile, program-artifact pin, or claim that
legacy SPL collateral is routeable today.

This crate specifies the common contract that a Realm-selected collateral
adapter must satisfy while keeping Egg issuance independently fixed to the
claim-token adapter:

- exact mint/program/decimals, collateral-adapter release, and external
  token-program deployment identity;
- initialized, visible integer atom balances and supply;
- no mint or freeze authority and a bounded positive supply;
- program-specific extension parsing with unknowns failing closed;
- no fee-on-transfer, hook, confidential, nontransferable, default-frozen,
  permanent-delegate, pausable, or mutable unit-scaling behavior;
- strict Hoard owner/delegate/close-authority checks;
- exact source debit, destination credit, unchanged supply, zero withheld
  balance, and zero foreign invocations after every transfer; and
- one immutable `Market -> Realm -> Profile -> policy -> adapter release`
  binding chain.

The two model releases intentionally use placeholder release identities. A real
release must bind the exact parser/CPI implementation and checked external token
program artifact/deployment in a reviewed manifest.

The model gives Egg issuance its own `ClaimIssuanceBinding`. A legacy SPL
collateral profile can therefore coexist with Token-2022 Eggs without deriving
either adapter identity from the other.

## Legacy SPL qualification

Legacy SPL has transparent fixed-layout atom balances and exact checked
transfers, so it can satisfy the arithmetic contract. It does not furnish a
real `ImmutableOwner`: instruction 22 is only a compatibility no-op. The model
therefore admits it only under a weaker, explicit custody theorem:

1. the Hoard owner is the canonical Clutch PDA;
2. no externally signable key can sign for that PDA;
3. the selected Clutch adapter release has no `SetAuthority(AccountOwner)`
   route; and
4. the checked release/deployment boundary is not silently changed.

That is enough to justify a separately named legacy profile; it is not the same
guarantee as Token-2022 `ImmutableOwner`. The current SBF program does not bind
or route such a release, so the DREGG reference profile remains non-executable.

Run the model:

```sh
cargo +1.93.1 test --manifest-path research/collateral-adapter-conformance/Cargo.toml
cargo +1.93.1 clippy --manifest-path research/collateral-adapter-conformance/Cargo.toml --all-targets -- -D warnings
```
