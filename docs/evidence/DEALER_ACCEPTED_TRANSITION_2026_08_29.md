# Dealer accepted transition: the executed lifecycle

Date: 2026-08-29
Status: executed end to end in ProgramTest against real ELFs; **not a live
capability** — no devnet instance, no selected Market, no public caller

This is a Dealer scenario accepted-transition campaign. It selects no price,
quotes nothing, and holds no inventory. It is not an AMM, an order book, or a
quote surface.

## What changed

The Dealer family had a kernel, a layout owner, artifact builders, equity
selectors and refusal tests, and no caller. `tests/physical.rs` said so in its
own header: neither of its cases was an acceptance test, because acceptance
needed a complete scenario chain that was "the unwritten campaign staged in
`crate::dealer_chain`".

It is written, and it runs.

## Reproduction

One command, from a clean checkout, no environment variable to set and no
fixture to prepare:

```sh
cd /Users/ember/dev/dclutch
bash programs/dclutch-dealer-accelerator-sbf/program-test/run-program-test.sh
```

Measured cold on 2026-08-29 from a deleted target directory: **3m05s wall, exit
0, 26 tests across four targets**, 21 of them the accepted campaign. Re-measured
warm on the same day with the delivery leg and its hostiles landed: **exit 0, 30
tests across the same four targets**, 25 of them the accepted campaign. The runner
builds five real artifacts — the Dealer accelerator, its test caller, Trading,
Custody, Claims and Core — and stages them as real loadable deployments.

## Why the lifecycle is split at all

The canonical unsplit admitted Hot instruction for a Dealer scenario resolves
more account locks than the runtime permits — for the campaign's own derived
frame, 119 distinct locks against a 64-lock ceiling — and no address lookup
table changes that ceiling. The unsplit form is unsubmittable everywhere:
devnet, mainnet, and this harness alike.

The lock-bounded checkpoint routes are the same transition in the form a caller
can send. `build_dealer_accepted_transcript_v4`
(`crates/dclutch-operator/src/dealer_scenario_checkpoint_v1.rs`) is that
transition as one ordered object: create, six canonical membership pages,
evaluate, one reservation per evaluated Custody effect, and the atomic commit.
It refuses the whole transcript unless every constituent transaction is
independently lock-bounded and packet-safe.

A second transport fact fell out of executing it: the commit route does **not**
fit the 1,232-byte packet ceiling with static addresses. A real caller must
build an address lookup table and submit commit as a v0 transaction. The
campaign creates a live table on chain and does exactly that.

## The endings

The lifecycle has two terminal shapes for the checkpoint, and both are executed.
They are not a chain; the second is not reachable from the first, by design. A
committed checkpoint has a third continuation, delivery, which is executed too
and has its own section below.

**Committed.** `Create → Page(0..5) → Evaluate → Reserve → Commit`. Trading
authenticates the whole frame and invokes the real Claims program at CPI depth
two; Claims accepts the SignedDelta and returns its receipt.

**Abandoned.** `Create → Page(0..5) → Evaluate → [expiry] → Cleanup`. The
checkpoint closes, keeps neither state nor lamports, and every lamport it held
reaches the beneficiary fixed at creation.

A committed checkpoint refuses cleanup and keeps its rent. That is deliberate
rather than missing: Custody delivery is a later permissionless, resumable
effect that still refers to the checkpoint, so its rent is not abandoned state
to sweep. `cleanup_beneficiary` rejects a `Committed` checkpoint, and the
durable journal refuses `record_cleaned` once committed. The campaign pins the
boundary as evidence.

Throughout, the durable `DealerScenarioCheckpointJournalV1` advances only on
observed chain state, so the campaign cannot claim progress the validator did
not make.

## Conservation, not acceptance

The commit case asserts where the value went, not that the transaction
succeeded:

- exactly what the trade acquires leaves the counterparty and arrives at the
  dealer;
- every other representation coordinate is untouched;
- the two Claims Positions net to zero at every coordinate;
- the obligation account is replaced by exactly the candidate body the request
  committed to, which commit independently re-derives from the current body and
  the request's own candidate vector.

