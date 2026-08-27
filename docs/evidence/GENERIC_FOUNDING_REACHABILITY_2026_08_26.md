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
| No route creates a pre-Market Trading capability root | **decided, `docs/decisions/0004-founding-capability-root.md`; implementation queued** |
| No live route creates the projected-Custody state the outer's Lock consumes | open; needs an implementation owner |

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
