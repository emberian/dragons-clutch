# Chain-state Sources: third-party on-chain entities as resolution providers — 2026-08

Status: research dossier plus a labeled design proposal. It is not an
architecture decision, release evidence, deployment evidence, or an accepted
closure of any `docs/OMISSION_INDEX.md` row. Sections 1–5 are ground truth
gathered from published program source, official documentation, and published
IDLs. Section 6 onward is a proposal and is marked as such.

Method and boundary: no public RPC read was performed. Every on-chain claim
below is derived from a published artifact — a program source file, an official
documentation file, or a published Anchor IDL — and nothing here is a claim
about live mainnet account contents at any slot. Anything requiring a chain
read to settle is listed in section 9 as unverified.

Evidence labels used on every factual claim:

- **verified-from-source-code** — read in the published program source.
- **verified-from-docs** — read in first-party documentation or a first-party
  published IDL.
- **derived** — exact arithmetic on a verified figure; the derivation is shown.
- **reported-secondhand** — third-party reporting only; not first-party.
- **unverified** — could not be settled within this task's boundary.

Bound labels follow `AGENTS.md`: mathematical, chain-derived, measured-profile,
or provisional.

---

## 0. Why this dossier exists

`ARCHITECTURE.md` §Resolution states the standing division: "A source adapter
authenticates provider release, feed identity, units, confidence, staleness,
schedule, and observation bytes. A Product resolution policy determines the
window, statistic, edge rules, repair path, and terminal failure result.
Provider transport is not Product truth."

Every implemented adapter today is Pyth-shaped: an off-chain publisher signs a
message, the message is verified on-chain, and dClutch reads a normalized
integer out of a verified structure. A **chain-state Source** inverts the
transport question. There is no publisher and no signature. The datum is
already on-chain, written by a third-party program as a side effect of ordinary
trading, and the adapter's whole job is to decode the right bytes of the right
accounts owned by the right program at the right slot.

The repository currently contains no precedent for this. A survey of the tree
found zero occurrences of `pump.fun`, `bonding curve`, `Raydium`, `Meteora`,
`Orca`, `Jupiter`, or `Switchboard`; `AMM` appears once, in
`crates/dclutch-dealer-contract/DESIGN.md:309`, as an explicit disclaimer
("It is not an AMM and does not claim durable or adaptive liquidity"); and
`TWAP` appears once, in `crates/dclutch-source-contract/DESIGN.md:111`
("A Pyth TWAP is not part of this release; it requires a separately pinned and
measured provider adapter"). This is a new adapter family, not an extension of
an existing one.

Three `OMISSION_INDEX` rows govern the shape of any answer:

- **O-007** (hard invariant): "Mocks are test-only; release state plus
  provider-authenticated evidence owns truth, while clients may submit untrusted
  witnesses."
- **O-018** (hard invariant, narrowly stated): "Adjacency alone is not
  authority."
- **U-009** (unfinished optional breadth): "One release-bound adapter at a time
  with real ABI/crypto and recovery evidence; no mock fallback."

---

## 1. pump.fun

### 1.1 Program identity

The Pump program is deployed at `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P`
on both mainnet and devnet. (verified-from-docs — `PUMP_PROGRAM_README.md`,
and the `address` field of the published IDL `idl/pump.json`.)

The sole `Global` configuration account is
`4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf`, PDA-derived from seeds
`["global"]`. (verified-from-docs.)

Each coin's `BondingCurve` account is PDA-derived from
`["bonding-curve", mint]`, one per mint. (verified-from-docs.)

The successor venue, PumpSwap (`pump_amm`), is deployed at
`pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA` on both mainnet and devnet, with
a sole `GlobalConfig` at `ADyA8hdefvWN2dbGGWFotbzWxrAvLW83WG6QCVXvJKqw`
(seeds `["global_config"]`). (verified-from-docs — `PUMP_SWAP_README.md`, and
the `address` field of `idl/pump_amm.json`.)

The `admin` published for PumpSwap's `GlobalConfig` and the `authority`
published for Pump's `Global` are the same pubkey,
`FFWtrEQ4B4PKQoVuHYzZq8FabGkVatYzDpEVHsK5rrhF`. (verified-from-docs — the two
README account dumps. Whether that pubkey is currently the value on-chain is
unverified.)

### 1.2 Source availability

**The Pump and PumpSwap program source is not published.** The `pump-fun`
GitHub organization's public repositories are `pump-public-docs`,
`transfer-hook-authority`, `react-native-pager-view`, `pump-segments-sdk`,
`pump-fun-skills`, and `carbon`; none contains the Pump or PumpSwap program.
(verified-from-docs — GitHub organization repository listing.) What *is*
published is the Anchor IDL for `pump`, `pump_amm`, and `pump_fees`, plus prose
documentation and per-instruction account tables.

Consequence for dClutch: an ELF digest observed for
`6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P` can be bound as an identity, but
it cannot be tied to reviewable source. Every guard condition inside the program
is a black box. This is a first-order argument in section 7.

### 1.3 Bonding-curve account layout — and the fact that it grew in place

Current `BondingCurve` layout, from the published IDL
(verified-from-docs), discriminator `[23, 183, 248, 55, 96, 216, 172, 96]`:

| Offset | Field | Type |
| --- | --- | --- |
| 0 | (Anchor discriminator) | `[u8; 8]` |
| 8 | `virtual_token_reserves` | `u64` |
| 16 | `virtual_quote_reserves` | `u64` |
| 24 | `real_token_reserves` | `u64` |
| 32 | `real_quote_reserves` | `u64` |
| 40 | `token_total_supply` | `u64` |
| 48 | `complete` | `bool` |
| 49 | `creator` | `pubkey` |
| 81 | `is_mayhem_mode` | `bool` |
| 82 | `is_cashback_coin` | `bool` |
| 83 | `quote_mint` | `pubkey` |
| 115 | (end) | |

`PUMP_PROGRAM_README.md`, still on `main` at the time of writing, documents a
**different, shorter** layout with six fields, in which offset 16 is named
`virtual_sol_reserves` and the account ends at offset 49.
(verified-from-docs — both artifacts are first-party and they disagree.)

The mechanism that produced the disagreement is documented: `extend_account`
"allows anyone to extend the data size of any program-owned account (`Global` or
`BondingCurve`) in order to allow adding new fields to the existing account
types." (verified-from-docs.) The `buy_v2` account table states of
`bonding_curve`: "Rent top-up to make the account data length 115 bytes if it is
smaller." (verified-from-docs.)

Three consequences, each load-bearing for a hostile decoder:

1. **Live accounts of several different lengths coexist.** 8 + 5·8 + 1 = 49,
   +32 = 81, +1+1+32 = 115. (derived — field-width arithmetic on the IDL layout;
   the 115 figure is independently confirmed by `BUY.md`.) A decoder must name
   the admitted length set and the meaning of a short account, not assume one
   width. This bound is chain-derived, not provisional: it comes from the
   venue's own documented growth mechanism.
2. **The Anchor discriminator does not change when the account grows.** The
   same eight bytes identify all three widths. Discriminator equality is
   therefore *not* a layout-version check.
3. **A field's meaning is overloaded by a sentinel.** `BUY.md` states: "For
   SOL-paired coins, `bonding_curve.quote_mint` is `Pubkey::default()` and
   `quote_mint` should be wrapped SOL." (verified-from-docs.) The all-zero
   pubkey is not "unset"; it means wrapped SOL. A decoder that treats zero as
   absent decodes the wrong unit.

The price-determining state is the pair
(`virtual_token_reserves`, `virtual_quote_reserves`), maintained under a
constant-product invariant: the README states "the relationship
`virtual_sol_reserves * virtual_token_reserves = k`", with `real_*` reserves
tracking the same deltas. (verified-from-docs.)

### 1.4 The graduation event

`complete` "is initially set to `false`. It is set to `true` at the end of a
`buy` instruction, when `real_token_reserves == 0`, so there are no more real
tokens left in the bonding curve." (verified-from-docs.)

The corresponding event is `CompleteEvent { user, mint, bonding_curve,
timestamp, quote_mint }`. (verified-from-docs — IDL event definition.)

