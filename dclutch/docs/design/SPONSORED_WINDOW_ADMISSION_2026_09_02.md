# The sponsored-push window: a griefing vector that is not there, and a widening that is not applied

Status: **finding, §4 amended.** It changes no program byte. Claims below are
verified-from-source, read at commit `62a0b7fb5` (`tools/lane.sh` HEAD at the
time of writing); symbols are the citation and line numbers are hints.

**Amendment, 2026-09-02 (TIDY).** §4 overstated twice and both are corrected in
place below, with the evidence: its census of `contains_observation`'s callers
missed a third one outside the crate, and its headline claim — that a market's
admissible window depends on which product family it bought — is false, and was
false at `62a0b7fb5`. §2's retraction and §5's account of where an outage's
money goes are unchanged and stand. New citations in §4 are read at
`60e9b860a`; where a line moved under `0b0a05e93` both numbers are given.

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
3. Two spellings of one admission predicate did exist (§4):
   `cadence_tolerance_seconds` widens the admissible window on the
   multi-observation routes and was inert on the single-snapshot Pyth routes.
   **They could not disagree on any constructible window**, so nothing was ever
   admitted or refused differently on account of it — this note originally said
   otherwise and §4 now carries the refutation. The duplication was still a
   second author for one predicate, and `0b0a05e93` removed it as the identity
   everywhere reachable. Not owed.
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

## 4. One predicate, two spellings — and the difference was unreachable

`WindowSpecV1::contains_observation`
(`crates/dclutch-source-contract/src/lib.rs`, ~line 1212) said of itself, at
`62a0b7fb5` and before `0b0a05e93` rewrote it:

> The tolerance widens the window symmetrically: a sample admitted at the first
> or last scheduled position may land up to the tolerance outside
> `[start, end]`, and **this is the one place that widening is stated.**

It is not the one place the widening is *read*. **This note first said
`contains_observation` had "exactly two callers, both inside
`dclutch-source-contract`". It had three, and the third was outside the crate:**

- `lib.rs:1549` — the scheduled-median statistic's per-sample admission;
- `lib.rs:2173` — `NormalizedProviderEvidenceV1::validate`, reached from
  `lib.rs:3965` (multi-observation statistic aggregation) and `lib.rs:4458`
  (the `SharedObservationChild` accumulation);
- `tools/local-validator/bootstrap/successor/src/flagship_resolution.rs:1941`
  — `validate_observation_fields`, the **offchain preflight** the flagship
  resolution runs before it submits a sponsored observation. Present at
  `62a0b7fb5` at that exact line, so it was there to be found.

The first two are **multi-observation** routes. The two **single-snapshot Pyth**
routes — relayed (`provider_v3.rs`) and sponsored (`sponsored_push_v1.rs`) —
reached the window only through
`PythProviderAdapterObligationV2::normalize_authenticated_update`, whose bound
was the raw `[start, end]` quoted in §2. Neither of them calls `validate`;
nothing outside `dclutch-source-contract` names `NormalizedProviderEvidenceV1`
at all. The third is offchain and is the subject of the next two paragraphs.

**"Both inside `dclutch-source-contract`" reported a sweep's scope as a
result.** A census run inside the crate cannot surface a caller outside it; the
sentence stated the absence anyway. That is §2's error one level out — there an
absent *symbol* was read as an absent conjunct, here an absent *hit* was read as
an absent caller — and the cheap defence is the same shape: before writing
"exactly N callers", name the tree the sweep covered.

**And the missed caller was the one that actually diverged.** The preflight used
`contains_observation`, the *wide* spelling, while the program route it stands in
front of compared `start`/`end` itself — an offchain gate strictly more
permissive than the onchain one it predicts, which is the drift direction worth
finding and the one this census scoped out. Since `0b0a05e93` the program site
calls the same predicate (`provider_join_v2.rs:244`), so all four sites — two
statistic routes, the Pyth route, the preflight — read one author.

