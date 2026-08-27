# Token-2022 CPI integration plan

Status: **implemented, 2026-08-18.** The first sentence of this
document used to read "No program was changed"; that is no longer true, and
§0 below says exactly what changed, what it is evidenced by, and what is still
only a plan. The rest of the document is left as written, with the places
where implementation contradicted it marked in place, so that the plan and its
outcome can be read against each other.

Two rows of §0.1 that used to read "not wired" and "not implemented" now read
"landed": `CreateMarket` creates the outcome mints and the Hoard token account
by CPI, and `Split`, `Merge` and `RedeemInternal` move real collateral. The
consequence is that the token legs are **mandatory** — the optional planes and
their `Absent` variants are deleted, and every emitter has to be regenerated.

Nothing here is verified, audited, or a deployment authorization. A test that
passes on an in-process bank is evidence about *this program on that bank*. It
is not a cluster, not an audit, and not a proof.

## 0. Implementation status

### 0.1 What is implemented

| plan item | state | evidence |
| --- | --- | --- |
| §3.2 three seed appends | **landed**, with one correction | `programs/clutch-sbf/program/src/seeds.rs`; `seeds::tests` |
| §3.2 six error appends, plus one | **landed** | `programs/clutch-sbf/program/src/error.rs` |
| §3.1 outcome-mint role, decimals `0`, freeze `None` | **landed** as `token::MintPolicy::outcome` | `token::tests`, SVM `e1_*` |
| §3.1 collateral-mint role from the Realm bitsets | **landed** as `token::MintPolicy::collateral` | `market_init::tests` |
| §3.4 extension refusal **at market init** | **landed**, mandatory | `market_init::tests`, SVM `create_market_founds…` |
| §3.4 extension refusal **at every token instruction** | **landed**, mandatory, over both mint roles | `merge_materialize::tests`, SVM |
| §3.2 `Materialize` → `MintTo` via `invoke_signed` | **landed** | SVM `e1_materialize` |
| §3.2 `Dematerialize` → `Burn` via `invoke` | **landed** | SVM `e1_dematerialize` |
| §3.3 steps 1-3, 5, 6 (validate, admit, snapshot, CPI, exact deltas) | **landed** | SVM + host |
| §3.5 `HoardAccount.collateral_atoms` checked mirror | **landed**, checked pre-CPI and post-CPI | SVM `no_wallet_signature…`, `split_and_merge…` |
| §3.2 `Split`/`Merge`/`RedeemInternal` collateral `TransferChecked` | **landed** | SVM `split_and_merge…`, `redeem_pays…`, host |
| §3.2 `CreateMarket` creating mints and the Hoard token account | **landed** | SVM `create_market_founds…` |
| §3.4 `ImmutableOwner` required rather than allowed on the Hoard | **landed** (open decision 4, taken) | `market_init::tests`, SVM |
| §4 E5 post-CPI rollback | **landed** | SVM `a_cpi_that_fails_after_the_kernel_ran…` |
| §3.6 account creation, rent exemption, `invoke_signed` seeds | **landed** — `SBF_BRINGUP.md` deferred checks 2, 4, 6 | SVM `create_market_founds…` |

### 0.2 The optional-leg hole, closed

It used to read: "`Materialize` and `Dematerialize` accept **ten** accounts
(the shadow-only instruction that existed before this lane) or **thirteen**
(the token leg). `CreateMarket` accepts **twelve** or **fifteen**. A caller may
present the smaller plane and get the weaker instruction." The stated
closing condition was "the hole closes when `CreateMarket` creates the mints:
the counts become fixed and the `Absent` variants are deleted."

`CreateMarket` creates the mints. The counts are fixed and the `Absent`
variants are deleted:

| instruction | accounts | leg |
| --- | --- | --- |
| `CreateMarket` | `19 + outcome_count` (21 at two outcomes) | creates every mint and the Hoard token account |
| `Split`, `Merge` | **16** | collateral in / out |
| `Materialize`, `Dematerialize` | **13** | outcome mint and holder account |
| `Resolve` | **12**, unchanged | none |
| `RedeemInternal` | **19** | collateral out |

Any other count is `ClutchError::AccountCount`, and *which* count an
instruction takes is a function of the intent rather than of the caller. The
SVM scenario `the_ten_account_plane_moves_only_the_shadow`, which measured the
hole, is replaced by `the_ten_account_plane_is_refused_and_moves_nothing` and
`the_shadow_only_plane_is_gone`, which measure its closure the same way: the
smaller plane refuses and the two truths are still equal because nothing moved.

**This is an ABI break and every emitter has to be regenerated.**
`programs/clutch-sbf/harness` is frozen this wave and still emits ten-account
seam transactions and a twelve-account `CreateMarket`; every one of them is now
refused `AccountCount` on chain. Its *host* leg — the fixture build and the
offline reference adapter inside `build_fixture` — is untouched.

### 0.3 Where the implementation contradicted the plan

**The proposed `hoard-authority` seed prefix does not fit a seed.**
`"dragons-clutch:hoard-authority:v1"` is 33 bytes and a single seed is capped at
32; `find_program_address` refuses it, which on-chain is a panic rather than a
refusal code. The landed prefix is `"dragons-clutch:hoard-auth:v1"` (28 bytes).
`seeds::tests::every_seed_prefix_fits_one_seed` checks every prefix in the
module, including the plan's original as a falsifier. The other two are the
plan's, unaltered.