`migrate(user, mint)` "allows any `user` to migrate the liquidity of a completed
bonding curve of the given `mint` to PumpSwap AMM… The `migrate` instruction is
idempotent… It is also permisionless, so anyone can migrate a completed bonding
curve." (verified-from-docs, quoted verbatim including the source's spelling.)
The IDL confirms the account list pins `pump_amm` to
`pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA` by exact address constraint, and
`migrate_v2` exists alongside it for non-SOL quote mints.
(verified-from-docs.) Its event is
`CompletePumpAmmMigrationEvent { user, mint, mint_amount, sol_amount,
pool_migration_fee, bonding_curve, timestamp, pool, quote_mint }`.

The graduation destination has changed. The IDL and README both retain
`withdraw(withdraw_authority, mint)`, described as "a now-disabled instruction
which allowed the `withdraw_authority` pubkey to withdraw the liquidity of a
completed bonding curve and migrate it to Raydium from an off-chain server."
(verified-from-docs.) `Global.enable_migrate` gates which of the two paths is
live. PumpSwap is reported to have launched 2025-03-20, after which graduations
went to PumpSwap rather than Raydium. (reported-secondhand — The Block and
others; the *fact* of the Raydium-to-PumpSwap change is verified-from-docs via
the `withdraw` description, only the date is secondhand.)

**Graduation threshold.** Using the `Global` values published in the README —
`initial_virtual_token_reserves = 1_073_000_000_000_000`,
`initial_virtual_sol_reserves = 30_000_000_000`,
`initial_real_token_reserves = 793_100_000_000_000` — the curve completes when
all 793.1 M real tokens are bought, leaving
`virtual_token = 1_073e12 − 793.1e12 = 279.9e12` and
`virtual_sol = k / 279.9e12 = 115_005_359_056` lamports, so
`real_sol = 85_005_359_056` lamports ≈ **85.005 SOL**. (derived — exact integer
arithmetic on the published constant product; bound is chain-derived *for those
parameter values only*. `set_params` can change them, and the current IDL adds
`initial_virtual_quote_reserves` and per-quote-mint parameters that the README
predates.)

### 1.5 The price-determining state is writable by non-trade instructions

The published IDL contains instructions that write bonding-curve or
global pricing parameters outside the trade path:

- `set_params(...)` — writes `Global`; signer `authority`. Eleven arguments
  including `initial_virtual_token_reserves`, `initial_virtual_sol_reserves`,
  `initial_real_token_reserves`. (verified-from-docs.)
- `set_virtual_quote_reserves(initial_virtual_quote_reserves: u64)` — writes
  `Global`; signer `authority`. (verified-from-docs.)
- `toggle_mayhem_mode(enabled: bool)` — writes `Global`; signer `authority`.
  (verified-from-docs.)
- `set_mayhem_virtual_params()` — writes `bonding_curve`. **Its IDL account
  list names no external signer**: the only signer entry is
  `sol_vault_authority`, marked as a PDA. (verified-from-docs — IDL account
  metadata.) Whether an unprivileged caller can land it, and under what guard
  conditions, is **unverified**: the program source is not published.

The design consequence stands regardless of that last unknown: on this venue,
the price-determining bytes are not a pure function of trade flow. A window
statistic computed over them inherits whatever those instructions can do.

### 1.6 Upgradeability

Whether `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P` is currently deployed
under an upgradeable loader, and who holds the upgrade authority, is
**unverified** — settling it requires reading the program's ProgramData account.

That the program *has been* upgraded repeatedly is verified by proxy. The
published IDL file `idl/pump.json` has 15 commits between 2025-04-04 and
2026-05-18, including "Idl and type files for mayhem mode and create v2",
"USDC paired coins", "feat: cashback update", and "Immutable global volume
accumulator and reserved fee recipients". (verified-from-docs — commit history
of the first-party docs repository.) The account layout growth in §1.3 is itself
only possible under program upgrade. Separately, coverage of the May 2024
flash-loan incident states the team "upgraded the contracts so the attacker
cannot siphon any more funds." (reported-secondhand.)

Treat as established: **this program changes, and its account layouts change
with it, on a cadence of weeks to months.**

---

## 2. AMM price-bearing state

The recurring structural question for every venue: *which account holds the
price, and does the venue maintain an on-chain time-weighted structure over it?*

### 2.1 Summary table

| Venue | Program | Price-bearing state | On-chain price accumulator | Source published |
| --- | --- | --- | --- | --- |
| pump.fun bonding curve | `6EF8rre…wF6P` | `BondingCurve.virtual_token_reserves`, `.virtual_quote_reserves` | **none** | no |
| PumpSwap | `pAMMBay6…fXEA` | `pool_base_token_account` + `pool_quote_token_account` balances; `Pool.virtual_quote_reserves` (`i128`) | **none** | no |
| Raydium CLMM | `CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK` | `PoolState.sqrt_price_x64` (Q64.64), `.tick_current` | **yes** — `ObservationState`, 100 slots | yes |
| Raydium CPMM | `CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C` | `token_0_vault` + `token_1_vault` balances, minus `PoolState` fee accruals | **yes** — `ObservationState`, 100 slots | yes |
| Orca Whirlpools | `whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc` | `Whirlpool.sqrt_price` (Q64.64), `.tick_current_index` | **no** — the `Oracle` account is adaptive-fee only | yes |
| Meteora DLMM | `LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo` | `LbPair.active_id` (`i32`) with `bin_step` | **yes** — `Oracle` ring, growable | no (IDL only) |
| Meteora DAMM v2 | `cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG` | `Pool.sqrt_price` (`u128`) | **no** | yes |
| Meteora DBC | `dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN` | `PoolState.sqrt_price` (`u128`), `.base_reserve`, `.quote_reserve` | **no** | yes |

All rows verified-from-source-code except the two pump.fun rows and Meteora
DLMM, which are verified-from-docs (published IDL and first-party docs).

### 2.2 Raydium CLMM

`PoolState` (`programs/amm/src/states/pool.rs`, `#[account(zero_copy(unsafe))]`,
`#[repr(C, packed)]`) carries `liquidity: u128`, `sqrt_price_x64: u128`
("The current price of the pool as a sqrt(token_1/token_0) Q64.64 value"),
`tick_current: i32`, and `observation_key: Pubkey`.
(verified-from-source-code.)

`ObservationState` (`programs/amm/src/states/oracle.rs`):

```rust
pub const OBSERVATION_SEED: &str = "observation";
pub const OBSERVATION_NUM: usize = 100;
pub const OBSERVATION_UPDATE_DURATION_DEFAULT: u32 = 15;

pub struct Observation {
    pub block_timestamp: u32,
    pub tick_cumulative: i64,
    pub padding: [u64; 4],
}

pub struct ObservationState {
    pub initialized: bool,
    pub recent_epoch: u64,
    pub observation_index: u16,
    pub pool_id: Pubkey,
    pub observations: [Observation; OBSERVATION_NUM],
    pub padding: [u64; 4],
}
```

`Observation::LEN = 44`; `ObservationState::LEN = 8 + 1 + 8 + 2 + 32 + 4400 + 32
= 4483`. (verified-from-source-code for the constants; the total is derived.)

Semantics: a ring of 100 entries; `update` returns early when
`delta_time < 15`, so a slot is written **at most once per 15 seconds**;
`tick_cumulative` accumulates `tick × delta_time` from the *previous* entry.
This is a geometric-mean (tick) TWAP in the Uniswap V3 style, with
`seconds_per_liquidity` omitted. (verified-from-source-code.)

Maximum representable window: **1500 seconds = 25 minutes** at the minimum
15-second spacing. (derived — 100 × 15 s. Bound is mathematical *given* the
constants; the realized span is longer whenever trades are sparser than one per
15 s, and undefined when the pool has not traded, because `update` is only
reached from a swap.)

### 2.3 Raydium CPMM

`PoolState` (`programs/cp-swap/src/states/pool.rs`) does **not** store reserves.
It stores `token_0_vault`, `token_1_vault`, `observation_key`, decimals, and the
accrued fee counters `protocol_fees_token_{0,1}`, `fund_fees_token_{0,1}`,
`creator_fees_token_{0,1}`. (verified-from-source-code.)

The spot price is a **joint decode of three accounts**:

