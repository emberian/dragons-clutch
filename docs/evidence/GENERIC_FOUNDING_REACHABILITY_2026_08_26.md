# Generic Market founding: reachability and the local founding campaign — 2026-08-26

## Evidence boundary

This is **local-validator evidence only**, at evidence-ladder level 4. Every
transaction below ran against one `solana-test-validator 4.0.2` bound to
127.0.0.1, from a fresh genesis prepared by
`tools/local-validator/bootstrap/successor`. Nothing here is devnet or mainnet
evidence, nothing here is a deployment, and no program in it has a checked
release manifest outside this lab. The Pyth inputs are the captured synthetic
local fixture in [`PYTH_SYNTHETIC_RELEASE_V1.md`](PYTH_SYNTHETIC_RELEASE_V1.md);
that is a lab projection, never a production provider release.

No formal verification is claimed. Every refusal recorded here is an executed
refusal on a specific input, not a proof over all inputs. The CU figures are
measured on this validator with these exact artifacts, and are labelled
measured-profile, not mathematical.

## Headline

**The Market is not Open, and it is not Found either.** The campaign reaches
canonical Core Found31 and Found31 exhausts Solana's per-transaction maximum of
1,400,000 compute units. With the real seven artifacts bound into the release
set, the infrastructure does not even activate. Three separate defects on the
path to a first market were found by *executing* it; two are fixed here, the
third is a measurement handed to the lane that owns it.

| Finding | Status |
|---|---|
| Host Found/RentV2 projections refused the real System Program | fixed, `c25de27` |
| Capability-root selection was a SHA-256 fixed point | fixed, `386f254` |
| Found31 exceeds the 1.4M CU maximum (on-chain full-ELF hashing) | measured, owner is the W2 registry fast path |
| Registry activation with the real seven artifacts also exceeds it | measured, same cause |
| No route creates a pre-Market Trading capability root | recorded, needs a protocol decision |
| No live route creates the projected-Custody state the outer's Lock consumes | recorded, needs an implementation owner |

## Artifacts

Built from the working tree at this commit with the pinned toolchain
(`cargo-build-sbf 4.0.0`, `platform-tools v1.53`, `rustc 1.89.0`), default
release profile, no `--lto` and no `--optimize-size`:

```sh
cargo build-sbf --manifest-path programs/<NAME>/Cargo.toml --sbf-out-dir target/w1-sbf
```

| Program | Bytes | SHA-256 |
|---|---:|---|
| `dclutch_registry_sbf.so` | 225,504 | `2554f99725f48d89b3121f1575709760cd5f956db999744a01cba25a00c1788f` |
| `dclutch_core_sbf.so` | 1,004,840 | `d5f9c1834eca97d3243a15a6b991cf11952ca716bfb70b5de6641509247b4fb8` |
| `dclutch_claims_sbf.so` | 1,073,776 | `37cb05c3fb34bdd47bf195785a90bb1c6935d09bb70f964f99158360187ad5f7` |
| `dclutch_trading_sbf.so` | 1,284,664 | `bd2d8441371e93e7a623ede2c080ad1cc2b0fbfda0f6943e3b0494229090a7da` |
| `dclutch_resolution_proof_sbf.so` | 463,576 | `9722b3241d252c1a0d51bf5eef178aa6aace963eaa0865da9e89cce54f8fcb8c` |
| `dclutch_custody_sbf.so` | 330,400 | `0f2a81fe9a46117a60565d8f65f1a8fafbd9b53706e186304cb96ac2296db0b6` |
| `dclutch_rent_sbf.so` | 152,352 | `2bfced4a7a5297796fd5cdaa7d3af1f2254af8a7158b1fd3968c3059f79747ea` |

`cargo build-sbf --lto` was attempted and abandoned: `dclutch-trading-sbf`
declares `crate-type = ["cdylib", "lib"]`, and the toolchain refuses LTO for a
`lib` output. The one program that did build with LTO, Registry, came out
*larger* (226,008 vs 225,504 bytes), so LTO would not have moved the compute
result below. `--optimize-size` was deliberately not used: `cargo build-sbf`
documents it as potentially *increasing* CU.