## The delivery leg, executed

Delivery is the third terminal shape, and it is now executed too:

**Delivered.** `Create → Page(0..5) → Evaluate → Reserve → Commit → Deliver`.
Custody re-authenticates the whole graph beneath the request — Market, Realm,
replay cursor, adapter profile, Mint, custody authority and vault addresses —
then invokes the real token program, moves the locked collateral out of its
escrow, closes the escrow, advances the replay cursor and writes its receipt.

It is not reachable any other way. `activate_batch` refuses every checkpoint
whose phase is not `Committed`, so delivery is chained after the executed commit
rather than staged beside it.

The case asserts where the value went:

- the destination is credited **exactly** the amount the reservation locked;
- the escrow is **closed**, not merely emptied — the account is gone;
- every lamport of escrow rent reaches the beneficiary fixed at reservation;
- the source vault the reservation already debited is byte-identical afterwards;
- the standard Custody replay cursor advances exactly one revision for one
  delivered effect;
- the batch and the reservation state are both `Activated`, and the activation
  receipt exists naming the checkpoint, request digest and batch it delivered.

Three things the leg needed, beyond what reservation needed:

**A real nested Custody request in each effect body.** The effect bank is built
by the semantic owner, `encode_dealer_scenario_custody_effect_artifacts_v1`, from
a `ScenarioCustodyEffectV3` carrying a real `CustodyRequestV1`. A zero payload
reserves and can never deliver: `require_activation_effect_join` decodes
`effect.custody` and joins its release set, Market, Realm, caller program, parent
request digest, generation, transfer index, Mint and token program against the
batch.

**A content-addressed Realm.** `require_realm_authority` checks
`hash(realm body) == request.realm`, so the campaign cannot *name* a Realm
identity: it builds the record and every layer below — the Core Market's own
`CoreState` included — restates the digest. The Realm's
`collateral_adapter_release_id` must hash-match an entry of
`PRODUCTION_ADAPTER_RELEASES` or Custody refuses `Realm`; the legacy adapter is
selected because `solana-program-test` genesis already carries the real token
program at the address that adapter names, so the transfer and the close are
executed by a real ELF with no external artifact and no environment variable.
The one-command reproduction is unchanged.

**A real replay cursor.** `CustodyReplayV1` at its derived PDA, Custody-owned,
whose byte image must hash to what the reservation batch pinned, and whose
`next_revision` must be exactly the request's `expected_revision`. It is the
reason a delivery can be submitted at most once.

`programs/dclutch-dealer-accelerator-sbf/program-test/src/custody_delivery.rs`
derives that whole collateral graph once, in the direction Custody derives it, so
no coordinate is chosen twice.

## Two more protocol defects, both always-refuses

The delivery leg was unreachable for **every batch that could ever have
committed**, for two independent reasons. Both are the same family as the two
above: an identity two sides derive independently and disagree about, invisible
to either side's own tests.

**The effect producer could not be in the frame.** Trading's commit refuses any
effect manifest whose `producer_program` is not the Trading program itself
(`dealer_scenario_checkpoint_v1.rs`, two sites, `program_id` equality). Custody's
activation frame carries the effect producer at index 9 *and* the calling Trading
release at index 3, and then runs a duplicate-key census over the whole frame. The
frame was required to repeat one key and forbidden to repeat any key. Both the
operator builder and the on-chain check refused, unconditionally. The equality is
now pinned positively on both sides, and that one slot is excused from the
census — because the protocol itself mandates the repetition.

**The System program's address is the default public key.**
`build_dealer_scenario_activation_v1` refused any frame containing
`Pubkey::default()`, as a guard against unset fields. The activation frame
mandatorily carries the System program, whose address is thirty-two zero bytes.
No activation packet could be built, for any input. The System program is now
pinned by identity and excluded from that census.

Worth generalizing: a "no unset field" guard written as a comparison against the
default public key is a trap in any frame that carries the System program.