```rust
pub fn vault_amount_without_fee(&self, vault_0: u64, vault_1: u64) -> Result<(u64, u64)> {
    // subtracts protocol_fees + fund_fees + creator_fees from each raw vault balance
}
pub fn token_price_x32(&self, vault_0: u64, vault_1: u64) -> Result<(u128, u128)> {
    let (token_0_amount, token_1_amount) = self.vault_amount_without_fee(vault_0, vault_1)?;
    Ok((
        token_1_amount as u128 * Q32 as u128 / token_0_amount as u128,
        token_0_amount as u128 * Q32 as u128 / token_1_amount as u128,
    ))
}
```

with `Q32 = 2^32`. (verified-from-source-code.) Two exactness notes: the price
is **Q32.32**, despite the swap path binding the result to locals named
`token_0_price_x64`; and it is a **raw-atom ratio with no decimal
normalization** — the mint decimals live in `PoolState` and are the caller's
responsibility.

`ObservationState` (`programs/cp-swap/src/states/oracle.rs`) is a 100-entry ring
of `Observation { block_timestamp: u64, cumulative_token_0_price_x32: u128,
cumulative_token_1_price_x32: u128 }` (40 bytes each), with the same
`OBSERVATION_UPDATE_DURATION_DEFAULT: u64 = 15` spacing. It is an
**arithmetic-mean** price accumulator, not a geometric one.
(verified-from-source-code.)

The manipulation-relevant detail: in `swap_base_input.rs`, `token_0_price_x64`
is computed from `get_swap_params` **before** the swap executes, and
`observation_state.update(...)` is called **after** the vault transfers, with
that pre-swap price. (verified-from-source-code.) The accumulator therefore
integrates the price that stood *before* the current transaction. A single
transaction cannot push its own manipulated price into the accumulator; it can
only make that price the one integrated by the *next* interaction, over the
interval until then. This is the standard Uniswap-V2 accumulator property and it
is a real, source-verified defence.

Maximum representable window: **1500 seconds = 25 minutes**. (derived, same
basis and same caveats as §2.2.)

### 2.4 Orca Whirlpools

`Whirlpool` (`programs/whirlpool/src/state/whirlpool.rs`) carries
`liquidity: u128`, `sqrt_price: u128` (Q64.64), `tick_current_index: i32`,
`token_vault_a`, `token_vault_b`. There is no `observation_key` and no
cumulative price field. (verified-from-source-code.)

The account named `Oracle` is **not a price oracle**:

```rust
pub struct Oracle {
    pub whirlpool: Pubkey,
    pub trade_enable_timestamp: u64,
    pub adaptive_fee_constants: AdaptiveFeeConstants,
    pub adaptive_fee_variables: AdaptiveFeeVariables,
    pub reserved: [u8; 128],
}
```

`AdaptiveFeeVariables` holds `volatility_reference`,
`tick_group_index_reference`, `volatility_accumulator`, and two timestamps — a
volatility measure used to compute a dynamic fee rate.
(verified-from-source-code.) A case-insensitive search of that file for `twap`,
`cumulative`, and `observation` returns **zero** matches.
(verified-from-source-code.)

**Orca Whirlpools maintains no on-chain price TWAP.** Any windowed statistic
over a Whirlpool must be built by an external observer.

Orca does publish a verifiable build: "The contract is deployed using verifiable
build, so that you can ensure that the hash of the on-chain program matches the
hash of the program in this codebase," with a pointer to Solana Verify CLI and
an Osec verification URL. (verified-from-docs — repository README.) This is the
only venue in the table that offers a first-party path from an observed ELF
digest to reviewable source.

### 2.5 Meteora DLMM

The program is `lb_clmm`, version `0.12.0` in the published IDL, at
`LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo`. **Program source is not
published**; `MeteoraAg/dlmm-sdk` ships the IDL, a TypeScript client, a Rust
`commons` crate, and a CLI. (verified-from-docs.)

Price state: `LbPair.active_id: i32` together with `bin_step: u16`; bin price is
a function of bin ID and bin step. `LbPair.oracle: Pubkey` points at the
observation account. (verified-from-docs — IDL and first-party account docs.)

The `Oracle` account is a fixed header plus a **dynamic trailing ring**:

- Header (in the IDL): `idx: u64`, `active_size: u64`, `length: u64`.
- Trailing entries, 32 bytes each, are not in the IDL. The first-party SDK
  decodes them as `cumulative_active_bin_id` (`i128`, two's complement),
  `created_at` (`i64`), `last_updated_at` (`i64`), starting at byte offset
  `8 + 24 = 32`. (verified-from-source-code —
  `ts-client/src/dlmm/helpers/oracle/wrapper.ts`, constants
  `ORACLE_METADATA_SIZE = 8 + 24` and `OBSERVATION_SIZE = 32`.)

Constants from the first-party Rust `commons` crate:
`DEFAULT_OBSERVATION_LENGTH: u64 = 100` and `SAMPLE_LIFETIME: u64 = 120`.
(verified-from-source-code — `commons/src/constants.rs`.) Meteora's own account
documentation states `length` is "Total observation capacity (default 100;
extend with `increase_oracle_length`)." (verified-from-docs.)

Maximum representable window at default capacity: **12 000 seconds = 3 h 20 m**.
(derived — 100 × 120 s. Bound is mathematical given those constants; whether the
program enforces `SAMPLE_LIFETIME` exactly cannot be confirmed from source
because the program is not published, so treat the 120-second figure as
verified-from-source-code *of the SDK*, not of the program.)

Two instructions matter disproportionately:

- **`increase_oracle_length(length_to_add: u64)`** — accounts are `oracle`
  (writable), `funder` (signer, writable), `system_program`, `event_authority`,
  `program`. The only signer is the rent payer. **Anyone can pay to grow any
  pool's observation ring.** (verified-from-docs — IDL account metadata, plus
  the first-party CLI implementation in
  `cli/src/instructions/increase_oracle_length.rs`.)
- **`go_to_a_bin(bin_id: i32)`** — accounts are `lb_pair` (writable),
  `bin_array_bitmap_extension`, `from_bin_array`, `to_bin_array`,
  `event_authority`, `program`. **The IDL account list names no signer at all**,
  and `lb_pair` — which holds `active_id`, the price — is writable.
  (verified-from-docs — IDL account metadata.) The guard conditions are
  **unverified**; the program source is not published. Meteora's user
  documentation describes the corresponding product behaviour (§3).

Meteora's own audit index lists DLMM reports for versions 0.9.0, 0.10.0,
0.10.1, 0.11.0, 0.12.0, and 0.13.0 across five firms.
(verified-from-docs — `resources/audits/dlmm.mdx`.) A six-version audit series
is direct evidence of a repeatedly upgraded program.

### 2.6 Meteora DAMM v2 and DBC

Both publish full program source.

**DAMM v2** (`cp-amm` 0.2.3, `cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG`):
`Pool` carries `sqrt_price: u128` ("current price"), `sqrt_min_price`,
`sqrt_max_price`, `liquidity: u128`, `token_a_vault`, `token_b_vault`, and — at
`layout_version = 1` — `token_a_amount`/`token_b_amount` tracked in the pool
account itself. A search of the repository for `oracle` and `twap` paths returns
nothing, and the pool state file contains no cumulative-price field.
(verified-from-source-code.) `layout_version: u8` is worth naming: this venue
carries an **explicit in-account layout version**, which pump.fun does not.

**DBC** (`dynamic-bonding-curve` 0.2.0,
`dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN`): `PoolState` carries
`sqrt_price: u128` ("current price"), `base_reserve: u64`, `quote_reserve: u64`,
`is_migrated: u8`, `migration_progress: u8`, `finish_curve_timestamp: u64`, and
a `volatility_tracker`. `MigrationProgress` is an explicit four-state enum:
`PreBondingCurve`, `PostBondingCurve`, `LockedVesting`, `CreatedPool`, with the
transition flows documented in a source comment. No oracle account.
(verified-from-source-code.) `const_assert_eq!(PoolState::INIT_SPACE, 416)`
pins the layout size in source. (verified-from-source-code.)

This makes DBC the **open-source structural analogue of pump.fun's bonding
curve**, with a richer and explicitly enumerated graduation state.

### 2.7 PumpSwap

`Pool` (published IDL) holds `pool_bump`, `index`, `creator`, `base_mint`,
`quote_mint`, `lp_mint`, `pool_base_token_account`, `pool_quote_token_account`,
`lp_supply`, `coin_creator`, `is_mayhem_mode`, `is_cashback_coin`, and
`virtual_quote_reserves: i128`. (verified-from-docs.)