The campaign uses the real Registry, Core, and Rent artifacts. The Claims,
Trading, Resolution, and Custody roles in the release set are bound as distinct
immutable Loader-v3 deployments of the Registry ELF. That is **not** a
convenience: binding the real four makes Registry activation itself exceed the
compute maximum, as measured below. Nothing in the Found path invokes those
four programs, and the generic founding outer that would is unreachable for the
reasons below.

## Defect 1 — host projections refused the real System Program (`c25de27`)

`dclutch-product-runtime-v2-operator` required the built-in System Program
observation to carry an empty body:

```rust
|| !state.system_program.data.is_empty()
```

in both `src/found.rs::authenticate_runtime_accounts` and
`src/lifecycle_rent_v2.rs`. A real Agave 4.0.2 `getMultipleAccounts` observation
of `11111111111111111111111111111111` carries fourteen bytes of NativeLoader
metadata (`system_program`), so **every** Found31 and RentCreditV2 plan built
from a live cluster snapshot refused with `AccountAuthority` before exporting
an instruction. The sibling record-publication planner had already dropped the
same requirement in `770610c`; these two sites were missed.

The crate's own tests could not catch it because their fixture modelled the
System Program as a vacant account. The fixture now carries the exact Agave
metadata, and a new adversarial test pins that key, owner, and executable
substitution still refuse on both planners.

## Defect 2 — the capability-root selection was a SHA-256 fixed point (`386f254`)

Core's generic founding authenticated the Trading capability root with, among
other conjuncts (`programs/dclutch-core-sbf/src/generic_founding_v1.rs`,
`authenticate_root`):

- `root.key == request.capability_root()`
- `find_program_address(header.seeds(), trading) == root.key`
- `header.selection().config() == SHA256(request encoded at the Found stage)`

`CapabilityRootSeedsV1`
(`crates/dclutch-capability-program-contract/src/lib.rs:536`) puts
`selection.config` **in the seeds**, and `GenericFoundingRequestV1` carries
`capability_root` **in the encoded body**. The conjunction therefore demanded

```text
root = PDA(.., config)        config = SHA256(request(.., root))
```

which is a SHA-256 fixed point. No honest founder can produce one: the route
was unsatisfiable for every well-formed artifact, independently of any missing
operator or any missing account.

The fix puts the sole root-free selection preimage in the codec that owns the
request encoding. `GenericFoundingRequestV1::selection_preimage()` fixes the
Found-and-permit stage and clears only the capability-root span, so the
selected config still binds the Market, generation, release set, context,
vaults, funding-list identity, widths, rents, and projected revision
byte-for-byte, and the commit-last Open stage authenticates against the same
root without a second activation. Core hashes that preimage.

`dclutch-market-founding-v1-operator` now owns the one acyclic construction —
config, then selection, then header, then root, then the finalized request —
and `capability_root_selection_is_acyclic_and_satisfies_core_authentication`
evaluates exactly the conjunction `authenticate_root` evaluates, so
satisfiability is *checked*, not asserted. Substituting any other coordinate
still moves the root and refuses.

## Defect 3 — Found31 exceeds Solana's compute maximum (measured)

The failing transaction, verbatim from the validator:

```text
Program ComputeBudget111111111111111111111111111111 invoke [1]
Program ComputeBudget111111111111111111111111111111 success
Program <CORE>     invoke [1]
Program <REGISTRY> invoke [2]
Program <REGISTRY> consumed 531543 of 537635 compute units
Program <REGISTRY> success
Program <CORE>     consumed 1399850 of 1399850 compute units
Program <CORE>     failed: Computational budget exceeded
```

Read it as: Core burns **862,215** CU before the Registry role CPI, the Registry
reauthentication CPI costs **531,543**, Core is left with **6,092** and dies.
The compute limit requested was already `MAX_COMPUTE_UNIT_LIMIT` (1,400,000),
so there is no headroom to buy, and the true requirement is strictly greater
than what was consumed.

The dominant term is **on-chain hashing of whole ProgramData ELFs**. Both
`programs/dclutch-registry-sbf/src/lib.rs:408` and
`programs/dclutch-core-sbf/src/infrastructure.rs:314` build a
`DeploymentObservationV1` containing `hash(programdata_view.elf())`.

The per-byte rate below is **inferred from these measurements**, not quoted
from a specification: the Registry CPI consumed 531,543 CU while hashing
1,004,795 ELF bytes, i.e. about one CU per two bytes. Applying that rate:

| ELF hashed | Bytes | Predicted CU | Hashed by |
|---|---:|---:|---|
| Core | 1,004,795 | ~502,400 | Core, `authenticate_immutable_core_release` |
| Registry | 225,459 | ~112,700 | Core, `authenticate_found` |
| Rent | 152,307 | ~76,200 | Core, `authenticate_found` |
| Core again | 1,004,795 | ~502,400 | Registry, role reauthentication CPI |
| **total** | | **~1,193,700** | |

That predicted total is ~85% of the entire per-transaction maximum, and it
matches the observed split closely: the Registry CPI's measured 531,543 is
~502,400 of Core-ELF hashing plus ~29,100 of everything else it does.

**The Core ELF is hashed twice in one transaction.** For an immutable Loader-v3
deployment the `(program, programdata, deployment_slot, authority = None)`
tuple already pins the bytes — the ELF cannot change — so re-deriving the
digest per transaction buys nothing the deployment binding does not already
give.

That argument is already written down and already adopted elsewhere:
`dclutch_registry_contract::immutable_release_elf_digest_v1` returns the
release's admitted digest when, and only when, the release is `Immutable`, its
recorded authority is `None`, **and** the observed on-chain authority is
`None`. `programs/dclutch-registry-sbf/src/batch_v2.rs:186` and
`programs/dclutch-trading-sbf/src/execution_strategy_v2.rs:584` both take it.

The two sites Found31 actually traverses do not:

- `programs/dclutch-registry-sbf/src/lib.rs:367` (`deployment_observation`,
  reached from `RegistryInstructionV1::Reauthenticate` at `lib.rs:142`) hashes
  the full ELF unconditionally — this is the measured 531,543 CU CPI;
- `programs/dclutch-core-sbf/src/infrastructure.rs:314` does the same for the
  Core, Registry, and Rent artifact releases.

Adopting the existing `batch_v2.rs` pattern at those two sites is the whole of
the arithmetic above. It belongs to the W2 lane that owns the registry fast
path and the 1.4M gate; this lane deliberately did not touch it. The same
arithmetic is the likeliest explanation for the ~2.87M CU measured on the
common Hot path.

The same rate independently predicts the other large number in the campaign.
Registry five-role activation authenticates Core plus four roles that this
harness binds to the Registry ELF: `502,400 + 4 × 112,700 = 953,300` predicted
against **1,089,297** measured, leaving ~136,000 for everything else activation
does. Two independent transactions, one rate, both explained.

That prediction has a sharp consequence, and the campaign was re-run with the
real seven artifacts bound into the release set to check it. The five roles
then hash Core (502,400) + Claims (536,900) + Trading (642,300) + Resolution
(231,800) + Custody (165,200) ≈ **2,078,600** CU of hashing alone, before
activation's other ~136,000 — half again over the maximum. **Predicted:
activation itself cannot execute. Observed:**

```text
Program <REGISTRY> invoke [1]
Program 11111111111111111111111111111111 invoke [2]
Program 11111111111111111111111111111111 success
Program <REGISTRY> consumed 1399850 of 1399850 compute units
Program <REGISTRY> failed: Computational budget exceeded
```

with `loadedAccountsDataSize: 4,389,755` — the five real ProgramData accounts.
**The successor infrastructure cannot be activated at all with its own real
artifacts.** Every campaign in this document therefore runs with Claims,
Trading, Resolution, and Custody bound to distinct immutable Loader deployments
of the much smaller Registry ELF; that substitution is what buys the 1,089,297
CU activation, and it is the only reason the campaign gets as far as Found31.

Two failures, two transactions, one cause, and the second was predicted from
the first before it was run. The campaign's early refusals cost 5,777 CU (wrong
infrastructure authority) and 6,958 CU (substituted lifecycle credit in
Found31), because they refuse before reaching the release membrane at all.

## Why the atomic outer is still unreachable

The outer itself
(`programs/dclutch-trading-sbf/src/generic_market_founding_v1.rs`,
magic `DCLTGMF1`) has the right shape: an 8-byte instruction, four readonly
raw-request accounts, and Lock → Found/permit → Realize → Claims FoundingV5 →
Core **Open last** in one rollback domain, 139 account references at three
funding states. Two structural facts keep it unreachable even with unlimited
compute.

