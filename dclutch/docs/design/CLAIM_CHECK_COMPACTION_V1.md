# Claim-check compaction — the perpetual claim without the perpetual market

**Head current at `330bbfaba` (2026-09-04), tree root `/Users/ember/dev/dclutch`; real-ELF campaign evidence, not devnet and not mainnet evidence.**
The body below `## History` is the design (§1–§14), its six ratified amendments (§15), the implementation record (§16) and the fractional half (§17), verbatim; several later sections correct earlier ones, and this head states only the survivors.

## What is true now

- **The native half is built, C0 through C10** (`crates/dclutch-claims-svm/src/claim_check_*.rs`,
  `programs/dclutch-claims-sbf/src/claim_check_{compaction,redemption}_v1.rs`):
  the `begin_retiring` weld (C0), the funded permissionless crank, the payout,
  the supply debit, the close and the split, `RedeemClaimCheck`,
  `CloseClaimCheckEscrow`, the end-to-end campaign and the holder's operator
  surface. A terminal market's unredeemed native positions become fixed-width,
  permanently redeemable claim-checks whose payout is already resolved in
  collateral atoms.
- **All six amendments shipped as written**: a zero-atom claim-check is refused
  at the constructor, across the wire and in conservation (`claim_check_rent ==
  0` iff `entitlement == 0`); the crank is paid before the opener (rent → crank
  → opener → residue — under the design's order the first crank paid itself
  nothing); aliased sinks fold by identity; the fee tolerance is confirmed
  unreachable and left in; C0 keeps its better shape; compaction embeds the
  terminal header verbatim. Two departures: the vault is a Claims-derived PDA,
  not an ATA; C5 and C6 landed as one commit.
- **§4.7's owner-kind precondition was wrong and the shipped route refused one
  of two kinds** (§17.1): `TradingRecord` — the Fractional reserve Position —
  was admitted though a PDA can never sign the redemption. Corrected by the
  fractional half.
- **The fractional half is built and ran** (§17.2–§17.10): a second record for
  mint-held claimants, the burn executed rather than argued, the frame measured
  at fifty accounts with the Rent program pinned from the reserve Position's
  admission and its own refusal `0x564D Rent`, the route at 579,240 CU against
  a ~928k projection, and the ruled fiftieth account read off the chain.
- **R3 is narrowed, not closed**: closed for native positions, and the
  fractional route is what closes it for mint-held ones. Owed: §6.2's
  dust-tolerant close receipt (the settlement's own receipt is the evidence
  today), a real `protocol_position_v2::Admit` caller in the campaign, and a web
  surface for the holder's path.

## History

# Claim-check compaction — the perpetual claim without the perpetual market

Status: **PARTLY IMPLEMENTED — read §15 before acting on §4.7 or §6.2, and
§17 before acting on §4.7's owner-kind precondition or §10's sizing.** The
contract layer has landed and carries six ratified amendments, recorded in §15
with their arithmetic. Two of them correct passages that are wrong as written:
§4.7 step 4 mints a claim-check unconditionally when it must not (§15.1), and
§6.2's rent split does not close arithmetically (§15.2). Both original passages
are left unaltered in place, marked, so each correction can be read against what
it corrects.

Sections 1–14 below are the original design, decision-ready and
implementation-ready. It exists so a later implementation lane can build the
whole feature without making a design judgement call. Where a choice was
available, this document makes it and says why the alternative was rejected.
Every factual claim carries a `file:line` and was read at the route, not taken
from a doc.