Reserves are therefore **not in the pool account**: the constant-product price
is the ratio of two SPL token account balances, adjusted by
`virtual_quote_reserves` where mayhem mode applies. There is no observation
account and no cumulative-price field in the IDL. `GlobalConfig` carries
`disable_flags: u8` — a bitmask that can disable instructions —
`lp_fee_basis_points`, `protocol_fee_basis_points`, and a mutable `admin`.
(verified-from-docs.)

### 2.8 The structural finding

Reserves-in-vault-accounts is the majority pattern. On Raydium CPMM and
PumpSwap the price is a *joint* decode of a pool account plus two SPL token
accounts, and on CPMM it additionally requires subtracting three fee counters
held in a fourth place (the pool account). A chain-state observation is
therefore an **account-set** observation, not a single-account read, and the
adapter must bind the vault pubkeys from the pool account rather than accept
them from a caller — the O-016 boundary applies inside a single observation.

---

## 3. "Sync with Jup" and what Jupiter actually offers as price truth

The phrase resolves precisely. Meteora's first-party user documentation, in
`user-guides/how-to-use-dlmm/dynamic-terminal.mdx`, §"Current Pool Price and
Sync", states (verified-from-docs, verbatim):

> The pool price may not always match the general market price — especially for
> newly created pools with low liquidity.
>
> Meteora uses Jupiter's price API as a market price reference. Before adding
> liquidity, compare the pool price with other markets to confirm it's in sync.
>
> ### Sync with Jupiter's Price
>
> If the pool price is out of sync, use the **"Sync with Jupiter's price"**
> button before depositing. This is available for pools where:
> - There is 0 liquidity between the active bin and the Jupiter price bin, or
> - The liquidity is in a bin close enough to the Jupiter price bin
>
> If there's liquidity in the bins between the active and market price that
> isn't close enough to sync automatically, you can either wait for arbitrage
> trades to bring it in sync, or make a few tiny manual swaps through the pool
> in the appropriate direction.

So: **"Sync with Jup" is a front-end button that moves a DLMM pool's active bin
to the bin nearest an off-chain Jupiter API price, permitted only when no
liquidity sits in between.** It is not an oracle, not a feed, and not a
protocol-level price authority. The on-chain instruction it corresponds to is
`go_to_a_bin(bin_id: i32)` (§2.5) — a direct write of the pool's price with no
signer named in the IDL account list. The correspondence between the button and
that instruction is a **strong inference from matched semantics, labeled
unverified**: the DLMM program source is not published and Meteora's docs do not
name the instruction on that page.

Design consequence: on a low-liquidity DLMM pool, `active_id` — the entire price
state — can be relocated by a permissioned-by-nothing instruction whose only
documented precondition is an absence of intervening liquidity. That is exactly
the condition of a longtail pool.

**Jupiter as price truth.** Jupiter's Price API V3 is off-chain and
unauthenticated in the cryptographic sense: endpoint `https://api.jup.ag/price/v3`,
REST, API-key header, up to 50 mint ids per query. It "prices tokens by using the
last swapped price (across all transactions)", working outward from reference
tokens like SOL whose price comes from external oracle sources, and applies
"heuristics" over asset origin, liquidity, holder distribution, and trading
patterns to "eliminate any outliers", omitting tokens judged unreliable rather
than returning nulls. (verified-from-docs — Jupiter developer documentation.)
There is **no on-chain component and no signature** described.

The documentation contains no statement either endorsing or prohibiting use as
a settlement oracle. (verified-from-docs — absence noted explicitly rather than
inferred.)

For dClutch this is disqualifying on its face under O-007: an unsigned HTTP
response is a client-supplied witness, not provider-authenticated evidence, and
no transport can make it one.

One disambiguation, because "Jup" appears in this ecosystem in unrelated senses:
Meteora's DBC source comments describe pool-state transition flows "without jup
lock" and "with jup lock" (verified-from-source-code), referring to token
vesting, not pricing; and Jupiter is also the dominant routing aggregator. None
of the three is an on-chain price attestation.

---

## 4. Longtail oracle alternatives

### 4.1 Switchboard

Switchboard's Solana product line is the only credible existing answer for
longtail price truth, and the interesting part is *what it already models*.

Its published Task Types reference includes task types that read Solana AMM
state directly (verified-from-docs):

- `LpExchangeRateTask` — "Fetch the current swap price for a given liquidity
  pool", with parameters including `raydium_pool_address`, `orca_pool_address`,
  `orca_pool_token_mint_address`, `saber_pool_address`,
  `mercurial_pool_address`, `port_reserve_address`, `defituna_pool_address`.
- `LpTokenPriceTask` — "Fetch LP token price info from a number of supported
  exchanges", with `use_fair_price`.
- `MeteoraSwapTask` — "Grab the swap price from a Meteora pool", parameters
  `pool`, `type`.
- `PumpAmmTask` — "Execute a swap task in the Pump AMM based on the given
  parameters", parameters `pool_address`, `in_amount`, `max_slippage`,
  `is_x_for_y`.
- `PumpAmmLpTokenPriceTask` — "Derive the fair LP token price for a given Pump
  AMM liquidity pool".

Verification model: "Every Switchboard oracle update requires two instructions
in sequence: 1. Ed25519 Signature Verification — Verifies the oracle operator's
signature, 2. Quote Program Storage — Stores the verified data in the canonical
oracle account." Results land in a `SwitchboardQuote` account at a canonical
address derived from `(queue, feed_id)`, and staleness is measured in slots by
comparing `clock.slot` against `quote_account.slot`.
(verified-from-docs — Switchboard Solana price-feed documentation.)

Permissionless feed creation is claimed by Switchboard's own marketing material
and repeated widely, but the current first-party developer pages I could reach
do not state it in those words; the tutorial page instead points at a
"Switchboard Explorer" for feed ids. Treat "anyone can create a feed" as
**reported-secondhand**. Concrete SOL costs for feed creation and per-update
consumption: **unverified** — no first-party figure was reachable.

The architectural point, which does not depend on the unverified parts:
**Switchboard already offers the "the chain is the provider" reading, computed
off-chain by an operator and attested with an Ed25519 signature.** A dClutch
chain-state adapter is the *same datum with a different trust path* — decoded
on-chain, with no operator quorum, no signature, and no off-chain compute. The
honest comparison is:

| | Switchboard over an AMM | dClutch chain-state adapter |
| --- | --- | --- |
| Who reads the pool | oracle operators, off-chain | the dClutch program, on-chain |
| What is trusted | operator set + Ed25519 signature + queue policy | the venue program's identity and the decode rules |
| Failure mode | operator collusion / liveness | venue upgrade, layout change, price manipulation |
| dClutch code required | one more signature-verifying adapter (Pyth-shaped) | a new account-decode adapter family |
| O-007 posture | provider-authenticated evidence | release-selected state, no provider at all |

That table is the strategic content of this section: choosing the chain-state
family is a decision to *remove* an intermediary and *accept* venue-program risk
directly, not a decision to get data that is otherwise unavailable.

### 4.2 Pyth

Pyth's coverage is publisher-driven: "over 120 institutions — including global
exchanges, trading firms, and market makers — publish price feeds", across a
feed catalogue described as 380+ low-latency feeds spanning crypto, equities,
ETFs, FX, and commodities. (reported-secondhand — aggregator and marketing
summaries; the exact current count was not read from a first-party feed index.)

I could not find any first-party Pyth statement of a listing policy for
memecoins or longtail Solana tokens, in either direction. **Unverified.**

The structural point is not in doubt and does not need the policy: Pyth prices
what institutional publishers quote. A pump.fun coin minutes after creation has
no institutional publisher, and no plausible mechanism by which one appears.
Pyth is not a candidate for the longtail case, and this is a property of its
design rather than a gap in its coverage.

The repository already records the narrower related boundary:
`crates/dclutch-source-contract/DESIGN.md:111` — "A Pyth TWAP is not part of this
release; it requires a separately pinned and measured provider adapter."

### 4.3 Anything else

Nothing else surfaced that is simultaneously live, longtail-capable, and
on-chain-verifiable. Off-chain indexers (Birdeye, GeckoTerminal, DEXScreener,
Bitquery, Codex) all price longtail tokens and none offers an on-chain
attestation, which places them in the same category as Jupiter's API under
O-007.

---

## 5. Manipulation economics