The operator's own activation test passed against the unbuildable frame, because
it supplied an arbitrary producer and an arbitrary system program — neither of
which the protocol admits. Corrected to the real frame, its lock census
legitimately drops from 37 to 36 (the producer is not a distinct lock) and its
wire size rises from 285 to 315 bytes (the System program cannot live in a lookup
table, so it is always a static address).

## The Claims graph is consumed, not reinvented

The Claims aggregate graph comes from FRAC's width-parameterized fixture,
`compile_narrow_fixture_v2` in
`programs/dclutch-claims-sbf/program-test/fractional-atomic/src/narrow_fixture.rs`,
consumed as a path dependency. Its `solana-program-test` dev-dependency does not
propagate, so a crate on a different program-test major consumes it cleanly. No
Dealer file restates a Claims coordinate.

One term did not map, and it is a real difference between the families rather
than a defect. The fixture plants Positions at revision zero, which is the
correct pre-founding shape for a founding campaign; the Dealer projection
refuses a Position at revision zero outright (`validate_projection` in
`programs/dclutch-trading-sbf/src/dealer/v3_trade.rs`), because a Dealer trade
is against Positions that have already been transacted. The campaign re-encodes
the aggregate and both Positions at a live revision through the supported
`dclutch-claims-svm` encoders — not a byte patch, and not a second derivation.

## Two protocol defects, found by driving the machine

Both sat exactly at a seam between two programs that each passed their own
tests. Neither was reachable by a component test, because no component test
derived the address the other program used.

**Four PDA domains exceeded the maximum seed length.** The reservation batch
(35 bytes), reservation state (35), reservation escrow (36) and activation
receipt (36) domains were all over Solana's 32-byte limit. A seed that long
makes the address *underivable* — not unusual, impossible. The entire Custody
reservation, escrow and activation address family could never be created by
Custody nor authenticated by Trading. It was unreachable code.

The fix that matters is the guard, not the shorter strings: every domain in
`dclutch-dealer-codec` now carries a `const _: () = assert!(len <= 32)`. There
was no seed-length guard anywhere in the tree before this.

**Trading and Custody disagreed on the batch address.** Custody signs the
reservation batch under two seeds — the domain and the checkpoint — at all five
sites including the `invoke_signed` that creates the account. Trading's commit
derived a third seed, the request digest, and so computed an address Custody can
never create. No Custody-produced batch could ever have been authenticated at
commit. Trading drops the redundant seed; the checkpoint is itself derived from
the request digest, and commit still checks the digest carried in the decoded
batch body. `dealer_scenario_reservation_batch_address_v1` is now the one
supported derivation, with a test pinning both directions.

A third, smaller correction: the Dealer evaluation artifacts must be
**Trading-owned**. Commit requires the effect manifest to name Trading as its
producer; evaluate requires it to equal the evaluate-producer account; the body
cannot change between them. An evaluator that is not Trading passes evaluate and
reserve in isolation and can never commit.

## The eighteen hostiles, by the check each reaches

Every case asserts the exact refusal code and re-reads the checkpoint to prove
no mutation survived. Where a case could have been answered by a shallower
check, it is built to get past that check first — a hostile that stops early is
not the hostile it claims to be.

Inside delivery that means re-sealing: a case that edits a Custody-owned body and
leaves the digests committing to it alone is answered by the digest, not by the
gate it names. Each such case recomputes the batch's or the reservation's own
pinned digests around its lie, through the supported codecs. The cursor case
carries its own control — the same lie submitted *unsealed* first, shown being
answered by the batch before any token program runs — so the seal is
demonstrably load-bearing rather than decorative.