**§3.3's step order is not exactly what runs.** The plan orders compute (4),
token effects (5), verify (6), commit (7). Steps 1-3 do all run before the
first write, and the CPI and the delta check run in the plan's order. But the
compute step is `split::kernel_step` and `split::ledger_step`, which **write
their accounts as they compute**: the kernel aggregate and the supply ledger are
therefore written *before* the CPI, not after it. Splitting them into
compute-and-commit halves would mean carrying a decoded `KernelAccount` — most
of an SBF frame on its own — across two frames, which is the constraint
`SBF_BRINGUP.md` records. On chain the deviation is invisible: SVM transaction
semantics discard every byte on any later refusal. Off chain it is observable,
and `merge_materialize::tests::refuses_after_the_kernel_ran` asserts exactly
which accounts are written and which are not, so the deviation is a measured
fact rather than a footnote.

**The per-owner external shadow is not reconcilable against tokens; the
market-wide term is.** §3.5 proposes replacing `ExternalAccount` with the mint's
supply. `ExternalAccount::balances[o]` is *one owner's* shadow balance and no
single token account is its counterpart — a holder may keep outcome tokens in
several accounts and transfer them to anybody. `SupplyLedgerAccount`'s
`external_supply[o]` **is** the mint's counterpart, and that is what the landed
reconciliation checks. This strengthens §3.5's argument for deleting the
shadow: the per-owner half is not merely redundant, it is not checkable.

### 0.4 The seventh error code

`error.rs` gained `ShadowSupplyMismatch = 0x001e`, one past the plan's table.
The plan's `HoardMirrorMismatch` names the *collateral* mirror; this names the
*outcome* mirror, and collapsing them would leave a diagnostic unable to say
which of two different single-truth cutovers broke.

Its refusal has a consequence worth naming rather than discovering: `Burn` is
permissionless for a token's owner, so a holder can destroy outcome tokens
outside this program. The mint's supply falls, the ledger's does not, and every
subsequent seam instruction on that outcome refuses — with nothing in this
program able to repair the ledger. That is a denial-of-service surface created
by carrying two truths, it is measured by
`a_supply_that_drifted_outside_the_program_is_refused`, and it is an argument
for the cutover of open decision 3 rather than against the check.

### 0.5 Measured resource envelope (obligation 10, partially)

Whole-transaction compute units against the real ELF and the real Token-2022
program, at two outcomes, with a fixed actor so the figures are a measurement
and not a sample of `find_program_address` iteration counts:

| instruction | accounts | CU |
| --- | --- | --- |
| `Materialize` with the outcome leg | 13 | 82 146 |
| `Dematerialize` with the outcome leg | 13 | 82 124 |
| `Split` with the collateral leg | 16 | 219 391 |
| `Merge` with the collateral leg | 16 | 219 427 |
| `RedeemInternal` with the collateral leg | 19 | 558 320 |
| `CreateMarket`, founding two mints and the Hoard | 21 | 867 425 |

The probe's 1 230 / 1 720 / 1 235 CU figures were bare instructions with no CPI
frame; these include the program's whole validation, the closure obligations,
the kernel step, the CPI, and account re-serialization. This is two outcomes,
not the maximum outcome count, so obligation 10 stays open.

**Three of these do not fit the 200 000-unit default and the reason is not the
CPI.** The collateral leg binds the Realm's 266 policy bytes by *recomputed
digest*, and `clutch-solana-layout` is a dependency-free crate carrying its own
**software SHA-256** rather than calling the `sol_sha256` syscall. Two digests
per collateral instruction is most of the difference between 82 146 and
219 391; `RedeemInternal` additionally recomputes the multi-kilobyte terms
digest, and `CreateMarket` recomputes it twice on top of seven CPIs. The SVM
scenarios raise the ceiling with a `ComputeBudget` instruction, which measures
the cost rather than hiding it. **Moving those digests onto `sol_sha256` behind
a feature is an obligation on the layout crate**; it is not something this lane
may do to a frozen codec, and until it happens `Split` and `Merge` need a
raised ceiling on any cluster.

### 0.6 What is still not evidenced

E6 (a mint the extension refusal misses whose transfer is not the identity),
E7 (the byte differential extended over token accounts), E8 at the maximum
outcome count, and the market-initialization *refusals* on a bank — market
initialization itself now has a bank scenario, because `svm-tests` builds the
immutable terms artifact it needed (`clutch_svm_fixture::fixture_terms`), but
only the accepting path and the re-initialization refusal are driven there.
The collateral admission refusals are covered on the host instead, over seven
matrix rows plus the digest binding, the cap ceiling, and the `ImmutableOwner`
requirement.

E5 **is** evidenced now, and it had to be: the kernel step and the ledger step
write their accounts before the CPI (§0.3), so a `Split` the kernel admits and
Token-2022 then refuses for insufficient funds has already written two program
accounts when the CPI fails.
`a_cpi_that_fails_after_the_kernel_ran_reverts_every_byte` drives exactly that
and compares all nine program accounts and both token accounts byte for byte
against the pre-transaction state, from a state a previous accepted `Split`
produced rather than from genesis — so the comparison is not vacuous.

## 1. Resolution record

### 1.1 It resolves