### 5.1 The atomicity bound

Solana gives an attacker two composition primitives:

1. **One transaction is atomic.** Any sequence of instructions within it either
   all commits or none does. An attacker can therefore manipulate a pool,
   trigger an observation, and restore the pool in a single transaction *if the
   observation is reachable from within that transaction* — which it is, if the
   dClutch observation instruction can be placed in the same transaction.
2. **A Jito bundle is atomic across transactions in one slot.** Jito's
   documentation states: "Bundles are groups of transactions (max 5) bundled
   together. The transactions are executed sequentially and atomically meaning
   all-or-nothing," and "Bundles execute within the same slot (e.g. a bundle
   cannot cross slot boundaries)." (verified-from-docs.)

So the correct threat bound is not "one transaction" but **five transactions,
sequential, atomic, same slot**. A design that separates manipulation from
observation into different transactions has bought nothing against a bundle. The
only structural defence is *time*: a statistic whose inputs span more than one
slot cannot be produced inside one bundle.

(mathematical bound, given Jito's stated bundle semantics; it does not bound an
attacker who is also the leader for consecutive slots.)

### 5.2 Cost of moving a constant-product price

For a constant-product pool with reserves `(R_base, R_quote)` and zero fees, the
quote-side input required to multiply the marginal price by a factor `m > 1` is

```
Δ_quote = R_quote · (√m − 1)
```

(mathematical, from `x·y = k` with price `= R_quote / R_base`: raising price by
`m` requires scaling `R_quote` by `√m` and `R_base` by `1/√m`. Fees and the
partial refund on unwinding are excluded, so this is a **lower bound** on cost
and the round-trip cost is higher.)

Worked bounds:

| Price move `m` | `Δ_quote` as a fraction of `R_quote` |
| --- | --- |
| 1.01 (+1 %) | 0.004988 |
| 1.10 (+10 %) | 0.048809 |
| 2.00 (+100 %) | 0.414214 |

For pump.fun the reserves that set the price are the **virtual** ones, so a
fresh curve with `initial_virtual_sol_reserves = 30 SOL` costs
`30 · (√2 − 1) ≈ 12.43 SOL` of real inflow to double its price, even though its
*real* SOL reserve is near zero. (derived, on the published `Global` values;
bound is chain-derived for those parameters.) The virtual reserve is therefore a
liquidity floor as well as a pricing device — a useful and slightly
counter-intuitive property for a threshold market on a young curve.

### 5.3 Flash-loan availability

Marginfi documents flash loans that allow "borrowing nearly all available
liquidity, provided it is returned within the same transaction", enforced by
instruction introspection. (verified-from-docs — marginfi documentation and SDK
examples.) Kamino and Solend are similarly reported to offer them.
(reported-secondhand.)

The relevant asymmetry: flash loans exist for **SOL and USDC**, which is exactly
the side needed to push a longtail token's price *up*. Longtail tokens
themselves have no lending market, so pushing a price *down* requires owning the
token. A threshold market is therefore not symmetric in attack cost, and a
design that assumes symmetry is wrong.

### 5.4 Cost of forcing a pump.fun graduation

A graduation is a discrete event, so its attack cost is not §5.2's marginal
formula. It is the unrecoverable loss on a buy-out-and-exit round trip, and
because `migrate` is permissionless and idempotent (§1.4), the whole round trip
fits inside one Jito bundle: buy the curve out, migrate to PumpSwap, sell into
the pool the migration just created.

Under the `Global` parameters published in the README:

| Step | Amount |
| --- | --- |
| Real SOL required to complete the curve | 85.005359 SOL |
| Buy fee at `fee_basis_points = 100` | 0.850054 SOL |
| Tokens the attacker now holds | 793.1 × 10¹² |
| Pool created by `migrate`: base | 206.9 × 10¹² (= `token_total_supply` − `initial_real_token_reserves`) |
| Pool created by `migrate`: quote | 84.990359 SOL (= 85.005359 − `pool_migration_fee` 0.015000001) |
| Gross proceeds selling 793.1 × 10¹² into that pool | 67.405854 SOL |
| Net of PumpSwap's 25 bps (20 LP + 5 protocol) | 67.237339 SOL |
| **Net cost to force graduation** | **18.618074 SOL** — 21.9 % of the 85 SOL nominal |

(derived — exact decimal arithmetic on the published constant product and the
published fee parameters. Bound is chain-derived *for those parameter values*
and is a **lower bound**: it excludes creator fee, cashback and buyback fee
components present in the current IDL but published as zero or unspecified in
the README, excludes priority fees and rent, and assumes the attacker's own buy
faces no competing flow.)

Three things follow, and the first two are corrections to the intuitive reading:

1. **The 85 SOL figure is not the attack cost.** Roughly 78 % of it is
   recovered on exit. A design that sizes a graduation market against 85 SOL
   over-states its safety by a factor of about 4.6.
2. **The round trip is atomic.** `migrate` requires only `complete == true` and
   `real_token_reserves == 0`, both set by the completing buy, and is
   permissionless. Whether the program forbids migrating in the same slot as
   completion is **unverified** — the source is not published — but a Jito
   bundle spans five transactions in one slot regardless (§5.1), so a same-slot
   guard would not help.
3. **The cost is a floor, not a ratio.** Unlike a marginal-price manipulation,
   it does not fall as the target pool thins, and it does not scale with the
   Market's size. It is therefore a usable input to §6.5's capacity predicate
   with an unusually clean interpretation: a graduation Market whose total Hoard
   principal is well below ≈ 18.6 SOL is not worth forcing.

### 5.5 Precedents worth learning from

**Loopscale, 2025-04-26 — the most directly relevant.** An attacker "deployed a
fake RateX market program that mimicked the legitimate interface", causing the
protocol to read an inflated exchange rate through a `get_pt_price` instruction,
and extracted 5,726,724.97 USDC and 1,211.4 SOL. The root cause was missing
**program identity verification** on one collateral path; the fix "enforce[d]
strict validation of RateX program IDs during loan health checks." Funds were
returned 2025-04-27 to 2025-04-29. (verified-from-docs — Loopscale's own
post-mortem.)

This is the canonical failure of the design space this dossier is about: the
protocol trusted an account's *shape* without binding the *program that owns it*.
It is the empirical argument for O-016 and O-018 in one incident, and it is why
program-identity binding, not decode correctness, is the first-order concern.

**Nirvana Finance, 2022-07-28.** A $10 M USDC flash loan from Solend was used to
mint against a bonding curve, inflating the protocol's own price reference; net
extraction ≈ $3.49 M. (reported-secondhand — The Block, CoinDesk and others; no
first-party post-mortem read.) Relevance: a Solana bonding curve, manipulated
inside one atomic borrow-and-repay.

**pump.fun, 2024-05-16/17.** ≈ $1.9–2 M extracted using marginfi flash loans
against bonding curves, combined with a compromised privileged key; trading was
paused and contracts upgraded. (reported-secondhand.) Relevance: the
`withdraw_authority` design of the era meant a key compromise was sufficient —
the venue's own privileged instructions are part of the threat surface, not just
its market microstructure.

**Mango Markets, 2022-10-11.** ≈ $110–117 M extracted by pushing MNGO's price on
the exchanges that fed the oracle, moving the reported price "over 13-fold
during a 30-minute span" and borrowing against the inflated collateral.
(verified-from-docs for the mechanics quoted — CFTC complaint and press release;
the dollar figures vary by source and are reported-secondhand.) Relevance: the
manipulated venue was thin *relative to the position taken against it*. The
lesson is a ratio, not a price: **the size of the claim must be bounded by the
depth of the thing that resolves it.**

**Venue TWAP windows, for calibration.** The venues that maintain their own
accumulators chose 15-second minimum sample spacing over 100 slots (Raydium
CLMM and CPMM: 25 minutes) and 120-second sample lifetime over 100 slots
(Meteora DLMM: 3 h 20 m). (verified-from-source-code / verified-from-docs, §2.)
These are the market's own revealed answer to "how long is long enough", and
they are a defensible starting point for a dClutch window rather than a number
invented here.

---

## 6. Proposal — mapping onto existing dClutch machinery

**Everything from here to the end of §8 is a proposal, not a decision.** No row
of `docs/OMISSION_INDEX.md` is closed by it, no ADR is implied by it, and it
must receive an architecture decision before any code is written.