### Blocker A — a founding capability root cannot be created

`authenticate_root` requires `header.market() == request.market()`
(`generic_founding_v1.rs:708`): the root is keyed to the Market **being
founded**. Core's Found stage simultaneously requires that Market account to be
vacant (`market.owner == system_program && data_len == 0`).

The only in-tree route that creates a Trading capability root is
`programs/dclutch-trading-sbf/src/outer.rs::process_activation`, and its
`authenticate_market_and_caller` (`outer.rs:402`) requires

```rust
market.owner == core_program && market.data_len() == STATE_BYTES
```

plus `CoreState::decode`, PDA re-derivation from the decoded identity, and
`envelope.parent_state_digest() == hash(market bytes)`. A capability root can
only be activated against a Market that already exists as Core state.

```text
capability root <= activation <= existing Core Market <= Core Found <= capability root
```

Resolving this is a decision about what a *founding* capability root means, and
it is deliberately not made here. The two coherent options:

1. key the founding root on the sponsoring context rather than the founded
   Market, and drop `header.market() == request.market()`; or
2. add a family-neutral pre-Market activation route whose parent authority is
   the Registry-selected manifest and the prepaying founder rather than an
   existing Market state.

Option 2 preserves the root's single-use binding to the exact founding and is
the likelier answer, but it is a new signing surface and needs its own
adversarial budget. **Queued for the Cycle-2 family wave; owner: whoever owns
the capability-activation membrane.**

### Blocker B — nothing can create the projected-Custody state the Lock consumes

`execute_lock` (`generic_market_founding_v1.rs:313`) invokes Custody with
`ProjectedCustodyOperationV1::LockHoardAndCloseSource`. Custody's
`authenticate_common` (`programs/dclutch-custody-sbf/src/projected.rs:194`)
requires, for every operation except `Initialize`, that the state account
already be Custody-owned and exactly `PROJECTED_CUSTODY_STATE_BYTES_V1` wide.
The Lock frame additionally consumes an already-open Hoard vault, a funded
funding-source vault, and that vault's replay account.

Only `Initialize` and `OpenHoard` can create them, and their caller must be a
PDA of `request.caller_program` under `ProjectedCustodyCallerSeedsV1` — only
the Trading program can sign them, and only if some live Trading route emits
those exact requests. None does:

- `programs/dclutch-trading-sbf/src/projected_custody_composition_v4.rs` is
  declared `#[allow(dead_code)] mod` at `lib.rs:127`, alongside its four
  `projected_*_composition_v4` siblings (`lib.rs:105,116,145,156`). Nothing
  dispatches to them.
- `programs/dclutch-trading-sbf/src/series/projected_custody_v3.rs:92` emits
  `Initialize`, but its only consumer is `series/execute_v3.rs`, which has no
  caller inside the program.
- The live hot route reaches Custody through `custody_composition_v3`, which
  emits `DelegatedCustodyRequestV2` — a different ABI.

The outer's first CPI consumes state that no live route can produce. This is a
missing first layer of a vertical slice, not a bug in the outer. **Queued for
the Cycle-2 family wave.**

### What was checked instead

The outer's cross-request join now has adversarial tests that need no chain
(`generic_market_founding_v1.rs`, `request_join_*`). The pure coordinate join
was split out of `authenticate_request_join` so it can be exercised without a
139-account frame. The tests construct a canonical four-request join — which by
itself demonstrates the join is satisfiable — and then substitute each Claims
coordinate the attacker controls (market, founder, hoard, funding source,
custody replay, rent credit, release set, custody request digest, generation)
and require `Content`; substituting the named Claims or Trading program
requires `Release`; breaking the Lock→Realize revision sequence requires
`Content`.

This is not the on-chain rollback case the campaign wants. A substituted Claims
*account* proving byte-exact rollback of the whole Lock→…→Open chain needs the
route to execute, which Blockers A and B forbid. Saying otherwise would be
claiming execution evidence for a pure function.

## The demo Market is no longer a placeholder

The campaign now compiles, publishes, and attempts to found a real demo product
instead of `[0x21; 32]`-style filler: a small categorical **SOL/USD
range-protection** Market over the captured local Pyth fixture. Its Realm,
Product graph, Source material, recovery policy, and capability manifest are
all finalized Registry records on chain; only the Found transaction itself
fails, on compute.

