# Claim-check compaction — the perpetual claim without the perpetual market

Status: **PARTLY IMPLEMENTED — read §15 before acting on §4.7 or §6.2.** The
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

Charter item: **3 — permissionless completion universalized** (`GOAL.md:39`).
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

Ember's ruling, verbatim (`WAVE.md:1183-1186`):

> Q3: option (c) ratified — perpetual CLAIM, not perpetual account:
> post-deadline compaction to a durable claim-check; market accounts close;
> the holder's right survives redeemable forever. No arbitrary actor may
> insert arbitrary delays into protocol operations.

And the rationale as given to this lane:

> liveness issues aren't ok, we can't be allowing random arbitrary actors to
> insert arbitrary delays into our own operations.

And on the follow-on (`WAVE.md:1186-1187`):

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
(`docs/design/MAINNET_STATE_RELAY.md:989-1006`): *"They are liveness deadlines,
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
(`GOAL.md:482`) and the wave's freeze policy rides everything after the first
probe-green revision to the next cohort (`GOAL.md:361-362`). Not on the devnet
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