### 6.1 What already exists, named exactly

The proposal composes existing contracts rather than inventing parallel ones.

Source identity and policy, `crates/dclutch-source-contract`:

- `ProviderReleaseV1 { provider_family_id, adapter_release_id,
  provider_deployment_release_id, decoding_rules_id, transport_profile_id }`
  (magic `DCLTPRV1`, 176 bytes).
- `SourceSpecV1 { domain_id, unit_id, provider_release_id, access_profile,
  adapter_config_id, capacity_profile_id }` (magic `DCLTSRC1`, 192 bytes).
- `SourceAccessProfile { PythTerminalOneTransaction = 1,
  SharedObservationChild = 2 }`.
- `WindowSpecV1 { source_spec_id, kind, start_unix_seconds, end_unix_seconds,
  max_age_seconds, max_future_skew_seconds, schedule_id }` with
  `WindowKind { Terminal, ScheduledInterval }`.
- `StatisticSpecV1` over `StatisticKind { TerminalSample, ExactScheduledAverage,
  Minimum, Maximum, AtLeastThreshold, AtMostThreshold, OddScheduledMedian }`
  and `RoundingBoundary { ExactRational, Floor, Ceiling }`.
- `Observation { atoms: i128, unix_seconds: i64 }`.
- `SourceCapacityProfileV1 { envelope, max_samples, max_recovery_attempts,
  verifier_release_id, envelope_basis_id, max_observation_bytes,
  max_shared_children }` with `CapacityEnvelope { Measured, Provisional }`,
  `MAX_RECOVERY_ATTEMPTS = 4`, `MAX_SHARED_OBSERVATIONS = 16`.
- `SourceMaterialV2`, `SourceResolutionStateV2`, and
  `SourceResolutionPhaseV1 { Primary, Recovery, Resolved, Exhausted,
  FailureCommitted, Retired }`.

Program-identity discipline, `crates/dclutch-registry-contract` and
`crates/dclutch-registry-svm`:

- `DeploymentObservationV1 { program, program_owner, program_executable,
  programdata, programdata_owner, programdata_executable, programdata_link,
  loader_program, deployment_slot, elf_digest, upgrade_authority: Option<[u8;32]> }`
  — "Chain-derived current observation of one Loader V3 deployment. An SBF
  adapter constructs this only after hostile parsing of the actual Program and
  ProgramData accounts and hashing the exact ELF tail."
- `ArtifactReleaseV1::authenticate_deployment(observed)`, which compares
  program/programdata/loader identity, the programdata link, owners,
  executability, `deployment_slot`, `elf_digest`, and `upgrade_authority` by
  **exact** equality.
- `ArtifactUpgradePolicyV1 { Immutable = 0, ExactAuthority = 1 }`.
- `ProgramDataV3View { deployment_slot, upgrade_authority, elf }`, which already
  handles the Loader-V3 quirk that stale authority bytes remain after a `None`
  write.

Account observation, `crates/dclutch-account-profile-contract`:

- `AccountObservationV1 { key, owner, lamports, data, signer, writable,
  executable, adapter_authenticated_variable_data }`.

Funded permissionless work, `crates/dclutch-resolution-codec`:

- `FundedTransitionReceiptV1` with `worker: [u8; 32]` — "Permissionless worker
  credited by this transition" — `work_paid: u64`, and `funding_remaining: u64`.
- `FundedTransitionActionV3 { FailNext, Exhaust, CommitFailure }`.

Operator-side snapshot discipline,
`crates/dclutch-provider-transport-v3-operator`:

- `require_same_finalized_observation`, which enforces that every input account
  was observed at the same finalized slot.

Notably: `DeploymentObservationV1` and `ProgramDataV3View` are already
shape-generic over any Loader-V3 deployment. Every current caller uses them on
dClutch's own roles or on Pyth's Receiver/router; nothing in them is
first-party-only.

### 6.2 Program identity binding, and the upgrade-mid-market question

Third-party venue programs are upgradeable and are upgraded (§1.6, §2.5). A
policy that does not say what happens on upgrade is not a policy.

`ArtifactReleaseV1::authenticate_deployment` compares `elf_digest` and
`deployment_slot` by exact equality. Under that primitive, three policies are
expressible, and only two are honest:

- **P-A accept-current.** Bind program pubkey, owner, and loader; accept any
  `elf_digest`. This requires a *new, weaker* predicate — the existing one
  cannot express it. It also breaks the tie between the decode rules and the
  code that wrote the bytes, which is precisely the Loopscale failure (§5.5)
  one level up. **Reject.**
- **P-B pinned digest, upgrade ⇒ failure outcome.** Bind the exact
  `elf_digest` at founding via an `ArtifactReleaseV1`-shaped record for the
  third-party program. A venue upgrade makes every subsequent observation
  refuse; the Source walks `Primary → Recovery → Exhausted → FailureCommitted`
  and the Market lands on the Product's named terminal failure outcome. Needs
  **no new authentication primitive**. Cost: a routine, benign venue upgrade
  converts a live market into a failure payout, and given §1.6's cadence that
  will happen.
- **P-C pinned digest set.** Admit a finite, founding-time-named set of
  `elf_digest` values. Strictly more expressive than P-B, strictly weaker than
  nothing, and it still cannot admit an upgrade that has not happened yet.
  Requires a new record shape and a new predicate.

Proposed: **P-B for the first slice, P-C as the named lift.** P-B is
simultaneously the cheapest to build and the strictest, which is an unusual and
welcome alignment. The failure-outcome semantics must be stated to the user at
founding, in the Product, not discovered at resolution — a market on a
third-party venue is a market that can terminate in "the venue changed."

Additionally, `DeploymentObservationV1.upgrade_authority` must be recorded and
compared. A venue whose program becomes immutable (`None`) or whose authority
rotates has materially changed, and `ArtifactUpgradePolicyV1::ExactAuthority`
already expresses the binding.

### 6.3 Observation = hostile account decode at slot

Proposed new `SourceAccessProfile` variant, e.g.
`ChainStateAccountSetAtSlot = 3`, whose observation input is an **ordered set**
of `AccountObservationV1` rather than a single provider message:

1. the venue program's Program and ProgramData accounts, for
   `DeploymentObservationV1`;
2. the primary state account (`BondingCurve`, `PoolState`, `Whirlpool`,
   `LbPair`, `Pool`);
3. every account the price depends on, **with its pubkey read out of (2), never
   from the caller** — the vault pairs of §2.8, and the observation ring where
   one exists.

`decoding_rules_id` in `ProviderReleaseV1` is the correct home for the layout
grammar, and for this family it must name, per venue:

- the owning program pubkey;
- the account discriminator, and an explicit statement that the discriminator is
  **not** a layout version (§1.3);
- the **admitted data-length set** — for pump.fun's `BondingCurve` that is
  `{49, 81, 115}` — and the field offsets valid at each length;
- sentinel semantics, e.g. `quote_mint == Pubkey::default()` ⇒ wrapped SOL;
- the fixed-point scale and whether decimals are normalized (Raydium CPMM's
  Q32.32 raw-atom ratio is a trap: §2.3);
- the derived `Observation { atoms, unix_seconds }` in the Source's declared
  `unit_id`, with one named `RoundingBoundary`.

`AccountObservationV1.owner` carries the owning program and must be compared
against the pinned program pubkey for **every** account in the set. An account
of the right shape owned by the wrong program is the Loopscale attack.

The `require_same_finalized_observation` invariant already used by the V3
transport operator is the operator-side analogue and should be reused verbatim
for constructing these submissions.

### 6.4 Policy layer owns window, statistic, and confidence

Two mechanisms are available and they are not interchangeable.

**Mechanism A — read the venue's own accumulator.** Where a venue maintains an
on-chain cumulative (Raydium CLMM `tick_cumulative`, Raydium CPMM
`cumulative_token_{0,1}_price_x32`, Meteora DLMM `cumulative_active_bin_id`), a
**single** observation at resolution yields a TWAP over a window the ring
already contains. Cheap: one submission, `WindowKind::Terminal`,
`StatisticKind::TerminalSample` over a pre-averaged datum.

Its three honest limits: the window is whatever the ring holds, not what the
Product chose; the ring only advances when someone trades, so a quiet longtail
pool's "TWAP" can be an arbitrarily old constant; and it is available on three
of the eight venues in §2.1 — not on Orca, PumpSwap, DAMM v2, DBC, or the
pump.fun curve.