- Coordinate domain: USD cents per SOL, `cut_denominator = 100`.
- Cuts `12000` and `18000`, so the three ordinary regions are *below $120.00*,
  *inside $120.00–$180.00*, and *at or above $180.00*, followed by the explicit
  failure outcome.
- Portfolio coefficients `[1, 0, 1, 0]`, denominator 1: one unit of the
  liability basis in either tail, nothing inside the band, nothing on failure.
  That is the payoff a holder buys as protection against SOL/USD leaving the
  range.
- Collateral: a real Token-2022 mint, six display decimals, 1,000,000,000 raw
  atoms, mint and freeze authority absent.

Every semantic identifier is a domain-separated digest over
`dclutch/local-demo-market/v1 || 00 || role || 00 || part…` naming the spec it
stands for, and the resolution identifiers additionally bind the captured
release's `adapter_id` and synthetic local label from
`fixtures/pyth/local-upgraded-2026-08-22`. The spec object is generated, not
hand-written:

```sh
cargo run --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml \
  --offline -- demo-market --registry-program-id REGISTRY_ID
```

**This is a lab demo Market.** The fixture is synthetic per
`PYTH_SYNTHETIC_RELEASE_V1.md`, the identifiers name specs that no production
registry publishes, and nothing about it is a price feed, a product offering,
or a deployment.

## Found31 does not fit a legacy packet either

Before the compute ceiling there was a packet ceiling. With its keys inline the
canonical 31-account Found frame serialises to 1,242 raw bytes — the validator
reported the base64 form as `too large: 1656 bytes (max: encoded/raw
1644/1232)` — so **Found31 had never been submitted at all**. It misses by ten
bytes, which is worth saying plainly: the frame is one 32-byte coordinate away
from fitting, and any future coordinate makes it worse. The runner
now publishes a finalized address lookup table and submits Found31 as a v0
transaction through `dclutch-versioned-message-operator`, which owns table
admission and packet geometry.

Routing is transaction data, never protocol authority, and the campaign proves
it: only non-signer coordinates and the invoked Program are routed, the fee
payer and every signer stay in the message's static key list, the table is
authority-owned rather than frozen so its rent stays recoverable, and it is
usable only strictly after the slot that last extended it. The new hostile case
substitutes the Market coordinate under that attacker-chosen routing inside a
two-instruction transaction whose first instruction is a lamport transfer; the
transaction must refuse and roll back to a fee-only debit, with neither the
substituted address, the transfer recipient, nor the canonical Market left
behind.

## Campaign transcript

Forty confirmed transactions in 690 seconds against one validator, ending in
the Found31 refusal above. Every line is a finalized transaction; the eight
lines that name a refusal are the campaign's hostile cases, and each was
required to fail and required to leave no poststate behind.

