# Evidence refresh: the founded world, as legitimately advanced

Status: **design.** It changes no program byte. It answers one question — how a
completed founding campaign's evidence stays authoritative after the market's
own lawful life — activation, admission, fills, settlement — advances the world
it describes, and closes each part with a mechanism and the refusal that
enforces it.

It exists because RETIRE-1 walled the first complete market life at resolution
and proved the sanctioned path was nonexistent. Claims below are
**verified-from-source** (read at HEAD, cited by symbol; line numbers are hints,
the symbol is the citation) or marked **ruling** (a decision this document
makes).

---

## 0. What this document decides

1. The pin protects **"resolution runs against the exact founded world,
   unforged."** It enforces that with byte-equality between a recorded row and a
   live re-read. For the finalized content-addressed records that relation is
   correct forever. For the **live** accounts it is correct only at the founding
   instant (§1).
2. The wall is not a strictness that should be relaxed. It is an **anchor in the
   wrong place**: the relation stays byte-equality, and the set of documents
   permitted to supply the row widens from *the founding generation* to *the
   founding generation plus a refreshed generation chained to it* (§2).
3. **This is a class, not a cause.** Activation is where the wall was first hit,
   but it is not special: admission moves `claims_admission` — itself
   byte-pinned, by a *thirteenth* pin the twelve-label list does not contain —
   and a fill and a fee settlement moved `founding_market` twice more on the
   measured substrate with no activation involved. The refresh therefore reads
   the whole advanceable set as-of a slot and reports what the chain says. It is
   not an activation patch, and nothing in it is activation-shaped (§1.2).
4. The chain link is carried by **the eleven records that cannot change**. A
   refresh must reproduce all eleven byte-identically. Their pins do not move
   (§2, R2). This is the anti-forgery spine, and it makes forging a refresh
   exactly as hard as forging a founding.
5. `direct_capability_root` **names two different addresses**, and this is the
   proximate cause of the all-or-none wall (§3). The founding checkpoint scalar
   is the founding-permit root, at which no account can ever exist. The terminal
   sequence's label means the execution root. **Ruling:** the refresh emits the
   execution root, and the two are given distinct names in the refresh schema.
6. The refresh accepts **no caller-supplied account bytes** (§4). Every field it
   writes is either read from finalized chain through the verifier's own row
   builder or copied from the founding evidence. O-016 holds by construction,
   not by review.

---

## 1. What the checks actually protect

### 1.1 The twelve-label pin

`flagship_resolution.rs`'s producer runs, for twelve `(label, selector)` pairs,
`authenticate_campaign_account(campaign, label, selected.account(selector)?,
&snapshot)`. That function (`flagship_resolution.rs:2371`) refuses on three
distinct grounds, and it is worth separating them because they protect different
things:

| Refusal | Protects |
| --- | --- |
| `completed campaign omitted {label}` | The evidence is complete — no silent drop. |
| `completed campaign substituted {label} address` | The evidence names the same *coordinates* the producer independently selected. This is the anti-substitution check. |
| `completed campaign {label} evidence differs from the current finalized account` | The recorded *content* is the live content: seven fields (`owner`, `lamports`, `executable`, `data_len`, `data_sha256`, `account_sha256`). |

The third is the one that walls. It is a **freshness** check, and freshness is a
coherent demand: the producer builds lookup tables and exact instruction bytes
against observed state, so requiring the state to be the state it recorded is a
real coherence guarantee, not ceremony.

What makes it wrong for one of the twelve is *which* account it is applied to.
Eleven are finalized content-addressed records — `source_material_record`,
`capability_manifest_record`, `source_spec_record`, `provider_release_record`,
`pyth_adapter_config_record`, `window_spec_record`, `statistic_spec_record`,
`product_record`, `result_domain_record`, `portfolio_record`, and
`resolution_funding_ledger`. For those, byte-equality-forever is exactly right,
and this document does not touch them.