**Mechanism B — median over a funded, scheduled window.** `StatisticKind::
OddScheduledMedian` over N ≥ 3 samples at equal cadence, each sample a separate
permissionless submission by a `worker` credited `work_paid` from the Source's
bounty compartment. This works on **every** venue, gives the Product control of
the window, and a median is the right statistic here: unlike a mean, a median
over an odd window is unmoved by any minority of manipulated samples, so an
attacker must hold the manipulated price across ⌈N/2⌉ of the scheduled sample
times — which, by §5.1, cannot be done inside one bundle for N ≥ 3 with cadence
exceeding one slot.

Proposed: **Mechanism B is the family-general mechanism. Mechanism A is a
per-venue optimization admitted only where the ring exists, and never as the
sole input.**

One implementation hazard must be surfaced now, not discovered later:
`OddScheduledMedian` currently requires **strict equal cadence**. Under Solana
congestion a submitter that misses its schedule slot breaks cadence and the
statistic refuses. Either `WindowSpecV1`'s `max_age_seconds` /
`max_future_skew_seconds` must be shown to absorb the jitter, or a cadence
tolerance is a prerequisite lift for this family. This is a **provisional**
judgement — no measurement exists.

**Confidence.** Pyth's analogue is `PythAdapterConfigV1.max_confidence_bps`,
enforced as `confidence · 10_000 ≤ |price| · max_confidence_bps`. The
chain-state analogue is **depth**, and §5.2 gives it a closed form: a pool's
`R_quote` and the target threshold's distance from spot determine the cost to
cross it. Proposed adapter-config field, per sample:

```
manipulation_cost_lower_bound = R_quote · (√m − 1)
```

with `m` the multiplicative distance from the observed price to the nearest
Product threshold edge, and a refusal when that bound falls below a configured
floor. Two properties matter: the bound is **mathematical** given constant
product and zero fees (it is a lower bound; round-trip cost is higher), and it
is computed from the *same observation* as the price, so it cannot be stale
relative to it. For a bin-based venue (DLMM) the analogous quantity is the
liquidity in the bins between spot and the threshold bin, which requires
decoding `BinArray` accounts and is a strictly larger observation.

### 6.5 Capacity profiles bounding market size against pool depth

`SourceCapacityProfileV1` already bounds `max_samples`,
`max_recovery_attempts`, `max_observation_bytes`, and `max_shared_children`, and
`CapacityEnvelope` already distinguishes `Measured` from `Provisional`. This
family needs those and one more idea.

Proposed founding-time admission predicate: a Market's total Hoard principal is
admitted only if

```
total_principal ≤ κ · manipulation_cost_lower_bound(observed depth at founding)
```

with κ a `Provisional` bound requiring a lifting plan. The Mango lesson (§5.5)
is precisely that the *ratio* of position size to venue depth is the invariant
that was violated, so this is the correct shape of predicate even before κ has a
defensible value.

The predicate is not sufficient by itself: **depth can fall after founding**
when LPs withdraw, and a market that was well-collateralized at founding can be
cheap to attack at resolution. Therefore depth must be re-evaluated **at
observation time** and the confidence refusal of §6.4 must fire on the observed
depth, not the founding depth. The founding predicate bounds what may be
created; the observation-time refusal bounds what may resolve. Both are needed
and neither substitutes for the other.

`max_samples` interacts with cost directly: Mechanism B's N submissions each
cost a transaction and a `work_paid` credit, all drawn from prepaid bounty
funding, so the window length is bounded by what the Market prepaid at founding.
This is the existing "deferred physical creation must be precommitted and
prepaid" discipline applied unchanged.

---

## 7. Candidate first products, ranked by architectural cleanliness

**Rank 1 — the graduation market.** "Does mint M's bonding curve reach the
completed state on or before time T?"

- The observable is a **discrete state transition**, not a price: one byte
  (`complete`) on pump.fun, or a four-state enum (`MigrationProgress`) plus
  `is_migrated` on Meteora DBC.
- No window, no statistic, no median, no cadence, no `OddScheduledMedian`
  cadence hazard. `WindowKind::Terminal` and `StatisticKind::TerminalSample`
  suffice.
- No fixed-point scale, no decimal normalization, no unit conversion, and
  therefore no rounding boundary to argue about.
- The manipulation cost is not a marginal-price question but a
  *buy-out-the-curve-and-exit* question, and §5.4 derives it exactly:
  **18.618074 SOL** under the published default parameters. Two properties make
  that number unusually usable. It is a **floor**, not a ratio: it is fixed by
  the curve's own parameters and does not fall as the coin's real liquidity
  thins, whereas §5.2's marginal cost falls in direct proportion to `R_quote`.
  And it is **unrecoverable**: it is realized loss, not capital at risk, so a
  flash loan does not reduce it.
- It exercises the entire new machinery — third-party program identity binding,
  upgrade-mid-market semantics, hostile account-set decode, admitted-length
  handling — with the *smallest possible* policy layer on top.

One correctness caveat that must not be glossed: **irreversibility of `complete`
is unverified.** No instruction in the published pump IDL is documented as
resetting it, but the program source is not published, and
`set_mayhem_virtual_params` demonstrably writes the bonding-curve account. The
adapter must therefore **latch on first authenticated observation** and must not
assume monotonicity as a decode-time invariant.

**Rank 2 — price-threshold markets on a venue that maintains its own
accumulator** (Raydium CLMM, Raydium CPMM, Meteora DLMM). Mechanism A applies,
so the first price product needs only one submission, and the CPMM accumulator's
pre-swap integration (§2.3) is a source-verified defence that comes for free.
Costs: fixed-point handling, decimal normalization, joint vault decode, and the
"ring has not advanced" staleness case.

**Rank 3 — price-threshold markets on a venue with no accumulator** (Orca,
PumpSwap, Meteora DAMM v2, Meteora DBC, the pump.fun curve). Requires the full
Mechanism B stack — scheduled cadence, N funded submissions, median, prepaid
bounty sizing — plus everything in rank 2. This is where the longtail-token
price market actually lives, and it is correctly last.

---

## 8. Smallest first slice, and its U-009 evidence requirements

### 8.1 The recommendation

**Build the graduation market, and build it first against Meteora's Dynamic
Bonding Curve rather than pump.fun.**

The product demand points at pump.fun and the architecture points at DBC, and
the tension resolves cleanly because they are structurally the same observable:

| | pump.fun | Meteora DBC |
| --- | --- | --- |
| Program source published | **no** | **yes** (`dynamic-bonding-curve` 0.2.0) |
| Graduation observable | `complete: bool` | `MigrationProgress` enum + `is_migrated: u8` + `finish_curve_timestamp` |
| Layout size pinned in source | no | `const_assert_eq!(PoolState::INIT_SPACE, 416)` |
| Account grows in place | **yes**, `extend_account`, 3 known widths | not observed |
| ELF digest bindable to reviewable source | **no** | yes |
| Longtail volume | dominant | smaller |

U-009 requires "real ABI/crypto and recovery evidence." On pump.fun, "real ABI"
can only mean "the published IDL", and the ELF digest the adapter pins binds to
an artifact nobody outside the team has read. On DBC, the ABI is the source, the
layout size is asserted in the source, and the graduation state machine is an
explicit enum with its transitions written down. **The first slice of a new
adapter family should discharge its evidence against reviewable source.**

Sequence: land the family against DBC; run the upgrade-mid-market policy (§6.2)
through at least one real venue upgrade; then add pump.fun as a **second
`decoding_rules_id` record under the same `provider_family_id`**, at which point
its three admitted account widths and its `quote_mint` sentinel are a decode
problem inside a proven family rather than a new family's first problem.

If the product decision overrides and pump.fun must be first, the slice is
unchanged in shape but must additionally state, in the Product and in the
release manifest, that the pinned ELF digest binds to an unpublished artifact.
That is a real weakening and it must be visible, not implicit.

Either way, the slice's capacity predicate (§6.5) has an unusually concrete
instance available on day one: bound total Hoard principal against the
curve's derived buy-out-and-exit floor — 18.618074 SOL for pump.fun's published
defaults (§5.4), and the config-derived analogue for each DBC curve. κ remains
`Provisional`, but the quantity it multiplies is chain-derived rather than
invented, which is the difference between a bound with a lifting plan and a
number with a footnote.