```text
slot=50 fee=5000 compute_units=5777 real-SBF wrong-authority initialization
slot=82 fee=5000 compute_units=227728 real-SBF infrastructure init
slot=114 fee=5000 compute_units=541555 real-SBF activation before revoke
slot=146 fee=5000 compute_units=2520 real-SBF Core Loader revoke
slot=178 fee=5000 compute_units=1091088 real-SBF Registry activation
slot=210 fee=5000 compute_units=27022 publish record: Begin
slot=242 fee=5000 compute_units=12849 publish record: Append
slot=274 fee=5000 compute_units=10689 publish record: substituted refund wallet refuses
slot=306 fee=5000 compute_units=19960 publish record: Finalize
slot=338 fee=15000 compute_units=5263 create real Token-2022 collateral and raw-atom wallet
slot=370 fee=5000 compute_units=24022 publish record: Begin
slot=402 fee=5000 compute_units=11349 publish record: Append
slot=434 fee=5000 compute_units=9187 publish record: substituted refund wallet refuses
slot=466 fee=5000 compute_units=17025 publish record: Finalize
slot=498 fee=5000 compute_units=24024 publish Product graph: Product Begin
slot=530 fee=5000 compute_units=11351 publish Product graph: Product Append
slot=562 fee=5000 compute_units=17028 publish Product graph: Product Finalize
slot=594 fee=5000 compute_units=30024 publish Product graph: ResultDomain Begin
slot=626 fee=5000 compute_units=14351 publish Product graph: ResultDomain Append
slot=658 fee=5000 compute_units=23188 publish Product graph: ResultDomain Finalize
slot=690 fee=5000 compute_units=24024 publish Product graph: Portfolio Begin
slot=722 fee=5000 compute_units=11351 publish Product graph: Portfolio Append
slot=754 fee=5000 compute_units=17156 publish Product graph: Portfolio Finalize
slot=786 fee=5000 compute_units=24024 publish record: Begin
slot=818 fee=5000 compute_units=11351 publish record: Append
slot=850 fee=5000 compute_units=17124 publish record: Finalize
slot=882 fee=5000 compute_units=18022 publish record: Begin
slot=914 fee=5000 compute_units=8349 publish record: Append
slot=946 fee=5000 compute_units=11409 publish record: Finalize
slot=978 fee=5000 compute_units=18024 publish record: Begin
slot=1010 fee=5000 compute_units=8353 publish record: Append
slot=1042 fee=5000 compute_units=8353 publish record: Append
slot=1074 fee=5000 compute_units=8351 publish record: Append
slot=1106 fee=5000 compute_units=12516 publish record: Finalize
slot=1138 fee=5000 compute_units=8621 create Market-scoped lifecycle RentCreditV2
slot=1170 fee=5000 compute_units=10661 create Found31 routing address lookup table
slot=1202 fee=5000 compute_units=11807 extend Found31 routing table page 0
slot=1234 fee=5000 compute_units=8869 extend Found31 routing table page 1
slot=1267 fee=5000 compute_units=6958 Found31 refuses substituted lifecycle credit
slot=1299 fee=5000 compute_units=829172 Found31 refuses a substituted Market coordinate and rolls the transaction back
```

The hostile cases in that transcript, in order:

| Refusal | CU | What it pins |
|---|---:|---|
| `real-SBF wrong-authority initialization` | 5,777 | only the plan-pinned ephemeral authority may create the sole infrastructure profile |
| `real-SBF activation before revoke` | 539,770 | a Core still carrying an upgrade authority is not an accepted immutable release |
| `publish record: substituted refund wallet refuses` (twice) | 10,689 / 7,688 | Registry Finalize refunds only the exact staging sponsor |
| `Found31 refuses substituted lifecycle credit` | 6,958 | Found31 will not accept a foreign account as its Market-scoped RentCreditV2 |
| `Found31 refuses a substituted Market coordinate and rolls the transaction back` | 829,172 | routing is not authority: the Market address is derived from the immutable identity, and the whole two-instruction transaction reverts to a fee-only debit |

Not shown, because it runs after the Found stage in the harness and the harness
aborts first: the late substituted-ProgramData activation rollback, which the
same campaign asserted in earlier runs.

The two Found31 refusals are worth contrasting. The substituted-credit case
costs 6,958 CU because the RentCredit coordinate is checked early. The
substituted-Market case costs **829,172 CU** because the Market identity is
only contradicted after Core has already re-derived the release membrane —
including the ELF hashing above. A refusal that expensive is itself an argument
for the fast path: the cheap check is behind the expensive one.

## What an open-market snapshot would unlock, and what to do next

Nothing downstream of Found is unblocked by this run, and it is worth being
plain about that: there is no Market account, so there is no Claims aggregate,
no founder Position, no Hoard, and nothing for the Hot path, the portfolio
route, or `/markets/:market` to read.

The ordered path to a first Open market is now concrete:

1. **Get Found31 under 1.4M CU** — stop hashing immutable ELFs on chain
   (W2, `310d018` direction). This is the only blocker that is a pure
   optimisation; the authentication it replaces is already implied by the
   immutable deployment binding.
2. **Decide what a founding capability root is** (Blocker A) and implement the
   route that creates one, together with the manifest's FundingState accounts.
3. **Add the family-neutral projected-Custody bootstrap** (Blocker B):
   `Initialize`, `OpenHoard`, and a funded funding-source vault under a Trading
   caller PDA.
4. **Then** drive `DCLTGMF1`. The frame is 139 account references at three
   funding states, so it needs the same address-lookup-table routing this lane
   added, and it will be measured against the same 1.4M ceiling with five CPIs
   in one rollback domain.