`founding_market` is the one **live mutable** account in the set. Core commits
its outstanding-capability count on activation
(`programs/dclutch-core-sbf/src/capability.rs:7`: *"Core commits only its
outstanding-capability count after the exact child acknowledgement and all
physical postconditions succeed"*), against
`CoreState.outstanding_capabilities: u64`
(`crates/dclutch-market-core-codec/src/generated.rs:407`, field index 10 in
`state_layout.rs:51`). So the Market's `data_sha256` changes, and the pin
refuses.

**The pin therefore says: this market must never have had a capability
activated.** That is not a security property anyone chose. It is the accidental
reading of an equality whose anchor was never re-set. And it is load-bearing in
the wrong direction — `retire_v1.rs:851` and `:1698` require
`outstanding_capabilities != 0` to be **false** before retire, which means the
protocol *expects* this counter to move up and back down over a market's life.
The evidence format simply had no way to say so.

### 1.2 The thirteenth pin, and why this is a class

**Verified-from-source, and the reason this document is not an activation patch.**

The twelve-label loop is not the only byte-pin. `admitted_campaign_resolver`
(`flagship_resolution.rs`) takes `claims_aggregate`, `founder_position` and
`claims_admission` by address from the evidence, and then runs
`authenticate_campaign_account(evidence, "claims_admission", admission_key,
snapshot)` — a **thirteenth** byte-pin on an account that **admission mutates**.

So the same wall is reachable by a second, independent route, and SIMLIFE-3 hit
it that way: *a resolution produced against a market that has since been admitted
to*. Neither route is the "real" cause. The real statement is:

> Every account the protocol is designed to advance between founding and
> resolution, and whose row a consumer pins, walls that consumer once it
> advances.

The measured substrate makes the point without needing either mechanism:
`founding_market` there moved at the fill (slot 7576) and again at the fee
settlement (slot 19651), before any of this was reasoned about.

The refresh is therefore defined over an **advanceable set**, re-read wholesale
as-of a slot, not over a list of known causes:

| Label | Advanced by | Pinned how |
| --- | --- | --- |
| `founding_market` | Core state: activation's outstanding-capability commit, phase, readiness — and any fill or settlement | byte-exact, twelve-label loop |
| `direct_trading_funding_ledger` | activation carrying the parked rent quote out | terminal sequence |
| `claims_admission` | admission | byte-exact, thirteenth pin |
| `claims_aggregate`, `founder_position` | admission | address + structural decode |

A refresh reads all of them and reports the chain's answer. Which ones actually
moved is not the refresh's business, and no code path in it asks.

### 1.3 The all-or-none carry

`require_direct_first_use_evidence_v1` (`terminal_lifecycle.rs`) admits both
`DIRECT_FIRST_USE_LABELS_V1` present or both absent, and refuses exactly one.
Its own comment states the rationale precisely: both are created by the same
first-use path, so *exactly one present* means "the collector saw the route run
and dropped half of what it left behind."

That reasoning is sound. **The premise is not** — see §3.

---

## 2. The refresh, and its authentication

**Ruling.** A second, separate document: the founding evidence file is never
edited. RETIRE-1 was right to decline to hand-edit it, and a design that made
hand-editing sanctioned would have destroyed the thing the pin protects.

A refresh envelope (`dclutch-successor-evidence-refresh-v1`) is emitted by the
collector, from finalized chain, after activation. Consumers take it through a
new **optional** argument; absent it, behaviour is byte-for-byte what it is
today. The refresh supplies an *effective accounts map* — founding rows,
overridden and extended by refreshed rows — and every existing check then runs
against that map **unchanged**.

The refresh is admitted only against all of:

- **R1 — Envelope binding.** `schema`, `cluster`, and `plan_sha256` match the
  same expectations `completed_campaign` already enforces, and
  `founding_evidence_sha256` equals the SHA-256 of the founding evidence bytes
  actually loaded in this run. The producer checkpoint already tracks a
  `campaign_evidence_sha256` (`flagship_resolution.rs:2548`), so this digest is
  the existing lineage coordinate, not a new one.
  *Refuses:* `refreshed evidence is not chained to this founding campaign`.
- **R2 — The immutable eleven do not move.** The refresh MUST carry a row for
  each of the eleven content-addressed labels, and each MUST be identical to the
  founding row in all seven fields. Any difference, and any omission, refuses.
  *This is the spine.* A refresh that cannot reproduce the founding's immutable
  records is not a later view of that world; it is a different world.
  *Refuses:* `refreshed evidence altered immutable founding record {label}`.
- **R3 — The market coordinate does not move.** The refreshed `founding_market`
  row's `address` equals the founding row's. Only its state fields may differ.
  *Refuses:* `refreshed evidence substituted the founding Market address`.
- **R4 — Equality is kept, not weakened.** `authenticate_campaign_account` runs
  against the effective row with no change whatsoever. The refreshed
  `founding_market` row is still pinned byte-exact to the current finalized
  account. A tampered refresh refuses on the *same* string as a tampered
  founding, and a *stale* refresh — one the market has moved past — refuses too.
- **R5 — As-of slot.** The refresh declares `as_of_slot` and finalized finality;
  `as_of_slot` must be `<=` the producer's observation slot.
  *Refuses:* `refreshed evidence as-of slot is ahead of the finalized
  observation`.
- **R5b — Only the advanceable set may advance.** A refresh may carry rows only
  for the eleven (identical), the advanceable set of §1.2, and the execution
  root. Any other label refuses: overriding a row this design never examined is
  exactly the smuggling the pin exists to stop.
  *Refuses:* `refreshed evidence carries unadmitted label {label}`.
- **R6 — Pairing is preserved, not bypassed.**
  `require_direct_first_use_evidence_v1` runs on the effective map, unchanged. A
  refresh that appends the root but not the ledger still refuses, on its
  existing string.

**Why this forges no easier than before.** Every byte in the refresh is either
(a) re-checked against live finalized chain at verify time (R4), (b) pinned
byte-identical to the founding evidence (R2, R3), or (c) an audit scalar bounded
by the observation (R5). There is no field an attacker can set that is *believed*
rather than *re-derived*. The refresh adds a document; it adds no trust.

---

## 3. Two roots, one label

**Verified-from-source, and the proximate cause of the all-or-none wall.**

`direct_capability_activation.rs:372-377` states it outright:

> The founding checkpoint's `direct_capability_root` is the FOUNDING-PERMIT
> namespace address (its selection config is the generic-founding preimage
> digest, decision 0004). No account can ever exist there: both the activation
> and hot paths force `selection.config == entry.config_id`, so the EXECUTION
> root derives from the manifest entry below. The permit address is reported for
> the record, never required.

So there are two addresses:

| | Derivation | Account? |
| --- | --- | --- |
| **Founding-permit root** | `coordinates.found.capability_root()` (`market.rs:6704`) | **Never.** By construction. |
| **Execution root** | `CapabilityRootHeaderV1::new(release_set, market, generation, CapabilityExecutionSelectionV1{entry_index, manifest digest, kind, release, config}, ..)` then `find_program_address(.., trading)` (`direct_capability_activation.rs:517-527`) | Created by activation. |

The founding collector's root probe is:

```rust
let root = Pubkey::new_from_array(coordinates.found.capability_root().to_bytes());
if let Some(root_account) = rpc.account(root)? {
    accounts.insert("direct_capability_root".into(), account_evidence(root, &root_account));
}
```

It probes the **permit** address. That address is permanently empty, so the
branch is unreachable and the row is never emitted — under any timing, on any
cluster, no matter how late the collector runs. The comment above it explains
the absence as first-use timing (*"no account exists at this address until the
Direct exterior first runs"*), which is true of the execution root and not of
this one. The two roots were conflated at exactly the point where the evidence
row is produced.

That fully explains the observed state — founding evidence carries
`direct_trading_funding_ledger` (read unconditionally at `market.rs:6723`,
`required_account`) and omits `direct_capability_root` — and it means
`require_direct_first_use_evidence_v1`'s premise, *both created by the same
first-use path*, is false as applied. The ledger exists at founding; the
execution root is created later, by activation.

**Ruling.** The refresh emits, under `direct_capability_root`, the **execution**
root — the address the terminal sequence's three consumers actually mean
(`terminal_sequence.rs:3473`, `:4996`, `:5395`, the last of which cross-checks it
against Direct close discovery's own `snapshot.root.key`). The founding-permit
coordinate keeps a distinct name in the refresh schema so the two can never again
be read as one. The refresh re-derives the execution root through the *same*
`CapabilityRootHeaderV1` construction activation uses — shared, not copied — and
then reads the account from chain.

The pairing rule is left standing exactly as written. It is satisfied by the
effective map because the refresh appends the root that founding could not have
had, next to the ledger founding did emit.

---

## 4. Why O-016 holds by construction

The refresh command takes an RPC URL, the plan, the founding evidence path, and
an output path. It takes **no account content**. Its rows are built by
`rpc.rs:2070 account_evidence(address, &RpcAccount)` — the identical function
the founding collector emits with and `authenticate_campaign_account` verifies
against. There is no second derivation to drift.

Coordinates are re-derived, never accepted: the market from the founding row's
address (then pinned by R3), the execution root from the manifest entry through
activation's own header construction, the ledger from the plan's Trading program
and the founding coordinates. Where the founding checkpoint carries a scalar, it
is used as a cross-check on a derived value and never as its source — the
posture `campaign.rs:135` already documents for
`checkpoint_direct_capability_root`: *"a routing coordinate, never account
authority: every consumer re-reads and re-authenticates the root account from
finalized chain state."*

No caller input becomes authority by inclusion.

---

## 5. Red-proofs this design owes

A relaxation-adjacent change is only as good as the refusals it keeps. Both
directions, each an executable test:

| Hostile input | Must refuse |
| --- | --- |
| Refresh with any byte of `founding_market` altered | `evidence differs from the current finalized account` |
| Refresh with any of the eleven immutable rows altered | `altered immutable founding record {label}` |
| Refresh omitting one of the eleven | `altered immutable founding record {label}` |
| Refresh naming a different Market address | `substituted the founding Market address` |
| Refresh chained to a different founding digest | `not chained to this founding campaign` |
| Refresh with `as_of_slot` ahead of the observation | `as-of slot is ahead of the finalized observation` |
| Refresh appending the root but not the ledger | existing all-or-none string |
| Refresh overriding any label outside the eleven, the advanceable set, and the root | `carries unadmitted label {label}` |
| Stale refresh (market advanced since) | `evidence differs from the current finalized account` |

And the green direction, in **both** of the ways the wall is reached: a refresh
the collector produced against a market advanced by activation admits and
appends the execution root; a refresh of a market advanced only by **admission**
admits, carries the moved `claims_admission`, and appends **no** root — because
there is none, and inventing one would trip the all-or-none pairing. In both,
the eleven immutable pins verify byte-identically to their founding values,
which is the assertion that no pin moved.

---

## 6. What the bridge unblocked, and the three walls behind it

**Measured**, on the preserved `~/jobs/dclutch-fill2` substrate (market
`7hRLVijyCd6FoGxAdPC1nsFAZ5VtN5kwvPvsaRMHLmky`, generation 2), 2026-08-31.

The refresh works and the wall it was built for is down: with
`--refreshed-evidence`, the flagship producer authenticates the campaign and
**plans the resolution** — the first time that has happened on this substrate.
Without it, RETIRE-1's exact refusal still stands, and a refresh tampered by one
byte hits that same refusal. Nothing was relaxed.

Behind it are **three further walls, none of them this bridge's**, all in the
flagship ALT provisioning path — which, on the evidence of these failures, has
never been driven end to end:

1. **The ALT journal never observes its own landed transaction.** The
   `--provision-tables` journal writes `phase: submitted` with an
   `expectedSignature`, and that transaction *finalizes on chain*
   (`43osSMuq…`, slot 58669, `Ok`) — but `finalized` stays `null` and
   `receipts` stays `[]` across fourteen further invocations, each re-reporting
   the identical journal. The route therefore never completes. **This is the
   blocking one.**
2. **The producer's resume comparison contains a wall-clock field.**
   `reclaim_after_unix_seconds` is `max(observation, window_end) + delay`
   (`flagship_resolution.rs`, `run_producer`). Once the observation passes the
   window end it tracks the clock, so `prior.planned_input != input` on *every*
   subsequent produce and the checkpoint refuses with `producer checkpoint
   immutable Market, authority, or typed table plan changed`. Measured here at
   ~76 hours past a 300-second window (start 1787893204, end 1787893504). The
   loop is convergent only while the window is still open — which is not when a
   window-observing resolution is normally driven.
3. **ALT creation slots expire under (2).** `lookup_creation_slots` reuses the
   prior checkpoint's slots, and a slot ages out of the slot-hashes sysvar in
   ~512 slots: `execute lookup-table creation slot 56500 expired before signing;
   produce a fresh three-table plan`. With (2) refusing the fresh plan, the
   checkpoint is stuck holding coordinates it can no longer sign and cannot
   re-derive.

(2) and (3) compose into a deadlock, and (1) sits in front of both. Sizing: (1)
is a bug in one driver's finalization poll and is the whole of the critical
path; (2) is a small, principled change — pin `reclaim_after_unix_seconds` from
the prior checkpoint exactly as `lookup_creation_slots` already pins the table
slots — but it touches an idempotence guard and so wants its own red-proofs;
(3) dissolves once (2) admits a re-plan.

Conservation is undisturbed throughout. The closing statement still reads
**CONSERVED, net drift +0 atoms**, and every one of the seventeen refreshed rows
still matches finalized chain byte-for-byte after all of the above.

---

## 7. The three walls, driven

**Measured** on the same substrate, 2026-08-31, continuing §6. Two of the three
walls were misattributed there; this section corrects them from the chain, and
records the one real change and the wall now standing behind it.

### 7.1 Wall (1) was the driver, not the journal

**Withdrawn.** The ALT journal observes its landed transaction correctly. Run
against the very journal §6 describes as stuck, `--provision-tables` advanced to
`phase: finalized` on its **first** invocation and wrote a complete receipt —
signature `43osSMuq…`, slot 58669, fee 75000, 10766 CU.

Nothing had changed but the clock. The fourteen invocations §6 counts all ran
within **one second** of the send: the transaction's `blockTime` is 1788167554,
and all fourteen logs and the journal itself carry the same 05:12 mtime and are
byte-identical. They polled before finalization was possible, exhausted the
loop's budget in under a second, and each correctly reported `Pending`.

The route is **poll-only once a transaction is `Submitted`** — deliberately, so
recovery can never do anything but re-read (`run_table_provisioner`, the
`DurablePhaseV1::Submitted` arm). A driver of a poll-only route owes it a wait.
The one in §6 had none. `retire/life.sh` polls at one second and the route
completed.

**There is no fix owed at the author.** A refusal that never fires is not a
defect in the thing that declined to fire.

### 7.2 Wall (2): the reclaim floor is a commitment, not a clock reading

This one is real and is now fixed, as §6 sized it.
`pinned_reclaim_after_unix_seconds` (`flagship_resolution.rs`) pins
`reclaim_after_unix_seconds` from the prior checkpoint exactly as
`lookup_creation_slots` pins the table creation slots.

**Why pinning is the correct reading, not a convenience.** The floor is encoded
into the provider submission the checkpoint plans
(`dclutch-resolution-codec::provider_transport_v3`, at byte offset 24). A resume
that re-derived it would plan a *different transaction* than the one the
checkpoint already committed to. Re-derivation is not merely inconvenient past
the window end; it is wrong at every point.

**What the guard still refuses.** Nothing was excluded from the comparison.
`prior.planned_input != input` still compares `PlanInputV1` **in full**, this
field included, against a fresh derivation of every other field. What changed is
only which document this one field is read from.

**The bounds a carried value earns.** Moving the field from derived to carried
gives it its own validity rule — the interval of values the producer could
legitimately have derived at any observation between founding and now:

| Bound | Refuses | Why it is not a weakening |
| --- | --- | --- |
| `pinned >= window_end + delay` | `producer checkpoint reclaim floor {pinned} is below the terminal window bound {floor}` | Exactly the floor a fresh derivation always clears, and exactly what `dclutch-provider-transport-v3-operator` refuses below (`intent.reclaim_after_unix_seconds < window.end_unix_seconds()`). |
| `pinned <= max(observation, window_end) + delay` | `producer checkpoint reclaim floor {pinned} is ahead of the derivation {derived} this observation admits` | A hand-edited checkpoint cannot push the floor forward and strand the reclaim. |

`FLAGSHIP_RECLAIM_DELAY_SECONDS_V1` is documented at its definition as *"not a
protocol liveness bound"* but a provisional operator delay, so the freshness a
pin necessarily gives up was never a protocol guarantee. The two protocol bounds
are kept, and now checked explicitly rather than inherited from the derivation.

**Red-proofs, both directions, executable.** Against the substrate, produced
through the real producer:

| Hostile input | Refused with |
| --- | --- |
| Floor below the window bound | `is below the terminal window bound` |
| Floor pushed into the future | `is ahead of the derivation` |
| `terminal_sequence` changed | `producer checkpoint immutable Market, authority, or typed table plan changed` |
| `submitter` changed | the same string |
| `certificate` selector changed | the same string |
| a table `creationSlot` changed | the same string |

Green: the resume that changes **only the clock** now produces. That is the
whole of the relaxation, and the four resolution-identity red-proofs are the
assertion that produce-idempotence survived it. Four unit tests carry the same
statements in-repo.

### 7.3 Wall (3) was mis-sized, and dissolves for a different reason

§6 says *"(3) dissolves once (2) admits a re-plan."* Both halves need correcting.

The pin admits **no re-plan at all** — it reproduces the prior plan exactly,
stale slots included. A checkpoint that has already stalled past ~512 slots
stays dead, and the one on this substrate did: with the pin in place it produced
cleanly and then refused at the next create, which is the correct behaviour.

What actually dissolves (3) is that **the expiry only ever threatened a stalled
loop.** `select_next_table_action` already provisions all three creates before
any extension or freeze, with a comment stating the reason outright: *"All three
creates consume a recent SlotHashes entry. Provision them before any
extension/freeze sequence so a long first-table sequence cannot strand the
still-vacant tables behind an expired creation slot."* The design had already
solved this. The slots expired in §6 because wall (2) killed the loop after
create #1 and left the checkpoint sitting for some four thousand slots.

Driven with (2) fixed, the whole route completes: **eleven receipts** — three
creates, five extends, three freezes — 825,000 lamports of fees, slots
65330–65696, and `flagshipInput` settled. On this validator the SlotHashes window
is about **76 seconds** (measured 6.77 slots/second), and the three creates
landed in roughly fifteen. That is the flagship ALT provisioning path driven end
to end for the first time.

**Ruling.** In-place re-plan of a stalled checkpoint is **not** implemented, and
deliberately so. It would have to let a resume move table coordinates, which
means restructuring both halves of the resume guard, and no green path needs it.
The sanctioned recovery for a stalled checkpoint is instead: **close what the
chain committed, then plan afresh.** A created-but-abandoned table is not lost
value — its authority may deactivate and close it, and on this substrate that
recovered 1,280,640 lamports for 10,000 lamports of fees, leaving nothing
stranded. A later lane that wants in-place recovery should build it against a
written design, not invent one at the guard.

### 7.4 The fourth wall: one byte, invisible because the refusal had no payload

With the tables frozen and `flagshipInput` settled, the submit stage refused:

```
submit atomic stage geometry: PacketTooLarge
```

`dclutch_versioned_message_operator::Error` is a fieldless enum, so
`PacketTooLarge` said the route did not fit and could not say by how much. That
is the difference between a wall that can be acted on and one that can only be
reported, and it is why nobody had sized this. `measure_v0_wire_bytes` compiles
the identical message, applies every other check, and returns the size rather
than refusing on it. The refusal now reads:

```
submit atomic stage geometry: 1233 wire bytes is 1 over the 1232 packet limit,
carrying 1 bundled top-up transfer(s)
```

**One byte.**

The cause is the top-up the stage bundles ahead of its action. A v0 message
cannot take an instruction's *program id* from a lookup table — `CompiledKeys`
excludes any key with `is_invoked` from table extraction — so adding a System
`transfer` evicts the System program from the submit table into the static keys:
**+32** for the key, **−1** for the readonly index it vacates, **+17** for the
instruction record. **+48 bytes to save one.**

**Ruling.** The top-up is packaging, not protocol. The Resolution program's
`initialize_lifecycle` (`provider_transport_v3.rs`) uses `allocate` + `assign`,
never `create_account`, and *requires* the lifecycle PDA to already hold rent —
`preflight_submit_outputs` refuses `OutputState` when
`lamports() < rent.minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3)`. The
program does not fund that account; it demands it be funded. Any prior lamport
delivery satisfies the identical on-chain predicate, and `vacant_top_up` is
lamports-blind about vacancy (`is_vacant` tests system-owned ∧ non-executable ∧
empty data), so a pre-funded PDA still classifies as Submit and still yields
`Ok(None)` — provided it is funded to the rent minimum **exactly**, since
holding more refuses with `above exact rent`.

Pre-funded to exactly 4,565,760 lamports, the stage compiles with 47 bytes to
spare and the packet wall is down.

**The deeper defect this exposes, unfixed.** The geometry probes that pass
*before* this one understate the real packet by about a hundred bytes.
`compile_provider_submit_v0` compiles the bare action; `prepare_stage` compiles
that action behind two prepended ComputeBudget instructions (**+52**) and the
top-up (**+48**). So the producer settles `flagshipInput` — declaring the plan
complete — on a measurement of a message it does not ship, and the same blind
spot sits in the execute and reclaim probes. A producer that measured what it
ships would refuse at produce time, while the table plan can still be changed.
It does not, and that is why a one-byte overrun surfaced only at the send.

### 7.5 The fifth wall: the durable prestate pins the clock

**This is the wall this document was written about, on a second anchor.**

Behind the packet wall the stage plans cleanly, writes its durable plan, and
then refuses:

```
provider full resolved account prestate changed
```

`authenticate_provider_prestate` re-reads all thirty-nine resolved accounts and
compares each one's full byte state against `plan.pre_accounts`, captured at
plan time. Measured across two independent fresh plans, **exactly one** of the
thirty-nine differs, and it is the same one both times:

| Account | Field | Planned | Live |
| --- | --- | --- | --- |
| `SysvarC1ock11111111111111111111111111111111` | `data_sha256` | `c8111722…` | `2519d387…` |

Every other account — every record, every program, every staging address, the
market, the freshly funded lifecycle PDA — matches byte-for-byte.

**The Clock advances every slot.** On this validator that is every 148
milliseconds. `versioned_message_balances` captures every resolved account's
bytes with no exemption, and the check runs at all four phase transitions
(`run_stage`, the four `authenticate_provider_prestate` call sites), so the plan
and its send can never observe the same Clock. This route can only pass where
the clock is frozen — `program-test` and banks harnesses, which do not advance
slots unless told to. That is precisely why §6 could say the flagship ALT path
"has never been driven end to end": on a live validator it cannot be.

And it is **the same class §1.2 named**:

> Every account the protocol is designed to advance between founding and
> resolution, and whose row a consumer pins, walls that consumer once it
> advances.

The Clock is the purest instance of that sentence. §1 moved one anchor for
`founding_market`; this is a second anchor in the same wrong place, and the
refresh does not reach it because this is a stage prestate, not campaign
evidence.

**Sized, not fixed — deliberately.** The remedy is the same move §1 made:
byte-equality is the wrong relation for an account no actor controls and the
runtime advances unconditionally. Pinning the Clock buys no anti-forgery value —
nobody can forge it — and guarantees liveness failure. But the plan's dependence
on the clock is **real**: `validate_observation_fields` admits the plan against a
window and a Pyth freshness band, and a send arbitrarily later is not the same
transaction semantically. So the Clock must not simply be dropped from the
comparison; it must be replaced by the semantic bound the byte-pin was standing
in for — *the clock still lies inside the band this plan was admitted under* —
with a refusal of its own when it does not.

That is a guard, and deriving each stage's admissible band is a design decision,
not a patch. It is therefore **named here and left undone**. The red-proofs it
owes are already clear:

| Hostile input | Must refuse |
| --- | --- |
| Any non-Clock resolved account changed after planning | `provider full resolved account prestate changed` |
| Clock advanced past the plan's admissible band | a new, specific band refusal |
| Clock advanced within the band | **must admit** — this is the green direction and the whole point |

### 7.6 The band, derived

**This section is the design §7.5 named and declined to write.** It is written
before the patch, because §7.5 left three things genuinely open — which fields of
the Clock row are released, where each stage's band endpoints come from, and what
bounds a released field earns — and each is a decision, not an implementation
detail.

**Ruling 1: the exemption is per-field, not per-row.** §7.5 says "replace the
Clock row's byte-pin with the semantic band." Read literally that releases the
whole `DurableAccountStateV1`, and four of its attributes do not advance.
Measured on this validator across three independent finalized reads at slots
76795 / 76804 / 76812: `owner` = `Sysvar1111…`, `lamports` = 1169280,
`executable` = false, `data_len` = 40, all identical; only the 40 data bytes
move. So those four **keep the original byte-pin and the original refusal**, and
only the account's decoded contents are released. Releasing more than advances
would be the generality leak §7.5's second red-proof forbids.

**Ruling 2: every released field earns a bound. Nothing is dropped.** The Clock
is five fields, and all five advance — three of them (`epoch`,
`leader_schedule_epoch`, `epoch_start_timestamp`) only at an epoch boundary,
which this single-epoch validator never crosses but a real cluster does. A band
on `unix_timestamp` alone would leave four fields unconstrained. Instead:

| Clock field | Replaced by |
| --- | --- |
| `unix_timestamp` | the stage's admissible band (Ruling 3) |
| `slot` | monotone: `>= plan.observation_slot` |
| `epoch`, `leader_schedule_epoch`, `epoch_start_timestamp` | monotone: `>= ` the planned row's value |

Monotonicity is not ceremony here. The runtime advances the Clock
unconditionally and no actor can rewind it, so a *lower* reading means the RPC
served a view behind the one the plan was built on — the one hostile thing this
row can express. `finalized_accounts` passes `minContextSlot`, which is a
minimum, not a pin; this is the check that says so.

**Ruling 3: band endpoints come only from rows that still carry a byte-exact
pin.** This is what makes the band un-widenable: moving an endpoint requires
forging a row that refuses on the original string. `validate_observation_fields`
admits a plan when

```
publication >= finalized_now - window.max_age_seconds
publication <= finalized_now + window.max_future_skew_seconds
```

which, solved for the clock rather than the publication, is exactly the closed
interval

```
finalized_now  ∈  [ publication - max_future_skew_seconds,
                    publication + max_age_seconds ]
```

That interval is *the band this plan was admitted under*, and it is derivable
because `publication` and the two tolerances are both pinned:

| Stage | Band on the live Clock's `unix_timestamp` | Endpoints read from |
| --- | --- | --- |
| Submit | `[max(planned, p − skew), p + max_age]` | `p` from `selected.post_update_body`; `skew`/`max_age` from the `window` row in `plan.pre_accounts` |
| Execute | `[max(planned, p − skew), p + max_age]` | the same two, both resolved at Execute |
| Accept | `[planned, unbounded)` | Core terminal accept reads no clock — monotonicity is the whole of its band |
| Reclaim | `[max(planned, reclaim_after), unbounded)` | `reclaim_after` from the `lifecycle` row in `plan.pre_accounts` |

where `planned` is `plan.observation_unix_timestamp` and `p` is the posted
observation's `publish_time`.

Two notes on why each endpoint is pinned. The `window` and `lifecycle` rows are
ordinary resolved accounts, so the byte-exact comparison this change *keeps*
runs on them first: an altered endpoint refuses on
`provider full resolved account prestate changed` before the band is ever
derived. And `selected.post_update_body` is pinned by
`authenticate_planned_stage_semantics`, which runs earlier in
`authenticate_provider_prestate` and compares the planned action against one
rebuilt from `selected` — so altering the body either changes the action bytes
and refuses there, or does not appear in the transaction at all, in which case it
cannot affect what the transaction means. The argument does not depend on the
intent's encoding.

If a stage's band needs a row its plan did not resolve, that is a structural
break and refuses on its own string rather than silently widening.

**Ruling 4: Accept's band is open above, and that is not a hole.** Nothing in
the Core terminal accept reads the clock, so there is no semantic bound to
restate, and inventing one would be exactly the improvisation §7.2 warned
against. The transaction's own freshness is already bounded, hard, by
`plan.last_valid_block_height` — the existing
`durable provider blockhash expired before key access` refusal — at roughly 150
slots. The same is true of Reclaim above its floor.

**Ruling 5: `pre_balances` is not touched.** The Clock's lamports are constant
(measured above), and the balance vector feeds the conservation arithmetic in
`authenticate_provider_finalized_projection`. Exempting an entry there would
weaken a conservation check to buy nothing. If a sysvar balance ever moves, that
is a new wall to record, not one to pre-empt.

**What this is not.** It is not a relaxation of the prestate check. Thirty-eight
of thirty-nine rows are compared exactly as before, on the same refusal string;
the thirty-ninth keeps four of its five attributes byte-pinned on that same
string, and trades byte-equality on its contents for a bound on each of its five
fields. The one thing given up is *the clock had not moved*, which was never a
security property — nobody can forge the Clock — and was a guaranteed liveness
failure anywhere slots advance.

**Refusals this adds.**

| Refusal | Fires when |
| --- | --- |
| `provider {stage} clock {observed} is outside the admissible band [{lower}, {upper}] this plan was admitted under` | the clock left the band the plan was admitted under |
| `provider {stage} clock rewound: {field} {observed} is behind the planned {planned}` | any Clock field read lower than the plan's |
| `provider {stage} clock sysvar is not the exact 40-byte Clock layout` | the row decodes to something that is not a Clock |
| `provider {stage} clock band lost its pinned {label} row` | a band endpoint's account is not in the plan's prestate |


### 7.7 Wall (6): an expired plan refused by naming the probe, not the cause

**Measured, and fixed.** With the band in place, the first drive refused:

```
getFeeForMessage omitted exact table fee
```

The plan was thirty-one minutes old and its blockhash had expired — 6,241 blocks
past `lastValidBlockHeight` 72477. `getFeeForMessage` answers `null` for a
blockhash the cluster has forgotten, and the fee probe ran *before* the block
height check, so the expiry announced itself as a missing fee quote. Same class
as §7.4: a refusal that cannot name its own cause.

The two checks are now in the other order, and an expired plan refuses with
`durable provider blockhash expired before key access`. Nothing else changed;
the fee comparison is unaltered.

**A liveness note this exposes.** On this validator a blockhash lives about 150
slots ≈ **22 seconds**. `prepare_stage` → sign → send happens inside one
invocation, so a fresh plan makes it. A plan left `Planned` across invocations
does not, and there is no re-plan path: the `Planned` arm authenticates the same
expired plan forever. A `Planned` stage has accessed no key and sent no packet,
so discarding it is safe by exactly the reasoning `authenticate_send_boundary`
encodes — but the driver does not do it, and this drive had to discard the
checkpoint by hand. **Sized, not fixed:** an automatic discard-and-re-plan for an
expired `Planned` stage is a small change to one arm, and it wants its own
red-proof that it can never fire on a stage that has signed.

### 7.8 Wall (7): the snapshot answered for accounts it had never read

**Measured, and fixed. This is the one that would have been a wrong answer
rather than a refusal.** With the packet and clock walls down, the submit
*landed* — signature `4j9ipdXYKq…`, slot 79334, `Ok` — and then the receipt
refused:

```
finalized provider submit projection: Poststate
```

`ProviderFinalizedProjectionErrorV3` is fieldless, so again it could not say
which of the four writables disagreed. Measured from chain, all four reconcile
exactly: submitter −1,903,521 = fee 80,000 + update rent 1,823,520 + provider fee
1 + top-up 0; the update account created at 1,823,520 under the Receiver; the
pre-funded lifecycle unmoved at 4,565,760 under Resolution; the treasury
890,880 → 890,881. The 528-byte lifecycle body the operator reconstructs from the
instruction request and the return-data receipt is **byte-identical** to the one
on chain.

The disagreement was not on chain at all. `observe` builds its snapshot from
`selected.accounts` plus the lifecycle, the tables and the Rent sysvar — and
**four of the accounts the projections read are not in that map**: the three
signing roles (`submitter`, `resolver`, `refund_recipient`, which the input
carries as top-level fields) and the Receiver's fee treasury (a derived address
the input never names). `observed_or_vacant` then read "never fetched" as
"vacant on chain" and handed the projection a fabricated zero balance, which it
correctly refused.

Two fixes, and the second is the one that matters:

1. `observe` now also inserts the three roles and
   `receiver_treasury_address(selected)`.
2. `observed_or_vacant` **returns a `Result`**, and refuses a key the snapshot
   never fetched with `finalized snapshot never observed {key}; a vacant reading
   would be a fabrication`. The map already distinguishes the two facts — a
   fetched-but-vacant key is present with `None` — so this is a fact the code had
   and was discarding. The legitimate vacancy callers (the staging cursors) are
   unaffected: their keys *are* in the map.

(1) unblocks this route. (2) is the general statement: no future instance of this
can be a silent wrong answer again, only a named refusal.

### 7.9 Wall (8): the execute table names only the accounts the input could name

**Measured and sized; the fix is named and left undone, deliberately.** With the
submit receipted, the execute stage refuses. §7.4 predicted the shape and this
lane gave the probe the margin its error could not carry — the same remedy §7.4
applied to `prepare_stage`, now applied to the three
`compile_provider_*_v0` geometry probes it named:

```
provider execute v0 geometry: the bare action alone is 1351 wire bytes,
119 over the 1232 packet limit, before the ComputeBudget prefix the sent
packet adds
```

**It is not structural.** The execute instruction is 47 accounts and 774 bytes of
data (72 + 608 + a 94-byte update body), and its frozen lookup table holds only
**40** addresses. Ten of the 47 are absent from it, and **nine of those ten are
structurally extractable** — `solana-message`'s `try_extract_table_lookup`
excludes only signers, the fee payer, invoked program ids and nonce accounts, so
of the 47 exactly two (`resolver`, the payer-signer, and `core_program`, the
invoked program id) can never leave the static vector.

The nine are `caller_authority`, `certificate`, `lifecycle`, and the six staging
cursors for `source_spec`, `source_provider_release`, `adapter_config`,
`window`, `statistic` and `pyth_release`. The cause is a **naming gap, not a size
limit**: `stable_lookup_union` can only emit a row for a label the input's
`accounts` map carries, and these are either derived inside the transport
builder (`caller_authority`, `lifecycle`, the six cursors) or named but never
pushed into the Execute arm (`certificate`, which *is* in the Reclaim union).
The tell is that the four staging cursors the input does name are exactly the
ones **Accept** needs — the Execute-specific cursors were never plumbed through.

Each newly extracted key trades 32 static bytes for a 1-byte index:

| Route | Today (40-address table) | Complete table |
| --- | --- | --- |
| Bare action | 1351 (**+119**) | 1072 (−160) |
| + ComputeBudget prefix | 1403 (**+171**) | 1124 (−108) |
| + a bundled certificate top-up | 1451 (**+219**) | 1172 (−60) |

**Ruling: this is a lane, not a patch, and four things are coupled.** The live
execute table is **frozen**, so it cannot be extended — a complete table is a new
table at a new slot-derived address. That changes `lookupTables.execute`, which
changes `input.json`, which changes `inputSha256`, which retires the checkpoint
holding the landed submit receipt; and `require_terminal_receipts` demands
exactly four receipts, so a checkpoint resumed at Execute can never reach
`verifiedTerminal`. Three further constraints bind the fix: `routing_stage` maps
**Accept → Execute**, so the three Accept-only rows (`capability_manifest`,
`capability_manifest_staging`, `funding_ledger`) must stay;
`authenticate_frozen_lookup_table` compares the address vector **in exact
order**; and its `expected_last_extension_start` moves with the new length. A
lane that does this should carry the union change, the derived-address plumbing,
a re-provision, and a sanctioned resume-at-a-later-stage — against a written
design, not invented at the union.

### 7.10 Wall (8)'s fourth coupling: the resume is an adoption, not a relaxation

**This section is the design §7.9 required before the union is touched.** §7.9
named four coupled changes and flagged the fourth — resuming a checkpoint at
Execute past `require_terminal_receipts`' exactly-four demand — as
guard-adjacent. It is. This section rules on it, and the ruling is that **the
gate does not move**.

**Ruling 1: the exactly-four count was never the obstacle.** Read the driver,
not the summary. `require_terminal_receipts` refuses on
`receipts.len() != 4 || stage_plan.is_some()`, and the receipt vector is grown
one entry per finalized stage by the loop at the `DurablePhaseV1::Finalized`
arm. A run that begins at Execute reaches Complete with three. But the reason it
begins with *zero* is not that gate — it is `authenticate_checkpoint_identity`,
which refuses to load the standing checkpoint because a new `input.json` carries
a new `inputSha256`, on

```
checkpoint format or input digest differs; cross-market resume refused
```

So the wall is at **load**, and it has two possible remedies:

- **Relax the count** — let terminal verification accept three receipts when the
  checkpoint declares itself resumed. This makes *"I skipped Submit"* and *"I
  did Submit under the previous input"* indistinguishable, on the checkpoint's
  own say-so. It is exactly the skip the gate exists to refuse.
- **Adopt the prior receipts** — carry the landed Submit receipt into the new
  checkpoint, every field re-derived from chain, so the run reaches four and the
  gate is never consulted about anything new.

Take the second. It is strictly stronger, and it leaves
`require_terminal_receipts` and `authenticate_receipt_prefix` byte-identical.

**Ruling 2: adoption is an explicit operation, never a widening of load.**
`authenticate_checkpoint_identity` keeps refusing a cross-digest *load* with the
same string. Adoption is a separate, named, one-shot construction —
`--adopt-receipts PRIOR_CHECKPOINT` — admissible only into a checkpoint that
does not exist or holds zero receipts and no stage plan. It may never extend or
overwrite a checkpoint that already has history. A silent fallback here would
re-open by accident precisely what Ruling 1 closed on purpose.

**Ruling 3: a receipt is admitted by its packet digest, not by its JSON.** The
receipt's `signedTransactionSha256` names a specific signed byte string. The
cluster is asked for that transaction by signature at finalized commitment and
hands back the bytes it actually executed; adoption admits the receipt only if

```
sha256(base64_decode(getTransaction(sig).transaction[0])) == receipt.signedTransactionSha256
```

and the decoded packet's first signature is `receipt.signature`. That single
equation is the anchor: it binds a name to bytes the cluster vouches for, and
every other field is then **re-read from chain and compared**, never trusted
from the file — `slot`, `fee`, `computeUnitsConsumed`, `preBalances`,
`postBalances`, the return-data bytes and their digest, the CoreAccept
empty-return rule, and `meta.err` null. The resolved key vector is rebuilt as
`static ++ loadedAddresses.writable ++ loadedAddresses.readonly` and compared in
order. This is the same evidence `provider_transaction_status` and
`finish_provider_stage` demand of a receipt they mint; adoption demands it of
one they inherit.

**Ruling 4: the adopted receipt must belong to *this* market.** A real, finalized,
correctly-digested Submit receipt from a *different* market would otherwise
satisfy Ruling 3. So the rebuilt resolved keys must contain the **new input's**
`lifecycle`, `market`, `source_state` and `update_account`, and the packet must
invoke the new input's `resolution_program` from its static vector. Cross-market
adoption refuses here, which is the same property
`authenticate_checkpoint_identity`'s refusal string was buying before, now
bought from chain instead of from a digest.

The binding is **per-stage, and the first draft of this ruling was wrong.** It
demanded `certificate` of every adopted receipt, and the landed Submit receipt
refused: measured on this validator, the Submit transaction resolves 39 keys and
the certificate is **not among them** — Submit creates the lifecycle, and the
certificate PDA is first named at Execute, which is where it is created. A
binding that names an account a stage cannot touch does not bind that stage; it
just refuses it. So `certificate` is demanded of every stage **except** Submit,
and the four accounts above — which every provider stage carries — do the work
for all four.

**Ruling 5: the chain decides how many receipts are owed — this is the no-skip
property.** The driver already classifies the live chain into a stage. Adoption
is admissible only when the adopted vector is **exactly** the stage prefix
`[Submit .. N)` for that classified `N` — same length as `N`'s index, same
stages, in order. Resuming at Execute owes exactly one Submit receipt; zero
refuses, two refuses, a `core-terminal-accept-v1` in the first slot refuses.

This is the sentence that makes the relaxation not a hole. **There is no input
under which the operator's assertion changes the count.** The count comes from
chain facts, and every receipt the count demands must be produced and
authenticated against chain. A resume cannot start at stage N without carrying
stage N−1's real, finalized, market-bound transaction.

**What the gate still refuses.** The point of the table is that every row below
was reachable before this change only because the change did not exist; each is
now refused by a named check rather than by the feature's absence.

| A resume that… | refuses on |
| --- | --- |
| adopts nothing and starts at Execute | coverage: 0 adopted, 1 owed |
| adopts a fabricated receipt | packet authentication: no finalized transaction for that signature |
| adopts a real receipt with an edited slot, CU, fee, or balance vector | field re-derivation from chain |
| adopts a real receipt whose `signedTransactionSha256` was swapped | packet authentication: digest ≠ executed bytes |
| adopts a real receipt whose `signature` was swapped for another real one | packet authentication: the packet's own first signature |
| adopts another market's real Submit receipt | market binding: lifecycle / certificate / program |
| adopts a later stage to skip an earlier one | prefix order in `authenticate_receipt_prefix`, then coverage |
| adopts into a checkpoint that already holds receipts | adoption is admissible only into an empty checkpoint |
| adopts a transaction that finalized with a runtime error | `meta.err` is not null |
| reaches Complete having adopted one and run two | `require_terminal_receipts`, **unchanged**, still exactly four |

**Refusals this adds.**

| Refusal | Fires when |
| --- | --- |
| `adopted {stage} receipt has no finalized transaction on this cluster` | `getTransaction` returns null |
| `adopted {stage} receipt does not authenticate against the finalized packet` | packet digest or first signature disagrees |
| `adopted {stage} receipt {field} differs from the finalized transaction` | any re-derived field disagrees |
| `adopted {stage} receipt resolved a different account vector` | rebuilt key vector disagrees in content or order |
| `adopted {stage} receipt belongs to a different market` | lifecycle, certificate, or program binding fails |
| `adopted receipts do not cover exactly the stages before {stage}` | coverage rule |
| `receipt adoption refused into a checkpoint that already has history` | target had receipts or a stage plan |

**Red-proofs, measured.** Driven live against the substrate validator at RPC
`127.0.0.1:42888`, adopting the landed Submit receipt (`4j9ipdXYKq…`, slot
79,334) into a fresh checkpoint under the new input digest
`fd38aa43d1f5…`. The honest case adopts and proceeds; every tamper refuses on its
own string, and the no-op control still adopts — so the battery is not refusing
for an unrelated reason.

| Case | Result |
| --- | --- |
| honest adoption | **adopts 1 submit receipt**, proceeds to Execute |
| no-op edit (control) | **adopts**, proceeds to Execute |
| `receipts: []` | `adopted receipts do not cover exactly the stages before execute` |
| stage relabelled to execute | `adopted receipts do not cover exactly the stages before execute` |
| a second receipt appended | `adopted receipts do not cover exactly the stages before execute` |
| `slot + 1` | `adopted submit receipt slot differs from the finalized transaction` |
| `computeUnitsConsumed + 1` | `adopted submit receipt computeUnitsConsumed differs from the finalized transaction` |
| `feeLamports + 1` | `adopted submit receipt fee differs from the finalized transaction` |
| `signedTransactionSha256` swapped | `adopted submit receipt does not authenticate against the finalized packet` |
| `signature` swapped | `adopted submit receipt has no finalized transaction on this cluster` |
| `postBalances[0] + 1` | `adopted submit receipt balances or return data differ from the finalized transaction` |
| a resolved key substituted | `adopted submit receipt resolved a different account vector` |
| adopting twice into one checkpoint | `receipt adoption refused into a checkpoint that already has history` |

Both red-proofs §7.9 owed are discharged: a resume **missing** a prior receipt
refuses on the coverage rule, and a resume carrying a **tampered** one refuses on
the receipt authentication — at seven distinct fields, never on a single check
that could be routed around.

**What is not touched.** `require_terminal_receipts` keeps `!= 4` and its
string. `authenticate_receipt_prefix` keeps every clause — the adopted vector is
run through it unmodified, before and after. `authenticate_checkpoint_identity`
keeps refusing cross-digest loads. Nothing in this section makes an existing
refusal fire less often.

### 7.11 Wall (9): the table tooling asserts a fresh life, and wall 8's fix needs a mid-life one

**Measured, sized, and STOPPED deliberately.** §7.9 ruled that wall 8's fix
couples four things and named the fourth, the resume, as the guard-adjacent one.
It was wrong about which coupling was hardest. §7.10 discharged the resume — the
gate did not even have to move. What stops the Execute stage tonight is a
coupling §7.9 did not see, because it assumed the table tooling was simply
available: *"the live one is frozen; create/extend/freeze is proven tooling."*

It is proven tooling **for a life that has not started**. Both halves refuse
mid-life, on their own strings, for the same reason.

**The producer refuses.** Re-running `--produce-input` against this market now
gives

```
provider authorities must be nonzero and the fresh Receiver update must be vacant
```

because the Receiver update account this market created at Submit is no longer
vacant. The producer's model is a life beginning, and the input it emits is the
input for a whole life.

**The provisioner refuses.** Even given a valid producer checkpoint, driving
`--provision-tables` gives

```
routing tables may be provisioned only before the flagship provider submission
```

from `classify(chain_facts(…))? != StageV1::Submit`. The block around it also
runs `provider_submit_report`, which rebuilds the Submit action — a second
fresh-life dependency in the same place, and one that would fail mid-life for the
same vacancy reason.

**What the guards protect, exactly.** A lookup table is part of what a
transaction *means*: a v0 message names most of its accounts by index into a
table, so a table that could change mid-life could change which accounts a
planned or signed transaction resolves to. Freezing every table before the life
begins makes that impossible by construction. This is a real property and it is
not ceremony.

**Why it nevertheless refuses something safe.** The table this lane needs is a
**new address that no landed transaction references**. Submit's and Reclaim's
unions did not change, so both keep their existing frozen tables — measured, the
re-plan leaves `lookupTables.submit` and `lookupTables.reclaim` byte-identical
and both routes already report `complete`. Only Execute's union grew, from 40
rows to 48, at a new slot-derived address that has never appeared in a message.

**The design the fix needs, and why this lane did not write it.** The guard's
content restated per-stage rather than per-life:

> A table for stage `S` may be provisioned only while the life has not yet passed
> `S` — `classify(chain) <= S` — and only while that stage has finalized no
> receipt. The freshness proof that accompanies it must rebuild **`S`'s own**
> action, not always Submit's.

That preserves the property exactly (you may never provision a table for a stage
already executed, so no landed or signed transaction can have referenced it) and
arguably strengthens it, since today a Reclaim table's protection is only
incidental to Submit's timing. But it is a **third** guard relaxation, on top of
the one §7.10 was chartered to make, and it is guard-adjacent in the same way:
it needs its own written ruling and its own red-proofs — a resume that tries to
re-provision a *passed* stage's table must refuse, and a table already named by a
receipt must refuse — before any of it touches a stage gate.

§7.9 ruled of wall 8 that *"this is a lane, not a patch."* The same ruling
applies here, and this lane declined to invent a second one at the union.

**Sizing, derived rather than measured.** The geometry probe runs only after the
table is resolved from a finalized snapshot, so with the table unprovisioned the
probe is unreachable — the run refuses first on `finalized snapshot is missing
stage lookup table GKKVLNP9Gr…`. The size is therefore **arithmetic against
§7.9's measurements, not a reading**, and is recorded as such. Each extracted key
trades 32 static bytes for a 1-byte index, so 31 bytes per row; §7.9's own table
is consistent with this (1351 − 9 × 31 = 1072, its "complete table" figure).

This lane seats **eight** of the nine, not nine. `caller_authority` is the ninth
and it is the one genuinely awkward one: it is a PDA over
`(release_set, market, role, source_state, role_request_digest)` where the digest
is the hash of an encoded `Request` built inside the transport builder, so naming
it in `input.json` means either re-deriving Core's request encoding in the
producer or plumbing the builder's report out. Eight fits with room, so the ninth
buys nothing this lane needs:

| Route | 40-row table (measured, §7.9) | 48-row table (derived) | 49-row (§7.9's "complete") |
| --- | --- | --- | --- |
| Bare action | 1351 (**+119**) | 1103 (−129) | 1072 (−160) |
| + ComputeBudget prefix | 1403 (**+171**) | 1155 (−77) | 1124 (−108) |
| + a bundled certificate top-up | 1451 (**+219**) | 1203 (−29) | 1172 (−60) |

The margin is thinner than the complete table's but real on every route, and the
derivation is exact except for compact-`u16` length prefixes at page boundaries,
which can move a byte or two either way.

**What this lane landed anyway.** Wall 8's naming gap is **closed in the source**
and its fix is provable without driving chain: the Execute union now names
`certificate`, `lifecycle`, and the six Execute-only staging cursors
(`source_spec`, `source_provider_release`, `adapter_config`, `window`,
`statistic`, `pyth_release`), the input carries the six as first-class
selectors, and a new key-free `--reprovision-execute-table` mode re-plans the
Execute table alone for a life already under way. That mode **ran green**: it
emitted a producer checkpoint whose Execute plan carries 48 rows in three ordered
extension pages (20 + 20 + 8), left Submit and Reclaim untouched, and was
admitted by `authenticate_producer_checkpoint` — the same authentication the
producer's own output must pass, which re-derives the union from the planned
input and so cannot be satisfied by a wrong address. The only thing between that
checkpoint and a driven Execute is the guard above.

**Refusals this lane did not weaken.** Both strings quoted at the top of this
section still fire, unchanged, on exactly the conditions they fired on before.

### 7.12 Wall (9), ruled: the table guard is per-stage, and the mid-life provision is named by the receipt chain

**This section is the design §7.11 required before the provisioner's stage gate
is touched.** §7.11 measured the wall, restated the guard in one sentence, and
declined to implement it — "a third guard relaxation … needs its own written
ruling and its own red-proofs." This is that ruling. It moves exactly one of the
two strings §7.11 quoted, and it moves it to a *stronger* place.

**What is not touched.** The producer's
`provider authorities must be nonzero and the fresh Receiver update must be vacant`
still fires on exactly its old condition. The producer's model of a life
beginning is correct and stays; the sanctioned mid-life path around it is
§7.11's `--reprovision-execute-table`, which re-plans one stage's table from an
already-authenticated producer checkpoint and never re-reads the vacancy the
producer asserts. Nothing below re-opens `--produce-input` mid-life.

#### Ruling 1: the gate's content is per-stage reachability

`run_table_provisioner` refuses on `classify(chain_facts(…))? != StageV1::Submit`.
Restated, for the routing stage `S` whose table the next action would write:

> a routing table for `S` may be written only while `classify(chain) <= S`.

`StageV1`'s ordering is `Submit < Execute < Accept < Reclaim < Complete`, and
`Accept.routing_stage()` is `Execute`, so the comparison is over the same
lattice the driver already classifies into.

This preserves the fresh-life path *exactly*: a life at `classify == Submit`
provisions all three tables, and `Submit <= Submit, Execute, Reclaim` holds for
every one of them. It refuses mid-life re-provisioning of a stage the life has
passed: at `classify == Execute`, `S = Submit` fails `Execute <= Submit`, and
Submit's meaning — a landed packet that names accounts by index into
`lookupTables.submit` — stays frozen. At `classify == Complete` every routing
stage fails, so a finished life can write no table at all.

§7.11 argued this "arguably strengthens" the old guard, and it does, for a
reason worth stating: today a Reclaim table's protection is *incidental* to
Submit's timing. Under the old gate a life still at Submit could re-provision
Reclaim's table and nothing objected, because the gate never looked at which
stage it was writing. Under this one, each table is guarded by its own stage.

#### Ruling 2: "landed" is read from chain, "planned" is read from the checkpoint

Ruling 1 speaks about chain, and chain cannot see a packet that is signed but
not yet sent. A `stage_plan` for `S` in the standing checkpoint is exactly that:
a v0 message whose account meanings are already fixed against `S`'s table
address. `next_table_provision` does not only *create* — it also emits `Extend`
and `Freeze` against an address that already exists — so extending a table a
signed packet already names would change what those bytes resolve to. That is
the property, and Ruling 1 alone does not buy it.

So the provision additionally refuses when the standing checkpoint holds a
receipt whose routing stage is `S`, or a stage plan whose routing stage is `S`.
The receipt clause is largely implied by Ruling 1 and Ruling 3's coverage rule;
it is written anyway, because it is the property being asserted and it should
not depend on two other clauses to hold.

#### Ruling 3: a life already under way must name itself, and the name is §7.10's receipt chain

`classify(chain)` says the *chain* is at Execute. It does not say that the
producer checkpoint being provisioned describes **that** life. Without a
binding, a table planned for market A could be provisioned while market B sits
at Execute, and every clause above would pass.

The binding cannot be the input digest. §7.9's third coupling guarantees a
re-plan changes `input.json` and therefore `inputSha256`, which is precisely why
§7.10 exists; demanding digest equality against the standing checkpoint would
either refuse the only case this wall is about, or force adoption to run before
provisioning for no gain — and it would be a file authenticating itself by its
own self-description, which is the thing §7.10 Ruling 3 declined to accept.

Take §7.10's remedy instead, unmodified. The operator hands the provisioner the
standing checkpoint (`--standing-checkpoint`), and every receipt in it is put
through `authenticate_adopted_receipt` **byte-identically** against the
`SelectedInputV1` the producer checkpoint plans. That is the digest chain:

```
receipt.signature -> the cluster's finalized packet
                  -> sha256(packet) == receipt.signedTransactionSha256
                  -> the packet's own first signature
                  -> static ++ loadedAddresses.writable ++ readonly
                  -> this input's lifecycle, market, source_state, update_account
                     (and certificate past Submit), and its resolution_program
                     in the static vector
```

A standing checkpoint from another market on the same cluster is a real,
finalized, correctly-digested packet and refuses on the last link:
`adopted submit receipt belongs to a different market`. Every other tamper
refuses on the seven fields §7.10 already red-proved.

`require_adoption_coverage(&standing.receipts, classify(chain))` is applied on
top: the standing checkpoint's receipts must be **exactly** the stage prefix
below where the chain is. A checkpoint behind the chain, ahead of it, or carrying
a relabelled stage refuses. Then `authenticate_receipt_prefix` runs over it
unmodified, and its `format` must be the cluster's — a checkpoint is not a
producer checkpoint and not a devnet one.

The count, again, comes from chain. No operator input changes it.

#### Ruling 4: required exactly when the life has begun

`--standing-checkpoint` is **required** iff `classify(chain) != Submit`, and its
absence there refuses on its own string. Supplied on a fresh life it is admitted
and still fully verified — coverage then demands zero receipts — so the flag can
never be used to loosen anything. Absent on a fresh life, every byte of the
existing path is unchanged, which is what keeps the flagship control meaningful
as a control.

#### Ruling 5: the freshness proof rebuilds the action of the stage the chain is at, and §7.11 was underspecified here

§7.11 wrote: *"The freshness proof that accompanies it must rebuild `S`'s own
action, not always Submit's."* Taken literally that is unimplementable, and it
would refuse the case it means to leave alone.

Provisioning is a **sequence**. A fresh life at `classify == Submit` writes all
three tables, and `provider_execute_report` cannot be built at Submit: it
observes the provider `lifecycle` and the Receiver `update`, and both are
accounts Submit *creates*. Demanding `S`'s own action would make the Execute and
Reclaim tables unprovisionable on every fresh life — the guard refusing the
founding path, not the mid-life one.

The statement that makes the proof a proof is:

> the freshness proof rebuilds the action of `classify(chain)` — the stage the
> life is at now.

Its subject is the position the chain is in, not the table being written. It is
a *freshness* proof: it asserts that the input still describes this live market
where the market actually is, and that the action the driver will next take
still compiles from the accounts the snapshot read. Three things follow, and
they are why this reading is the right one:

- At `classify == Submit` it is byte-identical to today's
  `provider_submit_report`, so the fresh-life path does not move.
- At the mid-life case this wall exists for, `classify == Execute == S`, so the
  two readings **coincide** and §7.11 gets the strength it asked for exactly
  where it asked for it.
- Where they diverge — `classify < S` — §7.11's reading has no content, because
  `S`'s action is unbuildable by construction.

The dispatch is total: `Submit -> provider_submit_report`,
`Execute -> provider_execute_report`, `Accept -> core_terminal_accept_report`,
`Reclaim -> provider_reclaim_report`, and `Complete` has no action to rebuild —
which needs no special case, because Ruling 1 has already refused every routing
stage before the proof is reached.

The geometry probe is still not part of this proof, and cannot be: it compiles
against the resolved table, and the table is what is being provisioned. §7.11's
sizing stays derived rather than measured until the table lands.

#### Ruling 6: the finalization re-check is the same clause, re-evaluated at the finalized slot

**Found by driving, not by reading.** §7.11 quoted two strings. There is a
**third** fresh-life assertion behind them, and it only fires once a table
transaction has actually landed — `finish_table_submission`, on the receipt
path:

```
Market resolution advanced while provisioning its routing tables
```

from the same `classify(chain_facts(…))? != StageV1::Submit`, evaluated on a
snapshot taken at the finalized slot. Measured: the mid-life Execute-table
`Create` was planned, signed, sent and finalized on chain — and then refused at
its receipt, leaving a real created table with no journal entry. The guard
`--provision-tables` opens is not the only one on the path; the one that mints
the receipt asserts the same thing again, later.

Its content is not ceremony either. A table action must not *land* after the
life has moved past the stage that table serves: the plan-time gate reads the
chain before the packet is sent, and between send and finalize the life can
advance. So it is Ruling 1 again, and it takes Ruling 1's restatement:

> the action's finalized snapshot must satisfy `classify(chain) <= stage` for
> the stage whose table it wrote.

At a fresh life this is byte-identical to today (`Submit <= Submit, Execute,
Reclaim`). Mid-life it admits the Execute table's own actions while the life sits
at Execute, and refuses any of them that land after the life reaches Accept —
which is exactly the moment the Execute packet's meaning becomes fixed.

**The recovery path is deliberately not re-bound.** A run resuming a journal that
already holds an intent reconciles it without re-reading the standing
checkpoint. That is correct and not a gap: the identity question is asked when
the intent is *created*, the intent is then byte-immutable under
`validate_table_intent`, and its packet is signed against those exact bytes.
Ruling 6's position check is what gates the finalization itself.

#### What the gate still refuses

| A provision that… | refuses on |
| --- | --- |
| writes Submit's table while the life is at Execute | `the submit routing table may not be provisioned: the life is already at execute` |
| writes any table while the life is Complete | the same clause, naming `complete` |
| runs mid-life with no standing checkpoint | `a life already at {stage} may be provisioned only against its standing checkpoint` |
| names a standing checkpoint from another market | `adopted {stage} receipt belongs to a different market` |
| names a standing checkpoint with a tampered receipt | §7.10's seven field checks, unchanged |
| names a standing checkpoint whose receipts are not the chain's prefix | `adopted receipts do not cover exactly the stages before {stage}` |
| writes a table for a stage the standing checkpoint has a receipt for | `the {stage} routing table may not be provisioned: the standing checkpoint holds a landed {stage} receipt` |
| writes a table for a stage the standing checkpoint has planned | `the {stage} routing table may not be provisioned: the standing checkpoint already plans an {stage} packet` |
| runs mid-life against an input whose stage action no longer builds | the `classify`-position report builder |

**Refusals this ruling adds.**

| Refusal | Fires when |
| --- | --- |
| `the {stage} routing table may not be provisioned: the life is already at {position}` | `classify(chain) > stage` |
| `a life already at {position} may be provisioned only against its standing checkpoint; pass --standing-checkpoint` | mid-life with the flag absent |
| `the {stage} routing table may not be provisioned: the standing checkpoint holds a landed {stage} receipt` | Ruling 2, receipt clause |
| `the {stage} routing table may not be provisioned: the standing checkpoint already plans an {stage} packet` | Ruling 2, stage-plan clause |
| `standing checkpoint format differs from this cluster` | the standing file is not a checkpoint for this cluster |
| `the {stage} routing table action finalized after the life advanced to {position}` | Ruling 6, at the finalized slot |

**Refusals this ruling removes.** Exactly two, both replaced by the per-stage
clause that names the cause: the plan-time
`routing tables may be provisioned only before the flagship provider submission`
and the finalization-time
`Market resolution advanced while provisioning its routing tables`.

#### Red-proofs, measured

Driven live against the substrate validator at RPC `127.0.0.1:42888`, with the
market mid-life at `classify == Execute` and the Submit receipt (`4j9ipdXYKq…`,
slot 79,334) landed. Every row is a read-only `--provision-tables` run against
the same re-planned producer checkpoint; only the standing checkpoint changes.
The honest case admits, so the battery is not refusing for an unrelated reason.

| Case | Result |
| --- | --- |
| honest mid-life provision | **admits**, plans `execute` / `create` |
| `--standing-checkpoint` omitted | `a life already at execute may be provisioned only against its standing checkpoint; pass --standing-checkpoint` |
| `receipts: []` | `adopted receipts do not cover exactly the stages before execute` |
| `slot + 1` on the standing receipt | `adopted submit receipt slot differs from the finalized transaction` |
| `format` altered | `standing checkpoint format differs from this cluster` |
| a planned Execute packet in the standing checkpoint | `the execute routing table may not be provisioned: the standing checkpoint already plans an execute packet` |
| a foreign receipt that **does** name this market | `adopted submit receipt belongs to a different market` |
| a foreign receipt that names none of the four | `adopted submit receipt belongs to a different market` |

The two foreign rows deserve their construction stated, because a weak one would
prove nothing. Each is a receipt **every one of whose fields is re-derived from a
real finalized transaction on this cluster** — packet digest, first signature,
resolved key vector, balances, return data, slot, fee, CU. Nothing earlier in
§7.10's chain can refuse them; the market binding is the only clause left. The
near-miss (`4iCBjUR332…`, slot 6,091, 60 resolved keys) is the interesting one:
it *does* resolve this input's `market`, and still refuses, because the binding
demands `lifecycle`, `source_state` and `update_account` too. The far case
(`5nVvgSxMdL…`, slot 4,741) names none of the four.

**Honest limit on the cross-market row.** This substrate carries exactly one
resolved market, so the adversary is another *life's* real finalized transaction
rather than another *market's* real Submit. The clause exercised is identical —
§7.10 Ruling 4's four-account binding — and §7.10's own table already carries the
cross-market row; but the literal "another market's Submit receipt" case is not
reproducible here and is not claimed.

**Two clauses are proved by test rather than by chain, and one of them provably
must be.** Ruling 1 is a pure function of two stages, and chain can only exhibit
the positions this substrate happens to occupy, so it is proved over the whole
lattice in `routing_table_stage_gate_is_per_stage_reachability` — including the
row this wall is about, a mid-life `Submit` table refusing on
`the submit routing table may not be provisioned: the life is already at execute`,
and the fresh-life row where a life at Submit still writes all three tables.

Ruling 2's **receipt** clause is *unreachable from chain by construction*, and
saying so is better than staging it. Ruling 3's coverage rule makes the standing
receipts exactly the stages below `position`; for any receipt of stage `s` with
`s < position <= stage`, `routing_stage(s) <= s < stage` — Accept's mapping to
Execute only lowers it. So no chain state can make the receipt clause fire ahead
of Ruling 1. It is kept as an assertion of the property rather than deleted, and
it is proved directly in
`routing_table_stage_gate_reads_planned_packets_from_the_standing_checkpoint`.

#### What it bought, measured

The mid-life provision ran green with keys: `create` (10,784 CU), three ordered
`extend` pages (11,957 / 11,960 / 8,433 CU) and `freeze` (1,817 CU) —
**44,951 CU and 375,000 lamports of fees** across five finalized transactions,
leaving a frozen 48-row table of 1,592 bytes at 11,971,200 lamports of rent.

The Execute packet then **fit**: 1,203 wire bytes on the bundled-certificate
route, against a 1,232 limit. §7.11 derived 1,203 for exactly that route without
being able to measure it, and the derivation was **exact to the byte**. Of the 48
rows, 43 are extracted (3 writable, 40 readonly) and 5 keys stay static — the
resolver as fee-payer-signer, the Core program as the invoked id, ComputeBudget
and System from the prefix instructions, and `caller_authority`, the ninth key
§7.11 declined to plumb. §7.9's wall 8 is discharged: the packet the geometry
probe refused at +119 bytes now clears by 29.
Every condition it fired on is still refused, by the clause that names the
cause — and conditions it *never* fired on (a fresh life re-provisioning
Reclaim's table; a mid-life provision naming a foreign market) are refused now
and were not before.

### 7.13 Wall (10): the Execute frame was written for a resolver that does not pay

**Measured, sized, and STOPPED deliberately.** With §7.12's guard ruled and the
48-row table frozen on chain, the Execute packet fit for the first time — 1,203
wire bytes against 1,232 — and reached the cluster. Core refused it:

```
Program CtbPLmAcVc8xpzjZMrPZ14QfapnSMbjRdouUZLjUTBPp invoke [1]
Program CtbPLmAcVc8xpzjZMrPZ14QfapnSMbjRdouUZLjUTBPp consumed 20517 of 1399550 compute units
Program CtbPLmAcVc8xpzjZMrPZ14QfapnSMbjRdouUZLjUTBPp failed: custom program error: 0x3001
```

`0x3001` is `CoreSbfError::AccountFrame` — "account count, order, privilege,
executable flag, or alias refused."

**The count is right; the privilege is not.** `EXECUTE_PROVIDER_ACCOUNT_COUNT_V3`
is 47 and the builder emits 47. The failing conjunct is in
`validate_outer_frame` (`programs/dclutch-core-sbf/src/execute_provider_v3.rs`):

```rust
let signer = index == RESOLVER;
let writable = matches!(index, SOURCE_STATE | CERTIFICATE | LIFECYCLE);
```

Account 1 is the resolver, and it must be a **readonly** signer. Decoded from the
durable signed packet this lane produced:

```
numRequiredSignatures  1
numReadonlySigned      0
static[0]              2SVqjPNYveWR2reX11JehENyV65zYbeR88ezapQysuaA  (the resolver)
```

`compile_provider_execute_v0` sets `required_signers = vec![resolver]` and then
`payer = required_signers.first()`, and Solana's message compiler
unconditionally promotes the fee payer to a writable signer. The runtime derives
every instruction account's `is_writable` from the **message**, not from the
instruction meta. So Core sees `accounts[1].is_writable == true` and refuses.

**Nothing here disagrees except the payer choice.** The builder emits
`AccountMeta::new_readonly(intent.resolver, true)`; Core demands readonly; Core
forwards the message's writability into the CPI
(`let writable = index != MARKET && value.is_writable;`) and the Resolution
program demands the same thing at its own index 1
(`account.is_writable != (matches!(index, 2 | 3) || index == tail_start - 1)`).
Builder and both programs agree the resolver is a readonly signer. **Only the
transaction compiler's fee-payer default contradicts them**, so this is a
transport wall, not a program defect — and Decision 0012's `ReleaseSuperseded`
is *not* engaged, because nothing on chain needs redeploying.

**Why Submit escaped and Reclaim will not.** Submit's frame expects its payer
writable — `account.is_writable != matches!(index, 0 | 1 | 2 | 34)`, with the
submitter at index 0 — which is why the landed Submit receipt has
`numRequiredSignatures: 2, numReadonlySigned: 0` and passes. Reclaim's frame is
Execute's shape, not Submit's: `authenticate_reclaim_privileges` demands
`is_signer != (index == 0)` **and** `is_writable != matches!(index, 1..=4)`, so
its sole signer at index 0 must also be readonly — and
`compile_provider_reclaim_v0` makes that same account the fee payer. **Reclaim is
blocked by this wall too**, and will be, on any market, the moment its packet
fits.

**Sizing, measured rather than derived.** Every row below is a real compile of
this market's Execute action against the frozen 48-row table, packet bytes read
off the serialized transaction:

| Variant | Wire bytes | Margin | Resolver readonly? |
| --- | --- | --- | --- |
| as sent — resolver pays, top-up bundled | 1203 | **+29** | no (payer *and* transfer source) |
| resolver pays, top-up unbundled | 1155 | +77 | no (payer) |
| a distinct payer, top-up bundled | 1299 | **−67** | no (transfer source) |
| a distinct payer, top-up unbundled | 1251 | **−19** | **yes** |
| …plus `caller_authority` extracted (49-row table) | 1220 | **+12** | **yes** |

Two facts fall out of that table and both are load-bearing:

- **The certificate top-up must leave the packet, for a reason beyond bytes.**
  Its `from` is the resolver, signer and writable
  (`transfers: [{destination: certificate, lamports: 3_062_400, purpose:
  "terminal certificate"}]`). Bundling it keeps the resolver writable no matter
  who pays. Measured: the distinct-payer bundled variant still reports
  `numReadonlySigned: 0`.
- **The fix does not fit in the 48-row table.** A distinct payer costs 96 bytes
  — 64 for a second signature, 32 for its static key — and unbundling returns
  only 48. The remaining 19 must come from `caller_authority`, the ninth key
  §7.11 measured and declined to plumb, worth 31 bytes. The margin afterwards is
  **12 bytes**.

**The lane this needs, and why this one did not take it.** Three coupled changes,
none of them local:

1. A fee payer distinct from the resolver, through `compile_provider_execute_v0`
   and `compile_provider_reclaim_v0` — which means the driver signs Execute with
   two keys, and the payer's debit joins the stage's exact arithmetic.
2. The certificate top-up unbundled into its own prior transaction, with its own
   prestate authentication and its own receipt — the bundled top-up is currently
   part of the stage's conserved arithmetic, so this moves a lamport-flow across
   a transaction boundary.
3. `caller_authority` named in `input.json` and so in the union — §7.11's "one
   genuinely awkward one": a PDA over
   `(release_set, market, role, source_state, role_request_digest)` whose digest
   is the hash of a `Request` encoded inside the transport builder. Naming it
   means either re-deriving Core's request encoding in the producer or plumbing
   the builder's report out. It becomes load-bearing here, where §7.11 could
   still decline it.

And a new table: the union grows to 49 rows at a new slot-derived address, which
is exactly the mid-life provision §7.12 just made admissible — so wall 9's ruling
is what makes wall 10's fix reachable at all.

§7.9 ruled of wall 8 that *"this is a lane, not a patch."* The same ruling
applies here. This lane declined to invent a payer-and-arithmetic change at the
end of a driving session, with a 12-byte margin and a conservation ledger that
would have to be re-derived across a new transaction boundary.

**What this means for the life.** The market stays at `classify == Execute` with
its Submit receipt landed, its widened Execute table frozen on chain, and no
further stage reachable: Execute refuses on the frame, Accept is behind Execute,
and Reclaim carries the same defect. Redemption and retirement both require Core's
terminal receipt, so both are behind wall 10 as well. The complete life table this
session can honour therefore ends at the routing tables, not at retirement — and
every lamport it does cover conserves.

### 7.14 Wall (10), driven — and the four walls standing behind it

**Driven.** §7.13 measured wall 10, sized its fix at three coupled changes with a
12-byte margin, and declined to invent a payer-and-arithmetic change at the end
of a driving session. This section records that lane. Execute compiled at
**1,220 wire bytes** against 1,232 — §7.13's derived figure to the byte — and
landed. Accept landed behind it. **The market holds an accepted, verified
terminal receipt**, and the complete life table stands at 44 acts, conserved,
residual and drift exactly zero.

#### What the three changes cost, measured

1. **A payer distinct from the resolver.** `compile_provider_execute_v0` and
   `compile_provider_reclaim_v0` take it explicitly and refuse, locally, any
   payer that is the resolver, is zero, or aliases an instruction account.
   Each of those compiled a sendable packet before; each would have been
   refused on chain after 20,517 compute units and a cluster round trip.
   Measured in the crate's own test: **exactly 96 bytes**, 64 for the second
   signature and 32 for its static key — §7.13's arithmetic, confirmed.

2. **The certificate top-up, unbundled.** Execute refuses a required top-up by
   naming the exact lamports and destination, because the transfer's source
   must be the resolver and a System transfer makes its source a writable
   signer. The top-up became its own accounted act (slot 125,210, 150 CU),
   and the stage's conserved arithmetic is unchanged: with no bundled
   transfer, `StageV1::Execute => top_ups` is zero and the distinct payer's
   only debit is its fee.

3. **`caller_authority`, and it was cheaper than §7.13 expected.** §7.13 said
   naming it meant "either re-deriving Core's request encoding in the producer
   or plumbing the builder's report out." **Neither is needed.** The address is
   a PDA over five coordinates and `chain_facts` already pins every one of them
   against the finalized Market before any stage runs: `market_id == market`,
   `generation`, `selected_release_set`, the Source state, and
   `market_account.owner == core_program`. So the union derives it from the
   authenticated input alone — and derives it by calling the transport
   builder's own extracted `provider_execute_caller_authority_v3`, not a second
   implementation. **The derived address reproduced the failed packet's fourth
   static key byte for byte:** `7WYY28aqi1bLHxd5qUzcWJXNqkX9Er11fJjzbtEymhRw`.

Reclaim carried the identical defect and got the identical fix preemptively.
**Accept did not need it**: `admit_accounts` places the caller authority at
index 0 as a readonly *non-signer* and names no signer at all, so the resolver
may pay there and the frame is indifferent. §7.13's "Accept is behind Execute"
was true only in ordering.

#### 7.14.1 Wall (11): two records, one word

Accept landed and its post-finalization `verify_terminal` refused. The failing
clause, once the refusal could name it, was `certificate.route`.

`provider_finalized_projection_v3` writes `route: request.provider_release`, and
the transport builder sets that to `pyth_id` — which it reads *out of* the
Source's ProviderRelease record and then pins to the Pyth release account with
`authenticate_raw`. So `certificate.route` is the **Pyth release record's**
digest. `verify_terminal` compared it against the **Source ProviderRelease's**.
Measured on chain: the certificate's bytes at offset 48..80 are exactly
`sha256(pythRelease)`. Two records, one word, and nothing had ever caught it
because no market had ever passed Execute.

Repointed, and strengthened rather than merely repointed: the Source
ProviderRelease must name the Pyth release the certificate routed through. Both
records stay read; neither is dropped to make a clause pass.

**And the refusal was given a payload.** Thirty-three conjuncts behind one
string is a refusal that can be reported and not acted on — §7.4's lesson,
unlearned in a second place. The relation is unchanged, in the same order, with
every clause still required; it now names the ones that failed. It named wall 11
on the first re-run.

#### 7.14.2 Wall (12): Reclaim wants a substrate this ledger is not

Reclaim refuses with `0x8006 ResolutionDeployment`. Exactly one conjunct of the
registry-deployment gate fails, and all the others were checked against chain:

```
registry_programdata.upgrade_authority() = Some(6H8Ks96rr…)   the gate demands None
```

This is **not a defect**. `DEVNET_DEMO_DEPLOY.md` is explicit — *"Registry and
Rent must already be immutable when Core initializes its infrastructure
profile"* — and this is the **only** `upgrade_authority().is_some()` refusal in
the entire program set: the sole site enforcing that documented invariant. The
local ledger was deployed with a live authority, which is the local-validator
convenience, and Reclaim is where that convenience is finally read.

The remedy is one command and it is **irreversible on the substrate every wall
of this session was driven on**: `set-upgrade-authority --final` on the
Registry. It is not a transport change, it would permanently forbid any redeploy
on this ledger, and it was deliberately not taken. Reclaim is the one stage of
this life that this substrate cannot complete.

#### 7.14.3 Walls (14) and (15): the retirement path, two walls further on

Redemption and retirement sit behind Core's terminal receipt, not behind
Reclaim, so both became reachable. Two more walls fell on the way.

**Wall 14 — the refresh existed and the terminal sequence could not read it.**
The sequence refused before touching chain, on the all-or-none pairing:
*"It carries `direct_trading_funding_ledger` and omits `direct_capability_root`."*
That is **this document's own §3**, whose ruling was already made — the label
names two different addresses, and the refresh is the document that emits the
execution root under it. Only the threading was missing: `--refreshed-evidence`
reached the flagship producer and never reached here. Threaded with the same
mechanism and no new ruling; the session binds the refresh digest, because a
resumable sequence must not change which document carried its rows between
invocations any more than it may change the founding bytes.

**Wall 15 — the sequence was retiring a Market one generation short.** Past wall
14 it refused that the Market's generation was not the founding input's. It was
right, and it was comparing the wrong two things: `market.rs` is explicit that
the founding's product is *"the DCLTGMF3 Market at generation + 1 … the one that
ends Open, which is the product of the whole founding"* (`PrestateLaneV1::
Founding`, offset 1). All **eight** uses of the input's generation in that
module mean that Market — its identity, its Source state PDA twice, the Direct
BeginRetiring request twice, and the Resolution closure receipt. All eight were
one short: a Source state that does not exist, an identity that never matches.
They now pass through one named derivation, and the chain comparison it feeds
became a *check* of that derivation instead of an assumption.

With both landed the sequence passes evidence admission, the market join, and
ALT projection, and reaches Direct native-close coordinate planning.

#### 7.14.4 Wall (13/16): one hash short, and one undriven route

**Measured, sized, and STOPPED deliberately.** Redemption and the terminal
sequence's fourth stage refuse for a single shared root cause, and it needs a
ruling this lane will not invent.

`derive_founding_coordinates` computes two values. The Custody coordinates are
all derived from `context_digest = SHA256(PROJECTED_HOARD_CONTEXT_DOMAIN_V1 ||
context)`. The campaign evidence records the **raw `context`**. Reproduced
exactly at this market's generation:

```
evidence founding_custody_context = 637d53a4…7080cda2   (the pre-image)
SHA256(domain || that)            = 82306216…4b3b200d   (what chain uses)
```

Three independent chain facts confirm the second: the Claims aggregate's
`custody_context` field, the founding Trading-role replay's own stored
`context`, and the Hoard vault holding the market's 500.000000 collateral.
Market, release set, role and program id are all correct — **only this one seed
is a hash step short**, and every consumer that reads
`evidence.founding_custody_context` is therefore addressing an empty universe.

Behind it sits a second, independent blocker: the Claims-role replay
`6REWMhjH…` does not exist and **nothing under `tools/` drives the route that
creates it.** `programs/dclutch-claims-sbf/src/custody_replay_v1.rs` is a
dedicated first-use creation route — *"Only the Claims program can create the
Claims-role replay"* — and `terminal_settlement_v3` decodes the replay rather
than creating it, deliberately: creation is never a side effect of a payout. The
terminal payout lane has no step that initializes it.

So the ruling this needs is not small. Which document is authoritative for the
custody context, and which side converts — fixing the emitter changes the digest
of every founding artifact that exists; fixing the consumers spreads the
projection across every reader. And the missing lane step is a lane, not a patch.
§7.9's ruling applies again, and this lane declined to invent a second one at
the union.

**What this means for the life.** The complete life table this session can
honour runs from the controller funding ledgers to **Core's accepted terminal
receipt** — 44 acts, every lamport and atom read back out of the chain's own
finalized records, residual `+0` and drift `+0`. Redemption and retirement stand
behind 7.14.4; Reclaim stands behind 7.14.2, which is a property of this ledger
and not of the protocol.

### 7.15 Walls (12) through (22): redemption, and the life at 82 acts

**Driven.** §7.14.4 stopped at redemption and retirement with two blockers and no
ruling. Both were ruled, both fell, and six more stood behind them. Every one had
never executed on any market, which is what driving a path nothing has driven
looks like.

**Wall (12) ruled and driven.** The immutable-Registry command Reclaim demands
was approved *for this substrate only* — a local scratch chain that exists to be
driven. The command and its rationale were recorded before it ran, at
`jobs/dclutch-fill2/retire/wall12/`, along with the before-state of all seven
programs and the four things checked because it cannot be undone: nothing left
to redeploy, Registry is not an `ExecutionRoleV1` so no release-record authority
pin covers it, the retirement paths only *observe* a live authority rather than
comparing it to a stored one, and the direction is toward
`DEVNET_DEMO_DEPLOY.md`'s own step 1. Registry only. **Reclaim landed at slot
146,219, 76,667 CU**, and the resolution lane is complete at four receipts with
`verifiedTerminal` true.

**Wall (13/16), ruled.** The chain's persisted form is authoritative. Measured
rather than argued: every raw-form address is vacant, and the two live
objects — the Trading-role replay and the Hoard vault holding the market's
500.000000 collateral — are both at `SHA256(projected-hoard-context ‖
pre-image)`. This is §3's hazard a second time, one label naming two values, so
it gets §3's remedy: the refresh emits the value the terminal consumers mean,
under `chain_persisted_custody_context`, read from the Claims aggregate's own
persisted field and admitted only if the chain agrees it is the founding
pre-image's digest. The consumer re-derives that digest and demands
byte-equality, so a refresh can *select* which of a founding's two values its
label meant and cannot introduce a third.

The founding evidence of an already-founded market cannot be rewritten — its
digest is pinned inside the refresh's own lineage field — so the refresh is the
only document that could have carried this at all.

**Wall (16)'s second half was a driver, and it was built.** `DCLCCR01` had a
program, a codec, and two program tests, and no caller anywhere under `tools/`.
The driver follows fc31812c's shape and takes nothing economic: release set,
Realm, generation and custody context come off the aggregate, the rent refund off
Core's `rent_beneficiary`, the rent off the Rent sysvar. It calls the program's
own `expected_request_v1` rather than restating it, because the caller
authority's fifth seed is that request's digest. **The projected address
reproduced `6REWMhjH` — the exact replay §7.14.4 named as nonexistent —
independently of the probe that first derived it.** Created at slot 155,467.

**Wall (17): an expired plan was a permanent dead end.** Finalizing the Registry
moved one pinned prestate row, and the Reclaim plan — signed, never sent, its
blockhash long past `lastValidBlockHeight` — refused forever on the prestate
while naming something other than the cause. A packet the network refuses by
consensus rule has no future in which it lands, so it may be discarded and
re-planned. That is §7.7's own lesson, which checked expiry before the fee probe
for exactly this reason and stopped one frame short. `Submitted` stays
permanently poll-only.

**Wall (18): a label that reads semantic and is ordinal.**
`founding_funding_ledger_v2_N` is indexed by the campaign's own sort of
controller subsets, and the selected trade entry sits at manifest index 0, so
ordinal 0 is the **Trading** ledger. Three sites read it as the Resolution one —
one while labelling its own error *"Resolution FundingLedger"* — which aliased
two of the native-close frame's thirty-eight accounts and tripped a distinctness
clause that names nothing. Admission now refuses the aliasing by name.

**Wall (19): the System Program is the one address that reads as unset.** Its id
IS the all-zero pubkey, so the ALT closure's vacancy guard refused
`ResolutionCloseFund` at frame index 18 of 19 for naming it. Every closure names
it as a constant and classes it `InlineProgram`, a class no derived coordinate
carries, so exempting exactly that position keeps the guard meaningful.

**Wall (20): the ALT intent would not admit that its authority is the fee
payer.** Solana promotes the fee payer to a writable signer at index 0 whatever
the meta asked for, so an intent built from raw metas described a packet that
cannot exist. §7.13 met this promotion from the other side, where Core demanded a
readonly signer and the fix had to be a distinct payer at 96 bytes; the Address
Lookup Table program asks only that its authority sign, so here the only defect
was the bookkeeping. One named projection records what the message will carry, so
the comparison stays an exact equality instead of being taught to forgive.

**Wall (21): every payout ever attempted would have died at 200,000 CU.** The
payout compiler emitted one instruction and no compute budget. Measured:
*exceeded CUs meter at BPF instruction*, with the program still running. It had
never been observed because the replay this route decodes had no creation caller
until one existed. The route costs **314,539 CU when it pays nothing and 463,809
when it moves the collateral**. The declaration is the transaction ceiling on
purpose — the cost is a function of claim count and composition graph, which the
compiler does not see, and there is no priority fee here for a tight limit to
save. The ComputeBudget program id compiles static, so the canonical lookup
census is unchanged and a table frozen before the prefix existed stays valid.

The payout input also took its custody context from campaign evidence and so
addressed the raw-form replay. Decision 0008 §1 says the aggregate is the sole
persisted owner of a Market's Custody namespace and no route may re-guess it, and
that command already holds a finalized RPC — so it reads the owner directly. The
decision applied, rather than a document threaded past it.

#### Redemption, complete

Both positions read `[0,0,0,0]`; the Claims aggregate reads `[0,0,0,0]`; the
Hoard vault is drained to zero atoms. The market resolved to claim index 2, so
the founder took the whole 500.000000 collateral and the participant's
100,000,000 of claim 0 paid nothing — a real outcome, not a degenerate one. Five
payouts, slots 159,735 through 161,684.

Zeroing the aggregate is what made `CoreBeginRetiring` reachable: its zero-claims
gate is not a formality, and this is the first time it has ever been satisfied.
**It landed at slot 165,065, 88,414 CU**, and the market is Retiring.

#### 7.15.1 Wall (22): `CloseMakerReplay` is not a driver, and not a patch

**Measured, sized, and STOPPED.** `DirectBeginRetiring` refuses with
`InvalidRootState`. Read from chain, the Direct root's
`open_maker_root_count` is **2** — the two maker roots the fill opened — and
`direct_begin_retiring_v1.rs` requires zero.

The route that would decrement it is named by this tree's own documentation, at
`crates/dclutch-direct-codec/src/execution_v3.rs:118`:

> `CloseMakerReplay`: the ONLY action that could ever decrement
> `DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT`, **which every artifact in the
> tree today only increases**. Its absence is what makes `CloseDirectRoot` dead
> rather than merely unbuilt.

This is not §7.14.4's shape. The Claims replay had a real program route and
merely lacked a caller, so a driver closed it. Here the thing the discriminant
names does not exist, and an audit of the whole tree confirms it three ways:

- **No handler.** The Direct dispatch is not a match over the action space; it is
  a two-line refusal. `programs/dclutch-trading-sbf/src/hot_v3.rs:3961` returns
  `UnsupportedContent` for anything that is not `InlineOrdinary`. The string
  `CloseMakerReplay` appears **zero times anywhere under `programs/`**.
- **No artifact, and the selector table cannot name it.** The published
  `ordinary_lifecycle_entries()` is a `[CapabilityProgramSetEntryV2; 4]` —
  `InlineOrdinary`, BeginRetiring, NativeClose, Activation. A request carrying
  selector 11 fails `select_descriptor` in the ProgramSet before any program
  logic runs.
- **No decrement on chain.** Both Effect instructions that write the counter are
  fed by add-only transitions; the released one is a `checked_add` of two values
  each proven `<= 1`, so it is *structurally incapable* of producing a smaller
  number. The tree's only `checked_sub` on the counter lives in
  `successor.rs:2479` `close_maker_replay_v2`, whose only callers are its own
  three unit tests.

**And it is worse than "unbuilt".** The gate is enforced in five independent
places — twice in the operator, once natively in
`programs/dclutch-trading-sbf/src/direct_begin_retiring_v1.rs:518`, once in
`terminal_retirement_v1.rs:1136`, and once as an on-chain artifact transition in
`native_close_bundle_v1.rs:409` — so bypassing the host is not available. There
is no admin override anywhere in the tree, and `selected_release_set` is part of
`CoreState.identity` with no setter: it is only ever compared.

**So building the action later cannot rescue a market that already exists.** New
artifacts mean a new `program_set_id`, existing roots are bound to the old one,
and `require_close_selection` refuses any proposal whose `capability_release()`
differs from the persisted one. **Every market that has ever been filled under
the current release set is permanently unretirable, and its rent permanently
unreclaimable.** Building `CloseMakerReplay` fixes markets founded after the cut,
and only those.

**The specification already has the decrement.** `DirectSuccessor.lean:434-455`
models `openMakerRootCount := root.openMakerRootCount - 1` and proves
`result.root.openMakerRootCount + 1 = root.openMakerRootCount`. The Lean model
and the host codec both describe a close; the chain has no route that performs
one. This is a spec-versus-implementation divergence, not an oversight in the
driving.

**Sizing.** Nine to eleven distinct pieces: the transition/economics program that
emits the only decrement in the tree, a codec bundle module, three published
records (AccountProfile, Effect, descriptor), a fifth ProgramSet entry at
ascending selector 11, a program handler or a new native route, an operator plan
builder — which needs a way to *enumerate* live maker replays, and no index for
them exists today — bootstrap authoring and publication, a new release set and
fresh activation, and fixtures. `CloseDirectRoot` is not additional work: the
physical root close already ships as the native-close selector, and only its
precondition is unreachable.

**What this means for the life.** Retirement's remaining five stages sit behind a
protocol capability that has never existed on chain. That is a property of the
protocol, not of this ledger and not of this market.

#### What the life table says now

**82 acts, conserved, residual `+0` and drift `+0`** — founding, fill, fee,
resolution through Reclaim, the Claims-replay creation, five redemptions, and
`CoreBeginRetiring`. The sweep that built it also found **five acts the 44-act
table had missed**: two collateral-funding transfers, a position prefund, a
participant wallet funding, and a wallet-and-delegate creation. Completeness is
now checked rather than assumed — every finalized transaction touching any
market-specific account in the table is in the table.

The atom column is the proof the whole session was for: 550,250,000 atoms in from
the collateral wallet and the participant's stake, 550,250,000 out to the
founder's payout and the fee, and **every intermediate — the founding source, the
Hoard, the participant's wallet — nets exactly zero.** The collateral made a
complete round trip.

One transaction is deliberately outside the table and named here rather than
silently dropped: slot 1,119 creates the mint and mints the collateral universe,
~4,200 slots before this market's first act, and is shared with every other
market on this ledger. It is the substrate's genesis, not this market's life.