### 8.2 Evidence the slice must produce (U-009-shaped)

1. **Real ABI, no mock authority.** Fixtures decode byte-exact real account
   layouts — for DBC, `PoolState` at exactly 416 bytes; for pump.fun when added,
   all three admitted widths — produced from the published source or IDL, never
   hand-shaped. Per O-007, fixtures are labeled synthetic observations and carry
   no provider authority; per O-018, no adjacency fact is authority.
2. **Real program-identity binding.** A `DeploymentObservationV1` constructed by
   hostile parse of a real Program + ProgramData pair, authenticated against a
   release record, with the `upgrade_authority` field compared and **not**
   ignored.
3. **Upgrade-mid-market refusal, executed.** A campaign in which the pinned
   `elf_digest` no longer matches, the observation refuses, the Source walks
   `Primary → Recovery → Exhausted → FailureCommitted`, and the Market pays the
   Product's named failure outcome — with exact three-ledger closure. Under
   O-005 there is no fallback path and no second decoder.
4. **Hostile-decode refusal corpus.** Adversarial cases per `AGENTS.md`: wrong
   owner (the Loopscale case), right owner and wrong discriminator, right
   discriminator and short data, vault pubkeys supplied by the caller instead of
   read from the pool account, sentinel `Pubkey::default()` mishandled, account
   set observed at mixed slots.
5. **Release-bound adapter.** One `ProviderReleaseV1` naming one
   `provider_deployment_release_id`, one `decoding_rules_id`, one
   `transport_profile_id`. No parallel legacy path (O-005), no mock fallback
   (U-009).
6. **Capacity and cost evidence.** A measured `SourceCapacityProfileV1` with
   `CapacityEnvelope::Measured` where a measurement exists and `Provisional`
   with a lifting plan where it does not, plus real CU, packet, and rent figures
   for the observation instruction. κ from §6.5 is `Provisional` on day one and
   must be labeled so.
7. **Vocabulary discipline.** Nothing in this family may be called a TWAP, an
   oracle, or a price feed without naming the exact statistic, its window, its
   sample count, and its refusal conditions. Local-validator execution is not
   mainnet evidence.

---

## 9. What could not be verified

Listed explicitly rather than filled in with plausible values.

1. **Current upgrade-authority state of every third-party program named here** —
   pump.fun, PumpSwap, Raydium CLMM/CPMM, Orca Whirlpools, Meteora DLMM/DAMM
   v2/DBC. Settling this requires reading each ProgramData account, which is a
   chain read and outside this task's authorization. Raydium's 3-of-4 Squads
   multisig with a 24-hour timelock is **reported-secondhand**; the first-party
   access-controls page did not resolve.
2. **Whether `set_mayhem_virtual_params` is callable by an unprivileged
   signer**, and its guard conditions. Its IDL account list names no external
   signer and it writes `bonding_curve`. The pump program source is not
   published.
3. **Whether pump.fun's `migrate` may execute in the same slot as the buy that
   completed the curve.** §5.4's atomic round trip does not depend on the
   answer — a Jito bundle spans one slot regardless — but the answer changes how
   many transactions the attack needs.
4. **Whether `go_to_a_bin` is callable by an unprivileged signer**, and its guard
   conditions. Its IDL account list names no signer at all and it writes
   `lb_pair`. The DLMM program source is not published. The correspondence
   between the "Sync with Jupiter's price" button and this instruction is a
   strong inference, not a verified fact.
5. **Whether pump.fun's `complete` flag is irreversible.** No published
   instruction is documented to reset it; the source is not published. §7 states
   the mitigation.
6. **Whether Meteora DLMM's program enforces `SAMPLE_LIFETIME = 120`.** The
   constant is verified in the first-party SDK; the program is not published.
7. **Switchboard's exact permissionless-feed-creation policy and its SOL
   costs.** The claim is repeated widely and appears in Switchboard's own
   marketing material, but no first-party developer page reachable in this task
   stated it in operative terms, and no cost figure was found.
8. **Pyth's listing policy for memecoins or longtail Solana tokens**, in either
   direction. No first-party statement found. §4.2's structural argument does
   not depend on it.
9. **Current live values of any account.** Every account dump quoted here is the
   snapshot published in a first-party README, not a read of chain state. The
   `Global` parameters used to derive the 85.005 SOL graduation threshold are in
   this category, and the current IDL indicates they may be superseded by
   per-quote-mint parameters.
10. **Realized observation-ring spans on any specific live pool.** The window
   figures in §2 are capacities under the venues' stated minimum spacings, not
   measurements. A quiet pool's realized span is longer and its newest entry may
   be arbitrarily old.

---

## 10. Sources

pump.fun and PumpSwap (first-party):
- https://github.com/pump-fun/pump-public-docs/blob/main/docs/PUMP_PROGRAM_README.md
- https://github.com/pump-fun/pump-public-docs/blob/main/docs/PUMP_SWAP_README.md
- https://github.com/pump-fun/pump-public-docs/blob/main/docs/instructions/BUY.md
- https://github.com/pump-fun/pump-public-docs/blob/main/idl/pump.json
- https://github.com/pump-fun/pump-public-docs/blob/main/idl/pump_amm.json
- https://github.com/orgs/pump-fun/repositories

Raydium (first-party):
- https://github.com/raydium-io/raydium-clmm/blob/master/programs/amm/src/states/oracle.rs
- https://github.com/raydium-io/raydium-clmm/blob/master/programs/amm/src/states/pool.rs
- https://github.com/raydium-io/raydium-cp-swap/blob/master/programs/cp-swap/src/states/oracle.rs
- https://github.com/raydium-io/raydium-cp-swap/blob/master/programs/cp-swap/src/states/pool.rs
- https://github.com/raydium-io/raydium-cp-swap/blob/master/programs/cp-swap/src/instructions/swap_base_input.rs
- https://docs.raydium.io/reference/program-addresses

Orca (first-party):
- https://github.com/orca-so/whirlpools/blob/main/README.md
- https://github.com/orca-so/whirlpools/blob/main/programs/whirlpool/src/state/oracle.rs
- https://github.com/orca-so/whirlpools/blob/main/programs/whirlpool/src/state/whirlpool.rs

Meteora (first-party):
- https://github.com/MeteoraAg/dlmm-sdk/blob/main/idls/dlmm.json
- https://github.com/MeteoraAg/dlmm-sdk/blob/main/commons/src/constants.rs
- https://github.com/MeteoraAg/dlmm-sdk/blob/main/ts-client/src/dlmm/helpers/oracle/wrapper.ts
- https://github.com/MeteoraAg/dlmm-sdk/blob/main/cli/src/instructions/increase_oracle_length.rs
- https://github.com/MeteoraAg/docs/blob/main/developer-guides/dlmm/program/accounts.mdx
- https://github.com/MeteoraAg/docs/blob/main/user-guides/how-to-use-dlmm/dynamic-terminal.mdx
- https://github.com/MeteoraAg/docs/blob/main/resources/audits/dlmm.mdx
- https://github.com/MeteoraAg/damm-v2/blob/main/programs/cp-amm/src/state/pool.rs
- https://github.com/MeteoraAg/dynamic-bonding-curve/blob/main/programs/dynamic-bonding-curve/src/state/virtual_pool.rs

Oracles and price APIs (first-party):
- https://developers.jup.ag/docs/price
- https://docs.switchboard.xyz/custom-feeds/task-types
- https://docs.switchboard.xyz/docs-by-chain/solana-svm/price-feeds/basic-price-feed

Execution semantics and incidents:
- https://docs.jito.wtf/lowlatencytxnsend/ (first-party, bundle atomicity)
- https://docs.marginfi.com/ (first-party, flash loans)
- https://blog.loopscale.com/posts/postmortem (first-party post-mortem)
- https://www.cftc.gov/PressRoom/PressReleases/8647-23 (Mango Markets, CFTC)
- https://www.theblock.co/post/159975/solana-stablecoin-nirvana-sinks-90-amid-3-5-million-flash-loan-exploit (secondhand)
- https://www.coindesk.com/business/2024/05/16/solana-meme-coin-factory-pumpfun-compromised-by-bonding-curve-exploit (secondhand)
- https://www.theblock.co/post/347360/pump-fun-launches-dex-called-pumpswap-to-instantly-migrate-graduated-tokens (secondhand)