`solana-program-test` and the Token-2022 client library both resolve, and the
whole graph builds and runs. This contradicts the ladder recorded in
[`SBF_BRINGUP.md`](SBF_BRINGUP.md) ("**(a) `solana-program-test` in-process bank
— unavailable.** The crate is absent from this host's offline cache in every
version"), and the reason is narrow and worth stating plainly: **that lane was
offline and this one was not.** Nothing about the host's crate cache changed.
`programs/clutch-sbf/.cargo/config.toml` sets `[net] offline = true`, and under
that constraint the finding was correct. With network access authorized for
dependency download, `cargo` fetches what the cache lacks.

The panamax mirror named in the lab notes still does not exist on this host, so
`SBF_BRINGUP.md`'s statement about it stands unaltered.

| package | version | note |
| --- | --- | --- |
| `solana-program-test` | 4.2.1 | requires feature `agave-unstable-api` |
| `solana-program-binaries` | 4.2.1 | transitive; ships the Token-2022 ELF |
| `spl-token-2022-interface` | 3.1.1 | client library: state, TLV, instructions |
| `solana-address` | 2.6.1 | with `curve25519`, for host-side PDA derivation |
| `solana-account` | 4.3.2 | |
| `solana-instruction` | 3.4.1 | |
| `solana-keypair` | 3.1.2 | |
| `solana-signer` | 3.0.1 | |
| `solana-system-interface` | 3.2.0 | with `bincode` |
| `solana-transaction` | 4.1.6 | |
| `solana-transaction-error` | 3.3.2 | |
| `solana-rent` | 4.3.0 | |
| `solana-program-error` | 3.0.1 | |
| `solana-program-option` | 3.1.0 | |
| `solana-program-pack` | 3.1.0 | |

The resolved graph is 731 packages. The exact set is pinned in
`toolchain/probes/token2022/Cargo.lock` and the probe runs `--locked`.

The `curve25519` feature is worth one line, because `SBF_BRINGUP.md` names it as
a blocker: enabling it made `cargo metadata` fail offline, which is why
`seeds::find` is `unimplemented!()` off-chain in `programs/clutch-sbf` and why
that harness shells out to `solana find-program-derived-address`. With network
access it resolves, and the probe derives program addresses in process. This
does **not** license enabling it in `programs/clutch-sbf`: that workspace is
built by `cargo-build-sbf`, which resolves for every platform and still needs an
archive for every package in the graph.

Two things the probe needed and did **not** need:

* **No Token-2022 `.so` had to be sourced, staged, or committed.**
  `solana-program-test` 4.2.1 installs SPL programs at genesis itself
  (`programs::spl_programs(&rent)`), and the ELF comes from the published
  `solana-program-binaries` crate. The Token-2022 program the probe drives is
  `spl_token_2022-10.0.0.so`, sha256
  `a794161408080f690dac00832f45b3c3e2b71f1339586667ad1f979cf91d5b68`, 506 896
  bytes, installed at `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` under the
  upgradeable loader. There is no fixture binary in this repository.
* **The `spl-token-2022` *program* crate is not a dependency and must not
  become one.** The on-chain behaviour comes from the ELF; the client side
  comes from the `-interface` crate. `spl-token-2022` 11.0.0 declares
  `spl-token-2022-interface ^3.0.0`, so 3.1.1 is the matching client library
  for both the 10.0.0 ELF used here and the 11.0.0 ELF available elsewhere.

### 1.2 The host toolchain pin does not reach

[`toolchain/PINNED_PROOF_TOOLS.md`](../../toolchain/PINNED_PROOF_TOOLS.md) records
`HOST_RUST_TOOLCHAIN` as `1.89.0-aarch64-apple-darwin`, and `SBF_BRINGUP.md`
verifies `rustc 1.89.0` on this host. **1.89.0 cannot compile the Agave 4.2.1
runtime.** Verbatim, 18 occurrences of
the same error, from `cargo +1.89.0 test --no-run`:

```text
error[E0658]: use of unstable library feature `maybe_uninit_write_slice`
   --> /Users/ember/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/solana-syscalls-4.2.1/src/sysvar.rs:248:17
    |
248 |             var.write_copy_of_slice(sysvar_slice);
    |                 ^^^^^^^^^^^^^^^^^^^
    |
    = note: see issue #79995 <https://github.com/rust-lang/rust/issues/79995> for more information

error: could not compile `solana-syscalls` (lib) due to 18 previous errors
```

The identical failure occurs on 1.92. `MaybeUninit::write_copy_of_slice`
stabilizes in **1.93**; a direct check compiles it on 1.93.1 and rejects it on
1.89.0 and 1.92. The probe therefore pins 1.93.1 in its own
`rust-toolchain.toml`.

A second gate, unrelated to the compiler: `solana-program-test`'s entire public
API sits behind `#![cfg(feature = "agave-unstable-api")]`. Without the feature
the crate compiles to an empty root and every import fails with "no
`ProgramTest` in the root". Anza's name for the feature is a stability warning
and should be read as one.

**Consequence for the repository.** A program-test harness cannot live in the
same workspace as anything built by the 1.89.0 pin or by `cargo-build-sbf`. The
probe is a standalone workspace for exactly this reason, and the eventual
adapter harness must be too — the same separation `programs/clutch-sbf` already
maintains against `programs/solana-layout` and `programs/solana-reference`.

### 1.3 Fallbacks, recorded but not needed

Neither was used; both are available if the primary harness has to be dropped.

* **`litesvm` 0.15.2** — already unpacked in this host's cache. Far smaller
  dependency graph, and `LiteSVM::new()` loads its own bundled
  `spl_token_2022-11.0.0.so` (sha256
  `495e9d7680dd555cb126a6a8e5464af5be9b01f02f2cd70634352722d22e3cad`, 615 936
  bytes) at the same program id.
* **`solana-test-validator` 4.0.2** — the harness `SBF_BRINGUP.md` actually
  used, present at
  `~/.local/share/solana/install/active_release/bin/solana-test-validator`. It
  costs a process and an RPC round trip per transaction, and it is the only one
  of the three that can commit a ledger, which matters for the replay
  obligations program-test cannot reach.

## 2. What the probe established

Recorded run: [`toolchain/probes/token2022/evidence/probe_run.txt`](../../toolchain/probes/token2022/evidence/probe_run.txt).
Six scenarios, all passing, driven by the in-process Agave bank against the real
Token-2022 ELF.

```text
test a_widened_profile_would_admit_the_fee_mint_and_lose_atoms ... ok
  PROBE widened_profile: admitted=[TransferFeeConfig] deposited=2000000 spendable=1980000 shortfall=20000
test base_mint_is_admitted_as_collateral ... ok
  PROBE base_mint_is_admitted_as_collateral: MintObservation { decimals: 6, supply: 5000000,
    mint_authority: None, freeze_authority: None, extensions: [] }
test mint_close_authority_mint_is_refused ... ok
  PROBE mint_close_authority: refusal=Refusal { code: ExtensionNotAllowed, extension: Some(MintCloseAuthority) }
test outcome_mint_lifecycle_conserves_atoms ... ok
  PROBE outcome_mint_lifecycle: mint_to=1230 CU  transfer_checked=1720 CU  burn=1235 CU
    (bare instruction, no CPI frame)
test pda_owned_hoard_admits_immutable_owner_and_refuses_wallet_withdrawal ... ok
  PROBE pda_owned_hoard: admitted=TokenAccountObservation { amount: 600000, frozen: false,
    delegate: None, close_authority: None, extensions: [ImmutableOwner] }
    wallet_withdrawal_refused_with=Custom(4)
test transfer_fee_mint_is_refused_and_would_break_conservation ... ok
  PROBE transfer_fee: sent=1000000 credited=990000 withheld=10000
    refusal=Refusal { code: ExtensionNotAllowed, extension: Some(TransferFeeConfig) }
    account_refusal=Refusal { code: ExtensionNotAllowed, extension: Some(TransferFeeAmount) }

test result: ok. 6 passed; 0 failed
```

Read out:

1. **Mint, burn, and transfer are reachable and conserve.** Across a mint of
   1 000, a transfer of 400, and a burn of 250, the mint's `supply` equals the
   sum of holder balances at every step. This is the shape obligation 6 assumes
   when it calls materialization a mint and dematerialization a burn.
2. **A base Token-2022 mint is admitted, and the admission is layered.** Before
   the mint authority is revoked the same bytes refuse with
   `MintAuthorityPresent`; after revocation they are admitted. A profile naming
   a different mint refuses `WrongMint`, and one naming the legacy token program
   refuses `WrongProgram`. Decoding is not admission.
3. **The extension refusal is real, and the harm it prevents is measured.** A
   mint carrying `TransferFeeConfig` (matrix row 1) is refused, and the refusal
   names the extension. On the same mint, sending 1 000 000 atoms into a
   Hoard-shaped account credits **990 000** and parks **10 000** in a
   `TransferFeeAmount` sub-balance the Hoard cannot spend and the mint's
   withdraw authority can take. `MintCloseAuthority` (row 3) is refused too, so
   the check is a policy and not a special case for fees.
4. **The refusal is falsifiable.** The last test runs the same bytes and the
   same predicate under a counterfactual profile that wrongly admits
   `TransferFeeConfig`: it is admitted, and a 2 000 000-atom deposit leaves the
   Hoard able to honour 1 980 000. A predicate that cannot accept is not
   evidence that it rejects the right things.
5. **A program-owned Hoard cannot be drained by a wallet signature.** A token
   account whose owner authority is a program address accepts a user-signed
   transfer *in*, admits under the V1 Hoard policy with `ImmutableOwner`
   present, and refuses a user-signed transfer *out* with
   `TokenError::OwnerMismatch`, `Custom(4)`. Redemption has to be an
   `invoke_signed` CPI; there is no other shape.
6. **Compute, for scale only.** `MintTo` 1 230 CU, `TransferChecked` 1 720 CU,
   `Burn` 1 235 CU, measured as bare top-level instructions. These do **not**
   include the CPI frame, account re-serialization, or the adapter's own work,
   and they are one fixture each. `SBF_BRINGUP.md` measured `Split` at 72 869 CU
   of a 200 000 default, so the token leg is not obviously the binding
   constraint — but it is unmeasured until it is measured under CPI.

What the probe does **not** establish: there is no Dragon's Clutch program in
it, no CPI, no PDA signature, no kernel, no clutch instruction, and no claim
about the adapter's correctness. An in-process bank is not a cluster.

### 2.1 Where the predicate lives, and why it is not duplication

`toolchain/probes/token2022/src/lib.rs` implements the admission predicate over
real account bytes, with `RefusalCode` numbered identically to `RefusalCode` in
`research/collateral-profiles/model.py`. The two are deliberately the same
decision procedure over different inputs: the Python model decides over a
hand-built snapshot, the probe decides over bytes the token program wrote. The
ordering matches too — authority and supply faults are reported before extension
faults, which is why the probe has to revoke the mint authority before the
extension refusal is the one that fires.

This is a probe, not the adapter's implementation. The adapter's copy has to be
written in the program, `no_std`, inside a 4 KiB SBF frame, against
`AccountInfo` rather than `Vec<u8>` — see §3.6. What carries over is the
predicate and its refusal numbering, not the code.

## 3. Integration plan

### 3.1 Two mint roles, which the current documents do not separate

Obligations 5-7 are about two different mints and the plan is unreadable until
they are named apart.

| role | who creates it | mint authority | freeze authority | decimals | extensions | admitted by |
| --- | --- | --- | --- | --- | --- | --- |
| **collateral mint** | nobody here; it pre-exists | must be absent | must be absent | profile-fixed | none | the Realm collateral profile |
| **outcome mint** (one per outcome) | the program, at market init | the market authority PDA | **must be `None`** | proposed `0` | none | by construction |

The collateral mint is the asset the Hoard holds and is subject to the whole
`COLLATERAL_PROFILES.md` matrix. The outcome mints are the program's own
liability tokens; they are Token-2022 base mints created by the program, and
their admission is by construction rather than by inspection.

Two consequences worth arguing about now rather than later:

* **Outcome mints must have no freeze authority.** A freeze authority on a
  claim token is discretionary seizure, which obligation 12 forbids
  introducing. `None` at creation is the only setting that cannot be abused
  later.
* **Outcome-mint decimals should be `0`.** Kernel quantities are integer
  complete-set counts. Any nonzero decimals introduces a UI-versus-atom
  distinction with no semantic content, which is exactly the unit confusion
  `COLLATERAL_PROFILES.md` refuses `ScaledUiAmount` over. This is a proposal,
  not a frozen decision; it changes nothing in the kernel either way, because
  the kernel already speaks in atoms.

### 3.2 Which instruction families gain CPIs

| instruction | token effect | CPI | authority | signer shape |
| --- | --- | --- | --- | --- |
| `CreateMarket` | create + initialize outcome mints; create + initialize the Hoard token account | System `CreateAccount`, `InitializeMint2`, `InitializeImmutableOwner`, `InitializeAccount3` | market authority PDA | `invoke_signed` |
| `Split` | collateral in: user token account → Hoard | `TransferChecked` | the user | `invoke`, signer propagates from the outer transaction |
| `Merge` | collateral out: Hoard → user token account | `TransferChecked` | Hoard authority PDA | `invoke_signed` |
| `Materialize` | outcome tokens out: mint to the destination token account | `MintTo` | market authority PDA | `invoke_signed` |
| `Dematerialize` | outcome tokens in: burn from the source token account | `Burn` | the user | `invoke`, signer propagates |
| `RedeemInternal` | collateral out: Hoard → user token account | `TransferChecked` | Hoard authority PDA | `invoke_signed` |
| `Resolve` | none | — | — | — |

Mapped onto the module layout `SBF_BRINGUP.md` now records, so no two lanes
edit one file:

| module | families | token work |
| --- | --- | --- |
| `instructions/split.rs` | `Split` | add the collateral-in `TransferChecked` and the delta check |
| `instructions/merge_materialize.rs` | `Merge`, `Materialize`, `Dematerialize` | the whole of obligations 5-6 plus collateral-out; this is the lane |
| `instructions/market_init.rs` | `CreateMarket` | create the outcome mints and the Hoard token account; run mint admission for the first time |
| `instructions/observe_resolve.rs` | `Resolve`, `RedeemInternal` | collateral-out on redemption only; `Resolve` stays token-free |
| `accounts.rs` | shared | the token-account and mint readers, appended |
| `seeds.rs` | shared | the missing token PDAs, appended — see below |
| `error.rs` | shared | the token refusal codes, appended — see below |

**`seeds.rs` is missing every token address.** It carries seeds for the Realm,
Profile, Market, Hoard, Position, kernel, external shadow, replay, supply,
feed, terms, grid, resolution, epoch, page, candidate, pot and receipt
accounts — and no outcome mint, no Hoard token account, and no Hoard authority.
Obligation 1 names "outcome-mint, external-token ... PDA" explicitly. Three
seed functions have to be appended before any of this is writable:

```text
outcome mint      "dragons-clutch:outcome-mint:v1", market_id, outcome_index (u8)
hoard authority   "dragons-clutch:hoard-authority:v1", market_id
hoard token       "dragons-clutch:hoard-token:v1", market_id
```

> **Landed with a correction, see §0.3.** The middle prefix is 33 bytes and
> cannot be a seed; the implementation uses
> `"dragons-clutch:hoard-auth:v1"`.

Proposed, not frozen, exactly as the rest of that table is. Note that the Hoard
*authority* and the Hoard *token account* are two different addresses: the
authority is the PDA the program signs as, the token account is the Token-2022
account that authority owns. Collapsing them is possible — a token account can
be owned by itself — and is a bad idea, because it makes the signing seeds and
the account seeds the same thing.

**`error.rs` is append-only and currently ends at `NotYetImplemented = 0x0017`.**
The token leg needs codes and must not renumber anything:

```text
0x0018  WrongTokenProgram          account owner is not the pinned token program
0x0019  MintNotAdmitted            collateral mint fails the Realm profile
0x001a  TokenExtensionNotAllowed   a present extension is outside the allowed set
0x001b  TokenAccountNotAdmitted    token account fails the Hoard/holder policy
0x001c  TokenDeltaMismatch         observed balance or supply delta != expected
0x001d  HoardMirrorMismatch        HoardAccount.collateral_atoms != token amount
```

`TokenExtensionNotAllowed` is deliberately separate from `MintNotAdmitted` so
that the on-chain refusal is as specific as the probe's, which names the
offending extension.

The asymmetry is the point and the probe measured it: moving value *in* needs
only the user's signature, which the runtime already propagates into a CPI, so
no delegate and no approval step is required. Moving value *out* — `Merge`,
`RedeemInternal`, and every payout — is impossible without the program signing
for the Hoard authority seeds. `Custom(4)` is what a wallet gets for trying.

`Materialize` and `Dematerialize` carry a 32-byte `destination` / `source`
field in the frozen `Intent` today (`programs/solana-layout/src/lib.rs`). Those
become token-account addresses. They are **named, not derived**, so they are
caller-supplied bindings and must be validated against the account list, the
mint, and the owner — never trusted, which is obligation 2 applied to a field
that already exists.

### 3.3 Atomicity ordering (obligation 4)

One frozen order, per instruction, no exceptions:

1. **Validate.** Account count, signer, writability, program ownership,
   executable bit, pairwise distinctness, data lengths, PDA derivation and
   canonical bumps, decode, linkage — everything `SBF_BRINGUP.md` already does,
   extended over the token accounts and mints.
2. **Admit.** Run the collateral predicate over the *mint account bytes as
   loaded in this transaction* and over the Hoard token account. §3.4 argues
   why this cannot be skipped on the grounds that it was done at init.
3. **Snapshot.** Read the exact pre-CPI `amount` of every token account this
   instruction will change, and the pre-CPI `supply` of every mint it will
   change, from the account data.
4. **Compute.** Run the kernel transition on the stack and its invariant check.
   No account bytes are written.
5. **Token effects.** Drop every outstanding `RefCell` borrow, then perform the
   CPIs. A live borrow across `invoke` is a runtime failure, not a lint.
6. **Verify the deltas.** Re-read every touched token account and mint and
   require the *exact* observed delta: `post.amount - pre.amount == quantity`
   on the credited side, `pre.amount - post.amount == quantity` on the debited
   side, and `post.supply - pre.supply == quantity` for a mint or burn. Not
   `>=`, not "at least". §3.5.
7. **Commit.** Encode program state and write it back.

Steps 3 and 6 are the ones that are easy to omit and expensive to omit. Steps
1-2 before any write, and step 7 last, is what makes SVM rollback sufficient:
a failure anywhere discards everything, including the CPI's effects on accounts
this program does not own.

That last sentence is **inherited, not demonstrated**. `SBF_BRINGUP.md`
deferred item 8 says so already ("Transaction-atomic ordering is asserted, not
demonstrated ... No test forces a failure after the first write"). Adding a CPI
makes it materially harder to hand-wave, because now a partial success would
move somebody's tokens. See §4, evidence item E5.

### 3.4 The extension-refusal enforcement point

**Two points, both required.**

*At market initialization*, the Realm's frozen collateral profile is checked
against the actual collateral mint: token program, mint identity, decimals,
supply ceiling, absent mint authority, absent freeze authority, and an extension
set that is a subset of the effective allowed set. The market records the
collateral mint identity in immutable state. A market whose collateral mint is
not admitted cannot exist.

*At every instruction that performs a token CPI*, the check runs again over the
mint account as loaded in that transaction. This is not defensive
belt-and-braces; it closes a specific hole the matrix itself names.
`MintCloseAuthority` is refused precisely because "a zero-supply mint can be
closed and reinitialized with different extensions" — a mint address is not a
stable description of a mint's behaviour, and an address recorded at
initialization does not bind the extension set forever. The mint account is
already in the account list for `TransferChecked`, so re-reading it costs a
decode of 82 bytes plus whatever TLV is present.

The refusal must fail closed on an unknown discriminant, which comes free from
the decoder: `get_extension_types()` returns `InvalidAccountData` for a
discriminant the build does not know, and the probe maps that to a refusal
rather than to an empty set. A future Token-2022 release adding extension 29
must make this program refuse, not shrug.

Two proposed changes to `COLLATERAL_PROFILES.md`, both surfaced by the probe
and neither adopted here:

* **Require `ImmutableOwner` on the Hoard, rather than allowing it.** V1 allows
  it and requires nothing. But the Hoard's whole security story is that its
  owner authority is a program address; `SetAuthority(AccountOwner)` is exactly
  the instruction that would break that, and `ImmutableOwner` is exactly the
  extension that forbids it. The probe creates the Hoard with it set. Making it
  required costs a `required_account_extensions` bit and forbids nothing a
  Realm would want.
* **State whether a fee-currency mint different from collateral gets the same
  matrix.** `COLLATERAL_PROFILES.md` names three currency identities and says a
  separately tokenized fee asset needs its own admission policy and a later
  schema. Whatever the eventual answer, the adapter must refuse a fee currency
  that is neither collateral nor native SOL, which is what V1 already says.

### 3.5 Conservation, and the two balance truths that must die

Obligation 6 says the external shadow "must not survive as a second balance
truth". The same sentence applies to a field that already exists on chain.

* `ExternalAccount` (reference-only) is replaced by the outcome mint's own
  `supply` plus the holders' token balances. It is deleted, not reconciled.
  The single-position closure `internal + external == total_supply` becomes
  `position.internal[o] + outcome_mint[o].supply == kernel.total_supply[o]` —
  which, note, finally makes the aggregate side of obligation 11 tractable,
  because the mint's `supply` field *is* the checked aggregate over all holders
  that a scan cannot provide. This is the single largest structural argument in
  favour of real tokens over the shadow, and it should be tested first.
* `HoardAccount.collateral_atoms` is a second truth about a balance the token
  program also tracks. Either it is checked equal to the Hoard token account's
  `amount` after every transition, or it is removed and the token account's
  `amount` is read directly. This plan proposes **checked equality**, because
  the field is in a frozen layout, and because the equality is itself the
  strongest available statement that the CPI did what the kernel thought it
  did.

The exact-delta rule of §3.3 step 6 is what makes both of these safe against a
collateral mint whose transfer is not the identity. Extension refusal already
rejects the fee mints; the delta check means solvency does not *depend* on the
refusal being complete. Defence in depth is cheap here: two account re-reads.

`ScaledUiAmount`, `InterestBearingConfig` and `amount_to_ui_amount` are all
refused, so the adapter never converts. Everything is atoms. The probe asserts
only atoms.

### 3.6 Account shapes

**Hoard token account.** Program-created at market init, owner authority = a
Hoard authority PDA, `ImmutableOwner` set, no delegate, no close authority. Not
an ATA: an ATA of a PDA is derivable but buys nothing here, since the Hoard's
address is already a market-derived PDA in the seed schema and the adapter must
check the token account by derivation anyway. Its rent comes from the liveness
currency, never from collateral principal.

**Outcome mints.** Program-created at market init, one per outcome, at PDAs
seeded by market id and outcome index. Mint authority = market authority PDA,
freeze authority `None`.

**User collateral account.** Any Token-2022 account for the collateral mint
whose owner authority is the authenticated signer. **Not** required to be an
ATA. Requiring an ATA is a convenience that would make the program refuse
legitimate accounts while adding no security property the mint-and-owner check
does not already give. The address is caller-supplied and therefore validated,
never trusted.

**User outcome-token account.** The `destination` / `source` field of the
`Materialize` / `Dematerialize` intents. Same rule: validated against mint and
owner authority, not derived. If a later ABI wants determinism it can require
an ATA, but that is an ABI decision, not a safety one.

**Account count.** `Split` today is nine accounts. Adding the collateral leg
adds the token program, the collateral mint, the user's token account, and the
Hoard token account: thirteen. `Materialize` adds the token program, the
outcome mint, and the destination: twelve or thirteen depending on whether the
collateral accounts are present. This is well inside the transaction account
limit but it is not free, and obligation 10 wants it measured, not estimated.

> **Landed, with two additions the estimate missed, see §0.2.** `Split` is
> **sixteen**, not thirteen: the collateral leg also needs the Hoard's *signing
> authority* (an `invoke_signed` must be handed the account whose seeds sign)
> and the Realm's **266 collateral-policy bytes**. The second is the one worth
> naming: the collateral mint's *identity* lives only in the Realm's frozen
> policy, so without those bytes in the account list the asset a `Split` funds
> a complete set with would be caller-supplied. `RedeemInternal` needs the
> Profile too, for the same reason and because its plane had no Profile to bind
> the policy against. The measured counts are in §0.2 and the measured units in
> §0.5.

**Frame size.** `SBF_BRINGUP.md`'s stack finding is the live constraint here.
Two functions in `clutch-solana-reference` already overflow the 4 KiB SBF frame
by passing whole states by value, and `clutch-sbf`'s processor only fits because
it splits decoded values into `#[inline(never)]` helpers. The admission
predicate must be written the same way: decode the mint into a small
observation, return the observation, drop the borrow. The probe's `Vec<u8>` and
`Vec<ExtensionType>` shapes do not survive the port and are not meant to.

**Reentrancy.** Token-2022 does not call back into the invoking program — with
one exception, and it is refused: `TransferHook` "invokes a configured external
program" during transfer. That is the matrix row that is also an obligation 3
row, and it should be cited as such in both places.

## 4. Evidence that will be required

None of this exists. Each item is a program-test scenario the adapter must pass
before its token leg is described as anything but a probe.

* **E1 — one-to-one materialization.** `Materialize` of *q* on outcome *o*
  increases `outcome_mint[o].supply` by exactly *q*, increases the destination
  token account by exactly *q*, and decreases `position.internal[o]` by exactly
  *q*. `Dematerialize` reverses it exactly. Obligation 6.
* **E2 — collateral conservation.** `Split` moves exactly *q · cap* atoms from
  the user's token account into the Hoard token account, changes no other token
  account, and leaves `hoard_token.amount == HoardAccount.collateral_atoms`.
  `Merge` and `RedeemInternal` reverse it. Obligation 7.
* **E3 — principal is never a source.** Across every instruction, the Hoard
  token account's balance never falls except by a `Merge` or a redemption whose
  amount the kernel computed. No fee, bounty, rent payment, or reserve ever
  debits it. Asserted as a per-transaction invariant, not spot-checked.
* **E4 — extension refusals, on chain.** For each refused matrix row that can be
  constructed on a live mint, market initialization refuses it, and refuses with
  a stable custom error naming the extension. At minimum
  `TransferFeeConfig`, `MintCloseAuthority`, `DefaultAccountState`,
  `PermanentDelegate`, `TransferHook`, `NonTransferable`, and `Pausable`. Plus:
  a mint that is admitted at init and then presented after a close-and-reinit
  with a different extension set must be refused at instruction time, which is
  the check §3.4 exists for.
* **E5 — post-CPI rollback.** A transaction whose token CPI succeeds and whose
  *subsequent* program step fails must leave every token account and every
  program account byte-identical to the pre-transaction state. This is the test
  `SBF_BRINGUP.md` deferred item 8 has been owed since before there was a CPI to
  fail after; a deliberate fault-injection instruction variant is the honest way
  to get it.

  > **Landed, and no fault injection was needed.** Because the kernel step and
  > the ledger step write before the CPI (§0.3), the *ordinary* failure is
  > already the shape the obligation wants: a `Split` whose quantity the kernel
  > admits and whose collateral the actor's token account cannot cover writes
  > two program accounts and then fails inside `TransferChecked`.
  > `a_cpi_that_fails_after_the_kernel_ran_reverts_every_byte` runs one accepted
  > `Split` first, so the comparison is against a state this program produced,
  > and then compares all nine program accounts and both token accounts byte
  > for byte after the failure. A deliberate fault-injection variant would have
  > been a second code path to trust; this is the one that ships.
* **E6 — the exact-delta check is load-bearing.** Present a collateral mint that
  the extension refusal does *not* catch but whose transfer is not the identity,
  and require the delta check to refuse. If no such mint can be constructed
  against the current matrix, record that, and keep the delta check anyway.
* **E7 — differential against the offline adapter.** Extend the existing
  six-account byte differential to cover the token accounts and mints, so the
  offline reference and the SVM disagree loudly rather than quietly. Obligation
  13.
* **E8 — resource envelope.** Compute units, account count, and transaction size
  for every instruction at the maximum outcome count, with CPIs, and the SBF
  frame check. The probe's 1 230 / 1 720 / 1 235 CU figures are bare-instruction
  scale, not an envelope. Obligation 10.
* **E9 — falsifiability.** Every one of the above must be shown to go red under
  a deliberate mutation, the way `SBF_BRINGUP.md` mutates one expected byte and
  the way the probe's sixth test admits a mint V1 forbids.

Evidence program-test **cannot** give, and which needs `solana-test-validator`
or a cluster: transaction replay, durable nonces, instruction duplication
within one transaction, batch retries, fee payment, rent collection over time,
and program upgrade. Those stay under obligations 3, 9, and 12 and are not
closed by anything here.

## 5. Open decisions

1. **Toolchain.** Adopting `solana-program-test` means the repository carries a
   second Rust pin (≥1.93) and a 731-package Agave graph in one isolated
   workspace, or it means using `litesvm`, or it means staying on
   `solana-test-validator`. This plan assumes the isolated workspace. It is not
   a decision this lane gets to make.
2. **`agave-unstable-api`.** The harness is behind a feature Anza labels
   unstable. That is acceptable for a test harness and would not be acceptable
   for anything that ships.
3. **`HoardAccount.collateral_atoms`.** Checked mirror (proposed) or removal.
   **Taken in the "checked mirror" direction**, and checked twice per
   collateral instruction: over the pre-state, so a market whose two truths
   already disagree refuses before anything moves, and after the CPI, which is
   the load-bearing one. The cutover to removal is now a deletion rather than a
   change of semantics, which is the whole point of checking it. Still open as
   a *decision*: `SBF_BRINGUP.md`'s frozen layout is why the field survives.
4. **`ImmutableOwner` required rather than allowed** on the Hoard. **Taken in
   the "required" direction**, at the one place it can be taken: `CreateMarket`
   creates the Hoard token account with the extension, and refuses a Realm
   whose policy does not admit it (`ClutchError::TokenAccountNotAdmitted`)
   rather than founding a Hoard whose owner authority could later be moved by
   `SetAuthority(AccountOwner)`. `COLLATERAL_PROFILES.md` is unchanged and V1
   still merely *allows* the extension; this adapter is stricter than the
   matrix, which is a divergence and is named here as one.
5. **Outcome-mint decimals `0`** and freeze authority `None`. Implemented as
   proposed and enforced by `token::MintPolicy::outcome`; still a proposal, and
   changing it is now a one-line change in one place plus new mint addresses.
6. **ATA or not** for user token accounts. Proposed: not, and validate instead.
7. **Which Token-2022 ELF is the pinned target.** The probe drove 10.0.0
   because that is what `solana-program-binaries` 4.2.1 ships; `litesvm` ships
   11.0.0; a cluster runs whatever it runs. Obligation 5 says "select and pin
   the exact SPL Token or Token-2022 program"; a program id is not a pin, and
   this is unresolved.

## 6. Reproducing

The probe, which is unchanged:

```sh
toolchain/probes/token2022/run_probe.sh
cargo clippy --manifest-path toolchain/probes/token2022/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path toolchain/probes/token2022/Cargo.toml -- --check
```

The implementation and its SVM evidence:

```sh
cargo test  --manifest-path programs/clutch-sbf/Cargo.toml -p clutch-sbf
cargo clippy --manifest-path programs/clutch-sbf/Cargo.toml -p clutch-sbf --all-targets -- -D warnings
programs/clutch-sbf/svm-tests/run_svm_tests.sh
```

`run_svm_tests.sh` builds the program ELF with `cargo-build-sbf` and drives it
on an in-process Agave bank; the recorded run is
[`programs/clutch-sbf/svm-tests/evidence/svm_run.txt`](../../programs/clutch-sbf/svm-tests/evidence/svm_run.txt).
That workspace carries its own 1.93.1 pin for the reason §1.2 gives, and it is
the plan's open decision 1 taken in the "isolated workspace" direction.

The probe needs network on a first build to fetch its dependency graph; after
that it is offline. It binds no port, starts no validator, holds no key, and
contacts nothing.

## 7. Correct description

Of the probe, unchanged: "A probe showing that a Token-2022 mint and token
account can be created and driven by an in-process Agave bank on this host,
that mint/burn/transfer conserve atoms across one fixture each, and that the V1
collateral matrix refuses a `TransferFeeConfig` mint whose transfer demonstrably
credits fewer atoms than were sent."

Of what this document now also describes: "A Token-2022 adapter in which
`CreateMarket` creates one outcome mint per outcome and the Hoard's token
account by CPI — `system_instruction::create_account` signed with each
account's own program-derived seeds, then `InitializeMint2` /
`InitializeImmutableOwner` / `InitializeAccount3`, then the same admission
policy every later instruction applies, run over the bytes the token program
just wrote; in which `Materialize` and `Dematerialize` perform a real `MintTo`
and `Burn`, and `Split`, `Merge` and `RedeemInternal` a real `TransferChecked`
— the outflows signed for with a program-derived address; in which every one of
them verifies the exact post-CPI supply and balance deltas, requires the
market-wide external term to equal the mint's supply, and requires
`HoardAccount::collateral_atoms` to equal the Hoard token account's `amount`;
with the collateral extension matrix enforced both at market initialization and
again at every token instruction. The token legs are **mandatory**: each intent
accepts exactly one account count and any other is refused. Demonstrated by an
in-process Agave bank executing the real program ELF against the real
Token-2022 program, over fifteen scenarios — market founding, the full
split/materialize/external-transfer/dematerialize/merge cycle, redemption,
the wallet-outflow refusal, and post-CPI rollback compared byte for byte." It
is not verified, not audited, not tested on a cluster, and not authorization to
deploy anywhere.