| case | reaches |
|---|---|
| substituted membership member | manifest-committed page digest |
| malformed membership manifest (PDA, owner, width and validity kept; two committed page digests swapped) | checkpoint-bound manifest digest |
| replayed page ordinal | page state machine |
| wrong dealer authority (a real signature from a wallet the request does not name) | request-named dealer owner; checkpoint left vacant |
| substituted candidate body under a genuine receipt | evaluation body digest |
| unactivated reservation producer (a real program, owning a coherent receipt and state, as a third distinct identity) | Registry activation-cache role |
| activation cache for another release generation (correct owner, width, all roles) | cache release-set header |
| locked value blocks abandonment | reservation/rollback balance |
| commit before any value is locked | checkpoint phase |
| Claims delta that is not the request packet (published *and* sealed, so every digest agrees with itself) | packet identity |
| locked batch naming another receipt (PDA, owner, width and validity kept) | batch/checkpoint receipt agreement |
| caller authority bound to another request (a real Trading PDA through the real seed constructor) | request-scoped authority derivation |
| Claims Position table permuted (both real Positions, real bodies, real privileges) | Claims canonical owner ordering |
| replayed commit (byte-identical, same live table) | checkpoint phase; Positions do not move twice |
| committed checkpoint refuses cleanup | committed-versus-abandoned boundary; rent retained |
| replayed delivery (byte-identical, same live table) | activation-receipt vacancy; collateral, cursor and batch do not move twice |
| replay cursor one revision ahead (batch prestate digest re-sealed over the tampered cursor) | `CustodyReplayV1::advance` — *after* the transfer, proven by the token program in the transaction log |
| substituted destination (a third token account, real, same Mint, same balance, different owner; reservation re-sealed to name it and commit to its prestate) | the external destination owner the request names (0x6006, past every 0x6005 reservation join) |

## Named debt

**The reservation evidence is staged, not Custody-produced.** The reservation
batch, state and receipt are installed as Custody-owned accounts carrying real
coordinates and real digests — the executed routes authenticate them exactly as
they would authenticate Custody's own output, and delivery refuses unless those
digests are what the chain actually holds. But Custody's reserve route did not
write them. That is the same evidence class the reserve leg already had, now
carried one leg further.

**One effect, one coordinate.** The delivery is a single-effect batch. The
activation frame admits up to four, and the operator builder is exercised at four
in its own tests, but only one is executed end to end here.

**The campaign trades one representation coordinate.** FRAC's fixture funds one
coordinate per Position, and the Dealer intent validator requires `acquired` and
`delivered` to be disjoint per coordinate (`validate_intent`: a coordinate
carrying both is `InvalidIntent`). So the executed commit moves value one way at
the funded coordinate. This is a narrowing of the campaign, not of Dealer:
Dealer scenarios trade across coordinates in general.

**The request's named destination is pinned only transitively.** At activation,
the frame's destination account is compared to the *reservation's* destination,
and the reservation is Custody-written, so in the live protocol the two agree.
But `original.destination` is never compared to the frame account directly: the
pin runs through the reservation rather than through the request. Nothing is
exploitable — a reservation state is a Custody-owned PDA and cannot be forged —
but it is one link longer than it needs to be, and the campaign's own staged
reservation is what made it visible.

**The membership frame's identities are derived but synthetic.** The six-page
transcript is the real physical frame — the account profile fixes every
coordinate, width and privilege, the semantic projection fixes the spans and the
caller-authority count, and `project_dealer_scenario_unsplit_topology_v4` is run
for real — but the frame is compiled from a fixture rather than observed from a
founded Market. Its collateral coordinates are no longer synthetic: the census
names the same Mint, token program, replay namespace and trading-principal vault
the executed delivery moves, and the principal balance it states is exactly the
sum of what the reservation left behind and what the escrow held.

**No devnet anything.** No selected Market, no live participant, no public
caller. This is executed evidence, not a capability.

## Anchors

- campaign: `programs/dclutch-dealer-accelerator-sbf/program-test/tests/accepted.rs`
- runner: `programs/dclutch-dealer-accelerator-sbf/program-test/run-program-test.sh`
- transcript and journal: `crates/dclutch-operator/src/dealer_scenario_checkpoint_v1.rs`
- frame projection: `crates/dclutch-operator/src/dealer_scenario_hot_v4.rs`
- on-chain routes: `programs/dclutch-trading-sbf/src/dealer_scenario_checkpoint_v1.rs`
- Custody side: `programs/dclutch-custody-sbf/src/dealer_reservation_v1.rs`
- consumed Claims graph: `programs/dclutch-claims-sbf/program-test/fractional-atomic/src/narrow_fixture.rs`