**So `cadence_tolerance_seconds` was inert on the single-observation routes.**
This note then wrote that *"a market that bought a positive tolerance gets it on
a scheduled-median product and does not get it on a Pyth snapshot product"*, and
that the failure mode is *"a market whose admissible set depends on which product
family it chose"*. **That is false, and it was false at `62a0b7fb5`.** No market
can buy a positive tolerance on a window a Pyth snapshot route will accept. Two
independent gates, both already at that commit:

- `WindowSpecV1::tolerating_cadence` (`lib.rs:1118`) is the **sole mutator** of
  `cadence_tolerance_seconds` — every constructor and `WindowSpecV1::decode`
  (`lib.rs:1153`) pass through it — and it refuses `InvalidWindow` for a nonzero
  tolerance on a `WindowKind::Terminal` window (`lib.rs:1119`, unmoved since).
  The single-snapshot obligation's join refuses any window whose kind is *not*
  `Terminal` (`provider_join_v2.rs:183`, `LinkageMismatch`). A positive
  tolerance and a Pyth snapshot route cannot occupy the same window.
- `validate_cadence_tolerance_pairing` (`lib.rs:1612`, `62a0b7fb5:1605`; called
  at `:1541` and `:3962`) separately refuses `NonCanonicalStatistic` for a
  nonzero tolerance under any statistic but `OddScheduledMedian`.

Either gate alone settles it, and neither is about the product family the market
chose: the tolerance is unrepresentable on the window, not read differently by
two consumers of it. Cohort-13's `cadence_tolerance_seconds = 0` is therefore
**not** why nothing showed — nothing could have shown, on any market.

The same overstatement runs one paragraph further.
`normalize_authenticated_update`'s doc comment (`provider_join_v2.rs:204`) did
introduce its two bounds as *"matching `NormalizedProviderEvidenceV1::validate`"*
while spelling the window bound itself, and that is a duplicated predicate worth
removing. But the claim built on it — that *"at any positive tolerance the
comment asserts an equality the code does not have"* — quantifies over states
the constructors refuse. At every tolerance reachable on that obligation the two
did match, and the comment was true.

So this is a `map_err`-shaped defect one level up — not a discarded cause, a
*duplicated predicate* — and its cost was a **second author**, not a wrong
answer. That cost is still real: a window kind that one day carries both a
cadence and a snapshot route would reintroduce the divergence with nothing
going red.

**The candidate repair landed at `0b0a05e93`**, and in the smaller of the two
directions: `normalize_authenticated_update` now calls `contains_observation`
(`provider_join_v2.rs:244`) rather than comparing `start`/`end` itself. Because
of the gates above it is **the identity everywhere reachable**, so the hostile
this note asked for — red before the change, green after — could not be written
against any window a Pyth route accepts. What stands in its place is
`a_terminal_window_cannot_reach_the_single_snapshot_route_with_a_tolerance`
(`provider_join_v2.rs:687`), which pins all three gates by construction and
then shows the widening is real wherever it *is* reachable, on a
`ScheduledInterval` window: `end + 120` admitted, `end + 121` refused
(`:734`–`:737`). Both doc comments were rewritten to say why
the tolerance is structurally zero on those routes rather than to assert a
sameness (`lib.rs:1216`, `provider_join_v2.rs:213`), which is the second
direction's obligation discharged as well.

**Owner.** The source contract's admission predicate is **not Lean-emitted**:
`crates/dclutch-source-contract/src/generated_window_spec_v1.rs` is emitted by
`formal/dclutch-semantics/EmitSourceWindowSpecV1Rust.lean` and carries the
record's *ABI only* — magic, width, and field offsets. `contains_observation`,
`normalize_authenticated_update` and `NormalizedProviderEvidenceV1::validate`
are hand-written Rust. So this is not a Lean-first repair, and it is not this
lane's to land: `crates/dclutch-source-contract` and
`programs/dclutch-resolution-proof-sbf` are both held by the lane that landed
`f6e9b8d08` (`source: the Source resolution state gets the second admission
type`) and that owns the window-spec emission. **Queued to that owner, and
closed by it at `0b0a05e93`** — which is also where the sharper reading above
came from.

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
