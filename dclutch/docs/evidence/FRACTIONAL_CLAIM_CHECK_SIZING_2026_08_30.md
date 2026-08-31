# The fractional half of R3 — a hole in the narrowing, and what closing it costs — 2026-08-30

## Result

**Not closed, and not merely deferred: the narrowing had a hole, and the hole is
welded.** Claim-check compaction shipped this morning refusing one of the two
owner kinds that cannot sign. The other one is the Fractional reserve Position.
Admitting it meant that past the deadline any caller could convert every shard
holder's collateral into a record nobody can ever open — turning R3's *delay*
into a *total loss* for exactly the holders §10 of the design said it was
leaving alone.

The fractional half of R3 remains open, as it was. It is now sized, with
measured numbers, and its real blocker turns out not to be the one the design
anticipated.

## The finding

`RedeemClaimCheck` pays the record's `owner` and requires that address to sign —
`0x5621`, plus the holder's signer role in the redemption frame spec. A
program-derived address cannot sign a top-level instruction and no CPI reaches
the route, so a claim-check minted for a PDA is collateral written to an address
that can never open it.

`ProtocolPositionOwnerKindV2` has three variants and two of them are PDAs:

| kind | owner | can sign |
|---|---|---|
| `User = 1` | a wallet | yes |
| `TradingRecord = 0` | a Trading-program PDA | **no** |
| `ClaimsCapability = 2` | a Claims-program PDA | **no** |

`claim_check_compaction_v1.rs` refused `ClaimsCapability` and admitted
`TradingRecord`. The design's own §16.4 states the reason correctly and then
does not act on it: *"a Trading record or Claims capability owner is a PDA and
cannot sign"* — two kinds named, one refused. §4.7's precondition list makes the
same error explicitly, admitting `User` **or `TradingRecord`**.

**`TradingRecord` is the Fractional reserve.** The Fractional family asserts it
itself, in `fractional_retirement_v3.rs`:

```rust
if admission.owner_kind() != ProtocolPositionOwnerKindV2::TradingRecord
    || admission.position_owner() != input.root
```

where `input.root` is the Trading-owned Fractional capability root PDA — the
same account `fractional_atomic_v3.rs` requires as the reserve Position's owner
on both the open and the terminal path. That Position holds the collateral
backing every outstanding shard of one coordinate.

So the reachable sequence was: market goes terminal with shards outstanding →
anyone opens the escrow → the deadline elapses → anyone cranks the reserve. The
collateral moves to the vault, a claim-check is minted naming a PDA, and the
Position every shard holder's own redemption reads is closed in the same
transaction. Nobody can redeem the record. Nobody can redeem the shards. The
escrow can never close, because its outstanding counter never returns to zero.

Same class as the design's own §2 finding — value destruction by an arbitrary
actor with no stake — and arrived at by implementing §4.7 faithfully. Not
exploitable today: the deadline is 38,880,000 slots (~180 days) and the feature
landed this morning. It ships in the next claims-sbf cohort.

## The weld

The gate is now an exhaustive function over the enum rather than a comparison
against one variant:

```rust
const fn owner_kind_can_open_a_claim_check(kind: ProtocolPositionOwnerKindV2) -> bool
```

Exhaustive on purpose. A fourth owner kind has to answer *can this address sign
for its own payout* rather than inherit whichever arm it was written beside.
This is C0's own lesson from §15.5 applied a second time: a named admission with
one author, not a predicate each site maintains separately.

Nothing is stranded by refusing `TradingRecord`. A Trading-owned Position has
its own parent-authenticated close; the Fractional reserve has
`fractional_retirement_v3`'s ordered route. Compaction was never the only way to
retire either — only the only way to destroy them.

### Tests

| test | what it pins |
|---|---|
| `a_position_whose_owner_could_never_sign_for_it_is_not_compacted` | both PDA kinds refuse at `0x560A`, on the fixture the differential test compacts **successfully**, so the gate is the only thing refusing. Asserts the three things a partial refusal would leave behind: no claim-check minted, the position keeps every atom, the position is not closed out from under its claimants |
| `only_an_owner_that_can_sign_may_be_promised_a_claim_check` | the answer is stated over the whole enum, so a kind added later is a compile error and not a silent third `true` |
| `exactly_one_owner_kind_is_a_wallet_and_the_other_two_are_program_derived` | the reason the gate reads the way it does, kept as an assertion a future edit has to argue with |

17 of 17 claim-check campaign tests pass against a freshly built claims-sbf ELF,
0 SBF frame diagnostics across the six ELFs the campaign loads. The 16
pre-existing tests were unchanged by the weld.

## Sizing the fractional close

### The arithmetic is kinder than §10 assumed

§10 sketched "a pro-rata entitlement per shard". There is no pro-rata.
`divide_exposure_shards_v2` is the sole quotient/remainder boundary, and the
payout below it is a multiplication by a per-coordinate constant:

```text
whole_claims     = shard_atoms / denominator     (floor, the only division)
consumed         = whole_claims * denominator    (burned)
change           = shard_atoms - consumed        (stays in the holder's account)
collateral_atoms = whole_claims * payout_per_claim[coordinate]
```

A fractional claim-check storing `denominator` and `payout_per_claim` therefore
pays to the atom what on-time redemption would have paid, from the same two
numbers and the same two operations — no second rounding boundary, no remainder
ledger, no last-burner residue. Sub-denominator dust is not a claim on
collateral before compaction (`NoWholeClaim` is a refusal, not a zero payout)
and must not become one after.

**Record-shape decision: a second record type**, `[FRACTIONAL_CLAIM_CHECK_SEED,
aggregate, shard_mint]`, sharing the escrow, vault, deadline and close gate
unchanged. The new fields are 20 bytes and would fit `ClaimCheckV1`'s 24
reserved body bytes — the decision is not about space. `entitlement_atoms` means
"paid once, then the record closes" natively and "the remaining balance, paid
down across many burns" fractionally; the lifetimes differ; and one width
meaning two field layouts turns the hostile decode into a union whose arms
diverge after the kind byte.

### Compute is not the blocker

Measured in this campaign, same fixture, same build:

| transaction | CU |
|---|---|
| the holder's own wallet payout | 472,599 |
| compaction of that same position | 503,554 |
| claim-check redemption | 20,958 |

Compaction costs **30,955 CU** more than the redemption it stands in for — the
record write, the close, the four-credit split, the plan, the escrow update.
Fractional terminal settlement's own measured table
(`program-test/fractional-atomic/tests/fractional_atomic.rs`) is

```text
width   8    16   32   48   64    96     98     99
units 463k 519k 593k 731k 897k 1356k  1393k  exhausted
```

so a fractional compaction at the supported width 64 lands near **928k of
1,400,000** — 6.6% over the settlement, inside the 503k headroom already there.
Frame: fractional terminal is 44 accounts, plus compaction's 6 is **50**, under
devnet's 64-lock limit, over the ALT these campaigns already serialise through.

### The blocker is that fractional retirement is not reachable at all

`fractional_retirement_v3.rs` dispatches only `RetireCoordinate`. It refuses
`FractionalRetirementActionV3::Begin` and `::Finish` outright, and no program
calls either — `FractionalRetirementCursorV3::begin` and `::finish` exist in the
contract with tests, but nothing on chain can create or close the cursor PDA the
coordinate walk advances.

**So a fractional claim-check would make `RetireCoordinate`'s gates satisfiable
and the market still would not retire.** Wiring `Begin`/`Finish` is its own lane
and is strictly upstream.

Two more structural items, both real and neither a surprise:

- `RetireCoordinate` requires the shard mint's supply to be exactly zero, twice.
  A compacted coordinate has nonzero supply by construction — the outstanding
  shards *are* the durable claim — so the gate needs a compacted arm and the
  mint must survive retirement. That adds one perpetual mint per unredeemed
  coordinate to the residue. Debt, named, not absolved.
- Shards live in ordinary holder-owned Token accounts. No crank can burn them
  and none should be able to. That is why the mint becomes the claim record
  rather than being retired: the claim-check answers to the instrument, and the
  holder redeems by burning, with their own signature, forever.

### Estimate

**One lane, eight commits, sequenced after the `Begin`/`Finish` lane.** Record
type and seeds; two conservation plans; a refusal sub-band (`0x5640` and
`0x5660` are free); the compaction route; the burn-and-pay redemption route; the
`RetireCoordinate` arm; the campaign; the operator surface. The expensive half
of the native lane — building a terminal fixture — is already paid:
`fractional-atomic` drives terminal redeem and zero-burn against real ELFs
today.

## Census accounting

R3 stays **narrowed, not closed**: closed for native positions, open for
fractional ones. What changed is that the narrowing is now safe. Before this
weld, "narrowed" was too generous a word — compaction had made the fractional
half strictly worse than the row it was narrowing.

## Not verified

- No devnet write, and no run of the `claims-extended` gauntlet witness set;
  the campaign was run directly with a scratch `SBF_OUT_DIR`.
- The reserve Position was **not** driven through a real
  `protocol_position_v2::Admit` — the campaign's fixture gap named in §16.5
  still stands, so the owner-kind tag is planted with the production codec
  rather than written by the Admit route. The gate reads exactly that one field,
  so the test exercises what the route reads; it does not prove the Fractional
  wrap writes `TradingRecord` on a live chain. `fractional_retirement_v3.rs`'s
  own admission join is the tree's assertion that it does.
- The fractional compaction CU figure is an addition of two measured numbers
  (897k + 31k), not a measurement of a route that does not exist.
- No fractional claim-check code was written. This lane welded a gate and sized
  the work.
