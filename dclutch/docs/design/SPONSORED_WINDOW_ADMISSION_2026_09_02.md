# The sponsored-push window: a griefing vector that is not there, and a widening that is not applied

Status: **finding.** It changes no program byte. Claims below are
verified-from-source, read at commit `62a0b7fb5` (`tools/lane.sh` HEAD at the
time of writing); symbols are the citation and line numbers are hints.

It exists to retract one claim and to keep two smaller ones the retraction
uncovered.

---

## 0. What this document decides

1. The griefing vector recorded in `COHORT13_SEALED_FOUNDED_2026_09_02.md`'s
   first resolution addendum (`d1ab23b2`) — *"a market whose window closes
   unobserved has a `max_age_seconds`-wide interval in which one permissionless
   capture makes it unresolvable forever"* — **does not exist.** Capture already
   carries the window conjunct. §2 is the proof.
2. The repair that addendum queued as owed (*"refuse a capture whose
   `publish_time` fails `window.contains_observation`"*) is therefore **not
   owed**, and must not be landed: it would be a second copy of a check the
   route already makes.
3. What is real and does remain owed is a **disagreement between two spellings
   of one admission predicate** (§4): `cadence_tolerance_seconds` widens the
   admissible window on the multi-observation routes and is inert on the
   single-snapshot Pyth routes, and nothing anywhere says that is deliberate.
4. An oracle outage on a market with no recovery policy converts, exactly and
   by design, into revenue for whoever minted the failure claims (§5). Cohort-13
   is the worked example and the numbers are named.

---

## 1. The claim under review

The first resolution addendum built a three-row table and concluded that one
permissionless `Capture` between `window.end` and `window.end + max_age` strands
a market permanently: `Settle` would refuse `ProviderWindow` on the
out-of-window candidate, `CommitFailure` would refuse because the head is no
longer vacant, and `CloseHead` would refuse because it demands a terminal
Source. Its stated reason:

> `process_capture` builds the candidate from whatever `publish_time` the
> account holds, then `initialize_head_account` runs on any vacant head. Nothing
> in that path consults `contains_observation`.

The last sentence is **true as literal text and false as inference**, and it is
the reason the whole finding is wrong.

---

## 2. Capture carries the window conjunct

`process_capture` (`programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs`,
~line 92) reaches the window through one call it makes before it builds
anything:

```
process_capture
  -> authenticate_live_update                                    (:478)
       -> PythProviderAdapterObligationV2
            ::from_authenticated_sponsored_push_records           (:551)
       -> obligation.normalize_authenticated_update(..)           (:568)
```

and `PythProviderAdapterObligationV2::normalize_authenticated_update`
(`crates/dclutch-source-contract/src/provider_join_v2.rs`, ~line 220) opens with

```rust
if publication_unix_seconds < self.window.start_unix_seconds()
    || publication_unix_seconds > self.window.end_unix_seconds()
{
    return Err(Error::InvalidObservationSchedule);
}
```

which `sponsored_push_v1.rs:579` maps to `ResolutionError::ProviderWindow`. Its
own doc comment states the intent in as many words: *"A publication after
`window.end` is the late case a real provider cadence produces when nobody
submitted in time. It refuses here rather than resolving the market on a price
from after the question closed."*

So a late `publish_time` is refused **at capture**, on the capture's own
transaction, and the head is never occupied by a candidate `Settle` cannot
consume.

**Why a correct reading produced a wrong conclusion.** The check is spelled
inline against `window.start_unix_seconds()` / `window.end_unix_seconds()`, not
through `WindowSpecV1::contains_observation`. A sweep for `contains_observation`
across the capture path finds nothing — and finds nothing *correctly*. The
absent symbol was read as an absent conjunct. This is the same failure mode the
ledger already records as *"verified facts, wrong inference"*: every line cited
in the addendum is really there, and the verdict built on them is not.

The cheap general defence: when a conjunct is claimed missing, name the
**refusal code** the route would have to produce if it were present, and grep
for *that*. `ProviderWindow` is on the capture path twice (`:579`, `:1151`) and
would have found the check on the first try.

---

## 3. What is actually true about the lifecycle

Restated, with each conjunct read at HEAD:

| action | site | conjunct | refusal |
| --- | --- | --- | --- |
| `Capture` | `:132` | `clock.unix_timestamp > primary_deadline` | `ProviderFreshness` |
| `Capture` | `provider_join_v2.rs:232` | `publish_time` outside `[window.start, window.end]` | `ProviderWindow` (`:579`) |
| `Capture` | `provider_join_v2.rs:238` | `publish_time` outside `[clock − max_age, clock + skew]` | `ProviderFreshness` (`:582`) |
| `Settle` | `:876` | `clock.unix_timestamp <= deadline` | `ProviderFreshness` |
| `Settle` | `:1139` | the SAME two bounds, re-run against the candidate's own `snapshot_unix_seconds` | `ProviderWindow` / `ProviderFreshness` (`:1151`) |
| `CommitFailure` | `:1664` | head must be System-owned and `data_is_empty` | `SponsoredPush` |
| `CloseHead` | `terminal_source_for_cleanup` `:1445` | Source in `Resolved`/`FailureCommitted`/`Retired` | `Transition` |

`primary_deadline = window.end + max_age` (`:1000`). Two facts make the
lifecycle total rather than strandable:

- **Settle re-normalizes against the capture clock, not its own.**
  `sponsored_normalized_observation` is called with
  `sealed.candidate.snapshot_unix_seconds` (`:1225`). Without that, no candidate
  could ever settle: settlement begins only after `end + max_age`, so a
  publication inside `[start, end]` is necessarily older than
  `settle_clock − max_age`, and re-checking freshness against the settle clock
  would refuse every candidate the market ever admitted. The recorded capture
  time is what makes the whole route work, and it is worth stating because it
  looks like a shortcut and is the opposite of one.
- **The candidate is a durable snapshot, not a pointer.** `Settle` parses
  `candidate.update_bytes` (`:1075`), never re-reading the mutable price
  account. A later sponsored push cannot invalidate a captured candidate.

So: every candidate the head can hold is one `Settle` can consume, and a head
that is vacant at the deadline is one `CommitFailure` can step past. The
`CloseHead` terminality demand is not a trap, because the states it would have
to rescue are unreachable.

**Consequently the two repairs the addendum queued are both withdrawn.** Adding
a `contains_observation` call to capture would duplicate an existing conjunct
(and, per §4, would silently *widen* the route). Letting `CloseHead` admit a
non-terminal head after `end + max_age` would weaken cleanup to rescue a state
that cannot occur.

---

## 4. The real finding: one predicate, two spellings, different answers

`WindowSpecV1::contains_observation`
(`crates/dclutch-source-contract/src/lib.rs`, ~line 1212) says of itself:

> The tolerance widens the window symmetrically: a sample admitted at the first
> or last scheduled position may land up to the tolerance outside
> `[start, end]`, and **this is the one place that widening is stated.**

It is not the one place the widening is *read*. `contains_observation` has
exactly two callers, both inside `dclutch-source-contract`:

- `lib.rs:1549` — the scheduled-median statistic's per-sample admission;
- `lib.rs:2173` — `NormalizedProviderEvidenceV1::validate`, reached from
  `lib.rs:3965` (multi-observation statistic aggregation) and `lib.rs:4458`
  (the `SharedObservationChild` accumulation).

Both of those are **multi-observation** routes. The two **single-snapshot Pyth**
routes — relayed (`provider_v3.rs`) and sponsored
(`sponsored_push_v1.rs`) — reach the window only through
`PythProviderAdapterObligationV2::normalize_authenticated_update`, whose bound
is the raw `[start, end]` quoted in §2. Neither of them calls `validate`; nothing
outside `dclutch-source-contract` names `NormalizedProviderEvidenceV1` at all.

**So `cadence_tolerance_seconds` is inert on the single-observation routes.** A
market that bought a positive tolerance gets it on a scheduled-median product
and does not get it on a Pyth snapshot product, and no comment on either side
says that is intended. Cohort-13 carried `cadence_tolerance_seconds = 0`, where
the two spellings coincide exactly, which is why nothing showed.

**And the code says it is an oversight, in its own words.**
`normalize_authenticated_update`'s doc comment (`provider_join_v2.rs:204`)
introduces its two bounds as *"matching `NormalizedProviderEvidenceV1::validate`"*
— the very function whose window bound is `contains_observation` and therefore
widened. At `cadence_tolerance_seconds = 0` the two match exactly and the
comment is true; at any positive tolerance the comment asserts an equality the
code does not have. A deliberate narrowing would not have been written as a
claim of sameness.

This is a `map_err`-shaped defect one level up: not a discarded cause, a
*duplicated predicate*. The failure mode when two spellings drift is a market
whose admissible set depends on which product family it chose, discoverable only
by reading both.

**The candidate repair** is the smaller of the two directions: make
`normalize_authenticated_update` call `contains_observation` instead of
comparing `start`/`end` itself, so the widening is stated once and read
everywhere, and pair it with a hostile at a positive tolerance that is red
before the change and green after. The alternative — declaring the single-shot
routes deliberately unwidened — is defensible but must then be *written down at
both sites*, because right now `contains_observation`'s doc comment asserts
something false.

**Owner.** The source contract's admission predicate is **not Lean-emitted**:
`crates/dclutch-source-contract/src/generated_window_spec_v1.rs` is emitted by
`formal/dclutch-semantics/EmitSourceWindowSpecV1Rust.lean` and carries the
record's *ABI only* — magic, width, and field offsets. `contains_observation`,
`normalize_authenticated_update` and `NormalizedProviderEvidenceV1::validate`
are hand-written Rust. So this is not a Lean-first repair, and it is not this
lane's to land: `crates/dclutch-source-contract` and
`programs/dclutch-resolution-proof-sbf` are both held by the lane that landed
`f6e9b8d08` (`source: the Source resolution state gets the second admission
type`) and that owns the window-spec emission. **Queued to that owner, with the
hostile named above as its gate.**

---

## 5. Where an oracle outage's money goes

This is not a defect and it is not hidden — `exhaust_after_primary_deadline`
documents the property in its own words: *"a silent provider cannot make a
market unresolvable, only drive it to a pre-disclosed outcome."* It should still
be said in one plain sentence, because the sentence is about who gets paid:

**When the oracle goes quiet, the market pays the outcome the founder minted and
kept, and it pays the strangers who bought a real outcome nothing.**

Cohort-13, 2026-09-02, with the numbers from chain:

| | |
| --- | --- |
| window `DCLTWIN1` | 1788369759–1788371559 (13:22:39–13:52:39 EDT), 1,800 s |
| honest observation | unreachable — the pinned account had moved to `publish_time` 1788372175, 616 s past the close |
| recovery policy | none (`RECOVERY_PRESENT` = 0), so the failure walk is the only terminal route |
| failure selector | 3 = `ResultDomainV2::failure_selector()` = `region_count` |
| founder's failure claims | **500,000,000**, the entire supply |
| participant-2's failure claims | **0** (they hold 200 atoms of outcome 0) |
| participant-1's failure claims | **0** |
| paid to the founder | **500,000,000** atoms of collateral, the Hoard vault emptied from 500,000,000 to 0 |
| paid to participant-2 | **0**, asserted from chain: `payout: "0"` for their 200 atoms at claim index 0, and *"payout quantity must be within 1..=0 atoms at claim index 3"* for the failure claim they do not hold |

The founder minted every failure claim at founding and no one traded for one, so
the failure outcome's entire supply sat with the party who also chose the
oracle, the window and the absence of a recovery policy. Nothing here was
misbehaviour: participant-2 bought SOL/USD below \$96.00 and the sponsored feed
read \$99.337 twelve minutes after the window closed, so on the honest reading
they would have lost anyway. But the honest reading never happened, and the
structure that decided it was *the founder's own configuration*.

Two consequences worth carrying, neither of them a code change here:

1. **A market with no recovery policy is a market whose oracle risk is priced
   entirely into the failure claims.** A buyer who cannot see the failure
   position's owner cannot see who is paid when the feed goes quiet. That is a
   disclosure surface (`apps/dclutch-web`), not a program defect.
2. **The founder's incentive under an outage is not neutral.** Nothing in this
   route lets a founder *cause* an outage — the feed is Pyth's sponsored account
   and the pin is immutable — but the payoff asymmetry exists and should be
   stated wherever a market's terms are shown, rather than discovered at
   resolution.
