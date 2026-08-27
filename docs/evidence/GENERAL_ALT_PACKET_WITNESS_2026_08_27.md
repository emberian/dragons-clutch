# General — the ALT-backed v0 packet witness, 2026-08-27

`docs/evidence/GENERAL_ACCELERATOR_CAMPAIGN_2026_08_27.md` closes on a standing
gap:

> The N=258 account sets need the ALT-backed v0 plan **measured**, not
> asserted. Until some campaign submits those six actions as v0 transactions
> and records their real wire size, the sentence "the production operator
> separately proves the same account set packet-safe" — which the suite's own
> comment carries — is a claim without a witness.

This is the witness. It is a **construction** measurement, not an execution
measurement: it records the exact wire bytes the operator's own compiler
produces for the real account sets. It is not local-validator, devnet, or
mainnet evidence, and it does not submit a transaction.

## What was missing

`crates/dclutch-operator/src/general_hot_v3.rs::compile_general_hot_v0` is the
plan the comment names, and it was already tested — against
`fn report(outcome_count, data_bytes)`, a fixture that fabricates ninety-one
metas and carries `outcome_count` as a struct field that moves no geometry.
The N=258 in that test was a label. No code path anywhere took a real General
account set through the ALT compiler.

Separately, `build_general_hot_instruction_v3` — the function that assembles
the real frame — has no caller and had no test at all.

## The geometry, and where every number comes from

A General Hot instruction is

```text
HOT_FIXED_ACCOUNT_COUNT_V3 (39, exactly one writable: the composite root)
  + ADMITTED_AOT_FIXED_EXTRAS_V3 (8) + one caller authority per scratch page
  + one account per physical AccountProfile coordinate past the five the
    fixed frame already carries
```

and its data is `HOT_FAMILY_REQUEST_OFFSET_V3` plus the exact 64-byte
`ControllerRequestV2`.

Nothing in the witness is a count copied from a table. The scratch-page span is
whatever `classify_bank_transport_v2` selects for General's own bank width
(`general_hot_scalar_count_v3` × 8 + `GENERAL_HOT_COMMON_IDENTITIES_V3` × 32).
Every runtime account, its privileges, and whether it is an alias of an earlier
coordinate come from `general_account_profile_rule_v3` — the generator that
produces the artifact the executor authenticates.

Account **identities** are synthetic, and cannot be otherwise without a founded
General capability. They do not enter a wire size: a v0 packet is a function of
the account count, the static/looked-up split, the signer count, and the data
width, and all four are derived above.

## The control

The derivation reproduces, exactly, all seven instruction-account counts the
real-ELF campaign recorded at N=258 — numbers produced by
`solana-program-test` execution of `dclutch_general_accelerator_sbf.so`, with
no input from this crate:

```text
2 harness accounts + ADMITTED_RUNTIME_ACCOUNTS_START_V3 (18) + logical coordinates
  Consider 47 · Freeze 45 · InitializeSettlement 102 · Collect 83
  Materialize 81 · Distribute 83 · Close 100
```

The accelerator frame carries one account per *logical* coordinate; the Trading
Hot frame carries the *physical* account once, so the two differ by exactly the
alias count — 0 for `Consider` and `Freeze`, 9 for the three per-order
settlement actions, 23 for `InitializeSettlement`, 27 for `Close`. For the two
alias-free actions the frames agree outright, and the test asserts that too.

The derived scratch-page count is **3 at N=1 and 17 at N=258**, which is the
campaign's own measured column.

## Measurements

`crates/dclutch-operator/src/general_hot_v3.rs::every_action_is_alt_packet_safe_at_the_canonical_runtime_width`.
Legacy packet maximum **1,232 bytes**. One canonical lookup table per action,
holding every non-signer account that is not the Trading Program.

| action | accounts | writable | tx signers | looked up | **v0 wire (N=258)** | v0 wire (N=1) | legacy (N=258, campaign) |
|---|---:|---:|---:|---:|---:|---:|---:|
| `Consider` | 86 | 4 | 2 | 84 | **664** | 608 | 1,273 |
| `Freeze` | 84 | 4 | 2 | 82 | **660** | 604 | 1,207 |
| `InitializeSettlement` | 118 | 9 | 4 | 114 | **918** | 862 | 1,328 |
| `Collect` | 113 | 10 | 3 | 110 | **813** | 757 | 1,309 |
| `Materialize` | 111 | 9 | 3 | 108 | **809** | 753 | 1,275 |
| `Distribute` | 113 | 10 | 3 | 110 | **813** | 757 | 1,309 |
| `Close` | 112 | 13 | 3 | 111 | **811** | 755 | 1,294 |

The two rightmost columns are not the same transaction — the campaign measures
the accelerator's own readonly caller frame, this measures the Trading Hot
instruction — so they are set side by side to show the transports, not to
subtract.

**Every action fits.** The widest is `InitializeSettlement` at 918 of 1,232
bytes: 74.5%, with 314 bytes of headroom. The claim now has its witness.

## Two things the witness also establishes

**The table is load-bearing, not decorative.** The same
`InitializeSettlement` account set compiled as a v0 message with *no* lookup
table refuses `PacketTooLarge`
(`the_same_account_set_without_a_table_is_not_packet_safe`). The ALT is what
buys the margin; a General operator that forgets to publish one does not
degrade, it fails.

**Runtime width moves only the bank transport.** For every action,
`accounts(258) − accounts(1) = 2 × (pages(258) − pages(1))` — one strategy
caller authority and one runtime scratch page per page — and the signer and
writable counts do not move at all
(`the_runtime_width_moves_only_the_scratch_page_span`). So the packet cost of
widening a General Market is bounded by the transport, not by N.

## What this does not discharge

- It is not execution. No General hot action has run through
  `hot_v3::process_hot_execution_v3`, because no General capability root can
  exist yet — see [Decision 0006](../decisions/0006-family-neutral-hot-dispatch.md) §7.
- `general-accelerator/process_instruction` stays NEVER-EXECUTED in the census.
  This witness closes the campaign's packet clause for the *operator's* plan;
  it does not make the ProgramTest campaign a fast lane, because that campaign
  still submits legacy messages.
- The N=1 fast lane the campaign describes remains admissible and unbuilt.