Charter item: **3 — permissionless completion universalized** (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:39`).
Census row **R3**, with **R13** partly subsumed, **Q6** folded in, and **one new
RED finding** (§2) that the census did not have.

Paths are `~/dev/dclutch` unless stated otherwise.

---

## 0. What this document decides

1. A terminal market's unredeemed positions become **claim-checks**: small,
   fixed-width, permanently redeemable records, one per holder, whose payout is
   **already resolved in collateral atoms** at the moment of compaction (§1.4 is
   why it cannot be otherwise).
2. Compaction is **permissionless and funded** — a crank anyone may turn, paid
   from lamports the position was going to surrender anyway.
3. It opens after a long deadline that is a **release constant**, not a founder
   field and not a governance field (§5).
4. The heavyweight machinery all closes. What survives is one escrow record, one
   token vault, and one claim-check per unredeemed holder — a residue
   proportional to unredeemed claims rather than to the market, which
   **shrinks to zero** as holders redeem (§6.4).
5. **A new RED finding ships first, as commit C0**: today anyone can
   permanently destroy every holder's redemption right with one transaction
   (§2). Compaction cannot work in `Retiring` until that is welded, and the weld
   is independently correct.
6. **Q6** is unblocked rather than merely sequenced after (§7). **R13**'s
   retirement half is subsumed, its admission half deferred (§8).
7. **Exactly one ELF changes: claims-sbf** (§12). That is a designed property,
   not luck — three separate decisions protect it, and each is flagged where it
   is made.

Deliberately out of scope: fractional-shard-backed positions. §10 names that as
remaining debt, with the shape its fix takes.

---

## 1. The problem, exactly

### 1.1 Two gates, no clock

**Zero outstanding supply** — `programs/dclutch-claims-sbf/src/market_closure_v1.rs:669-681`,
inside `authenticate_empty_aggregate` (`:636-693`):

```rust
    let mut claim_index = 0;
    while claim_index < market.claim_count {
        if market.supply(&bytes, claim_index)
            .map_err(|_| ClaimsMarketClosureSbfErrorV1::Identity)? != 0
        {
            return Err(ClaimsMarketClosureSbfErrorV1::Liability.into());   // 0x5503
        }
        claim_index = claim_index.checked_add(1)
            .ok_or(ClaimsMarketClosureSbfErrorV1::Liability)?;
    }
```

A per-outcome scan of the entire runtime-width supply vector — every one of
`claim_count` cells must be exactly zero. No tolerance, no dust threshold, no
write-off branch.

**An empty Hoard** — `programs/dclutch-custody-sbf/src/lib.rs:950`, in
`close_vault` (`:929-993`):

```rust
    if token.amount != 0 || vault_lamports != request.rent_lamports {
        return Err(CustodySbfError::TokenState.into());                    // 0x6006
    }
```

Neither gate is wrong; both are exactly right. A market must not retire while it
still owes someone collateral. The defect is that **no Clock is read anywhere on
this path** — verified absent from `market_closure_v1.rs` (its imports at
`:33-41` are `account_info, entrypoint, hash, program::set_return_data,
program_error, pubkey`), from all of `programs/dclutch-claims-sbf/src/`, from
`retire_v1.rs`, from `begin_retiring.rs`, from all three
`retirement_replay_handoff_v1.rs`, and from Custody's five-op path. So the gates
have no time dimension and therefore no bound.

One holder who loses a key holds the market open forever, and every downstream
recovery inherits the block: the aggregate's rent, the Custody replay rent
(R10/Q6), the RentCredit, the Core state, every capability.

### 1.2 Why the holder's own route cannot save this

Holder redemption is genuinely good — **GREEN-SELF**, owner-signed, caller is the
payee. For `CallerRole::Claims` the signature required is the position owner's
own wallet: `terminal_settlement_v3.rs:570-572` binds
`accounts[0].key.to_bytes() != input.owner`, re-checked at
`signed_delta_v3.rs:516-519`. Nothing about it needs fixing.

But a *right* is not a *liveness guarantee*. A route only its beneficiary can
call stalls when its beneficiary is absent, and everyone else — the founder
awaiting rent, the protocol, the next market — is hostage to one person's
attention.

### 1.3 Positions are never enumerated on chain

Nothing in the closure path iterates positions; the zero-supply scan reads the
*aggregate* vector only. A position lives at
`[b"dclutch:lbv2:position", aggregate, owner]`
(`crates/dclutch-claims-svm/src/protocol_position_v2.rs:31,242-248`) and is
discoverable only by knowing its owner. So compaction is necessarily
**per-position and driven from off chain**, and the on-chain design must make
each single-position crank independently correct and independently funded. There
is no batch to make atomic and no list to walk.

### 1.4 The forced fact: a payout cannot be re-derived after retirement

This determines the whole design; an implementer who misses it will build
something that cannot work.

The payout is computed by `encode_product_basis_terminal_signed_delta_v3`
(`programs/dclutch-claims-sbf/src/rational_terminal_v3.rs:199-230`; evaluator at
`crates/dclutch-claims-svm/src/product_basis_terminal_v3.rs:351-488`). Its
inputs:

| input | account read | fate at retirement |
|---|---|---|
| `market_bytes` | Claims aggregate | closed, or handed to Core as a 256-byte checkpoint and then closed (`market_closure_v1.rs:696-762`) |
| `position_bytes` | Position PDA | closed |
| `product_basis_bytes` | `linked_basis_record` | closed |
| `composition_exposure_bytes` | `graph_record` | closed |
| `hoard_before` | the Hoard token account | emptied and closed |

The aggregate additionally carries `custody_context`, `release_set` and
`basis_id` that redemption reads (`liability_basis_state_v2.rs:22-31`;
`terminal_settlement_v3.rs:209-231`). Every input is destroyed by the act the
claim-check exists to permit.

**Therefore a claim-check must store what the holder is OWED, in collateral
atoms, resolved once, at compaction time.** Compaction is not a deferral of
settlement. It *is* settlement, performed on the holder's behalf, into an escrow
only the holder can open.

### 1.5 The second forced fact: there is no terminal slot

`CoreState` carries `terminal_receipt: Option<Identity>` and **no slot and no
timestamp** (`crates/dclutch-market-core-codec/src/generated.rs:353-364`). It is
Lean-generated (`generated.rs:1`), so adding a field means a Lean edit,
regeneration, a re-proof, and — because `CoreState`/`Action` are a Core↔Trading
contract — a second ELF. That is the exact cost trap the census documented for
Q5 and it is not worth paying here.

The stamps that do exist are path-specific: `consumed_slot`, "Clock slot at
terminal consumption" (`crates/dclutch-resolution-codec/src/sponsored_push_v1.rs:569`),
is on the sponsored-success path only. No field is uniformly readable for "when
did this market go terminal".

Consequence, decided in §5.4: **the clock origin is established inside
claims-sbf**, by the permissionless act that opens the escrow.

### 1.6 What R13 adds

R13 makes the hostage-taking cheap and deliberate. Admission and close both
compare a keyless, off-curve, system-owned PDA's live balance to a
*caller-declared* snapshot — Admit at
`programs/dclutch-claims-sbf/src/protocol_position_v2.rs:1043-1044`:

```rust
        || accounts.position.lamports() != request.observed_position_lamports
        || accounts.admission.lamports() != request.observed_admission_lamports
```

and Close at `:597-598` in the same `!=` shape. Anyone may send one lamport
between snapshot and landing and force a refusal, every slot, forever. **Close is
on the retirement path.**

---

## 2. The new finding: `begin_retiring` destroys redemption, and anyone can call it

This was not in the census. It was found while establishing which phase
compaction must run in, and it is worse than R3.

**`begin_retiring` is permissionless and refuses all signers** —
`programs/dclutch-core-sbf/src/begin_retiring.rs:57`:

```rust
    if accounts.iter().any(|value| value.is_signer)
```

Five accounts, no Clock, no payer, no rate limit (`:13-19`). Module doc:
*"Permissionless terminal-to-retiring transition under current Core release."*

**Every holder redemption route gates the Core phase on exact equality with
`Phase::Terminal`.** The shared check is `core.phase != expected_core_phase`
(`programs/dclutch-claims-sbf/src/affine_batch_v2.rs:669`), and both Hoard-payout
routes pass `Phase::Terminal`: `terminal_settlement_v3.rs:410` and
`rational_terminal_v3.rs:262`. Grep-verified across `programs/dclutch-claims-sbf`
and `crates/dclutch-claims-svm`: the only site accepting `CorePhase::Retiring` is
`rational_lifecycle_v2.rs:609`, which is capability *deactivation*, not
redemption.

**So: anyone, for one transaction fee, moves a Terminal market to Retiring, and
every holder's redemption refuses from that instant forever.** Retirement then
also cannot complete, because zero supply is unreachable. The market is bricked
in `Retiring` and the collateral is unreachable by anyone. That is value
**destruction** by an arbitrary actor — the same class as Y3b (funded
anti-liveness), which LIVE-2 welded at `04f00387`, and strictly worse than R3's
delay.

**Two independent facts say this is a regression, not a decision:**

1. The transition's own codec doc comment is
   *"Begin retiring while retaining permissionless redemption."*
   (`crates/dclutch-market-core-codec/src/generated.rs:1030`.)
2. `phases_join` **already admits the pair**
   `(CorePhase::Retiring, EconomicPhase::Retiring(w))`
   (`programs/dclutch-claims-sbf/src/lib.rs:1204-1213`), so the Claims phase
   model expects redemption to survive `Retiring`.

And the comment beside the hardcoded argument reasons only about why `Open` is
unsatisfiable (`terminal_settlement_v3.rs:406-409`) — `Retiring` is never
considered. Both the semantics and the phase model expect two-phase tolerance;
only the routes' hardcoded argument does not.

**The weld (commit C0, §11).** Redemption's phase gate becomes a two-arm match
admitting `Terminal` and `Retiring`, restoring the documented intent. It is small,
it is independently correct, and it is a **precondition of this design**:
compaction must run in `Retiring` (§4.3), and it uses redemption's own derivation
(§4.6), so without C0 the derivation refuses in exactly the state a prematurely
retired market is stuck in.

Safety of admitting `Retiring`: `market_closure_v1` requires
`core.phase == Phase::Retiring` (`:587`) and zero supply, and redemption is what
drives supply toward zero; once a cell is zero the derivation refuses anyway
(`product_basis_terminal_v3.rs:416-424` refuses
`selected_supply < quantity || selected_balance < quantity`). There is no race
with a consequence.

**This finding should be owned independently of this design.** It is posted to
the wave board. It also refutes the premise of the census's Q6 Correction 4 —
"redemption must stay possible during Retiring" is stated there as a constraint
on the gate, but today redemption is *not* possible during `Retiring`.

---

## 3. Ruled constraints

Ember's ruling, verbatim (`docs/ledger/WAVE_2026-08-26_to_2026-09-02.md:1183-1186`):

> Q3: option (c) ratified — perpetual CLAIM, not perpetual account:
> post-deadline compaction to a durable claim-check; market accounts close;
> the holder's right survives redeemable forever. No arbitrary actor may
> insert arbitrary delays into protocol operations.

And the rationale as given to this lane:

> liveness issues aren't ok, we can't be allowing random arbitrary actors to
> insert arbitrary delays into our own operations.

And on the follow-on (`docs/ledger/WAVE_2026-08-26_to_2026-09-02.md:1186-1187`):

> Q6: CloseReplay gated on the terminal receipt, shaped by Q3(c).

Four constraints fall out, and every section is answerable against them:

- **C1 — The claim survives forever.** No expiry on the claim-check, no escheat,
  no sweep to a beneficiary. A holder returning in twenty years is paid.
- **C2 — The market's accounts close.** "Perpetual claim, not perpetual account"
  is a *scope* instruction: the residue must be proportional to unredeemed
  claims, not to the market's machinery (§6.4 states it exactly).
- **C3 — No arbitrary actor may insert arbitrary delay.** This binds in every
  direction and is the sharpest tool here. It rules out the sleeping holder, the
  1-lamport griefer, the `begin_retiring` griefer of §2, *and* — the non-obvious
  one — **a founder setting a long deadline** (§5.2).
- **C4 — Q6 is shaped by this, not merely ordered after it** (§7).

Wave rules that bind the implementation: named-file commits only
(`git commit --only -- <paths>`), never `git add -A`, never `git stash`,
unsigned commits fine; and never an unfiltered `-p <crate>` test run.

---

## 4. The design

### 4.1 In one paragraph

After a terminal market has been redeemable for a long, release-fixed number of
slots, **anyone** may create a per-market **claim-check escrow** (a small record
plus a token vault), and then, one position per transaction, **anyone** may crank
each unredeemed position through the *same* payout derivation the holder's own
redemption would have used — sending the resulting collateral atoms to the vault
instead of the holder, writing a fixed-width **claim-check** whose address is
derived from the position's own seeds, debiting the aggregate's supply exactly as
redemption does, and closing the position and admission accounts. The crank is
paid from the position's recovered rent. When the last position is compacted,
supply is zero and the Hoard is empty, so retirement proceeds by its existing,
unmodified gates. The holder redeems from the claim-check whenever they like,
forever, with their own signature, against a vault that outlives the market.

### 4.2 The three new accounts, and the decision that keeps this to one ELF

| account | address | lifetime |
|---|---|---|
| `ClaimCheckEscrowV1` | PDA `[CLAIM_CHECK_ESCROW_SEED_V1, aggregate]`, claims-owned, ~128 B | open → last redemption |
| escrow vault | **the associated token account of the escrow PDA** for the collateral mint | same |
| `ClaimCheckV1` | PDA `[CLAIM_CHECK_SEED_V1, aggregate, owner]`, claims-owned, ~168 B | compaction → redemption |

`aggregate` is the **Claims aggregate account's pubkey** — confirmed as the
position's own seed material: `protocol_position_v2.rs:1027,1060` pass
`accounts.market.key.to_bytes()`, and the aggregate itself derives from the
logical market at `:904-908` under `LIABILITY_BASIS_MARKET_SEED_V2 =
b"dclutch:lbv2:market"`. PDA derivation needs only the address, never the
account, so all three derive correctly after the aggregate is gone. That is what
makes redemption survive retirement.

**Decision 1 of 3 that keeps the blast radius to one ELF: the vault is not a
Custody compartment.** `CompartmentV1` is a fixed enum
(`crates/dclutch-custody-contract/src/lib.rs:181-206`, `None=0, External=1,
Settlement=2, HoardPrincipal=3, TradingPrincipal=4, FeeVault=5, LivenessVault=6,
SeriesEscrow=7, RecoveryReserve=8`), and adding a variant is a custody-contract
plus custody-sbf change. It is unnecessary: the Hoard→vault move is a Custody
`Transfer` (`OperationV1::Transfer = 2`) with
`source_compartment: HoardPrincipal, destination_compartment: External` — which
is **exactly what the holder payout does today**
(`rational_terminal_v3.rs:342-343`). From Custody's side the vault is just
another external token account. **No Custody change at all**, and the transfer is
the existing `execute_terminal_custody_v3` (`:328-435`) with the recipient
swapped.

### 4.3 Which phases compaction runs in

**`Terminal` and `Retiring`, both.** `Terminal` is the ordinary case. `Retiring`
is mandatory, for two independent reasons: `market_closure_v1` requires
`core.phase == Phase::Retiring` (`:587`), so a market must be in `Retiring` for
closure to be attempted at all; and §2's griefer can put a market there
unilaterally, so a compaction that refused in `Retiring` would leave exactly the
bricked markets unrescuable. This is why C0 is a precondition and not a nicety.

### 4.4 Identity authorship — the position's own seeds, not a caller field

Design question (1), answered structurally by the tree.

```rust
pub struct ProtocolPositionSeedsV2 { aggregate: [u8; 32], owner: [u8; 32] }
// crates/dclutch-claims-svm/src/protocol_position_v2.rs:225-228
pub fn as_slices(&self) -> [&[u8]; 3] {
    [PROTOCOL_POSITION_STATE_SEED_V2, &self.aggregate, &self.owner]
}   // :242-248
```

The position's **address is a proof of its owner**, and the owner is also
persisted in the header at offset 56 (`liability_basis_state_v2.rs:36`,
`POSITION_OWNER_OFFSET`). So the compaction route accepts no holder identity as a
wire field. It takes `(aggregate, owner)` as coordinates, re-derives the position
PDA and requires the passed account to be that address, re-derives the
claim-check PDA from *the same two seeds* under `CLAIM_CHECK_SEED_V1`, and writes
the owner from the seeds. A caller naming the wrong owner derives an address that
is not the account they passed, and the route refuses before touching anything.
**The wrong-holder hostile is closed by derivation, not by a check.**

This is the SER-POL discipline, and the tree states it in one sentence
(`programs/dclutch-trading-sbf/src/series/account_profile_v4.rs:128-132`):

> The root's own immutable header is the sole author of the root PDA derivation
> … a recipe built from any other source would be a second author for the
> derivation.

Mirror `ProtocolPositionSeedsV2::new`'s refusals exactly rather than restating a
weaker set: both identities nonzero and `aggregate != owner` (`:233-237`).

### 4.5 The wire

Magics — the tree's live `DCLT****` namespace was enumerated and these five are
free:

| constant | value | tags |
|---|---|---|
| `CLAIM_CHECK_OPEN_MAGIC_V1` | `DCLTCCO1` | open-escrow request |
| `CLAIM_CHECK_COMPACT_MAGIC_V1` | `DCLTCCC1` | compaction request |
| `CLAIM_CHECK_REDEEM_MAGIC_V1` | `DCLTCCR1` | redemption request |
| `CLAIM_CHECK_RECORD_MAGIC_V1` | `DCLTCCK1` | the `ClaimCheckV1` account |
| `CLAIM_CHECK_ESCROW_MAGIC_V1` | `DCLTCCV1` | the `ClaimCheckEscrowV1` account |

**Naming hazard:** "compact" already means something unrelated —
`RationalLifecycleCompactHotRequestV4`
(`crates/dclutch-rational-lifecycle-hot-v3/src/compact_operator_v4.rs:22`) is
hot-path *request* compaction. Name every symbol `ClaimCheck*`, never `Compact*`
alone.

Follow the house record layout exactly. `LifecycleRentCreditV2` is the closest
analogue and the one to copy (`crates/dclutch-rent-contract/src/lifecycle_v2.rs:41-48`):
8-byte ASCII magic at 0, `u16` version at 8, a one-byte discriminant/bump at 10,
zeroed reserved runs, 32-byte identities on 32-byte boundaries from 16, `u64`s at
the tail; decode is `require_header` plus `require_zero` per reserved run
(`:200-217`). The hostile-decode order to match is
`SourceClosureReceiptV2::decode` (`crates/dclutch-resolution-codec/src/v2.rs:464-495`):
`exact_width` first, then magic, version, kind, every `require_zero`, then
field-by-field.

`ClaimCheckV1` — fixed width, **independent of market width**:

| field | type | source |
|---|---|---|
| magic / version / kind / bump / reserved | — | constants |
| `aggregate` | `[u8; 32]` | seed |
| `owner` | `[u8; 32]` | seed |
| `market` | `[u8; 32]` | logical market, for audit |
| `release_set` | `[u8; 32]` | the market's pinned release set |
| `vault` | `[u8; 32]` | the escrow vault address |
| `collateral_mint` | `[u8; 32]` | authenticated against the Realm |
| `entitlement_atoms` | `u64` | **the observed vault credit** (§6.3) |
| `position_atoms_digest` | `[u8; 32]` | hash of the per-cell atom vector |
| `compacted_slot` | `u64` | clock at compaction |
| `generation` | `u64` | market generation |

**Why a digest instead of per-cell atoms.** The mission's sketch put per-cell
atoms in the record. They are not needed to pay — §1.4 settled that the payout is
resolved at compaction, so the atoms are *evidence*. A position's atom vector is
`u64[claim_count]` at stride 8 from offset 128
(`liability_basis_state_v2.rs:98-100`), i.e. width-sized; storing a 32-byte
digest instead makes the record fixed-width, which buys three things that matter:
rent is predictable and provably covered by the position's own rent for markets
of any width (§6.2), redemption CU is constant, and a 256-outcome market's
claim-check costs the same as a binary market's. The evidence survives — an
indexer holding the position bytes can prove the vector against the digest — and
nothing on chain needs the vector again.

### 4.6 Route 1 — `OpenClaimCheckEscrow` (`DCLTCCO1`)

Permissionless. Copy `begin_retiring`'s frame discipline
(`begin_retiring.rs:48-62`) except for the payer.

Preconditions: `CoreState.phase` is `Terminal` or `Retiring`;
`terminal_receipt.is_some()` checked explicitly even though the phase invariant
implies it (`generated.rs:379-383`) — a checked invariant is one an implementer
cannot silently delete; the escrow does not already exist.

Effects: create `ClaimCheckEscrowV1`, stamping `opened_slot = Clock::slot`,
`opener`, `opener_outlay`, `outstanding_claim_checks = 0`; create the vault as the
escrow PDA's associated token account, with the mint authenticated equal to the
Realm's `collateral_mint` and the token program equal to the Realm's collateral
token program.

The clock starts here and nowhere else (§5.4).

### 4.7 Route 2 — `CompactPositionToClaimCheck` (`DCLTCCC1`)

Permissionless, funded, **one position per transaction** — forced by §1.3 and by
CU: the derivation allocates four width-sized scratch vectors plus a packet
(`rational_terminal_v3.rs:174-182`).

Preconditions:

- `Clock::slot >= escrow.opened_slot + COMPACTION_DEADLINE_SLOTS_V1`, inclusive
  `>=` — matching the deployed record-abort precedent
  (`crates/dclutch-record-contract/src/lib.rs:1699`), not the dealer
  checkpoint's exclusive `<=`-refuses (`scenario_checkpoint_v1.rs:711`). Else
  `Deadline`.
- Core phase `Terminal` or `Retiring` (§4.3).
- The position is at the address derived from `(aggregate, owner)`, non-empty,
  claims-owned.
- The claim-check PDA is **vacant** — else `AlreadyCompacted`.
- Owner kind is `ProtocolPositionOwnerKindV2::User` or `TradingRecord`, not
  `ClaimsCapability` (`protocol_position_v2.rs:193-200`) — see §10.
  **AMENDED — admitting `TradingRecord` here is a value-destruction bug, and
  `TradingRecord` is the Fractional reserve Position. §17.1 carries the
  replacement. The line is left unaltered so the correction can be read against
  what it corrects.**

Effects, **in this order, and the order is forced**:

1. **Derive the payout by calling**
   `encode_product_basis_terminal_signed_delta_v3`
   (`rational_terminal_v3.rs:199-230`) with the inputs redemption passes, the
   only difference being the recipient. It must be *called*, never
   re-implemented: a second author for the payoff function is how a compaction
   that pays a different number than redemption would have gets built and passes
   its own tests. **This is the single most important instruction in this
   document.**
2. Read the vault balance; move the payout Hoard→vault via the existing
   `execute_terminal_custody_v3` (`:328-435`) with the vault as the `External`
   destination; read the vault balance again; set `entitlement_atoms` to the
   **observed delta** (§6.3).
3. **Debit the aggregate supply and the position row via the same signed-delta
   executor redemption uses.** This must happen *before* the close, because
   `signed_delta_v3::build_candidates` requires a live position at the canonical
   `(aggregate, owner)` PDA (`:758-790`, joins at `:771-788`) and
   `apply_coordinate` (`:827-846`) decrements the aggregate cell and the position
   cell by the same quantity. Supply cannot be retired without a live position
   row — there is no other hook.
4. Write `ClaimCheckV1`; increment `escrow.outstanding_claim_checks` — **but
   only when the entitlement is nonzero. AMENDED, see §15.1.** A position whose
   terminal payout resolves to zero atoms is compacted and closed without a
   record.
5. Close the position and admission accounts, sweeping their entire balances and
   splitting per §6.2. This must happen *after* step 3 for a second, independent
   reason: the existing close already refuses a non-zero balance vector —
   `protocol_position_v2.rs:586`, `|| balances.iter().any(|value| *value != 0)`.

**Dust tolerance is mandatory here and is not an optimisation.** The route must
never compare a live PDA balance to a caller-declared value for equality;
declared lamport figures are **floors** and the swept truth is what the receipt
records. §8 explains why this is what makes the design work.

### 4.8 Route 3 — `RedeemClaimCheck` (`DCLTCCR1`) — GREEN-SELF, forever

The holder signs; nobody else can call it and nobody else needs to. The point of
the design is what is **absent** from the frame:

| present | absent |
|---|---|
| the claim-check PDA | the Claims aggregate |
| the escrow record | the Core state |
| the escrow vault | `linked_basis_record`, `graph_record` |
| the holder's token account | the Hoard, the Realm, the Custody authority |
| mint, token program | **the Custody replay cursor** |
| the holder (signer) | any release-set or activation cache |

Effects: authenticate the signer is the record's `owner` and the record is at
`[CLAIM_CHECK_SEED_V1, aggregate, owner]`; transfer `entitlement_atoms`
vault→holder, signed by the escrow PDA; decrement
`escrow.outstanding_claim_checks`; close the claim-check, lamports to the holder.

**Anti-replay is the account's own existence.** The record is created once (a
non-vacant PDA refuses a second compaction) and closed on redemption. A closed
account cannot be redeemed; re-creating one would need a compaction crank, which
refuses because the position is gone. No cursor, no revision, no counter — which
is §7's entire argument.

### 4.9 Route 4 — `CloseClaimCheckEscrow`

Permissionless. Gate: `outstanding_claim_checks == 0`. Residual vault dust (a
transfer-fee mint can leave some, §6.3) sweeps to the caller with both accounts'
rent, which is what funds this crank — it needs no escrow of its own.

If a holder never redeems, this never fires and the vault stays open holding
their collateral. **That is the ruling working as intended**, not a gap: C1 says
the claim survives forever, and collateral has to be somewhere.

### 4.10 What retirement then does

Nothing new. After the last compaction, supply is zero and the Hoard is empty, so
`market_closure_v1.rs:669-681` and `custody-sbf/lib.rs:950` pass by their
existing, unmodified predicates. **This design adds no gate to the retirement
path and removes none.** It adds a second, permissionless way to reach a
predicate that previously had only one, holder-gated way to be reached.

One detail that stays true and should be asserted rather than assumed: the
closure receipt writes `liability_units = 0` as a hardcoded constant
(`market_closure_v1.rs:242`, enforced at `:264`). Because compaction retires
supply through real debits rather than writing it off, that constant remains
correct and no receipt field moves.

---

## 5. The deadline

### 5.1 Recommendation: a release constant, denominated in slots

`COMPACTION_DEADLINE_SLOTS_V1`, a `const` in the claims contract crate, compiled
into the claims-sbf ELF.

**Slots, not unix seconds.** The tree's split is consistent by layer: every
expiry/cleanup route that pays a caller is slot-based (record staging
`record-contract lib.rs:820,1035`; dealer checkpoint
`scenario_checkpoint_v1.rs:146-149`; founding permit `series_permit.rs:76`),
while unix seconds appear only where an off-chain proposition or a cross-cluster
observation is being timed. The one written ruling agrees
(`docs/design/MAINNET_STATE_RELAY.md:1027-1044`): *"They are liveness deadlines,
not claim semantics, and devnet time is the right clock for 'has this market
waited long enough.'"* This is a liveness deadline.

**Recommended value: 38_880_000 slots ≈ 180 days** at ~2.5 slots/second
(216,000 slots/day). The arithmetic is stated so it can be argued with. The job
of the value is that "no honest holder is plausibly asleep" holds for a person
who checks their positions twice a year, and being generous is cheap because §5.5
shows the holder loses almost nothing when compaction fires.

Being a release constant gives ember's property with **no new mechanism**:

- **Founder-visible at founding**: a market pins a release set at founding, which
  pins the claims-sbf ELF digest, which pins the constant. The founder reads the
  deadline by reading the release they found on.
- **Never shortenable post-founding**: changing the constant needs a new ELF,
  hence a new release set; a live market's `selected_release_set` is write-once
  with no re-point route (R1), so today shortening a live market's deadline is
  not merely forbidden but structurally impossible.

The tree already states this doctrine for the analogous case
(`crates/dclutch-record-contract/src/lib.rs:395-399`):

> Keeping limits outside the cursor permits a release to tighten future Begin
> admissions **without changing an in-progress record**.

### 5.2 Why not founder-set — the griefing analysis

Rejected, on ember's own words.

The hostile is a founder setting a very *long* deadline — a hundred years — which
re-creates R3 with a named beneficiary. Against C3 that is not a grey area: **a
founder choosing when other people's markets may retire is an arbitrary actor
inserting an arbitrary delay into protocol operations.** The delay lands on
parties who never agreed to it — the RentCredit's `refund_wallet`, everything
waiting on closure, and the protocol's claim to have no liveness dependency on an
identified party.

A protocol-enforced band `[FLOOR, CEILING]` bounds the hostile without removing
it; it moves the founder's lever inside a smaller range while adding a wire
field, a founding-time validation, a weld to keep it immutable, and a hostile per
band edge. Four new surfaces for a lever nobody asked for.

**The contrary-looking precedent, addressed.** The record staging cursor *does*
take a founder-chosen expiry bounded by a release policy, checked once at
creation (`record-contract lib.rs:1244-1250`) and then structurally immutable
(the only successor constructor is struct-update over two fields, `:1377-1381`).
Why not copy it here? **Because the incidence of the delay differs.** A record's
expiry governs the sponsor's own staging, prepaid by the sponsor, and the delay
lands on the sponsor. A market's compaction deadline governs third parties who
never chose it. Same mechanism, opposite ethics — which is why the precedent is
right there and wrong here.

| alternative | why rejected |
|---|---|
| founder-set, unbounded | C3 directly: an identified party choosing others' delay |
| founder-set within a protocol band | same hostile, smaller; four new surfaces |
| governance-set, mutable | a mutable deadline *is* a shortening authority, and a new trusted party |
| derived from the market's `expiry_slot` | founder-chosen and unbounded above (`generic_founding_v1.rs:396-399` checks only that founding is *before* it) — founder-set in disguise |
| **release constant** | **selected** |

### 5.3 Devnet, and why there is no test override

180 days of slots is unusable in a campaign, and the temptation is a
`#[cfg(feature = "short-deadline")]`. **Do not.** A feature flag that shortens the
deadline is a shortening authority travelling with the build, and §5.1's whole
guarantee rests on the deadline being a property of the artifact everyone
verifies.

The correct mechanism already exists: **a devnet release compiles a different
constant**, with a different ELF digest and a different release-set identity,
visible to everyone who reads it. Devnet markets are founded on the devnet
release and get the devnet deadline. Suggested devnet value: **5_400 slots
(~36 minutes)** — long enough that a campaign genuinely exercises the wait,
short enough to fit a session.

### 5.4 The clock origin, and its hostiles

§1.5 established there is no readable terminal slot, so the origin is
`escrow.opened_slot`.

- **Started early?** No — the route refuses any phase before `Terminal`, so the
  earliest origin is the market going terminal, which is the honest one.
- **Delayed by an adversary?** No — the route is permissionless, so no actor can
  withhold the start from anyone else.
- **Nobody calls it?** The real residual risk, named rather than waved at.
  Opening costs rent and pays nothing at that instant, so on its own the open is
  **YELLOW** by the census's doctrine — permissible rather than live. Mitigation:
  the escrow records `opener` and `opener_outlay`, and **the first compaction
  crank repays the opener in full before paying itself** (§6.2). The open is
  therefore a funded position for anyone who intends to crank, and the party who
  intends to crank is the party who wants the rent — which every market has.
- **Is stamping at open rather than at terminal a shortening?** It is a
  *lengthening*: the wait runs from when someone noticed, which is at or after
  terminal. It can only ever be more generous to the holder, never less. State
  that asymmetry in the code comment.

**V2 simplification, recorded:** if a `terminal_slot` lands in `CoreState` for
other reasons, move the origin to it and let `opened_slot` go vestigial. Do not
add it for this feature alone — Lean edit, regeneration, re-proof, second ELF.

### 5.5 What a holder actually loses when compaction fires

Compaction does not take the holder's value; it moves their already-computed
payout into an escrow only they can open. What they lose is (1) a capped slice of
the position's rent that funds the crank — which was going to a `refund_wallet`
they do not control anyway — and (2) the convenience of the familiar route, since
their client must know about claim-checks.

That is the entire downside, and it is why the deadline's job is to prevent churn
and fee-farming on live markets rather than to protect against expropriation:
there is none to protect against. **This is the structural advantage of ember's
option (c) over the escheat option (a) the census recommended: under escheat the
deadline is load-bearing for value; here it is load-bearing only for
convenience.**

---

## 6. Conservation

Three ledgers must close, each stated as a **plan struct** whose `new()` refuses
to exist unless the movement balances, plus `validate_post()` against observed
post-balances. Canonical example `WorkEscrowClosePlanV1`
(`crates/dclutch-general-adapter-contract/src/escrow_v1.rs:660-737`), whose own
comment states the doctrine (`:676-679`):

> The conservation conjunct is the point: everything the account held is
> accounted for by the two credits, so a close cannot strand lamports in an
> account it is about to leave at zero length, and cannot pay out more than it
> held.

Both plan structs go in `dclutch-claims-svm`, pure, no `AccountInfo`; the SBF
program supplies observations and checks `validate_post`. Never inline arithmetic
in the program.

### 6.1 Atoms — supply

For each compaction with position quantity `q`:

```
supply_after == supply_before - q          (the identical debit redemption performs)
```

executed by `signed_delta_v3::apply_coordinate` (`:827-846`) against both the
aggregate cell and the position cell. Terminal statement: **when the last
position is compacted every cell of the supply vector is 0**, which is exactly
what `market_closure_v1.rs:669-681` scans for. The design adds no supply
arithmetic; it reuses redemption's debit, which is why the two paths cannot
diverge.

### 6.2 Lamports — the rent split, and the receipt that makes it possible

The position and admission accounts close, releasing
`R = position_lamports + admission_lamports`, swept in full today at
`protocol_position_v2.rs:1227-1229`. `R` splits **in this order**:

> **AMENDED — the order below is wrong and §15.2 carries the replacement.** It
> does not close arithmetically: for a binary market the first crank pays itself
> exactly zero, which is R3 returning through the funding door. The implemented
> order is rent, **crank**, opener, residue. The block is left here unaltered so
> the correction can be read against what it corrects.

```
claim_check_rent   := rent-exempt minimum for the fixed ClaimCheckV1 width
opener_repayment   := escrow.opener_outlay, if not yet repaid, else 0
crank_reward       := min(COMPACTION_CRANK_REWARD_LAMPORTS_V1,
                          R - claim_check_rent - opener_repayment)
rent_credit_residue:= R - claim_check_rent - opener_repayment - crank_reward

R == claim_check_rent + opener_repayment + crank_reward + rent_credit_residue
```

**Decision 2 of 3 that keeps this to one ELF, and it is load-bearing.** The
existing close cannot carry this split. `ProtocolPositionCloseReceiptV2::new`
binds an *exact* equality —
`rent_credit_before + observed_position + observed_admission == rent_credit_after`
(`crates/dclutch-claims-svm/src/protocol_position_v2.rs:876-885`) — re-checked on
decode at `:974-979` and `:1015-1017`, on `validate_request` at `:1028-1048`, and
as an on-chain postcondition at
`programs/dclutch-claims-sbf/src/protocol_position_v2.rs:1243`. That is five
sites, and those receipts are consumed by **trading-sbf, which is frozen**.
Therefore **the compaction close writes a new receipt type of its own** and never
touches `ProtocolPositionCloseReceiptV2`. The existing close path is left
byte-identical. This is also what keeps Q8b's receipt-ABI work off this design's
critical path (§8).

Two properties to preserve:

- **It cannot refuse for underfunding.** `ClaimCheckV1` is fixed-width and
  strictly smaller than a position (`128 + 8*claim_count`) plus an admission
  (512 bytes fixed, `protocol_position_v2.rs:15`) of any width, so
  `claim_check_rent < R` always. The reward is the *residual*, capped — never a
  demand — so a thin position yields a small reward rather than a refusal. **A
  compaction that could refuse for lack of funds would reintroduce R3 through the
  funding door.**
- **Nothing new is taken from the Hoard.** The crank is funded entirely from
  lamports already leaving these accounts. Today 100% flows to `RentCredit` and
  thence to a creation-fixed `refund_wallet` — an identified party. Redirecting a
  capped slice to the caller is the census's P1 upgrade verbatim ("pairing it
  with a caller-directed crank fee is what upgrades YELLOW to GREEN"), and the
  `refund_wallet` is the party who benefits most from retirement happening.

**The precedent to copy is the record `Abort` verb**, not the candidate escrow.
`WorkRewardV1` (`candidate_v1.rs:237-258`) is the census's canonical GREEN shape
and the plan structs should follow its discipline, but it has **no deployed SBF
dispatcher** — its only non-crate consumer is a harness test
(`programs/dclutch-general-accelerator-sbf/program-test/tests/lifecycle.rs`). The
tree's one *deployed* funded permissionless post-deadline crank is record Abort
(`programs/dclutch-registry-sbf/src/record_v1.rs`, contract
`crates/dclutch-record-contract/src/lib.rs:1699-1722`), and its shape is this
design's shape: inclusive `>=` expiry, permissionless after it, caller paid,
bounty floor chain-derived rather than caller-invented (`lib.rs:478-492`), post
check conservation and vacancy (`record_v1.rs:636-659`), and no signer required
once expired (`:562-568`). Read that route before writing this one.

`COMPACTION_CRANK_REWARD_LAMPORTS_V1` should exceed a priority-fee-inclusive
transaction cost by a comfortable margin; **suggested 200_000 lamports**,
reviewable against measured CU.

### 6.3 Collateral — atoms across Hoard → vault → holder

```
hoard_after       == hoard_before - payout
vault_credit      := vault_after - vault_before        (OBSERVED, not assumed)
entitlement_atoms := vault_credit
```

**`entitlement_atoms` is the observed credit, never the intended transfer.** The
collateral is an SPL/Token-2022 token account (the Hoard is exactly one token
account at
`[b"dclutch:custody-vault:v1", market, release_set, context, HoardPrincipal]`,
`crates/dclutch-custody-contract/src/lib.rs:390-401`), and a Token-2022 mint may
carry a transfer-fee extension, in which case `vault_credit < payout`. Recording
the observation rather than the intention means conservation holds against
reality at every hop and the holder is promised exactly what is there to pay
them. Redemption then moves exactly `entitlement_atoms`, and fee-induced residue
is swept by the escrow close (§4.9) rather than stranding.

This is the same discipline as the R13 fix — **observe the truth, do not assert a
prediction**. A route asserting `vault_after == vault_before + payout` would be a
fresh instance of the exact bug class the census spent a lane finding.

Terminal statement, and the invariant a hostile test should attack directly:
**`sum over live claim-checks of entitlement_atoms == vault balance`**, at every
instant between the first compaction and the escrow's close.

### 6.4 The residue, stated exactly

| | today (R3, one sleeping holder) | after this design |
|---|---|---|
| survives forever | aggregate, Core state, positions ×N, admissions ×N, `linked_basis_record`, `graph_record`, Hoard vault, Custody replay, RentCredit, every capability | 1 escrow record + 1 vault + 1 claim-check per unredeemed holder |
| shrinks? | **no** | **yes** — to zero, the last redemption enabling the escrow's close |
| size | tens of KB, width-dependent | hundreds of bytes, width-**in**dependent |

**Named as debt, not absolution:** the vault and the escrow record *are*
perpetual accounts for as long as any claim is unredeemed. This design does not
make that disappear. It makes the residue proportional to unredeemed claims
instead of to the market, and self-liquidating. That is the whole of the
improvement and should not be described as more.

---

## 7. Q6 folded in — the `CloseReplay` gate

Q6 posed a question it said must be answered before building:

> **Answer before building: does any Claims redemption's safety rest on the
> cursor, or only on position state?**

The hazard it found is real: Custody's only guard on `CloseReplay` is
`open_vault_count != 0` (`crates/dclutch-custody-contract/src/lib.rs:906-913`),
and claims-sbf never opens or closes a vault, so the guard is **vacuous for this
role** — an ungated closer would be a close-and-recreate primitive resetting
`next_revision` to 1. And ordinary redemption *does* read the cursor:
`expected_custody_replay_revision` at `rational_terminal_v3.rs:284`, consumed at
`:374` with `resulting_revision = +1` at `:375-378`.

**This design answers the question in the affirmative direction and removes the
hazard rather than fencing it.**

- Before the terminal predicate holds, ordinary redemption is live, the cursor is
  load-bearing, and `CloseReplay` must be refused.
- After the predicate holds — supply zero and Hoard empty, reached either by
  every holder redeeming or by compaction, indistinguishably — **no route that
  reads the cursor remains reachable.** Ordinary redemption needs the aggregate
  and the position; both are gone. Claim-check redemption (§4.8) reads no cursor
  at all, by construction, and its anti-replay is the record's own existence.

So the gate is **the existing terminal zero-supply predicate itself**
(`market_closure_v1.rs:669-681`) — one predicate, now reachable two ways — not a
new disjunction and not `Phase::Retiring`. Concretely the Q6 closer must require
the terminal receipt and the zero-supply predicate, and must be built **after**
this design, because before it the predicate is not reliably reachable and a
closer gated on an unreachable predicate is dead code with a hazard attached.

Note that compaction itself *does* advance the cursor — step 2 of §4.7 goes
through `execute_terminal_custody_v3`, which requires
`expected_revision == replay.next_revision` and advances by one
(`rational_terminal_v3.rs:374-378`, joined at `:586-598`). That is correct and
expected: the cursor is live during compaction and dead only after it.

The census's Correction 1 stands unchanged and is not softened: only claims-sbf
can mint a Claims-role caller authority, so the closer is a 300-400 line
standalone route. The exact seed shape it needs is
`CallerAuthoritySeedsV1::as_slices`
(`crates/dclutch-release-set-contract/src/lib.rs:301-311`) —
`[b"dclutch:role-authority:v1", release_set, market, caller_role, context,
role_request_digest]` — minted under the *caller's* program id, with the
`invoke_signed` pattern at
`programs/dclutch-claims-sbf/src/custody_replay_v1.rs:454-471`.

**This design does not make Q6 cheap. It makes Q6 safe, and it makes Q6's gate
reachable.**

Correction 4 of Q6, however, is now known to be **factually wrong about the
present**: it argues the gate must be the predicate rather than `Phase::Retiring`
"because redemption must stay possible during Retiring". Today redemption is
*not* possible during `Retiring` (§2). The conclusion is still right — gate on
the predicate — but the reason only becomes true once C0 lands.

---

## 8. R13 — what this subsumes and what it defers

**Subsumed: the retirement half.** Close is on the retirement path today, which
is why R13 scores RED-adversarial. After this design it is not the *only* path: a
position an adversary keeps griefing through ordinary Close can be compacted
instead, by a dust-tolerant route the adversary cannot block. R13's effect on
retirement degrades from **"blocks retirement forever, for ~1 lamport per slot"**
to **"delays this position's close until the compaction deadline, after which
anyone cranks it and is paid"**. A strand converted into a bounded funded wait.

**Conditional on one thing, and it is not optional:** the compaction route must
not itself compare a live PDA balance to a declared value for equality. Ship it
with the same `!=` shape and the same lamport blocks compaction too, and this
document delivers nothing. §4.7's dust-tolerance requirement belongs in a hostile
test, not a comment.

**Deferred: the admission half.** Blocking Admit griefs a holder *entering* a
market. Real, on no terminal path, untouched here. It stays **Q8b**, whose
corrected shape the census recorded: relabel `observed_*` as floors, record the
swept truth in the receipt evidence, re-check every consumer — noting the
five-site exact-equality binding of §6.2 and that those receipts are consumed by
frozen trading-sbf.

**Decision 3 of 3 that keeps this to one ELF, and the note Q8b's implementer
needs:** because the compaction close writes a *new* receipt type (§6.2), it is
free to be dust-tolerant immediately without touching the frozen consumer. Q8b's
harder half — the receipt-ABI change reaching trading-sbf — is not on this
design's critical path.

---

## 9. Hostiles

Every row is a test to write, not a risk to note.

Error codes follow the refusal registry
(`crates/dclutch-refusal-registry/src/lib.rs`, ADR
`docs/decisions/0007-namespaced-refusal-codes.md`): band = `code >> 12`, span
`0x1000`, Claims is band 5 with `CLAIMS_REFUSAL_BASE = 0x5000`
(`refusal-registry lib.rs:145-170`), one round sub-band per request family
(`:43-52`). Occupied in claims-sbf: `0x000, 0x100, 0x140, 0x160, 0x180, 0x200,
0x210, 0x260, 0x500`. **`0x600` and `0x620` are free tree-wide** and are taken
here. Each enum carries the two `const _: () = assert!` band pins in the house
form (`market_closure_v1.rs:90-102`).

**`claim_check_compaction_v1.rs` — block `0x5600`**

| code | name | hostile refused |
|---|---|---|
| `0x5600` | `Accounts` | frame shape, ownership, writability |
| `0x5601` | `Authority` | a signer the route does not admit |
| `0x5602` | `Identity` | **wrong-holder claim-check** — coordinates that do not derive the passed position; zero identity; `aggregate == owner` |
| `0x5603` | `Deadline` | **premature crank** — `slot < opened_slot + COMPACTION_DEADLINE_SLOTS_V1` |
| `0x5604` | `Phase` | **compaction during Open**; open-escrow with no terminal receipt |
| `0x5605` | `AlreadyCompacted` | **double compaction** — the claim-check PDA is not vacant |
| `0x5606` | `Conservation` | a plan whose atoms or lamports do not balance |
| `0x5607` | `Economic` | payout derivation failure |
| `0x5608` | `Receipt` | postcondition mismatch on observed balances |
| `0x5609` | `Escrow` | escrow absent, mint mismatch, wrong token program |
| `0x560A` | `Scope` | a position kind V1 does not compact (§10) |

**`claim_check_redemption_v1.rs` — block `0x5620`**

| code | name | hostile refused |
|---|---|---|
| `0x5620` | `Accounts` | frame shape |
| `0x5621` | `Authority` | **a non-holder redeeming** — signer is not the record's `owner` |
| `0x5622` | `Identity` | record not at its derived address; vault mismatch |
| `0x5623` | `Conservation` | vault debit ≠ `entitlement_atoms` |
| `0x5624` | `Receipt` | postcondition mismatch |
| `0x5625` | `Vault` | escrow close attempted with `outstanding_claim_checks != 0` |

**Refused by construction — each still gets a test asserting the *structural*
refusal**, since a later edit could turn a structural refusal into a checkable
one and nobody would notice:

| hostile | why no code |
|---|---|
| forged holder identity in the wire | there is no holder field; the owner is a PDA seed (§4.4) |
| replayed redemption | the record is closed; a closed account decodes to nothing |
| re-created claim-check after redemption | needs a compaction crank, which needs a position that no longer exists |
| **deadline tamper** | the deadline is a compiled constant in a digest-pinned ELF (§5.1); the only tamper surface is a release re-point, handed to Q1 as a refusal it must implement (§13) |
| draining another holder's entitlement | the vault is debited only by a redemption closing exactly one record for exactly its own `entitlement_atoms` |

**Three that deserve a campaign, not a unit test:**

1. **The dust griefer.** 1 lamport to the position, the admission, the vacant
   claim-check PDA, and the vault, in every combination, in the slot before the
   crank lands. All absorbed. This proves §8's subsumption claim and is the test
   most likely to be skipped.
2. **The interleaving.** A holder redeems normally while a cranker compacts a
   different position in the same slot; a holder redeems after the deadline but
   before their own compaction; two crankers race the same position. Supply must
   decrement exactly once per position in every ordering and §6.3's vault
   invariant must hold at every intermediate state.
3. **The §2 griefer, end to end.** A stranger calls `begin_retiring` the instant
   the market goes terminal. With C0, holders still redeem; without it they
   cannot. Assert both directions — the second as a regression test that fails if
   C0 is ever reverted.

---

## 10. Scope — what V1 does not cover, named as debt

**Fractional-shard-backed positions are out of scope and refuse with `0x560A`.**

The reason is structural. Fractional retirement closes "the zero native
Position/admission pair" and then a **zero-supply Token-2022 mint**
(`programs/dclutch-claims-sbf/src/fractional_retirement_v3.rs:1-8`), and shard
redemption is by the shard actor (`fractional_atomic_v3.rs:1114-1120`). The
position's owner in that arrangement is the Claims capability PDA
`[b"dclutch:rational-claims:v2", descriptor, outcome]`
(`crates/dclutch-claims-svm/src/protocol_position_v2.rs:33,276-282`, pinned at
`rational_terminal_v3.rs:136-144`), not a wallet — so its claimants are the
*holders of a mint*, plural and unknown to the position, and a one-owner
claim-check cannot represent them.

**The remaining gap, stated honestly: after this design lands, a market with one
unredeemed fractional shard still blocks retirement forever.** R3 is closed for
native positions and open for fractional ones. That is a narrowing, not a fix, and
the census's next revision should record it that way rather than scoring R3
closed.

**The shape its fix takes**, so V1 is not a dead end: the shard mint is *already*
a durable per-holder claim record. A fractional compaction moves the position's
collateral into the same vault and writes a claim-check whose claimant is the
**mint**, with a pro-rata entitlement per shard; shard holders redeem by burning
shards against the vault, forever, with their own signature. The vault, the
escrow, the deadline and the close gate are all reusable unchanged — only the
claimant kind and the redemption arithmetic are new. A V2 route in the same
module, not a second design.

---

## 11. Implementation plan

Per commit, named files only. Each commit states its own gate; no gate is "the
suite is green". `git commit --only -- <paths>`, never `git add -A`, never
`git stash`.

**C0 — the phase weld (§2). Ships first and is independently correct.**
**LANDED as `f6b53cc9`, with coverage in `552097c7`.** What follows is the plan
as written, then a correction the implementation had to make. Read the
correction: the plan as written does not close the bug.

*As planned.*
`programs/dclutch-claims-sbf/src/{terminal_settlement_v3,rational_terminal_v3,affine_batch_v2}.rs`.
Replace the hardcoded `Phase::Terminal` argument and the `!=` comparison with a
two-arm match admitting `Terminal` and `Retiring`, matching `phases_join`
(`lib.rs:1204-1213`) and the codec's stated intent (`generated.rs:1030`).
*Gate*: a holder redeems successfully after a stranger has called
`begin_retiring`; the same scenario fails on the parent commit. Assert the
`(Retiring, Retiring(w))` join is exercised, not just the `Terminal` one.

*Correction, established by experiment rather than by reading.* **The gate is in
five places, and the two the plan does not describe are the ones that bind.**
The plan describes the threaded `expected_core_phase` argument and
`affine_batch_v2`'s shared comparison (`:669`). Those are real, but the
flagship wallet payout submits a `TerminalSettlementRequestV3` straight to
Claims, so it is refused earlier, by a **route-local**
`core.phase != CorePhase::Terminal` at `terminal_settlement_v3.rs:597`
(reached from `authenticate_and_prepare` at `:231`, long before the `:410`
argument the plan means). An ELF built with the plan's literal fix — argument
welded, `:597` left alone — **still fails the C0 gate test at `0x5002`**; that
was built and run, not reasoned about. The fifth site is
`rational_product_v3.rs:196`, the `RedeemTerminal` arm of
RationalRepresentation, in a file this plan does not name at all; unwelding
only that site fails `552097c7`'s test at the same code.

*Shape actually used.* The bare `expected_core_phase: CorePhase` parameter
became `CorePhaseGateV3 { Exactly(CorePhase), TerminalOrRetiring }`
(`affine_batch_v2.rs`), so widening redemption could not silently widen the
`Open` and `Founding` routes that share the parameter — each of those now reads
`Exactly(..)` at its own call site. The variant is named for the phase set it
admits rather than for the routes that use it, so a later phase cannot join it
by the name staying plausible.

*Note for later commits in this plan.* Where §11 names files, treat the list as
a starting point and grep the predicate instead: this plan's own C0 entry named
three files for a change that touched five sites across six.

**C1 — contract types.** `crates/dclutch-claims-svm/src/claim_check_v1.rs` (new)
plus its `lib.rs` module line. `ClaimCheckV1`, `ClaimCheckEscrowV1`, three seed
structs, five magics, version and kind constants, encode/decode in the
`LifecycleRentCreditV2` layout idiom and the `SourceClosureReceiptV2` decode
order.
*Gate*: round-trip plus hostile decode — truncated, wrong magic, wrong version,
non-zero reserved, zero identity, `aggregate == owner`. No program code.

**C2 — conservation plans.** `ClaimCheckCompactionPlanV1` and
`ClaimCheckRedemptionPlanV1`, each `new()` + `validate_post()`, in the
`WorkEscrowClosePlanV1` shape. Pure, no `AccountInfo`.
*Gate*: a test per §6.1/§6.2/§6.3 statement, plus the underfunding test proving
§6.2 cannot refuse — construct the thinnest admissible position and assert a
small reward, never an error.

**C3 — error blocks.** `programs/dclutch-claims-sbf/src/claim_check_compaction_v1.rs`
and `claim_check_redemption_v1.rs` (new; enums and band asserts only) plus
`lib.rs` module lines.
*Gate*: the two `const _: () = assert!` band pins compile; `inventory
--check-unique` passes; grep-verify `0x5600`/`0x5620` are otherwise unused.

**C4 — `OpenClaimCheckEscrow`.** Frame, phase and terminal-receipt
authentication, both creations, mint and token-program authentication against the
Realm, `opened_slot`/`opener`/`opener_outlay` stamping.
*Gate*: opens on `Terminal`; opens on `Retiring`; refuses on `Open` (`0x5604`);
refuses a second open; refuses a wrong mint (`0x5609`).

**C5 — compaction: derivation, transfer, supply debit, record.** Coordinate
re-derivation, deadline check, the **call** into
`encode_product_basis_terminal_signed_delta_v3`, the Hoard→vault transfer with
observed-delta entitlement, the signed-delta supply debit, the record write, the
counter increment. Close deferred to C6 so this gate is about value, not
lamports.
*Gate*: **the differential test** — the compacted `entitlement_atoms` equals to
the atom what the same position's ordinary redemption pays in a sibling scenario.
This is the single most important assertion in the feature. Plus: premature crank
`0x5603`; wrong coordinates `0x5602`; second compaction `0x5605`; a
`ClaimsCapability`-owned position `0x560A`; supply cell decremented exactly once.

**C6 — the close and the rent split.** Position and admission close, the §6.2
split, opener repayment, the **new** dust-tolerant close receipt (never
`ProtocolPositionCloseReceiptV2`).
*Gate*: §6.2's identity holds exactly; the thin-position test still pays a
reward; **the dust hostile** (§9.1) in every combination; assert
`ProtocolPositionCloseReceiptV2` and its five binding sites are untouched.

**C7 — `RedeemClaimCheck`.** Holder-signed transfer and record close.
*Gate*: assert on the **frame spec** — not by inspection — that no market,
aggregate, basis, graph, Hoard or replay account appears, so a later edit adding
one fails. Redeems after full retirement in the same harness run. Non-holder
`0x5621`. Double redemption refuses.

**C8 — `CloseClaimCheckEscrow`.** Counter gate, dust sweep, both accounts closed.
*Gate*: refuses at `outstanding_claim_checks != 0` (`0x5625`); succeeds at zero;
§6.3's vault invariant asserted at every intermediate state of a three-holder
scenario.

**C9 — the end-to-end campaign.** One harness scenario: found → trade → resolve →
one holder redeems → one holder sleeps → advance past the deadline → open → crank
→ **retire the market** → the sleeper redeems from the claim-check against a
market that no longer exists → close the escrow.
*Gate*: retirement succeeds with a sleeping holder — the sentence R3 says is
impossible today — and both ledgers close over the whole run.

**C10 — operator and client.** `crates/dclutch-operator/src/claim_check_v1.rs` in
the `terminal_retirement_v1.rs` idiom: coordinate-only projection, caller
preflight, unsigned instruction plus expected-poststate facts
(`terminal_retirement_v1.rs:492-497,570-575,655-662`). Plus the SDK/CLI surface a
holder needs to discover and redeem a claim-check for a market that is gone.
*Gate*: the CLI redeems a claim-check on a retired devnet market.

**Docs, with C9:** update `docs/evidence/LIVENESS_CENSUS_2026_08_29.md` — R3
**narrowed, not closed** (§10); R13 retirement half subsumed (§8); Q6 unblocked
with its gate named and Correction 4's premise corrected (§7); **§2 added as a
new ranked RED row**.

**Explicitly not in this plan:** the Q6 closer itself, Q8b's receipt-ABI work,
and any change to Core, Trading, Registry, Custody, or the Lean semantics.

---

## 12. Cohort target and blast radius

**One ELF: claims-sbf.** No Core, Trading, Registry or Custody change, no Lean
regeneration, no change to `Action` or `CoreState`. Three decisions protect that
and each is flagged where it is made: the vault is an ordinary `External` token
account rather than a new `CompartmentV1` variant (§4.2); the compaction close
writes a new receipt type rather than amending the five-site exact-equality
binding of `ProtocolPositionCloseReceiptV2` (§6.2, §8); and the clock origin
lives in a new Claims account rather than in Lean-generated `CoreState` (§5.4).
If an implementer finds themselves editing a second crate, one of those three
decisions has been dropped — go back and find which.

**Cohort: the first cut after cohort-7.** claims-sbf is cohort-critical
(`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:482`) and the wave's freeze policy rides everything after the first
probe-green revision to the next cohort (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:361-362`). Not on the devnet
cut's critical path and must not be forced onto it — nothing on devnet is waiting
on a market retiring past a sleeping holder.

**C0 is the exception and should not wait for this cohort.** It is a five-line
weld against a live value-destruction verb, it is independently correct, and its
regression test stands alone. Ship it as soon as a claims-sbf window opens.

**Ships with, not before:** Q6's closer. They gate on the same predicate and
share a scenario, and a cohort carrying compaction without the closer strands the
Claims replay rent for exactly the markets compaction just made retirable.

---

## 13. Cross-lane constraint handed to Q1

Q1's ruled direction (a) — release-set lineage migration
(`docs/design/RELEASE_LINEAGE_MIGRATION_V1.md`, the sibling lane) — creates the
route that makes a live market's release set changeable. The moment it lands,
§5.1's "structurally impossible" becomes "must be refused deliberately".

**Constraint:** a re-point must **refuse** a target release whose
`COMPACTION_DEADLINE_SLOTS_V1` is *shorter* than the one the market was founded
on, and must carry the founding value forward for markets already past terminal.
Lengthening is permitted; shortening is not.

The reason is C3 read in the other direction: **a shortened deadline applied to a
live market is a migration authority reaching into other people's markets and
moving their holders' grace period, which is confiscation with extra steps.** The
refusal belongs in the migration's own hostile table with its own reserved code.

---

## 14. What is left to the implementer

Deliberately short; everything else here is decided.

1. **Measure CU for one compaction** and confirm a single position fits a
   transaction at the widest supported market. If not, split at the natural seam:
   derive-transfer-debit-record (C5) in one transaction, close (C6) in a second,
   with the claim-check's existence as the resume cursor. The design already
   supports this because the record is written before the close.
2. **Pick the two constants** against measured reality —
   `COMPACTION_DEADLINE_SLOTS_V1` (§5.1: 38_880_000 mainnet, 5_400 devnet) and
   `COMPACTION_CRANK_REWARD_LAMPORTS_V1` (§6.2: 200_000). Both are release
   constants; changing them later is a release, by design.
3. **Confirm the escrow PDA can hold an associated token account** for the
   collateral mint under the market's token program, and that
   `execute_terminal_custody_v3` accepts it as the `External` destination with
   `recipient_owner` set to the escrow PDA. §4.2 argues this from the fact that a
   holder payout already targets an arbitrary external token account; verify it
   in C5 before writing C6. If it needs anything from Custody that does not
   exist, stop and re-cost — that would add a second ELF and change §12.

---

## Appendix — census rows this document answers

| row | before | after |
|---|---|---|
| **R3** sleeping holder blocks retirement forever | RED | **narrowed** — closed for native positions, open for fractional (§10) |
| **R13** 1-lamport front-run, retirement half | RED (adversarial) | **subsumed** — a bounded funded wait, conditional on §4.7's dust tolerance (§8) |
| **R13** admission half | RED (adversarial) | **deferred** to Q8b, unchanged (§8) |
| **R10 / Q6** Claims replay has no closer | RED, blocked on Q3 | **unblocked and made safe** — cursor question answered, gate named, Correction 4's premise corrected (§7) |
| **Y1/Y2** cleanup pays a fixed beneficiary, never the caller | YELLOW | the crank is caller-funded from rent already moving (§6.2) — P1 applied |
| **NEW** `begin_retiring` permanently destroys redemption, callable by anyone | *not censused* | **RED (adversarial)** — weld specified as C0 (§2, §11) |

---

## 15. Amendments from implementation

Written during CLAIMCHECK's implementation lane, ratified 2026-08-30. Each was
found by building the thing rather than by re-reading the design, which is the
argument for implementing *from* a design rather than executing one. The
arithmetic is recorded here so the next reader inherits the reasoning instead of
rediscovering it.

Landed in `crates/dclutch-claims-svm/src/claim_check_v1.rs`,
`claim_check_conservation_v1.rs`, `claim_check_request_v1.rs`,
`claim_check_compaction_request_v1.rs`, and
`programs/dclutch-claims-sbf/src/claim_check_{compaction,redemption}_v1.rs`.

### 15.1 A claim-check promising zero atoms is refused — §4.7 step 4 amended

§4.7 writes the record unconditionally. It must not.

A position's terminal payout is nonzero only on the outcomes that won. **Every
holder of a losing outcome resolves to zero atoms**, so a zero payout is the
*common* case, not an edge — in a binary market roughly half the positions, and
in an N-outcome market most of them.

Minting a record for each would be actively harmful, not merely wasteful. The
escrow may close only at `outstanding_claim_checks == 0` (§4.9), and a record
promising nothing gives its holder no reason ever to redeem it. Every zero-atom
claim-check therefore pins the counter above zero **forever**, and the escrow and
its vault become exactly the perpetual account this design exists to remove — a
worse outcome than the one it was written to fix, arrived at by following the
design as written.

So: a zero-payout position is compacted and closed *without* a record. Supply
still retires through the same signed-delta debit, the position and admission
still close, retirement still proceeds. Only the useless record is skipped.

The refusal is enforced in the contract, at the constructor **and across the
wire**, so a route cannot mint one by accident and a hostile cannot forge one
into existence. Conservation binds the two halves together:
`claim_check_rent == 0` if and only if `entitlement_atoms == 0`, checked in both
directions, so a plan that funded rent for an empty claim — or minted a claim
without funding it — cannot be constructed.

The tree agrees from the other side. `execute_terminal_custody_v3` opens with
`if input.payout == 0 { return Ok(None); }`: a zero payout moves no collateral
at all, so there is genuinely nothing escrowed and nothing to promise.

### 15.2 The crank is paid before the opener — §6.2's order amended

§6.2 orders the split rent → **opener** → crank → residue, and says the first
compaction crank "repays the opener in full before paying itself". That
presumes one position's rent covers the whole opener outlay. It does not.

Rent-exempt minimum on Solana is `(128 + bytes) * 6960` lamports. For a binary
market:

| account | bytes | lamports |
|---|---|---|
| position (`128 + 8*2`) | 144 | 1,893,120 |
| admission (fixed) | 512 | 4,454,400 |
| **`R` released by the close** | | **6,347,520** |
| `ClaimCheckV1` (fixed width) | 288 | 2,895,360 |
| escrow record | 256 | 2,672,640 |
| escrow vault (token account) | 165 | 2,039,280 |
| **opener outlay** | | **4,711,920** |

`R - claim_check_rent = 6,347,520 - 2,895,360 = 3,452,160`, which is **less than
the 4,711,920 the opener advanced**. Under §6.2's order the opener absorbs
everything available and `crank_reward` is `min(200_000, 0) = 0`: **the first
crank pays itself exactly nothing.**

An unfunded crank is an unturned crank. That is R3 walking back in through the
funding door — the exact failure §6.2 names two paragraphs earlier, when it
insists the reward be a residual rather than a demand because "a compaction that
could refuse for lack of funds would reintroduce R3 through the funding door". A
compaction that *pays* nothing is refused by the same logic, just by the market
instead of by the code.

The implemented order:

```
claim_check_top_up := max(0, claim_check_rent - lamports already at the address)
crank_reward       := min(COMPACTION_CRANK_REWARD_LAMPORTS_V1,
                          R - claim_check_top_up)
opener_repayment   := min(escrow.opener_outlay,
                          R - claim_check_top_up - crank_reward)
rent_credit_residue:= R - claim_check_top_up - crank_reward - opener_repayment

R == claim_check_top_up + crank_reward + opener_repayment + rent_credit_residue
```

The opener is not disadvantaged, and this is why the reordering costs nothing:

- They are the party who *wants* to crank — §5.4's whole argument for why
  opening is a funded position — so they earn `crank_reward` on every turn.
- `opener_outlay` is carried in the escrow record as an **outstanding debt** that
  decrements, not a fixed figure, so partial repayment is expressible and the
  balance discharges over the second crank. Asserted as a test.
- The escrow's own close (§4.9) returns the escrow and vault rent — *exactly*
  what they advanced — to whoever closes it.

### 15.3 The three lamport sinks may alias, and credits fold by identity

§6.2 names cranker, opener and RentCredit as three recipients and does not say
what happens when two are the same account. They frequently are: **the opener is
usually the cranker**, which is precisely what §5.4 argues makes opening a funded
position rather than a donation.

A plan naming them as separate sinks computes two expected post-balances for one
account and fails *both*, so the honest case refuses. Credits are therefore
folded by identity before the postcondition is formed: an aliased sink is
expected to receive the sum. A sink that is also an account being closed or
created is refused outright (`IdentityAlias`) — it would be credited and zeroed
in the same movement, and the ledger would appear to close while a lamport went
missing.

### 15.4 §6.3's transfer-fee tolerance is unreachable today — recorded, not relied on

§6.3 requires `entitlement_atoms` to be the **observed** vault credit rather than
the intended transfer, because a Token-2022 mint may levy a transfer fee. The
discipline is right and is implemented. The premise, however, is not currently
reachable through this path.

`execute_terminal_custody_v3` ends with an exact equality on both sides:

```rust
if source_before.checked_sub(input.payout) != Some(source_after)
    || recipient_before.checked_add(input.payout) != Some(recipient_after)
```

A fee-bearing mint is therefore refused by the **executor**, before any short
credit could reach the conservation plan. So today `entitlement_atoms == payout`,
always.

Observing rather than assuming is still correct and costs nothing: it is defence
in depth, and it is already right if that check is ever relaxed for fee mints.
What would not have been correct is *claiming* the fee path is exercised. If
fee-bearing collateral is ever wanted, the thing to revisit is that exact
equality in the **holder redemption** path — a pre-existing question this design
neither creates nor closes.

### 15.5 Two §14 unknowns resolved, and C0's better shape

§14 left three things to the implementer. Two are settled by reading the routes:

- **§14.3 — the escrow PDA can own the vault, and one ELF holds.**
  `execute_terminal_custody_v3` takes `recipient_owner: [u8; 32]` and an
  arbitrary `frame.recipient`, checking only that the account's mint and owner
  match what was declared. Nothing constrains the owner to be a wallet, so a
  claims PDA is admissible. claims-sbf already creates a PDA-owned token account
  (`initialize_account3`, `rational_lifecycle_v2.rs:1105`) and already depends on
  both `spl-associated-token-account-interface` and `dclutch-token-svm`. No
  Custody change, no new dependency. §4.2 stands as written.
- **§4.6's mint authentication mostly already exists.** Custody refuses any
  `OpenVault`/`Transfer`/`CloseVault` whose declared mint is not the Realm's
  `collateral_mint` and whose token program is not the Realm's `token_program`
  (`custody-sbf/lib.rs:573-581`). Compaction's Hoard→vault move *is* a Custody
  `Transfer`, so that policy is enforced by the program that owns the fact. The
  open route reads the same Realm record to *stamp* mint and token program into
  the escrow and to refuse a mismatch at open — one author, two readers, which is
  a different thing from two authors.

And **C0 shipped in a better shape than §11 specified.** §11 asked for a two-arm
match replacing the hardcoded `Phase::Terminal` in three files. What landed
(`f6b53cc9`) is a `CorePhaseGateV3` enum — `Exactly(phase)` and
`TerminalOrRetiring` — so the admission is a named thing with one author rather
than three hand-edited comparisons free to drift apart, and every site that must
*not* admit `Retiring` keeps `Exactly(...)` visibly. It also found a **fifth**
site the design's own plan missed (`rational_product_v3.rs:196`); see
`85d535bd`.

### 15.6 Compaction's wire carries the terminal header verbatim

§4.7 step 1 says the payout derivation must be *called*, never re-implemented,
and calls that the single most important instruction in the document. The
implementation makes it structural rather than a rule to remember.

`TerminalSettlementRequestV3` already carries `recipient_owner` and
`recipient_token_account` as explicit fields, so a compaction genuinely *is* a
redemption with the recipient swapped. The compaction request therefore embeds
one verbatim at its own exact width, decoded by that header's own decoder and by
nothing else. One author for every coordinate the derivation reads; a corrupted
byte inside is caught by the terminal decoder rather than by a copy of it; a
future edit to that header reaches compaction automatically.

The two recipient fields are the entire attack surface — a cranker who could
choose them would redirect a sleeping holder's collateral — so they are
**derived, not accepted**, and naming the holder as recipient is refused
outright, that being the holder's own redemption with the signature deleted.

---

## 16. What shipped, reconciled against what was planned

Written when the feature was whole, so a reader inherits the truth rather than
the plan. Every amendment, departure and named debt from §15, with its status
and where it actually lives.

### 16.1 The commits

| commit | what |
|---|---|
| `a2ad25ed` | C1 — `ClaimCheckV1` (288 B), `ClaimCheckEscrowV1` (256 B), seeds, magics |
| `dd63777c` | the two release constants and the argument for their being constants |
| `23b6bb93` | C2 — both conservation plans |
| `9854c583` | C3 — 17 refusal codes in sub-bands `0x5600` / `0x5620` |
| `a526f808` | the open, redeem and escrow-close request wires |
| `6490faff` | compaction's request: the terminal header carried verbatim |
| `dbb41f3c` | §15, the six amendments, with their arithmetic |
| `7b584743` | C4 — `OpenClaimCheckEscrow` |
| `706a6f48` | C5+C6 — the crank, the payout, the supply debit, the close and the split |
| `7f78f48a` | C7 — `RedeemClaimCheck` |
| `b7688fe7` | C8 — `CloseClaimCheckEscrow` |
| `748b5730` | C9 — the end-to-end campaign |
| `dff38069` | C10 — the operator surface a holder uses |

### 16.2 Every amendment, and whether it survived contact

| § | amendment | shipped as |
|---|---|---|
| 15.1 | zero-atom claim-checks refused | **as written.** Enforced at the constructor, across the wire, and in conservation (`claim_check_rent == 0` iff `entitlement == 0`, both directions). Confirmed from the other side by `execute_terminal_custody_v3`, which no-ops on a zero payout. |
| 15.2 | crank paid before opener | **as written.** `ClaimCheckCompactionPlanV1` orders rent → crank → opener → residue. C5's test asserts from chain state that the crank is paid, that the opener is *still owed*, and that the four credits exhaust the released rent exactly. |
| 15.3 | aliased sinks fold by identity | **as written.** `fold_credit` sums credits per distinct address; a sink that is also a closing account is refused. |
| 15.4 | §6.3's fee tolerance unreachable | **confirmed, and left in.** The observation discipline is implemented and costs nothing; the executor's exact equality still refuses fee mints first, so `entitlement == payout` always today. |
| 15.5 | §14.3 verified, C0's better shape | **held.** No Custody change, no new dependency, one ELF. |
| 15.6 | compaction embeds the terminal header verbatim | **as written**, and it is what makes C5's differential structural. |

### 16.3 Two further departures, made during implementation

**The vault is a Claims-derived PDA, not the escrow's associated token account
(§4.2).** An ATA's address derives under the associated-token program, so this
program cannot sign for it and cannot create it with the tree's own
`allocate`/`assign` idiom — it would have to CPI a third-party program into a
frame that otherwise needs none. Deriving it under `CLAIM_CHECK_VAULT_SEED_V1`
keeps creation on the house pattern, keeps the open frame at twelve accounts,
and makes the vault recoverable from the aggregate alone, which is exactly what
a holder needs once the market is gone. §4.2's reasoning survives intact: its
point was that the vault is an ordinary `External` token account rather than a
new `CompartmentV1`, and it still is — Custody authenticates a destination by
its mint and its *owner*, never by how its address was derived.

**C5 and C6 landed as one commit.** The plan split them to gate value
separately from lamports, but the claim-check's rent comes from the swept
position, so splitting would have meant writing a cranker-funded path only to
delete it. Both gates are present: the differential for value, the four-credit
identity for lamports.

### 16.4 The authority relaxation, stated plainly

Compaction stands in for a signature the holder is not there to give, so it had
to relax exactly one check without weakening it for anybody else.

`ParentAuthorityV3::ClaimCheckCrank` asks coordinate 0 for a signature and
nothing more — it is the cranker, who is anybody. "Coordinate 0 is a signer in
every mode" stays literally true; only *whose* signature changes, which is the
axis that enum already varied on, and every other role is refused the relaxation
by an explicit match arm.

The entitlement is not carried by that signature. It is proved before the mode
can be selected: the deadline has elapsed under checked arithmetic, the
recipient is **derived** from the market's own aggregate rather than accepted
from the caller, and the claim-check address is vacant.

Making the *escrow* PDA sign was considered and rejected: a PDA is a signer only
when the current instruction arrived via `invoke_signed`, so a top-level
compaction cannot make its own PDA sign to itself, and buying that would have
cost a claims→claims self-CPI and its re-entrancy surface.

One hole opens and is closed explicitly. The owner's signature was silently
doing a **second** job the enum never named: proving the position is wallet-held,
because a Trading record or Claims capability owner is a PDA and cannot sign.
The route replaces that inference with the persisted
`ProtocolPositionOwnerKindV2` tag, read off the admission record, refusing
`ClaimsCapability` with `0x560A`.

> **AMENDED — this paragraph names two kinds that cannot sign and the shipped
> code refused one.** `TradingRecord` was admitted, and `TradingRecord` is the
> Fractional reserve Position. §17.1 carries the weld and the exposure.

### 16.5 Debt, named rather than absolved

- **§6.2's dust-tolerant close receipt is not written.** The settlement's own
  receipt is the evidence today and conservation is already checked by the plan,
  so this is an off-chain consumer's convenience rather than a safety gap. It
  remains owed.
- **R3 is narrowed, not closed**, exactly as §10 said it would be. Closed for
  native positions; open for fractional ones, whose claimants are the holders of
  a mint and cannot be represented by a one-owner claim-check. §17 sizes the
  fractional half and corrects the gate that made the narrowing unsafe.
- **C9 does not drive `market_closure_v1`.** That is a property of the campaign's
  fixture, not of the feature: its market carries Claims capability positions, so
  retiring it needs the fractional route. What C9 proves is the R3 claim
  precisely — after the crank the sleeping holder's position does not exist and
  the supply it held is gone from the aggregate, so that holder is no longer a
  reason retirement cannot proceed.
- **C9 does not drive a real `protocol_position_v2::Admit`.** The campaign's own
  test caller has no Admit forwarding; `sparse-chain-caller` does, in a different
  crate with no resolved market. The admission record is planted with the
  production codec, so the format keeps one author and compaction reads exactly
  one field from it. The gauntlet already builds `dclutch-rent-sbf`, so what is
  missing for whoever closes this is the caller verb, not the ELF.
- **No web surface.** The operator crate carries the holder's path; the site
  copy belongs to the lane that owns the site's voice.

---

## 17. The gate §4.7 got wrong, and what the fractional half actually costs

Written by FRACR3, 2026-08-30, from the fractional side of §10's named debt. The
first half is a correction to a shipped route. The second half is the size of
the work §10 sketched, measured rather than guessed.

### 17.1 §4.7's owner-kind precondition admits a position that cannot be paid

§4.7 lists, among the compaction preconditions:

> Owner kind is `ProtocolPositionOwnerKindV2::User` or `TradingRecord`, not
> `ClaimsCapability` (`protocol_position_v2.rs:193-200`) — see §10.

That is wrong, and §16.4 says so two sentences before repeating it:

> A Trading record or Claims capability owner is a PDA and cannot sign. The
> route replaces that inference with the persisted `ProtocolPositionOwnerKindV2`
> tag, read off the admission record, refusing `ClaimsCapability` with `0x560A`.

The sentence names **two** kinds that cannot sign and the code refused **one**.
`RedeemClaimCheck` pays the record's `owner` and requires that address to sign
(`claim_check_redemption_v1.rs`, `0x5621`, plus the holder's signer role in the
redemption frame spec). A program-derived address cannot sign a top-level
instruction, and no CPI reaches this route, so a claim-check minted for a PDA is
collateral written to an address that can never open it.

**`TradingRecord` is not a hypothetical.** It is the Fractional reserve
Position. `fractional_retirement_v3.rs` joins its admission with

```rust
if admission.owner_kind() != ProtocolPositionOwnerKindV2::TradingRecord
    || admission.position_owner() != input.root
```

where `input.root` is the Trading-owned Fractional capability root PDA — the
same account `fractional_atomic_v3.rs` requires as the reserve Position's owner
on both the open and the terminal path. That Position holds the collateral
backing **every outstanding shard of one coordinate**.

So, before this amendment: past the deadline, any caller could compact the
Fractional reserve. The collateral moves to the escrow vault, a claim-check is
minted naming a PDA, and the Position every shard holder's own redemption reads
is closed in the same transaction. Nobody can redeem the record and nobody can
redeem the shards. **Compaction converted R3's delay into a total loss, for
exactly the holders §10 said it was leaving alone.** Same class as §2 — value
destruction by an arbitrary actor — arrived at by implementing §4.7 faithfully.

**The weld.** The gate becomes an exhaustive function over the owner-kind enum,
`owner_kind_can_open_a_claim_check`, admitting `User` and refusing both PDA
kinds. Exhaustive on purpose: a fourth owner kind has to answer the question
rather than inherit whichever arm it was written beside. This follows C0's own
lesson (§15.5) — a named admission with one author beats a comparison each site
maintains separately.

Nothing is stranded by refusing `TradingRecord`. A Trading-owned Position has
its own parent-authenticated close (`protocol_position_v2.rs:798-830`), and the
Fractional reserve has `fractional_retirement_v3`'s ordered route. Compaction
was never the only way to retire either; it was the only way to *destroy* them.

### 17.2 What a fractional claim-check must carry — and why it is a second record

§10 sketches "a claim-check whose claimant is the **mint**, with a pro-rata
entitlement per shard". The tree's arithmetic is kinder than that. There is no
pro-rata. `divide_exposure_shards_v2`
(`crates/dclutch-fractional-claim-kernel/src/exposure_v2.rs:441-473`) is the
sole quotient/remainder boundary, and the payout below it is a multiplication:

```text
whole_claims     = shard_atoms / denominator     (floor, the only division)
consumed         = whole_claims * denominator    (burned)
change           = shard_atoms - consumed        (stays in the holder's account)
collateral_atoms = whole_claims * payout_per_claim[coordinate]
```

`payout_per_claim` is a per-coordinate constant that the terminal evaluator
produces once (`product_basis_terminal_v3.rs:425-432`, and the kernel mirror at
`exposure_v2.rs:521-524`). So a fractional claim-check that stores
`denominator` and `payout_per_claim` pays, to the atom, what on-time redemption
would have paid — using the same two numbers and the same two operations, with
no second rounding boundary to get wrong and no last-burner remainder to
dispose of. Sub-denominator dust is not a claim on collateral before compaction
(`NoWholeClaim` is a refusal, not a zero payout) and must not become one after.

Beyond the native record that needs: `denominator` (u64), `payout_per_claim`
(u64), `representation_coordinate` (u32) — 20 bytes — and the shard mint, which
is the *claimant* and therefore belongs in the seeds
(`[FRACTIONAL_CLAIM_CHECK_SEED, aggregate, shard_mint]`), so the record's
address proves which instrument it answers to, exactly as the native record's
address proves its holder.

Twenty bytes fit `ClaimCheckV1`'s 24 reserved body bytes with four to spare.
**Take the second record type anyway.** Three reasons, none of them space:

1. `ClaimCheckV1::decode` runs `require_zero` over that reserved run, and the
   house decode order (`exact_width`, magic, version, kind, every `require_zero`,
   then fields) is what makes a hostile decode cheap to audit. One width meaning
   two field layouts turns that into a union whose arms diverge after the kind
   byte.
2. `entitlement_atoms` means "the payout, paid once, then the record closes"
   natively. Fractionally it means "the remaining escrowed balance, paid down
   across many burns". Same name, different invariant, is how a conservation bug
   gets written.
3. The lifetimes differ. A native record is created once and closed on its one
   redemption; a fractional record survives every partial burn until its
   coordinate's shards are exhausted.

The escrow, the vault, `COMPACTION_DEADLINE_SLOTS_V1`,
`COMPACTION_CRANK_REWARD_LAMPORTS_V1`, `CloseClaimCheckEscrow`, the plan-struct
idiom and `fold_credit` are all reusable unchanged. §10's "a V2 route in the
same module, not a second design" survives; only "one record" does not.

### 17.3 The size, with the numbers

**Compute is not the blocker.** Measured in this campaign, at the same fixture
and the same build:

| transaction | CU |
|---|---|
| the holder's own wallet payout | 472,599 |
| compaction of that same position | 503,554 |
| claim-check redemption | 20,958 |

Compaction costs **30,955 CU** more than the redemption it stands in for — the
record write, the close, the four-credit split, the plan and the escrow update.
Fractional terminal settlement's own measured table
(`program-test/fractional-atomic/tests/fractional_atomic.rs`) is

```text
width   8    16   32   48   64    96     98     99
units 463k 519k 593k 731k 897k 1356k  1393k  exhausted
```

so a fractional compaction at the supported width 64 lands near **928k of
1,400,000** — about 6.6% over the settlement, inside the 503k headroom that
already exists there. It would narrow the untested upper range (96 has 44k of
headroom and would lose most of it), not close the feature. Frame: fractional
terminal is 44 accounts, plus compaction's 6 is **50**, under devnet's 64-lock
limit, over the ALT these campaigns already serialise through.

**The blockers are structural.**

1. **Ordered fractional retirement is not reachable on chain at all today.**
   `fractional_retirement_v3.rs` dispatches only `RetireCoordinate`; it refuses
   `FractionalRetirementActionV3::Begin` and `::Finish` outright, and nothing in
   any program calls either. `FractionalRetirementCursorV3::begin` and
   `::finish` exist in the contract with tests, but no route can create or close
   the cursor PDA the coordinate walk advances. **A fractional claim-check would
   make `RetireCoordinate`'s gates satisfiable and the market still would not
   retire.** Wiring `Begin`/`Finish` is its own lane and is strictly upstream of
   this one.

   > **CLEARED 2026-08-30.** FRACLIFE shipped `Begin` and `Finish`
   > (`27d2c28e`..`b17e9bc3`) and drove a fractional market through retirement
   > end to end against the real Token-2022. This blocker is gone; blockers 2
   > and 3 stand, and §17.4 adds the one this list missed.
2. **`RetireCoordinate`'s zero-supply gate has to gain a compacted arm.** It
   requires the shard mint's supply to be exactly zero — twice, once through
   `check_mint`'s `expected_base_supply` and once explicitly. A compacted
   coordinate has a *nonzero* supply by construction: the outstanding shards are
   the durable claim. So the mint must survive retirement rather than be closed,
   which adds one perpetual mint account per unredeemed coordinate to §6.4's
   residue — smaller than the market, larger than nothing, and it must be named
   as debt rather than absolved.
3. **Shards cannot be compacted, only their backing.** Shards live in ordinary
   holder-owned Token accounts. No crank can burn them, and no crank should be
   able to. This is why the mint has to become the claim record rather than be
   retired: the claim-check answers to the instrument, and the holder redeems by
   burning, with their own signature, forever.

   > **AMENDED — the last clause is false, and it was the premise the estimate
   > below was built on.** A shard holder's own signature can never burn a
   > shard. It is true that no crank can burn a holder's shards, but the reason
   > is not that they sit in ordinary Token accounts; it is that the Mint's
   > burn authority is *another program's PDA*, and that same fact stops the
   > **holder** too. §17.4 carries the execution and the sound shape.
   >
   > What survives intact is the sentence before it: the Mint becomes the claim
   > record, and the claim-check answers to the instrument. That is the part
   > that dissolves the unsignable-owner problem, and it needs no correction.

**Estimate: one lane, eight commits, after the `Begin`/`Finish` lane.** Record
type and seeds; two conservation plans; a refusal sub-band (`0x5640` and
`0x5660` are free); the compaction route; the burn-and-pay redemption route; the
`RetireCoordinate` arm; the campaign; the operator surface. The expensive half of
the native lane — building a terminal fixture — is already paid: `fractional-atomic`
drives terminal redeem and zero-burn against real ELFs today.

> **AMENDED — eight was short by six, and the shortfall is a whole program.**
> The estimate costed a burn the holder performs alone. That burn does not
> exist, so the redemption route it costed does not either, and the correction
> in §17.4 adds a Trading-composed compaction and a split-controller Mint
> profile — neither of which is a bigger version of anything in the list above.
> **Fourteen commits, two programs, two cohorts.** Four have landed; the ten
> that remain are the Trading half and what depends on it. Everything §17.3
> says about compute, frames and the already-paid terminal fixture still holds.

---

### 17.4 The burn nobody can perform, executed rather than argued

Written by FRACCHECK, 2026-08-30, from building §17.3's estimate. Evidence:
`docs/evidence/FRACTIONAL_CLAIM_CHECK_2026_08_30.md`.

**Every shard Mint carries Token-2022's `PermissionedBurn` extension, and it is
required rather than incidental.** `Token2022BehaviorProfileV2::read_mint`
refuses any Mint that lacks it, and pins it to the Mint's controller:

```rust
// crates/dclutch-token-svm/src/behavior_profile_v2.rs
PERMISSIONED_BURN_EXTENSION if !burn_seen => {
    require_extension(entry, PERMISSIONED_BURN_EXTENSION, AUTHORITY_EXTENSION_BYTES)?;
    require_key(entry.value, expected_controller)?;
    burn_seen = true;
}
// ...
if !close_seen || !burn_seen || pointer_seen != metadata_seen {
    return Err(Error::InvalidExtensionLayout);
}
```

For a Fractional coordinate that `expected_controller` is `root_account.key`
(`fractional_atomic_v3.rs::process_terminal`) — the capability root, derived
under the **Trading** program. Claims cannot sign it, and it does not outlive
the market.

**The consequence, run rather than reasoned about.** Against the audited
`spl-token-2022` v11 fixture, on a Mint carrying that extension, the account's
own owner signs a standard `BurnChecked` and the chain answers:

```text
Program log: Instruction: BurnChecked
Program log: Error: Invalid instruction
Program Tokenz…Pxu failed: custom program error: 0xc
```

`0xc` is `TokenError::InvalidInstruction`. The processor is explicit about why
— *"Standard burns cannot be used when the permissioned burn extension is
present"* — and its permissioned variant requires the configured authority as a
**second signer**, refusing `MissingRequiredSignature` when it is present but
unsigned and `InvalidAccountData` when a different key signs. All four
transactions, including the double-signed control that succeeds, are pinned in
`program-test/fractional-atomic/tests/permissioned_burn_wall.rs`.

So the redemption route §17.3 costed — claims-only, holder-signed, surviving the
market — cannot be built. A frame containing the Fractional root could not
answer `survives_retirement()`, which is the frame spec catching this correctly
rather than a limitation of it.

**The sound shape: compaction hands the burn over.**
`SetAuthority(AuthorityType::PermissionedBurn)` moves the authority and requires
the *current* one to sign (`processor.rs:996-1007`). So fractional compaction
re-points the Mint's burn authority from the Fractional root to the **escrow
PDA**, while the root is still alive to authorize it. After that one
instruction a redemption needs exactly two signatures: the holder's, over their
own shards, and the escrow's, which Claims produces for itself. The hand-off is
executed in the same campaign — a stranger attempting it is refused
`OwnerMismatch`, the old authority is powerless afterwards, and the
holder-signed escrow-approved burn goes through.

Two costs follow, and they are what the estimate was missing:

- **Fractional compaction becomes Trading-composed.** The root's signature
  exists nowhere else — `protocol_position_v2.rs` requires both the Trading
  caller-authority PDA and the root to sign the close that `RetireCoordinate`
  already performs. This costs the permissionless property nothing;
  `fractional_retirement_v3.rs` is permissionless *and* Trading-composed today.
  It costs a route in a second ELF.

  > **AMENDED — "a route in a second ELF" is three arms in three crates, and
  > the precedent this sentence leans on has only one of them.** See §17.5.
- **A re-pointed Mint no longer satisfies `read_mint`.** That function requires
  one controller to be the mint authority *and* the close authority *and* the
  burn authority. After the hand-off the burn authority is the escrow and the
  other two are still the root, so `dclutch-token-svm` needs a split-controller
  sibling, and blocker 2's compacted arm must read it instead of `check_mint`.

**Impounding was considered and rejected.** Shards could be transferred to a
Claims-owned sink rather than burned — transfer is not permissioned — which
needs no Trading route and no new profile. It was rejected because it replaces
a supply the whole family already reads with a balance in a second account:
blocker 2's gate, the terminal evaluator's `expected_base_supply`, and the
record's own escrowed-equals-`floor(supply / denominator) × payout_per_claim`
invariant would each have to learn about the sink, and the sink is itself a
perpetual account holding instruments nobody can destroy. Burning keeps one
number meaning one thing.

---

### 17.5 The burn, executed; and the composition layer nobody had looked at

Written by FRACCHECK-2, 2026-08-31, from building §17.4's Trading half.
Evidence: `docs/evidence/FRACTIONAL_COMPACTION_TRADING_HALF_2026_08_31.md`.

**§17.4's sound shape is no longer a design sentence.** A Mint carrying the
whole shard profile — not the burn half — has its permissioned-burn authority
moved from a program-derived root to a program-derived escrow while the root is
still alive; the old authority is powerless afterwards; and the holder's own
signature plus the escrow's `invoke_signed` burns the shards. The escrow is
built from `ClaimCheckEscrowSeedsV1`, the shipped recipe, so what Token-2022
accepts is a signature this tree knows how to produce rather than one a test can
always manufacture.

**The split-controller sibling exists, and the property is a disjointness.**

```rust
Token2022BehaviorProfileV2::read_compacted_shard_mint(
    program_id, mint_key, mint_data, expected_controller, expected_burn_authority,
) -> Result<Token2022CompactedShardMintFactsV2>
```

It refuses `burn == controller`, and `read_mint` requires those to be equal, so
**no Mint bytes are admitted by both arms under any nomination**. The compacted
arm therefore cannot stand in for the live one on a coordinate nobody compacted,
and the live arm cannot be reached by a Mint whose burn the root gave away. Both
directions are executed against the bytes Token-2022 wrote either side of a real
`SetAuthority`, which is the half a hand-built fixture can never supply.

Supply is reported and never pinned, and there is deliberately no
`check_compacted_shard_mint`: the outstanding shard supply *is* the durable
claim, and any holder's redemption lowers it between a request being built and
it landing, so pinning it would refuse an honest retirement because somebody
else redeemed first.

**The correction, and it is upstream of the estimate again.** §17.4 costs the
Trading half as *"a route in a second ELF"*, on the precedent that
`fractional_retirement_v3.rs` is Trading-composed today. A Claims route reached
from Trading's Hot path has to be admitted at **three** layers, and
`RetireCoordinate` is present at one:

| layer | `RetireCoordinate` |
|---|---|
| execution — `claims_composition_v3.rs` (`route_authority`, `fractional_root_signer`, receipt verifier) | **present** |
| composition decode — `composition_v3.rs::decode_selected_with_external`, `hot_v3.rs::decode_claims_composition_boxed_v3` | **absent** |
| artifact geometry — `artifacts_v4.rs::action_geometry` / `encode_effect` / `encode_account_profile` | **absent** |

`FRACTIONAL_RETIREMENT_REQUEST_MAGIC_V3` occurs in `programs/dclutch-trading-sbf/src/`
exactly twice, both inside `claims_composition_v3.rs`, and zero times in
`composition_v3.rs`. `decode_claims_composition_boxed_v3` admits an external
once-route only for `FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2`. So *ordered
fractional retirement is reachable from a test caller and not from a Market*,
and "Trading-composed" is true of the signature propagation while not yet being
true of the route selection.

That is not an argument against this design. It is a correction to what the
remaining work is: a fractional compaction inherits the gap rather than
borrowing a solved problem, and the two missing arms have no precedent in this
family to copy.

> **AMENDED — fourteen was short by three, and the three are composition
> surface rather than route code.** **Seventeen commits**; eight have landed
> (FRACCHECK's four, plus the split-controller reader, the derived-escrow
> campaign, the compaction request, and this amendment). Nine remain against
> the ten FRACCHECK handed over: four landed and three were added. Commit 6 —
> the Claims compaction route, a ~48-account frame wrapping the 36-account
> terminal frame — is a lane on its own. Everything §17.3 says about compute,
> frames and the already-paid terminal fixture still holds.

---

### 17.6 Two of the three layers, and the frame cost no build reports

Written by FRACCHECK-3, 2026-08-31, from building §17.5's two missing arms.
Evidence: `docs/evidence/FRACTIONAL_COMPACTION_COMPOSITION_2026_08_31.md`.

**A fractional compaction request now reaches route selection.** The composition
decode builds a `ClaimsExternalOnceV3` for `DCLTFCC1`, and `route_authority`
resolves it to its own caller authority and its own receipt kind. What stops the
transaction is one named place: the receipt verifier refuses, because no receipt
type exists to verify against, and admitting the route unverified would make
"verified" mean "unchecked" for one kind.

**The frame is 48, and 48 is 36 + 12.**
`FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1` reads the terminal frame's own constant and
adds twelve declared roles; the terminal half is never re-enumerated, for the
reason the payout derivation is called rather than re-implemented. Six of the
twelve are the native crank's own. The other six are what §17.4's hand-off costs:
the capability root and the Trading caller authority (which together *are* what
"Trading-composed" means), the shard Mint and **its own** Token program — separate
from the terminal frame's, which is the collateral mint's — the exposure terms,
and the terms-selected TokenBehavior.

Four more roles are declared and **refused**, each with a stated reason rather
than a bool: the two holder token accounts because §1.3's "positions are never
enumerated" is exactly what lets one transaction stand in for every holder; the
native claim-check record because FRACR3's unsignable-payee weld is the reason
this route exists at all; and the retirement cursor because a stalled ordered
walk must not block a permissionless crank.

**§17.5's table put the composition gap one crate too wide.**
`decode_selected_with_external` needed **no edit**: it already admits any
caller-authenticated external request at an exact fixed count and counts it as
the single mutation. The hole was that nobody ever *built* the value for anything
but the exposure magic. Edits in `dclutch-claims-svm` for that layer: zero.

**THE FINDING WORTH CARRYING: a 640-byte frame cost that every build reported as
zero.** The compaction request is 744 bytes and decodes into a struct embedding
the whole terminal header. Written as an ordinary arm, it put that struct on
`route_authority`'s frame — **3,072 bytes before, 3,712 after**, spare 1,024 down
to 384, on a link whose deepest function already sits at 4,032 of 4,096.
`cargo build-sbf` emits its diagnostic only at or past 4,096, so the jump was
invisible to the gate `tools/ci/run.sh` runs. `#[inline(never)]` on the arm
restores the exact base frame.

**So: any arm decoding a wire that embeds the terminal header must be split
behind `#[inline(never)]`, and the split must be measured.** Commit 6 decodes the
same 744-byte request inside a 48-account frame, and `claims-sbf`'s deepest
function already holds 3,776 of 4,096.

> **AMENDED — seventeen becomes nineteen, and both additions are surface.** The
> frame declaration was inside commit 6 and is separable (every layer above
> consumes it). And **5c is two commits**: `encode_request_profile` pins the
> exposure magic and action byte into the profile itself, so a compaction wire at
> a different width with different offsets needs its **own request profile**, not
> a fourth arm in a geometry match. **Nineteen commits, eleven landed, eight
> remain.** Commit 6 is still a lane, but it is now assembly rather than
> invention: every piece it calls exists and is named in the evidence.

---

### 17.7 Assembly met the frame, and the frame was not finished

Written by FRACCHECK-4, 2026-08-31, from attempting §17.6's commit 6.

§17.6 sized commit 6 as *"assembly rather than invention — every piece it calls
exists and is named."* Every piece does exist. Assembling them found **three
places where the declared frame cannot support the route it was declared for**,
and none of the three is visible from reading the declaration. They are recorded
here in the order the assembly hit them.

**1. The frame could not authenticate two of the records it carries. FIXED.**
`ExposureTerms` and `TokenBehavior` each had one account. Both are finalized
Registry records, and this tree authenticates one by its **raw/staging pair**:
`authenticate_finalized_rational_record` derives both PDAs, requires the raw
half to hash to the expected digest, and requires the staging cursor to be
*vacant* — the half that proves the record is not mid-update. Every sibling
carries the pair; `fractional_retirement_v3` carries it three times over
(`TERMS_RAW`/`TERMS_STAGING`, `TOKEN_BEHAVIOR_RAW`/`TOKEN_BEHAVIOR_STAGING` in
each of its begin, coordinate and finish frames), and the terminal frame this
route wraps carries its own exposure raw/staging at 21 and 22.

With only the raw halves the route can compare a digest and cannot prove the
record is settled — on the account that authors the denominator every holder's
payout is divided by. That weakening is invisible afterwards, because such a
route still reads its terms and still looks authenticated. **The frame is
therefore 50, not 48: `36 + 14`.** Still far below the lock limit, which the
existing compile-time assertion goes on proving.

**2. Trading never gives the root its signature on this route. CLEARED
2026-08-31** — ruled in §17.8 (ruling 1) and landed by FRACCHECK-5 in
`b3e3821c`; the gate admits the compaction kind, writability inverted, w1–w4
green and mutation-checked. See §17.9.
§17.6's third hazard says the root's signature "arrives because Trading's
`fractional_root_signer` adds it." It does not. That function's `matches!` gate
admits `FractionalAtomic`, `FractionalTerminalAtomic` and
`FractionalRetirementCoordinate`; `FractionalClaimCheckCompaction` falls to the
early `Ok(None)` and the root's meta is never marked a signer. The route
requires that signature for two jobs it cannot do without — the `SetAuthority`
that re-points `PermissionedBurn`, and the reserve Position's close — and Claims
can never produce a Trading PDA's signature. **The route is unreachable until
that gate gains the compaction kind**, with root index
`FractionalCompactionRoleV1::FractionalCapabilityRoot.index()` and the revision
read from the request's own `expected_root_revision`, which is the field's only
purpose.

Note the arm must *not* copy the exposure arms' `!root_account.is_writable`
requirement: those need a writable root because their effect program writes the
revision commit-last. A compaction revises nothing, and the frame accordingly
declares the root `(signer, not writable)`.

**3. `TradingCallerAuthority` has no program to derive it against. CLEARED
2026-08-31** — ruled in §17.8 (ruling 2, veto window exercised and signed off
in WAVE.md `794b2eda`) and landed by FRACCHECK-5 in `fa964511`: resolution
three, the role dropped to declared-and-refused, frame 49. w5–w6 green;
w7–w8 await the route. See §17.9.
The role is declared a required signer, its doc naming it as what "the
parent-authenticated close requires". Two facts collide. First, hazard 2 says
to *share* `close_and_split` rather than copy it — and `close_and_split`
performs no parent authentication; it zeroes two accounts' lamports and splits
the rent. Second, `execute_parent_authenticated_close`, which does authenticate
a parent, parses `CloseAccounts` out of whatever slice it is handed and so needs
the borrowed 15-account close frame to be a slice the caller can produce.
`fractional_retirement_v3` can hand it one because that frame *begins* with the
close frame (`accounts.get(..PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2)`). A
compaction frame begins with the 36-account terminal frame instead, so there is
no such prefix to hand over.

And the derivation is not available either way: `CallerAuthoritySeedsV1` must be
derived under the **Trading program id**, and no Trading program account is in
the frame. The terminal frame's `accounts[14]` is the caller program, but it is
read only on the `CallerRole::Trading` path; a fractional compaction request
pins `caller_role == CallerRole::Claims`, and under
`ParentAuthorityV3::ClaimCheckCrank` coordinate 0 is *the cranker, who is
anybody* — the deliberate relaxation that makes the crank permissionless.

So the frame carries a signer nothing can check. Whoever resolves this chooses
between: adding a Trading program account and verifying the authority directly
(51 accounts); restructuring so the borrowed close frame is presented at its own
coordinates; or **dropping the role**, on the ground that a permissionless crank
authorized by an elapsed deadline and a derived recipient does not additionally
need a parent's signature, and the reserve Position's close is entitled by the
root's own signature which the route already requires. The third is the smallest
and most likely correct, but it is a design decision and not a lane's to take
silently — it changes what "Trading-composed" means for this route.

> **AMENDED — commit 6 is not assembly, and the count is unchanged only because
> nothing was added.** Nineteen commits, twelve landed (FRACCHECK-3's eleven plus
> this frame correction), seven remain. Commit 6 stays a lane, and it is blocked
> on findings 2 and 3 rather than on effort: both are decisions about what
> authorizes this crank, and both must be answered before the route can be
> written truthfully. §17.3's compute and frame numbers are untouched.

---

### 17.8 Adjudication: one signature is load-bearing, the other never was

Ruled by FRAC-RULE, 2026-08-31, on §17.7's findings 2 and 3. Ground truth
read: `fractional_root_signer` and the receipt table
(`claims_composition_v3.rs`), `authenticate_parent_authority` /
`execute_parent_authenticated_close` / `authenticate_authority`
(`protocol_position_v2.rs`), the native crank and its `close_and_split`
(`claim_check_compaction_v1.rs`), the crank relaxation (`signed_delta_v3.rs`),
the role declaration (`fractional_claim_check_v1.rs`), and O-016.

**The sibling's answer first, because it is the measuring stick.** The native
compaction requires from Trading **nothing**: no Trading account in its frame,
`0x5601 Authority` refusing any signer the route does not admit, and a close —
`close_and_split` — with no authentication inside it, because the entitlement
(elapsed compiled deadline, coordinate re-derivation, zero balances,
conservation plan) was proved before it is called. That is the authority model
of a permissionless crank, and it is O-016 kept: every authoritative fact from
records, no caller input becoming authority by inclusion. Fractional differs in
exactly one fact the sibling does not have: a shard Mint whose
`PermissionedBurn` authority is a Trading PDA. Everything below follows from
letting it differ in only that.

**RULING 1 — finding 2: extend the gate. The root's signature is load-bearing
exactly once, and its once is the hand-off.** On this route the root signs one
thing that can happen no other way: `SetAuthority(PermissionedBurn)`, root →
escrow, which Token-2022 refuses without the *current* authority's signature
(`permissioned_burn_wall.rs`) and which nothing but a Trading `invoke_signed`
can produce — and nothing can produce after retirement. Not ceremonial
inheritance; it is the reason §17.4 made compaction Trading-composed at all.
The reserve Position's close is **not** a second job for it (ruling 2), and
after the hand-off the root signs nothing ever again — redemption is holder +
escrow. So: `fractional_root_signer`'s `matches!` gate gains
`FractionalClaimCheckCompaction`, and the kind-match gains an arm decoding the
compaction request — revision from the request's own `expected_root_revision`
(the field's only purpose), root index
`FractionalCompactionRoleV1::FractionalCapabilityRoot.index()`. Every existing
root-authentication predicate is retained — derivation, Trading ownership,
non-signer-on-entry, non-executable, release-set/market/terms/bump/revision —
except writability, which **inverts**: the exposure arms demand a writable root
because their effect program commits a revision; a compaction revises nothing
and the frame declares `(signer, not writable)`, so this arm requires root and
meta non-writable. Witnesses the builder must land: **(w1)** the arm marks the
meta signer and returns the root's seeds+bump (sibling of the gate's existing
tests); **(w2)** `expected_root_revision` mismatch refused; **(w3)** a
*writable* root refused on this kind — the inversion witnessed, so nobody later
"fixes" it back toward the exposure arms; **(w4)** the kept control: absent the
arm, the route refuses — today's state, preserved as the mutation witness. No
authority pin weakens — the gate grows one arm under the same checks, one of
them strictly tighter. No veto-window needed.

**RULING 2 — finding 3: drop the role. `TradingCallerAuthority` refuses no
attack, and the close it was declared for is owner-signed without it.**
Resolution three of §17.7's three, taken on this ground:

- *What it would refuse: nothing.* The crank is anybody —
  `signed_delta_v3.rs` asks of coordinate 0 under `(Claims, ClaimCheckCrank)`
  only that somebody signed. A caller-authority PDA is derived from the
  caller's own request digest; it proves Trading processed this exact request
  — and the root's signature already proves that, strictly more strongly,
  because Trading marks the root signer only after `fractional_root_signer`
  authenticates the root's bytes against the same request. A stranger without
  Trading dies at the hand-off (`MissingRequiredSignature`); a cranker through
  Trading is legitimate by design. A second Trading signer standing beside the
  first is O-016's exact shape of ceremony: inclusion mistaken for authority.
- *The close is entitled without it.* The reserve Position's owner **is** the
  root, and the root signs this frame for the hand-off — so the compaction
  close is owner-signed, deadline-entitled, and record-authenticated: strictly
  stronger than the native close whose `close_and_split` it shares (§17.6
  hazard 2, kept). `execute_parent_authenticated_close` and
  `authenticate_parent_authority` remain untouched, retirement's own: their
  caller-PDA-plus-root pair is real *there*, where the close is ordered by
  Trading's retirement walk rather than entitled by a deadline.
- *What "Trading-composed" now means for this route:* composed **for
  signature, not for authority**. Trading is where the root's signature
  exists; the deadline and the records are what authorize the crank — the same
  authorizer as the native sibling, which is the agreement the design wanted.

The builder lands: the role flips from admitted to **declared-and-refused**
with its own reason variant beside `RefusedNamesOneHolder` and kin — refused
because a deadline-entitled permissionless crank takes no parent's authority,
and the owner's own signature, the root's, already covers the close. The
`FractionalCapabilityRoot` doc-comment's "whose signature the
parent-authenticated close requires" ground is rewritten to this section's in
the same commit. Frame becomes **49 = 36 + 13**; the lock assertion and the
indexed-or-refused test carry it. Witnesses: **(w5)** exactly one signer among
the thirteen added roles — the root; **(w6)** frame count 49 and
`TradingCallerAuthority.index() == None`; **(w7)** the route executes with no
caller-authority account anywhere in frame — the drop run, not asserted;
**(w8)** direct-entry hostile: the Claims route without Trading, root
unsigned, refuses at the hand-off — Trading-composition enforced by the root
signer alone.

**⚠ VETO-WINDOW REQUIRED for ruling 2.** It removes a declared required signer
from a landed frame declaration. The analysis says that signer authorized
nothing; removing it is still a weakening of the declared authority surface,
and it ships only with the orchestrator's sign-off — not on a lane's judgment,
including this one's.

**Refused, and deliberately not done.** Refused: the 51-account
Trading-program addition (an account whose only job is deriving a PDA that
authorizes nothing) and restructuring the frame to expose a close prefix
(terminal-frame-first is why the payout derivation is called rather than
re-implemented). Not done: no change to retirement's close path, to the native
crank's `TradingRecord` refusal, or to any §17.3 compute number; the root
keeps its hand-off job — the close stops needing parent ceremony, not the
root. Count unchanged: nineteen commits, twelve landed, seven remain — both
rulings are edits inside commit 6's lane plus the frame-declaration amendment
it must carry; nothing added.

---

### 17.9 Both signatures settled, and the frame measured rather than assumed

Written by FRACCHECK-5, 2026-08-31, building §17.8's two rulings.
Both landed. Witnesses w1–w6 are green and every one of them was checked by
mutation; w7 and w8 are route and campaign witnesses and remain unwritten
because the route does.

**Ruling 1 landed** (`b3e3821c`). `fractional_root_signer`'s gate admits
`FractionalClaimCheckCompaction`. Revision comes from the request's own
`expected_root_revision`, root index from
`FractionalCompactionRoleV1::FractionalCapabilityRoot.index()`, and every
existing root predicate is retained except writability, which inverts.

The inversion is **not** a second `matches!` beside the predicates. The arm
that decodes a request now returns a `FractionalRootExpectationV3` carrying its
own `writable`, so a kind cannot acquire one arm's revision and another arm's
privileges — the two facts have one author each, which is the same rule the
frame declaration applies one level down. The meta check inverts with the
account check, because authenticating a read-only root and then handing the
child a writable one is precisely what checking the meta separately is *for*.

**Ruling 2 landed** (`fa964511`). `TradingCallerAuthority` is
declared-and-refused under its own reason, `RefusedTakesNoParentAuthority`, and
the frame is **49 = 36 + 13**. `execute_parent_authenticated_close` and
`authenticate_parent_authority` are untouched.

#### The frame cost, measured on both sides of the split

§17.6 left commit 6's author a hazard and a method: any arm decoding a wire
that embeds the terminal header must be split behind `#[inline(never)]`, and
the split must be *measured*. Ruling 1's arm is the first such arm, and it was
measured — three real builds of the Trading link, not two:

| function | base | `#[inline(never)]` | `#[inline(always)]` |
|---|---:|---:|---:|
| `fractional_root_signer` | 1,088 | **1,088** | 1,408 |
| `fractional_compaction_root_expectation` | — | 1,984 | inlined away |
| `claims_composition_v3::route_authority` | 3,072 | 3,072 | 3,072 |
| deepest frame in the link | 4,032 | **4,032** | 4,032 |

So the split is load-bearing rather than ritual: without it the shared signer
function pays **+320 bytes** for a route most of its callers never take, and
`cargo build-sbf` reports zero either way. The third build is the point — the
hazard §17.6 wrote down was previously supported by one observation on
`route_authority`; it now has a second, on a different function, in the same
link, with the cost isolated to the attribute.

The Claims link's baseline is unchanged and re-measured for whoever writes the
route: `custody_replay_v1::process` at **3,776 of 4,096, 320 spare** — exactly
where §17.6 left it.

#### What the witnesses actually caught

Every witness below was run against a mutation that should red it. This matters
more than the count: §17.6's own lane found a consecutive-index test whose name
described a property it did not have, and only the mutation could tell.

| # | property | mutation that reds it |
|---|---|---|
| w1 | the arm authenticates a read-only root, marks the meta signer, returns seeds that re-derive that root | remove the kind from the `matches!` → fails at "returns a signer rather than None" |
| w2 | `expected_root_revision` off by one is refused | — (direct assertion) |
| w3 | a **writable** root is refused on this kind | **drop the writability predicate entirely** → w1, w2, w4 stay green and ONLY w3 reds |
| w4 | a kind the gate does not name gets `Ok(None)`, meta untouched | (is itself the control preserved from before ruling 1) |
| w5 | exactly one signer among the thirteen, and it is the root | make `ShardMint` a signer → `left: 2, right: 1` |
| w6 | frame count 49, `TradingCallerAuthority.index() == None` | restore the role properly → **fails to compile** on the spelled-out order array |

w3 is the one worth keeping in view. Its neighbours require the opposite
writability, so the plausible future edit is not malice but tidiness — a reader
"restoring consistency" with the three arms above. The permissive mutation
(deleting the predicate rather than inverting it) is invisible to every other
witness in the file.

#### Two pieces of arithmetic that were already wrong

Found while editing, not looked for, and recorded because both had survived a
lane each:

- the role enum's doc said "the other **six** are what §17.4's hand-off costs"
  while FRACCHECK-4's raw/staging pairs had made it eight. It is seven now.
- three comments said "a **thirteenth** account", meaning the next one past
  twelve, and had been wrong since the frame reached fourteen. They are
  count-independent now, so they stop rotting on the next change.

Neither was load-bearing. Both are the kind of drift that makes a reader trust
a number they should have recomputed.

#### `action_geometry` no longer has a wildcard

(`e6467b91`, part of 5c-ii.) It ended in `_ => Err(InvalidInput)`, which
conflated two different facts: an action refused because somebody looked at it
and it has no Claims frame, and an action refused because nobody has looked at
it yet. An eighth `FractionalExposureActionV2` would have compiled and been
silently unsupported by the file whose job is to say what each action's
artifacts are. All seven variants answer now; `Transfer`, `Terminalize` and
`ZeroSupplyRetire` are refused in a named arm with a reason each. Verified by
mutation: dropping one arm is E0004 at `artifacts_v4.rs:888`. The test that
covered the wildcard covers all three refusals instead of `Transfer` alone.

#### The honest state, and it is narrower than §17.7's

**Nineteen commits, twelve landed, seven remain — the count does not move, and
that is §17.8's own accounting, not a lane's modesty.** Both rulings are "edits
inside commit 6's lane plus the frame-declaration amendment it must carry;
nothing added". So the three commits below buy commit 6 its preconditions and
part of 5c-ii; they do not retire a numbered commit between them.

| # | commit | status |
|---|---|---|
| 1–4, 5a, 5b/5, 5d, 10a, 12 | FRACCHECK…FRACCHECK-4's twelve | landed |
| — | ruling 1: the gate's compaction arm | **landed** (`b3e3821c`), inside 6's lane |
| — | ruling 2: the frame at 49, the role refused | **landed** (`fa964511`), the frame amendment |
| 5c-ii *(part)* | `action_geometry` made exhaustive | **landed** (`e6467b91`) |
| 5c-i | a request profile for the compaction wire | not written |
| 5c-ii *(rest)* | `encode_effect` / `encode_account_profile` + the lock-count row | not written |
| 6 | the Claims compaction route | not written — **still a lane**, now unblocked |
| 6b | the receipt type, and the verifier arm it turns green | not written |
| 7, 8, 9, 10, 11 | redemption, `RetireCoordinate`, escrow close, campaign, operator | not written |

The one number that *did* move is the one §17.7 left open: commit 6's blockers
went from two to zero.

**Commit 6 is unblocked and is still not assembly.** §17.7 reported it blocked
on two decisions; both are now made and implemented, so the two named stops are
gone. What is left is what §17.6 sized: the frame guard and authentication
walk, the `SetAuthority` CPI and its post-hand-off re-read, the plan, the record
write, the escrow increment, the close-and-split, and the dispatch arm — against
a native sibling of 1,190 lines, carrying seven more accounts and one more CPI
leg. That is a lane, and this lane did not open it. A route written but not
driven would be the thing §17.6's own evidence warns about: green, and unproved
where it counts.

**One thing the next lane should not rediscover.** The private-native-helpers
hazard (§17.6 hazard 2) still stands — `write_claim_check`, `close_and_split`,
`allocate_and_assign`, `observation` and `token_balance` are all bare `fn`
inside `claim_check_compaction_v1`. This lane deliberately did **not** widen
their visibility, because widening it for a caller that does not exist yet is a
change with no test behind it; the sharing and its first consumer belong in one
commit, so the amended lamport order (rent, crank, opener debt, residue) never
has two authors even briefly.

> **CORRECTION to hazard 2, checked against the signatures: the five do not
> share alike. Four can; one cannot.**
>
> - **`close_and_split` shares unmodified, and it is the one that matters.** It
>   takes `&ClaimCheckCompactionPlanV1`, and
>   `FractionalClaimCheckCompactionPlanV1::shared()` returns exactly that type
>   *by value* — so `close_and_split(position, admission, cranker, opener,
>   rent_credit, &plan.shared())` type-checks today with no signature change.
>   This is what gives the amended four-credit order one author across both
>   routes, which is the whole of what hazard 2 was protecting.
> - **`allocate_and_assign` shares unmodified**, being generic over seeds,
>   width and owner. `observation` and `token_balance` likewise.
> - **`write_claim_check` cannot be shared, and asking it to would be the
>   mirror.** It is hard-typed to `ClaimCheckSeedsV1`, `ClaimCheckV1` and
>   `CLAIM_CHECK_BYTES_V1` (**288**). The fractional record is
>   `FractionalClaimCheckV1` at `FRACTIONAL_CLAIM_CHECK_BYTES_V1` (**320**)
>   under `FractionalClaimCheckSeedsV1`. Both seed types do return `[&[u8]; 3]`,
>   so the *body shape* matches — but the honest share is
>   `allocate_and_assign`, which `write_claim_check` already delegates to.
>   Writing a thin fractional writer over it is not duplication; re-deriving
>   the seed order would be.
>
> And one decision the sharing forces, so it is taken deliberately rather than
> inherited: `close_and_split`'s internal refusals are
> `ClaimCheckCompactionSbfErrorV1` (the `0x5600` band). A fractional route
> calling it surfaces native-band codes for a close failure rather than its own
> `0x5640` band. That is arguably correct — it *is* the native close — but it
> is a visible consequence in a validator log, and the alternative (wrapping at
> the call site) costs the shared authorship this correction is about.
>
> `execute_claim_check_compaction` needs no change at all: already
> `pub(crate)`, already the right shape, already pinning
> `caller_role == CallerRole::Claims`. Neither does the SBF refusal enum —
> `FractionalClaimCheckCompactionSbfErrorV1` and its `0x5640`–`0x564C` band
> assertions are landed and waiting for a route to raise them.

#### Not verified

- **No route, so no route CU.** §17.3's ~928k projection is still a lower bound
  on a route that does not exist, and the 49 is still a declaration rather than
  an observation: nothing has yet built a 49-account transaction.
- **w7 and w8 are unwritten**, and they are the two that would make ruling 2
  empirical rather than argued — w7 that the route runs with no caller-authority
  account in frame, w8 that a direct Claims entry without Trading refuses at the
  `SetAuthority` hand-off. Ruling 2's analysis stands on the ground §17.8 gives
  it; it has not been driven.
- **The gate arm is unit-tested only**, at exactly the level its three
  neighbours are: no Trading program-test drives `fractional_root_signer` for
  any kind, because fractional retirement is reachable from a test caller and
  not from a `Market*` (§17.5). The four witnesses run over real encoded
  requests and a real decoded root, not a mock; they do not run on chain.
- **`cargo check --workspace --all-targets` is clean** and the three existing
  gate tests are green, which is the umbrella control for the 50→49 change.
- **Unrelated pre-existing red, not this lane's:** `cargo clippy -p
  dclutch-direct-codec` fails with three `slicing may panic` denials. Identical
  at this lane's base commit and after; recorded so the next lane does not
  attribute it.
- **The remainder still goes nowhere**, the 180-day deadline is unchanged, and
  **`ClaimsCapability` is still stranded** — all three exactly as ruled.
- **No devnet write.**

---

### 17.10 The route ran, and the table came off the chain

Written by FRACCHECK-7, 2026-08-31, from building the ruled fiftieth account and
the campaign. Commits: `604215bd` (the frame at 50), `6d624b6e` (the unset-owner
adjudication), `d704283e` (shared fixture encoders), `4fb425ec` (the campaign).

**Commit 10 is done, and §17.9's "not verified" list is now three lines
shorter.** The route has a CU number (579,240, against §17.3's ~928k lower-bound
projection), the 50 is an observation rather than a declaration, and w7 and w8
are driven rather than argued.

#### The ruled fiftieth account, and where its authority actually comes from

WAVE `b4546291` ruled the Rent program into the frame so `authenticate_rent_credit`
could run. The question the ruling left open is the one that matters: pinned
against *what*? The compaction request carries no rent field, and letting the
supplied program name itself is what `fractional_retirement_v3`'s finish does —
safe there only because the cursor fixes the address first.

It comes from the **reserve Position's admission**, which this route already
decodes one screen earlier for its owner kind, and which persists the RentCredit
and its Rent program together (`EVIDENCE_RENT_CREDIT_OFFSET` /
`EVIDENCE_RENT_PROGRAM_OFFSET`). Both halves pinned from one Claims-authored
immutable record — one conjunct stronger than the sibling, and the change is
monotone: FRACCHECK-6's three content conjuncts are made by
`authenticate_rent_credit` itself, so nothing checked stopped being checked.

The account went **last** in the frame, not beside the credit it authenticates.
An existing witness settled it: the readable placement pushes `SystemProgram` off
index 41 and silently ends the asserted parity in which the first six of this
frame are the native crank's own six in the native crank's own order. That parity
is what lets the two routes' tails be read side by side, and it is load-bearing
for a thread whose whole discipline is one author per number.

Its refusal is its own code, `0x564D Rent`, not a fold into `Identity`. On this
route `Identity` means a coordinate did not derive an account — all of them this
program's own PDAs. This one means the residual beneficiary is wrong. Folding
them makes "your rent is going somewhere else" indistinguishable from a mistyped
escrow in a validator log.

#### What the campaign found that reading the code could not

Three, and each cost a real refusal to find:

1. **Coordinate 14 is the caller program, and a compaction's caller is Claims.**
   Copying the sibling terminal frame — which puts its Trading-role test caller
   there — is refused `0x5202`. The release authentication resolves that
   coordinate against the activation cache's binding for whichever role the
   *request* states.
2. **Coordinate 0 must be writable.** The sibling carries a read-only caller
   authority; a compaction carries the party the sweep rewards, and the runtime
   itself refuses `ReadonlyLamportChange` the moment `close_and_split` credits it.
3. **The Claims-role Custody replay must exist first.** `authenticate_custody_accounts`
   refuses a cursor whose bytes are not exactly `CUSTODY_REPLAY_BYTES_V1`, and an
   empty account is the shape a campaign gets for free. Created by its own real
   route, never planted.

#### The conservation table, and the two things it said that arithmetic would not

| | |
|---|---:|
| hoard | 10,000 → 9,993 |
| vault | 0 → 7 |
| payout / whole claims / rate | 7 / 7 / 1 |
| swept (position + admission) | 6,681,624 |
| → claim-check rent | 3,118,080 |
| → opener repaid | 3,363,544 |
| → cranker reward | 200,000 |
| → RentCredit residue | 0 |
| → **burned** | **0** |
| opener outlay / still out | 4,711,920 / 1,348,376 |

**The residue is zero, and the first version of the campaign asserted it could
not be.** Zero is correct — the record's rent and the crank rank above the
opener, and two closed accounts did not hold more than those three claims.
A campaign demanding a positive residue is demanding a fixture rich enough to
leave one, and would have called a correct route wrong. "The remainder goes
nowhere" is the absence of a fifth term in the equation, not a positive number.

**One compaction does not make the opener whole** — 1,348,376 short here. That is
the amended order working, and it is now asserted from the other side (the sweep
may never pay the opener *more* than they advanced), so a later change that
over-repays them out of the residue has to argue with a line.

#### The no-claim branch is reachable only through supply, never through rate

Added after the campaign landed, from writing the witness for the other half of
the `mints_claim_check() == (escrowed_atoms != 0)` weld. The paying campaign
proves the minting direction; the non-minting direction is the one where a bug
is unrecoverable, because an authority handed to an escrow that will never hold
a claim is a Mint whose shards nobody can burn — and after retirement the root
cannot hand it back.

Writing it found that **a zero rate cannot express this case at all**. The wire
refuses `payout_per_claim == 0` as `InvalidEntitlement`, on the stated ground
that "a rate of zero promises a record nobody would ever redeem". So the plan's
`escrowed_atoms == 0` branch is unreachable through the rate, and the conservation
plan's own note — *"there is deliberately no separate refusal for a zero rate"* —
is true but describes a case the request type has already refused one layer up.

The branch is reached through **supply**: a coordinate the market resolved away
from has had its shards burned by its holders (`TerminalZeroBurn`), so the
outstanding supply is zero, zero whole claims form, and `whole_claims × rate` is
zero whatever the rate. That is the lifecycle rather than a fixture convenience,
and it is the only state the protocol can actually be in when a fractional
compaction escrows nothing. The witness models it that way and observes: no
collateral moves, no record is minted, and the shard Mint's bytes are
**byte-identical** across the crank — asserted over the whole account rather
than over the extension a reader would think to check.

It also caught a defect in the campaign's own host-side reproduction: the
terminal scenario was hardcoded to the reserve's coordinate, so on a market that
resolved elsewhere the reproduction went on computing a paying scenario while
the chain computed a worthless one. The two would have disagreed silently, which
is exactly the failure the reproduction exists to make impossible. It now reads
the winner from the same fact the chain does.

#### Still not verified, and named

- **No devnet.** Everything is `solana-program-test` against real ELFs.
- **No ALT.** A `ProgramTest` bank enforces the 64-account lock limit and not the
  1,232-byte packet size, so the campaign sends a 51-account legacy transaction
  exactly as the sibling terminal campaign does. On a real cluster this frame
  needs the table, and that is untested here.
- **The root is derived under the test signer, not under Trading**, which
  `fractional-compaction-caller` documents and cannot avoid. The two halves meet
  in the design, not in one test.
- **The campaign runs at width 8 on one market with one coordinate.** It is an
  existence proof for the route and the arithmetic, not a sweep.
- **The route collapses every inner error to `Economic`.** Debugging the campaign
  required temporarily un-mapping it to see `0x5202` and `0x5002` at all. That
  lossiness is real and is left as named debt: widening it is a refusal-surface
  change, and this lane had already spent its frame budget.
- **`ClaimsCapability` is still stranded**, the 180-day deadline is unchanged, and
  the remainder still goes nowhere — all three exactly as ruled.
