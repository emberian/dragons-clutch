# Prefund-safe predictable account creation

Status: **real-bank regression evidence landed**

Scope: the shared System-program constructor introduced by `ceac012`, as used
by `CreateMarket`, `PlaceOrder`, and first-owner `Endow`

Evidence: `programs/clutch-sbf/svm-tests/tests/prefund_creation.rs`

## The attack

A Solana address does not need to sign to receive SOL. Anyone who knows a
future PDA can send one lamport to it before the protocol constructs it. A
constructor that equates `lamports > 0` with initialization lets an unrelated
wallet permanently squat every predictable market, reservation, Position, or
Replay address.

That does **not** mean every pre-existing account is harmless. SOL on an empty
System account is only a donation. Existing data, a non-System owner, or the
executable bit is semantic occupation and must never be overwritten.

## Required transition

For target `T`, requested space `s`, rent minimum `r(s)`, and pre-balance `b`,
the admitted prestate is exactly:

- `T` is writable;
- `T` is non-executable;
- `T.data_len == 0`;
- `T.owner == SystemProgram`;
- `b` is unrestricted.

The constructor then performs, atomically:

1. transfer `max(r(s) - b, 0)` from the authenticated instruction payer;
2. PDA-signed System `Allocate(s)`;
3. PDA-signed System `Assign(expected_owner)`;
4. require exact poststate:
   `data_len == s`, `owner == expected_owner`, and
   `lamports == max(b, r(s))`.

Consequences:

- a one-lamport target costs the payer exactly `r(s) - 1`;
- a target already at or above rent costs the payer zero;
- excess SOL remains on the target as an unowned donation;
- the constructor cannot sweep or credit that excess;
- owner, data, and executable squats refuse before creation;
- any later instruction/CPI refusal restores the pre-transaction account
  image, including the SOL top-up, allocation, assignment, and protocol writes.

The lamport balance is deliberately not an ownership signal. The runtime owner
and data are.

## Real-bank matrix

The dedicated SVM suite drives the real Clutch SBF artifact, System program,
and Token-2022 program. It does not mock `Allocate`, `Assign`, transfer, token
failure, or transaction rollback.

| Plane | Prestate / construction path | Observed requirement |
| --- | --- | --- |
| canonical `PlaceOrder` Reservation | genesis-injected 1-lamport, System-owned, zero-data target | succeeds; target ends program-owned at exact Reservation width and rent minimum; actor debit is exactly rent minus one |
| canonical `PlaceOrder` Reservation | ordinary public System transfer of rent plus `77,777` lamports | succeeds; donation is retained byte-exactly and actor pays zero creation lamports |
| canonical `PlaceOrder` Reservation | foreign owner / non-empty System data / executable System account | each refuses; target, page, and Position remain byte-exact |
| canonical `PlaceOrder` Reservation | one-lamport target; first placement succeeds, duplicate placement later in the same transaction refuses | the target returns to one lamport, zero data, System ownership; payer, page, and Position also return byte-exactly |
| `CreateMarket` Market PDA | public System transfer of full future-account rent plus `77,777` lamports | succeeds; excess is retained; the creator funds exactly the other absent market/token targets and contributes zero to Market |
| first `Endow` Position + Replay | public System transfers above each future account's rent minimum | succeeds; both targets retain the donations, become canonical program state, and charge the owner zero creation lamports |
| first `Endow` Position + Replay | genesis-injected one-lamport targets followed by a Token-2022 overdraw | both System transfers, Allocates, and Assigns run before Token-2022 refuses; Position, Replay, owner token, Hoard token, and payer all return byte-exactly |

The hostile-prestate checks are exercised on Reservation because all three
planes use the same `require_creatable` / `create_pda_account` implementation.
Market state preflights all seven targets before its first CPI, and owner-state
creation preflights both Position and Replay before its first CPI.

## Commands and observed result

Run from the repository root:

```sh
cargo test --manifest-path programs/clutch-sbf/svm-tests/Cargo.toml \
  --offline --locked --test prefund_creation -- --nocapture --test-threads=1

cargo clippy --manifest-path programs/clutch-sbf/svm-tests/Cargo.toml \
  --offline --locked --test prefund_creation -- -D warnings
```

Observed on 2026-08-19:

- `prefund_creation`: **6 passed, 0 failed**;
- exercised ELF SHA-256:
  `07b759e09867a13a89b6f0c27fdfb3f65b03fb4a2e186b94ea5ac87a21ac80a3`.

The test file itself compiled without warnings. The final strict-Clippy
invocation was not a green repository gate: concurrent source/archive fixture
work left unused imports and dead fixture constants in
`programs/clutch-sbf/svm-tests/src/lib.rs`. Those diagnostics are outside this
lane and are not attributed to the prefund tests. No shared file was edited to
hide them.

## What this proves, and what it does not

This is evidence that the in-process Agave bank executes the intended
prefund-safe transitions and rollback boundaries for the current ELF. It also
closes the concrete predictable-PDA squatting counterexample for the three
creation families tested here.

It is not a cluster deployment claim. It does not establish rent collection
over time, durable-nonce behavior, loader upgrades, malicious validator
behavior, or future constructors that bypass the shared helper. Every new
predictable PDA constructor must reuse this prestate rule or add equivalent
real-bank evidence.
