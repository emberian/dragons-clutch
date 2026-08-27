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

## Superseded in part on 2026-08-26 by the W1b lane

**The Market is now Found.** Everything below the "Headline" heading is the
historical record of the first campaign and is kept verbatim as such. Two of its
findings no longer hold, and the current truth is in
[the W1b supersession section](#w1b-supersession-2026-08-26), which carries the
new measurements, the new artifact digests, and the new campaign transcript.
Read that section for what is true now; read the rest for how it was found.

| Then | Now |
|---|---|
| Registry activation with the real seven artifacts could not execute | executes; five transactions, worst role **682,276** CU |
| Found31 exhausted the 1,400,000 CU maximum | executes; **234,043** CU, and the Market account exists |
| The Market is not Open, and it is not Found either | the Market is **Found**. It is still **not Open** |

## Superseded again on 2026-08-26 by the W1c lane

Blockers A and B are **implemented**, and a third structural blocker was found
beneath them. The current truth about reachability is in
[the W1c supersession section](#w1c-supersession-2026-08-26). That section adds
**no on-chain evidence**: W1b's transcript and CU figures remain the newest
measurements. The Market is still **not Open**.

| W1b said | W1c |
|---|---|
| Blocker A decided, implementation queued | implemented, `728299a`; frame 139 to 137 |
| Blocker B needs a new Trading vertical | implemented, `28d2da6`; `DCLTPCB1`, 60 accounts |
| (unrecorded) | the projected-Custody caller PDA seed domain was 35 bytes and **could never derive an address**, so the entire projected family was dead at runtime; fixed `f30d087` |
| "plus a funded funding-source vault", as part of Blocker B | **Blocker C**, its own Custody-side vertical: the Lock stage's funding source cannot be created, and cannot be built on the existing normal-custody handlers |

## Headline

*(historical, 2026-08-26, first campaign)*

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
| Found31 exceeds the 1.4M CU maximum (on-chain full-ELF hashing) | **fixed, `c61376d`** |
| Registry activation with the real seven artifacts also exceeds it | **fixed, `c61376d`** |
| No route creates a pre-Market Trading capability root | **decided, `docs/decisions/0004-founding-capability-root.md`; implemented, `728299a`** |
| No live route creates the projected-Custody state the outer's Lock consumes | **implemented, `28d2da6`** |
| The projected-Custody caller PDA seed domain exceeded 32 bytes, so no projected transition could ever sign | **fixed, `f30d087`** (found by W1c) |
| No route creates the funding source the outer's Lock consumes and closes | open; needs a Custody-side implementation owner |

## Artifacts

*(historical, 2026-08-26, first campaign; the current artifacts are in the
supersession section above)*

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


## W1b supersession 2026-08-26

Same evidence boundary as the rest of this document: one
`solana-test-validator 4.0.2` bound to 127.0.0.1, fresh genesis, local-validator
evidence only. Not devnet, not mainnet, not a deployment, no formal verification
claimed. Every CU figure is measured-profile on this validator with these exact
artifacts.

### What changed

`c61376d` split the two defects that wore the same symptom.

**Recurring readers stopped recomputing an authenticated fact.** Registry
reauthentication (`programs/dclutch-registry-sbf/src/lib.rs`) and Core's Found
path (`programs/dclutch-core-sbf/src/infrastructure.rs`) now take
`immutable_release_elf_digest_v1` through one shared
`cached_role_deployment_observation`, exactly as `batch_v2.rs` already did. Core
gains the same split for its immutable infrastructure profile: first admission
in `process_initialize` still hashes, recurring `authenticate_profile` does not.
The fast path is **strictly stronger** than the hash it replaces — it requires
the immutable policy, an absent recorded authority, and an absent live upgrade
authority, none of which the hashing path demanded on its own — and identity,
link, ownership, executability, deployment slot, and authority are all still
rechecked.

**Activation could not be optimised, so it was split.** First admission is the
one site that checks an artifact record's *claimed* `elf_digest` against the
bytes actually deployed; a finalized record is attacker-publishable until then.
`RegistryInstructionV1::ActivateRole` therefore admits exactly one role per
transaction, so the largest single hash is one artifact rather than five. The
activation cache was already an incrementally written, idempotent, alias-checked
buffer, and a partially written cache cannot `decode`, so no reader can consume
a half-activated release set.

### Artifacts

Built from `dcd7ac3` in an isolated `git archive HEAD` tree with the pinned
toolchain (`cargo-build-sbf 4.0.0`, `platform-tools v1.53`, `rustc 1.89.0`),
default release profile, no `--lto` and no `--optimize-size`.

| Program | Bytes | SHA-256 |
|---|---:|---|
| `dclutch_registry_sbf.so` | 220,728 | `954ebcf92cbbed25e3f22d817f894275a566cf2f4d1903b52bc2cb893e727f79` |
| `dclutch_core_sbf.so` | 1,008,472 | `5b75d2f4e358514dc6da1c19911d101416047df1c4d9707dd368981b299f8e1e` |
| `dclutch_claims_sbf.so` | 1,074,256 | `66ddc6c9daa23dc022f42be9ed15cd8274de8e791d0cb3d66745ba38e5d849b2` |
| `dclutch_trading_sbf.so` | 1,287,344 | `43fb1bad091bd89ef9a6cc9114f15612044ef97b7fe18e441743200dcba3fbb3` |
| `dclutch_resolution_proof_sbf.so` | 463,576 | `9722b3241d252c1a0d51bf5eef178aa6aace963eaa0865da9e89cce54f8fcb8c` |
| `dclutch_custody_sbf.so` | 330,440 | `5ae26631d815e944d7d55e8d0544fe684b2d01d25909833e009e5858d85260fe` |
| `dclutch_rent_sbf.so` | 152,312 | `3486a8197af492317a756e2fce659d399c5e32ff16323edac34fc1f1cafa7b8b` |

**This campaign binds all seven real artifacts.** The earlier substitution of the
much smaller Registry ELF for the Claims, Trading, Resolution, and Custody roles
is gone; it existed only because five-role activation could not otherwise
execute.

### Measured: activation now fits, one role at a time

| Role | ELF bytes | Measured CU | Share of the 1,400,000 maximum |
|---|---:|---:|---:|
| Core | 1,008,472 | 549,108 | 39.2% |
| Claims | 1,074,256 | 570,883 | 40.8% |
| Trading | 1,287,344 | **682,276** | **48.7%** |
| Resolution | 463,576 | 273,751 | 19.6% |
| Custody | 330,440 | 219,442 | 15.7% |

The worst single activation transaction is Trading at 682,276 CU. The previous
five-role transaction with these same artifacts consumed 1,399,850 and failed
with `Computational budget exceeded`.

The rate inferred in the historical section holds: Trading hashes 1,287,344
bytes for roughly 642,000 CU of the 682,276, or about one CU per 2.0 bytes.

### Measured: Found31 executes

```text
slot=1461 fee=5000 compute_units=234043 create canonical Found31 Market
```

**234,043 CU, 16.7% of the maximum**, against 1,399,850-and-failing before. The
predicted saving was ~1,193,700 CU of ELF hashing; the observed transaction is
1,165,807 CU cheaper than the one that died at the ceiling, which is that
prediction to within the noise of what the failed transaction never got to run.

The Market account exists. Both Found31 hostile cases still refuse, and the
expensive one got much cheaper because the membrane it had to re-derive before
contradicting the Market identity is no longer expensive:

| Refusal | Then | Now |
|---|---:|---:|
| `Found31 refuses substituted lifecycle credit` | 6,958 | 6,958 |
| `Found31 refuses a substituted Market coordinate and rolls the transaction back` | 829,172 | 141,896 |

### Campaign transcript

Forty-six confirmed transactions in 747 seconds against one validator. Eight
lines are hostile cases; each was required to fail and required to leave no
poststate behind.

```text
slot=52 fee=5000 compute_units=5777 real-SBF wrong-authority initialization
slot=84 fee=5000 compute_units=226831 real-SBF infrastructure init
slot=116 fee=5000 compute_units=538050 real-SBF activation before revoke
slot=148 fee=5000 compute_units=2520 real-SBF Core Loader revoke
slot=180 fee=5000 compute_units=549108 activate immutable release-set role: Core
slot=212 fee=5000 compute_units=570883 activate immutable release-set role: Claims
slot=244 fee=5000 compute_units=682276 activate immutable release-set role: Trading
slot=276 fee=5000 compute_units=273751 activate immutable release-set role: Resolution
slot=308 fee=5000 compute_units=219442 activate immutable release-set role: Custody
slot=340 fee=5000 compute_units=27020 publish record: Begin
slot=372 fee=5000 compute_units=12848 publish record: Append
slot=404 fee=5000 compute_units=10688 publish record: substituted refund wallet refuses
slot=436 fee=5000 compute_units=19959 publish record: Finalize
slot=468 fee=15000 compute_units=5263 create real Token-2022 collateral and raw-atom wallet
slot=500 fee=5000 compute_units=27022 publish record: Begin
slot=532 fee=5000 compute_units=12850 publish record: Append
slot=564 fee=5000 compute_units=10688 publish record: substituted refund wallet refuses
slot=596 fee=5000 compute_units=20027 publish record: Finalize
slot=628 fee=5000 compute_units=24022 publish Product graph: Product Begin
slot=660 fee=5000 compute_units=11350 publish Product graph: Product Append
slot=692 fee=5000 compute_units=17027 publish Product graph: Product Finalize
slot=724 fee=5000 compute_units=30022 publish Product graph: ResultDomain Begin
slot=756 fee=5000 compute_units=14350 publish Product graph: ResultDomain Append
slot=788 fee=5000 compute_units=23187 publish Product graph: ResultDomain Finalize
slot=820 fee=5000 compute_units=24022 publish Product graph: Portfolio Begin
slot=852 fee=5000 compute_units=11350 publish Product graph: Portfolio Append
slot=884 fee=5000 compute_units=17155 publish Product graph: Portfolio Finalize
slot=916 fee=5000 compute_units=24022 publish record: Begin
slot=948 fee=5000 compute_units=11350 publish record: Append
slot=980 fee=5000 compute_units=17123 publish record: Finalize
slot=1012 fee=5000 compute_units=18020 publish record: Begin
slot=1044 fee=5000 compute_units=8348 publish record: Append
slot=1076 fee=5000 compute_units=11408 publish record: Finalize
slot=1108 fee=5000 compute_units=18022 publish record: Begin
slot=1140 fee=5000 compute_units=8352 publish record: Append
slot=1172 fee=5000 compute_units=8352 publish record: Append
slot=1204 fee=5000 compute_units=8350 publish record: Append
slot=1236 fee=5000 compute_units=12515 publish record: Finalize
slot=1268 fee=5000 compute_units=7121 create Market-scoped lifecycle RentCreditV2
slot=1300 fee=5000 compute_units=10661 create Found31 routing address lookup table
slot=1332 fee=5000 compute_units=11807 extend Found31 routing table page 0
slot=1364 fee=5000 compute_units=8869 extend Found31 routing table page 1
slot=1397 fee=5000 compute_units=6958 Found31 refuses substituted lifecycle credit
slot=1429 fee=5000 compute_units=141896 Found31 refuses a substituted Market coordinate and rolls the transaction back
slot=1461 fee=5000 compute_units=234043 create canonical Found31 Market
slot=1493 fee=5000 compute_units=22977 real-SBF late substituted activation
```

The late-substitution rollback case that the earlier run never reached now runs:
a substituted role ProgramData in a Custody role activation, in a
two-instruction transaction whose first instruction is a lamport transfer, must
refuse and leave neither the transfer recipient nor any cache mutation behind.
It costs 22,977 CU rather than the old five-role frame's cost, because a
ten-account activation contradicts the substituted deployment before it hashes.

### Why the Market is still not Open

Blocker C is closed. The Open-last chain still needs the atomic outer
(`DCLTGMF1`), and that is gated on the two remaining structural facts.

**Blocker A is decided but not implemented.**
`docs/decisions/0004-founding-capability-root.md` resolves the lifecycle cycle:
the founding capability root is *derived* by Core from the authenticated
Market-selected capability manifest entry and never persisted or read, and the
root account is created afterwards by the unchanged ordinary activation route.
The historical section below proposed two options and guessed the second was
likelier; both were rejected, because the root account is never dereferenced
anywhere except `authenticate_root` and every field of its header is a pure
function of the request plus the manifest. The ADR carries the exact wire
consequence, the frame arithmetic (139 to 137 accounts), the required refusals,
and the file plan.

**Blocker B is open, and its shape is not what this document assumed.** The
historical section names
`programs/dclutch-trading-sbf/src/projected_custody_composition_v4.rs` as the
dead family-neutral route. Read at
`projected_custody_composition_v4.rs:256-264` and `:418-433`, that module is a
**Lock** adapter: it can emit only `LockHoardAndCloseSource`, and it *requires*
the projected state to already be in phase `HoardOpen`. Dispatching to it
bootstraps nothing. The real gap is that Custody's `Initialize` (42 accounts,
`programs/dclutch-custody-sbf/src/projected.rs:382`) and `OpenHoard` (15
accounts, `:490`) each require a signing `ProjectedCustodyCallerSeedsV1` PDA
under the Trading program (`:156`, `:201-205`), so they are reachable only by
CPI from a Trading dispatch branch that does not exist; and the only in-tree
constructor of those two requests, `series/projected_custody_v3.rs:85
project_prepare_v3`, is Series-shaped and has no non-test caller. A
family-neutral bootstrap is a new Trading route, not a wiring change.

## Why the atomic outer was unreachable

*(historical, 2026-08-26, first campaign; Blocker A is now decided and Blocker C
is closed — see the supersession section above)*


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

*(historical, 2026-08-26, first campaign)*

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

The Market account now exists, so the founder-facing surfaces have a real Core
state to read for the first time. What still does **not** exist is everything
the atomic outer would create in the same rollback domain: there is no Claims
aggregate, no founder Position, and no Hoard, because Found is not Open.

The ordered path to a first Open market, updated:

1. ~~**Get Found31 under 1.4M CU**~~ — **done, `c61376d`.** Found31 costs
   234,043 CU and the worst activation transaction costs 682,276 CU, both
   measured above. The saving is structural, not a tuning pass: recurring
   readers stopped recomputing an authenticated digest, and first admission was
   split one role per transaction rather than weakened.
2. **Implement the derived founding capability root** (Blocker A) —
   *decided* in `docs/decisions/0004-founding-capability-root.md`, with the
   wire change, the frame arithmetic, the required refusals, and the file plan
   already written. No route needs to be *created*: Core derives the root
   address instead of reading an account, and ordinary activation keeps sole
   authority over creating the account later.
3. **Add the family-neutral projected-Custody bootstrap** (Blocker B) — a new
   Trading dispatch route that emits `Initialize` and `OpenHoard` under a
   `ProjectedCustodyCallerSeedsV1` signer, plus a funded funding-source vault.
   This is a new vertical slice, not a dispatch wiring change; see the
   supersession section for why `projected_custody_composition_v4.rs` cannot
   serve.
4. **Then** drive `DCLTGMF1`. After Blocker A the frame is 137 account
   references at three funding states, so it needs the same address-lookup-table
   routing this lane added, and it will be measured against the same 1.4M
   ceiling with five CPIs in one rollback domain. Found31's 234,043 CU is the
   only one of those five stages measured so far.

The on-chain founding-outer hostile case this document wants — a substituted
Claims request account proving byte-exact rollback of the whole
Lock -> Found -> Realize -> Claims -> Open chain — still needs the route to
execute, and therefore still waits on Blocker B. The pure cross-request join
tests remain what they were: a pure function checked without a chain, and not
execution evidence.

## W1c supersession 2026-08-26

**This section adds no on-chain evidence.** No validator campaign was run for it.
Everything below is source-level: implemented routes, unit-tested refusals, and
one structural impossibility established by reading the code that decides it. The
CU figures and the transcript in the W1b section remain the newest measurements
and are unchanged. Read this section for what is now *implemented* and for why the
outer is still not executable; read W1b for what has actually run.

### Blocker A is implemented

`docs/decisions/0004-founding-capability-root.md` was decided at `dcd7ac3` with an
exact file plan and no implementation: the account counts were still 35 and 24 and
`authenticate_root` still read a root account. `728299a` implements all seven
steps.

Core now rebuilds `CapabilityRootHeaderV1` from facts it has already
authenticated and requires the request to name the address that derives from it.
The Found stage's funding authentication already decodes the Market-selected
manifest, so the derivation shares that decode instead of paying for a second one
inside the founding transaction. The Open stage does not re-derive: Found
persisted the derived address in the Core-owned permit, and `authenticate_permit`
and `authenticate_open_request` both require it, so an Open stage that disagreed
with its own Found stage cannot be constructed. That is the ADR's "must be
impossible" refusal, discharged by construction rather than by a second
derivation.

The strengthening the ADR predicted is real and now enforced: `manifest`,
`entry_index`, `kind`, and `capability_release` were previously supplied entirely
by the founder and checked only for self-consistency, so **nothing bound the
selection to the Market's own authenticated capability manifest**. The manifest
identity is now the one Found authenticated and the kind and release come from
that manifest's own indexed entry, as Decision 0003 already required of the
ordinary route. The operator no longer accepts the four coordinates as free
parameters at all.

Wire and frame, exactly as the ADR specified: the request keeps its 400 bytes and
spends its previously unchecked `392..400` tail on `capability_entry_index` plus
reserved bytes `decode` now requires to be zero; Found goes 35 to 34, Open 24 to
23, and `DCLTGMF1` at three funding states goes **139 to 137**. The frame-width
test in `generic_market_founding_v1.rs` asserted 139 and now asserts 137.

Unit refusals added: a foreign manifest, an entry index outside the authenticated
entry count, a sibling entry's exact kind and release, non-canonical manifest
bytes, every nonzero reserved tail byte, and a substituted `capability_root` in
the outer's `request_join` tests, which had never substituted it at all.

### Blocker B is implemented — `DCLTPCB1`

`28d2da6` adds the family-neutral projected-Custody bootstrap the W1b handoff
specified. It is one Trading dispatch branch, bound to one terminal
`LockHoardAndCloseSource` request and one founding artifact, driving Custody
`Initialize` (42 accounts) and `OpenHoard` (15 accounts) under their single-use
`ProjectedCustodyCallerSeedsV1` signers in a single rollback domain, so a Market
is never left holding a replay with no Hoard. The frame is 60 accounts.

The family-neutrality is structural rather than asserted. A replay reaches
`HoardOpen` by exactly two transitions, and Custody's `authenticate_next` admits a
successor only when all thirty of its non-transition fields match the persisted
request byte-for-byte. `ProjectedCustodyRequestV1::founding_prestate_v1` therefore
builds both prestates by functional update from the terminal request, varying
exactly the four fields Custody permits to vary. No family, escrow shape, or
ticket namespace can enter, and the superseded Series-shaped constructor is
deleted.

The found-to-lock conjunction the outer evaluated inline is now a shared predicate
both routes call. A prestate this route creates is admissible at Lock because the
two routes evaluate the *same* predicate, not because two constructors agree.

Child CPI metas are built from this route's own authenticated frame — a direct
instruction, never an Effect-V3 route adapter, so it never consults a downgraded
privilege view. Writable and signer masks are asserted rather than mirrored from
the runtime. The Custody program is taken from the Registry-activated release set
the request names, so a substituted program cannot receive a Trading-derived
caller signature. Neither transition returns data, so the persisted replay is the
receipt: it is read back and required to be exactly the poststate of the request
just signed.

### Defect 4 — the projected-Custody caller PDA could never be derived

The projected family was unreachable for a reason no one had recorded, underneath
both blockers above.

`PROJECTED_CUSTODY_CALLER_PDA_DOMAIN_V1` was `dclutch:projected-custody-caller:v1`
— **thirty-five bytes**. A Solana PDA seed is capped at thirty-two, so
`find_program_address` refuses every one of the 255 bumps and **no address exists**.
Custody demands that signature for `Initialize`, `OpenHoard`, `LockHoard`,
`RealizeAndClose`, `AbortOpenAndClose`, and `LockHoardAndCloseSource`, so *every*
projected-Custody transition was dead at runtime — the Series prepare and consume
path, `projected_custody_composition_v4`, and the atomic outer's own Lock and
Realize stages included.

It compiled, unit-tested, and reviewed clean for as long as it existed, because
nothing in the tree had ever derived the address. It surfaced the first time a
test did: the assertion that each prestate has its own single-use caller authority
panicked with `Unable to find a viable program address bump seed`. This is the
sharpest available argument for the project's own rule that a slice must include
operator construction and executable evidence — a pure-kernel review of that
constant finds nothing wrong with it.

Fixed at `f30d087`: the domain is now `dclutch:proj-custody-caller:v1` (thirty
bytes). Static assertions cover every Custody PDA domain, and a test asserts the
real precondition on the real seed vector rather than on the constant. **Every
projected-Custody caller PDA address has moved**; any fixture pinning one must be
regenerated. A repo-wide sweep of `*SEED*`/`*PDA*` byte-string constants found one
other over-long domain, not in this lane's ownership and with no seed use today:
`GENERAL_CANDIDATE_PAGE_PDA_DOMAIN_V1` is thirty-three bytes at
`crates/dclutch-general-contract/src/lib.rs:120`.

### Blocker C — the Lock stage's funding source is not creatable, and cannot be built on the existing normal-custody handlers

`DCLTGMF1` is **still not executable**, and the reason is a third structural
blocker that this lane established and that the W1b handoff had folded into
Blocker B as the parenthetical "plus a funded funding-source vault". It is not a
sub-task of Blocker B. It is a Custody-side protocol gap, and the two requirements
that create it are mutually unsatisfiable today.

`LockHoardAndCloseSource` consumes and closes two accounts that
`projected_custody_bootstrap_v1` deliberately does not touch — the funding source
vault at frame index 8 and the funding source replay at index 12:

```text
custody-sbf/src/projected.rs:1213-1250  source vault  = PDA[CUSTODY_VAULT_PDA_DOMAIN_V1,
                                          request.market, release_set,
                                          funding_source_context, compartment]
custody-sbf/src/projected.rs:1226-1245  source replay = PDA[CUSTODY_REPLAY_PDA_DOMAIN_V1,
                                          request.market, release_set,
                                          funding_source_context]
                                        decoded as a NORMAL CustodyReplayV1
custody-contract/src/projected.rs:749-790  replay.market == request.market,
                                          caller_role == Trading,
                                          open_vault_count == 1,
                                          generation == request.generation
custody-sbf/src/projected.rs:971,1257-1271  request.market must be VACANT:
                                          owner == system_program, data_len() == 0
```

The source must therefore name the very Market being founded, while that Market's
account is still a vacant System account. Every route that can write a normal
`CustodyReplayV1` requires the opposite:

- normal `InitializeReplay`, `OpenVault`, and `Transfer` all pass through
  `authenticate_market` (`custody-sbf/src/lib.rs:216-278`), which unconditionally
  requires `market.data_len() == STATE_BYTES` and `market.owner == core_program`,
  and re-derives `MarketCoreStateSeedsV2` against the live state. `CustodyReplayV1::initialize`
  has exactly one caller, and it is behind that check.
- the only other producer of a normal replay is
  `normal_replay_from_realization_v1`, which mints precisely the shape Lock wants
  (Trading role, one open vault, `next_revision` 1) but is reachable only from
  `realize_and_close`, which also requires a live Core-owned Market at
  `request.market` (`custody-sbf/src/projected.rs:857-864`).

The Market address is a PDA over an identity that includes `generation`, so a
previous market's realized leftovers can never sit at the next founding's address
either. A first founding has no reachable prestate for those two accounts.

**This must not be resolved by relaxing `authenticate_market`.** That check is
what makes normal custody's whole surface a live-Market membrane; widening it to
admit a vacant Market would be a permanent enlargement bought to fix one
transaction's ordering — the same trade Decision 0004 rejected for
`activate_capability_child`. The shape that fits the existing design is a new
projected-family Custody operation that opens a *source* compartment against a
vacant Market and takes `market_vacant` explicitly, exactly as `OpenHoard` already
does for `HoardPrincipal`, plus its Trading bootstrap branch. `OpenHoard` cannot
serve: it pins the compartment to `HoardPrincipal`, which
`ProjectedCustodyRequestV1::validate` explicitly forbids as a funding source, and
it writes a `ProjectedCustodyStateV1` where Lock requires a `CustodyReplayV1`.

That is a Custody protocol addition with its own wire, its own frame, its own
refusals, and its own owner. It is the last thing between here and an Open Market.

### Artifacts

Built from `28d2da6` in an isolated `git archive HEAD` tree with the same pinned
toolchain W1b used (`cargo-build-sbf 4.0.0`, `platform-tools v1.53`,
`rustc 1.89.0`), default release profile, no `--lto` and no `--optimize-size`.
Only the three programs this lane changed were rebuilt; Registry, Claims,
Resolution, and Rent are unchanged from the W1b table above. **These ELFs have
not been deployed or executed** — no campaign was run for this section.

| Program | Bytes | SHA-256 |
|---|---:|---|
| `dclutch_core_sbf.so` | 1,007,032 | `c212c8ea3907e2256c441717e538fdf2b6b0fed22e6c2f7836a42883889490d2` |
| `dclutch_custody_sbf.so` | 330,432 | `6434093093bf14615e47c58fd2bcef784af05ed7dae1b8b4e5a14f84d3fcf4ac` |
| `dclutch_trading_sbf.so` | 1,309,048 | `47931749058f0ee0023836cb6020571c0955ac6632d3d3e1d4a6feeaf7515382` |

Core shrank by 1,440 bytes: dropping the capability-root account read removed
more code than the derivation added. Trading grew by 21,704 bytes for the new
bootstrap route.

### The path to a first Open Market, corrected again

1. ~~Get Found31 under 1.4M CU~~ — done, `c61376d`, 234,043 CU measured.
2. ~~Implement the derived founding capability root (Blocker A)~~ — done,
   `728299a`.
3. ~~Add the family-neutral projected-Custody bootstrap (Blocker B)~~ — done,
   `28d2da6`, plus the underivable-seed fix at `f30d087` without which neither it
   nor anything else in the family could have signed.
4. **Make the Lock stage's funding source creatable (Blocker C)** — open, unowned,
   and a new Custody-side vertical as described above. No amount of Trading wiring
   reaches it.
5. **Then** drive `DCLTGMF1` at 137 account references over the address-lookup-table
   routing this campaign already builds, and measure five CPIs in one rollback
   domain against the 1.4M ceiling. Found31's 234,043 CU remains the only one of
   those five stages measured.

The on-chain founding-outer hostile case this document has wanted since the first
campaign — a substituted Claims request account proving byte-exact rollback of the
whole Lock to Open chain — is **still not claimed**, and now waits on Blocker C
rather than Blocker B. The cross-request join tests added in this lane, including
the capability-root substitution, are what they have always been: pure functions
checked without a chain. They are not execution evidence and are not counted as
any.

## W1d supersession 2026-08-27

Blocker C is **implemented**, and a fourth structural blocker was found beneath
it and **also implemented**. This section adds **no on-chain evidence**: W1b's
transcript and CU figures remain the newest measurements, and **no campaign was
run this lane**. The Market is still **not Open**. Saying so plainly because the
lane's gate was an Open Market and the honest answer is that the founding outer
became *assemblable* here, not *executed*.

| W1c said | W1d |
|---|---|
| Blocker C is a new Custody-side vertical, unowned | implemented, `d3ba6a1`: `OpenSourceCompartment` |
| `DCLTPCB1` overflows its SBF verifier frame by 4,480 bytes | fixed, `d3ba6a1`; the whole seven-program build now emits **zero** frame diagnostics (`9258bce`) |
| generic founding's `projected_resulting_revision` must be 4 | it is **5** |
| (unrecorded) | **Blocker D**: no route in the protocol could create the FundingState prestate Core's Found stage consumes. Implemented, `2fffe79` |
| the founding outer needs a runner | the runner still does not exist; the complete frame maps for both routes are recorded below so the next lane does not have to rediscover them |

### Blocker C is implemented — `OpenSourceCompartment`

`d3ba6a1` adds one projected-family Custody operation. It creates the normal
`CustodyReplayV1` and the funded source Vault that `LockHoardAndCloseSource`
consumes and closes, against a Market account that does not exist.

**`authenticate_market` is byte-for-byte untouched, and is not on the new path.**
That was the constraint W1c named and it held. Normal Custody's live-Market
membrane still requires `data_len() == STATE_BYTES` and Core ownership for every
ordinary operation. What admits the new transition instead is the projected
family's own membrane, which already existed:

```text
custody-sbf/src/projected.rs  authenticate_common      single-use Trading caller PDA
                                                       + persisted ProjectFound projection
                                                       + release reauthentication CPI
                                                       + the exact prior revision
custody-sbf/src/projected.rs  require_vacant_market    owner == system_program, data_len() == 0
```

`require_vacant_market` is the **inverse** of `authenticate_market`, not a
relaxation of it, and `OpenHoard` already asserted exactly it. The new operation
asserts the same thing and adds nothing to the ordinary surface.

**The ladder gains one ordered step.** `Initialize` `0→1` and `OpenHoard` `1→2`
are unchanged. `OpenSourceCompartment` runs `2→3` and moves the phase to
`SourceFunded`, a tag no previously reachable state can hold.
`LockHoardAndCloseSource` now admits exactly two disjoint prestates:

| prestate | who | `locked_amount` |
|---|---|---|
| `HoardOpen` | a family whose principal is already custodied elsewhere — Series escrow | must be `0` |
| `SourceFunded` | a generic founding, whose own `OpenSourceCompartment` funded the source | must equal `request.amount` |

Because `SourceFunded` is a new value, **no state that existed before is admitted
by this that was refused before**. Series is untouched and still reaches Lock
from `HoardOpen` at revision two.

**The replay is the kernel's, not the adapter's.** Every field of the minted
`CustodyReplayV1` is a function of the authenticated request, and its cursor is
pinned by `SOURCE_COMPARTMENT_REPLAY_REVISION_V1 = 1` rather than chosen — the
same value `normal_replay_from_realization_v1` mints for the family's other
replay. A founder cannot choose what the Lock stage will later read back.

**Funding provenance is closed by the request that already existed.** Rent for
both created accounts comes from `request.payer`, the same prepaid creation payer
that funds the projected replay and the Hoard vault, and both accounts' lamports
return to `request.rent_credit` when Lock closes them. The principal comes from a
token account owned by `request.refund_owner`, who must sign — the same party
`RefundAndClose` pays the principal *back* to. Whoever may reclaim the principal
is exactly who must supply it. No new identity coordinate enters, and
`ProjectedCustodyRequestV1::validate` already forbids `HoardPrincipal`, `None`,
and `External` as the source compartment.

Adversarial coverage added at the kernel (`crates/dclutch-custody-contract`,
23 tests green): a live Market (the inverse of the operation's whole admission),
a funder debit that does not equal the principal credited, a pre-funded source
vault, a vault that does not end holding exactly the request's principal, a zero
replay address, a zero poststate commitment, every other operation attempted at
the funded phase, a replayed creation at the same cursor, and the closed-out
attempt (`AbortOpenAndClose` at `SourceFunded`). The persisted-state round trip
is pinned, and the minted replay is shown to be exactly the one the terminal
Lock accepts. **These are pure functions checked without a chain. They are not
execution evidence.**

### Blocker D — the capability-funding prestate had no creator at all

Found underneath C, by reading what Core's *Found* stage requires rather than
what its Lock stage requires.

```text
core-sbf/src/generic_founding_v1.rs:808-884   one FundingStateV1 per manifest entry
                                              owner == trading_program
                                              data_len() == FUNDING_STATE_BYTES (320)
                                              status == FundingStatus::Pending
                                              manifest_content_id == the authenticated manifest
                                              entry_index == its position
                                              lamports - rent == the manifest's quoted native total
                                              address == CapabilityFundingDerivationV1 PDA under Trading
                                              ordered list digest == request.funding_list_id()
                                              manifest.entry_count() == funding_count, and it may not be zero
```

The only allocator of such an account anywhere in the repo is
`programs/dclutch-trading-sbf/src/series/accounts.rs:223 stage_pending_funding`.
It is Series-shaped — its lamport source is a Trading-owned Ticket — and
**`grep -rn stage_pending_funding` returns only its own definition.** It has no
caller. Every other site (`outer.rs:716`, `general/activation.rs`,
`core-sbf/src/capability.rs`) only consumes or validates existing states.

A host cannot supply them either, and this is the part that makes it structural
rather than merely missing: a `FundingStateV1` is a **program-derived address
owned by the Trading program**. No private key for it exists, so no wallet can
sign its creation, and `system_program::assign` requires the account itself to
sign. Only Trading can create one, from inside Trading, under
`invoke_signed`. So a generic founding could not assemble its Found frame at
all — this was upstream of every compute or wire concern.

**Implemented at `2fffe79`** as a fourth `DCLTPCB1` stage in the same rollback
domain as the other three. Nothing in it is a caller choice:

- the manifest is the **exact record Core itself authenticated** during the
  `ProjectFound` projection at stage one, reused from that sub-frame rather than
  supplied a second time;
- every address, every prepayment, and every persisted byte is derived from it;
- the ordered list of created addresses must equal the founding artifact's own
  `funding_list_id`, so a manifest that is not this Market's cannot leave a
  single funded account behind — the refusal rolls the whole bootstrap back;
- funding provenance is the founding's prepaid payer, the same account that
  funds the projected replay, the Hoard vault, and the source compartment. These
  are lamports, precommitted and prepaid exactly as the manifest quotes them. No
  Hoard, fee, liveness, or reserve compartment is touched and no principal moves.

The bootstrap frame is therefore no longer a constant. It is
`PROJECTED_CUSTODY_BOOTSTRAP_FIXED_ACCOUNT_COUNT_V1 = 78` plus one account per
manifest entry, and the tail's length is asserted against `funding_count` before
anything is created. For the demo Market's three-entry manifest: **81 accounts**.

### A named new hazard this lane introduced, and why it was chosen

The `SourceFunded` resting state holds real principal, and **no terminal accepts
it**. `AbortOpenAndClose` admits only `HoardOpen`.

That is deliberate. It means the authority over funded principal cannot be
destroyed: closing the projection out from under a funded source would strand
the principal permanently, because the source replay names a Market that will
never become live at that generation. Refusing the abort is the safe direction.

It is still a real liveness hazard, and it is new: before this lane a founder
could only strand rent. A founder who runs `DCLTPCB1` and never runs `DCLTGMF1`
has principal that can move only forward, through Lock. The founding is
retryable indefinitely — the Lock request is deterministic from the founding
artifact and the state rests at `next_revision = 3` — so nothing is lost while
the founding remains satisfiable.

**The closure is a new `AbortSourceAndClose` terminal** at `SourceFunded`, after
expiry, returning the source principal to a `refund_owner`-owned token account
and closing the source vault, the source replay, the empty Hoard vault, and the
projection to `RentCredit`. It is **not** implemented here and **not** an
extension of `AbortOpenAndClose`: Series drives that terminal
(`series/projected_custody_v3.rs:142`) and its frame width is fixed, so widening
it would reshape a live family's route to serve a different one. Queued, named,
and owned by whoever takes the next projected-Custody lane.

### The gate, honestly

**Not met.** No validator campaign was run. The Market is Found, not Open. The
Claims aggregate, the founder Position, and the Hoard still do not exist anywhere
on any chain. The on-chain founding-outer hostile case — a substituted Claims
request account proving byte-exact rollback of the whole Lock→Found→Realize→
Claims→Open chain — is still not claimed, and now waits only on a runner.

What changed is that it no longer waits on a *protocol gap*. Every prestate the
outer's five stages consume now has exactly one live route that creates it:

| prestate | creator | status |
|---|---|---|
| capability root | derived, never created | ADR-0004, `728299a` |
| projected replay | `DCLTPCB1` stage 1 | `28d2da6` |
| Hoard vault | `DCLTPCB1` stage 2 | `28d2da6` |
| source vault + source replay | `DCLTPCB1` stage 3 → Custody `OpenSourceCompartment` | `d3ba6a1` |
| FundingState × manifest entries | `DCLTPCB1` stage 4 | `2fffe79` |
| `LifecycleRentCreditV2` | the campaign's existing RentV2 stage | already live |
| Claims aggregate / Position / admission | the Claims stage itself allocates them | already live — **but see below** |

**One thing the runner must do that is easy to miss**: Claims `FoundingV5`
allocates the aggregate, the founder Position, and the admission with
`System::allocate` + `assign` only. It never transfers lamports. The runner must
**pre-fund those three vacant program addresses** so that each holds at least
`rent.minimum_balance(width)`, or the founding refuses inside Claims. A plain
System transfer to a derived address does that; no protocol route is needed.

### What the runner has to build, exactly

Recorded here because it is the whole remaining distance and it was expensive to
establish. Two transactions, both requiring an address lookup table.

**`DCLTPCB1` — 81 accounts at three funding states.** Eight-byte instruction
data, no payload. Layout: two readonly raw-request accounts (the 400-byte
founding artifact and the 768-byte terminal Lock request), the Custody program,
the 42-account Initialize sub-frame (whose indices 11..41 are Core's 31-account
`ProjectFound` sub-frame, all forwarded readonly and all mutually distinct), the
15-account OpenHoard sub-frame, the 18-account OpenSourceCompartment sub-frame,
then one FundingState account per manifest entry. **49 distinct keys**, well
inside the 64-account lock limit. Exactly four must be writable-and-or-signer at
the transaction level: the projected state, the payer (signer), the two vaults,
the source replay, the funding tail, and — as signer only — the principal
`refund_owner`.

**`DCLTGMF1` — 137 accounts at three funding states** (`134 + funding_count`).
Eight-byte instruction data. Four readonly raw-request accounts (400-byte
founding artifact, 768-byte Lock, 768-byte Realize, 832-byte Claims request),
then Lock (14), Found (`34 + funding_count + 15`), Realize (12), Claims (32),
Open (23). **Eleven distinct keys must be writable**: the projected replay, the
rent credit, the Hoard vault, the source vault, the source replay, the Found
caller PDA, the Market, the Core permit, and the Claims aggregate, Position, and
admission. **No account in the frame may be a transaction-level signer** — every
stage's signer is a PDA signed by `invoke_signed` — so the fee payer must be a
138th key that appears nowhere in the frame.

Three derivations the runner must get exactly right, because each is a caller-PDA
seed input and a wrong one produces an address for which no signature exists:

```text
context_digest       = sha256(b"dclutch:projected-hoard-context:v1" || found.context())
funding_source_context = found.context(), undigested
projection_receipt_digest = sha256(ProjectFoundReceiptV1 bytes)
```

The third is derivable off-chain without a transaction: the receipt is a pure
function of facts the runner already holds (market, generation, realm, mint,
token program, collateral release, product record, product, source, release set,
rent program, and the digest of the encoded `Found` request). It does **not**
require simulating the CPI.

Chain-derived rent facts, not choices: `state_rent_lamports =
minimum_balance(808)`; `vault_rent_lamports = funding_source_vault_rent_lamports
= minimum_balance(165)`; `funding_source_state_rent_lamports =
minimum_balance(288)`; each FundingState holds `minimum_balance(320)` plus the
manifest entry's quoted native total.

And two request-shape pins that will otherwise fail late: the terminal Lock must
carry `expected_revision = 3` and `funding_source_replay_revision = 1`, and the
founding artifact must carry `projected_resulting_revision = 5`.

### Artifacts

Built from `2fffe79` in an isolated `git archive HEAD` tree with the same pinned
toolchain (`cargo-build-sbf 4.0.0`, `platform-tools v1.53`, `rustc 1.89.0`),
default release profile, no `--lto` and no `--optimize-size`. **These ELFs have
not been deployed or executed** — no campaign was run for this section.

The seven-program build emits **zero frame diagnostics**. It emitted twenty at
`ae5c93b` (`process_projected_custody_bootstrap_v1`, 8,576 bytes estimated
against a 4,096 maximum), and twenty again mid-lane for the new Custody
operation before it was split across three verifier frames.

Built from **`05656a3`**, not from this lane's last commit. `bd06c53` and every
commit back to `ee1dc7d` cannot build four of the seven programs from a clean
checkout: `ee1dc7d` committed a `use dclutch_capability_seal_contract::{…}` into
`crates/dclutch-account-profile-contract/src/lifecycle_v3.rs` while the matching
`Cargo.toml` dependency line was still only in the shared working tree. That is
another lane's defect and it is repaired at `05656a3`, so these figures are from
the first commit at or after this lane's work that builds. They therefore also
contain other lanes' concurrent changes to Claims, Core, Trading, and
Resolution; only `dclutch_custody_sbf.so` is attributable to this lane alone,
and its digest is byte-identical to an isolated build of this lane's commits.

| Program | Bytes | SHA-256 |
|---|---:|---|
| `dclutch_registry_sbf.so` | 220,728 | `954ebcf92cbbed25e3f22d817f894275a566cf2f4d1903b52bc2cb893e727f79` |
| `dclutch_core_sbf.so` | 1,007,096 | `c6373ba564e9c7230409eb549143b83998d3b038fbead9dfc08732caf450edb3` |
| `dclutch_claims_sbf.so` | 1,074,256 | `79869b5dec2d60e961c3ac9f9ff5d39780a69bf492cbff35cf393c79fd597f80` |
| `dclutch_trading_sbf.so` | 1,333,064 | `44c15378aa892ad7aa3962302fa8ee28c376ef5e0e2d4e2a2f03e7a770a30bc1` |
| `dclutch_resolution_proof_sbf.so` | 463,576 | `ae18567499be52880db335f06ba1e00596e7489f1297812e6796cb3e3df1c4d9` |
| `dclutch_custody_sbf.so` | 347,536 | `83eb5121559f1d41f75a9e47a4cdfd7cb8927236d8079ba42c8eee032b0195f9` |
| `dclutch_rent_sbf.so` | 152,312 | `3486a8197af492317a756e2fce659d399c5e32ff16323edac34fc1f1cafa7b8b` |

Custody grew by 17,104 bytes over W1c's `6434093093bf…` for the new operation;
Trading grew by 24,016 bytes over that section's figure for the fourth bootstrap
stage and the funding staging, though that delta is not attributable to this
lane alone.

### Gates this lane ran

Filtered, and stated with their controls rather than as a suite-wide green.

| Gate | Result |
|---|---|
| `cargo test -p dclutch-custody-contract` | 24 passed — the kernel's whole adversarial surface for the new operation |
| `cargo test -p dclutch-custody-sbf --lib` | 7 passed — frame widths are operation-exact, including the new eighteen |
| `cargo test -p dclutch-trading-sbf --lib` | 236 passed |
| `cargo clippy --lib -- -D warnings` on all three | clean |
| `cargo fmt --check` on all three | clean |
| `cargo build-sbf`, all seven programs | exit 0, **zero frame diagnostics** |
| the successor runner (`cargo test`, its own workspace) | 16 passed |

The `--all-targets` clippy on `dclutch-custody-sbf` and `dclutch-trading-sbf`
fails on pre-existing test-target lints this lane did not touch
(`programs/dclutch-custody-sbf/tests/program_test.rs:645` `needless_borrow`, and
`indexing_slicing`/`assertions_on_constants` across `dclutch-trading-sbf`'s
integration tests). Recorded rather than fixed: they are not this lane's and
fixing them would put another lane's files in this lane's commits.

