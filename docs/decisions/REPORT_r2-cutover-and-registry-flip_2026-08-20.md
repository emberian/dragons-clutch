# Decision report: R2 cutover and registry flip (E2 + E3, with E1/E4/E5 as inputs)

Register entries: `r2-identity-freeze` (E2, **dated 2026-08-26**),
`r2-registry-flip` (E3), with `r2-model-close-ratification` (E1),
`r2-runtime-capabilities-merge` (E4), and `r2-legal-tos-lane` (E5) as inputs,
from [`DECISION_REGISTER_2026-08-20.md`](DECISION_REGISTER_2026-08-20.md).
E6 (`p1-source-backlog`) is out of scope per the register's fan-out note.

Owner: **ember** (E1, E2, E3, E4), **ember+counsel** (E5). This report decides
nothing and promotes nothing; the default ELF's source registry remains empty
and Endow keeps refusing `SourceReleaseUnavailable` (`0x79`)
(`docs/design/SOURCE_PROVIDER_V1_SELECTION.md:6-8`, `CURRENT_TRUTH.md:300`).
Every cite below was read in this working tree at `e5b0503`; the one
experiment this report ran — the E4 trial rebase — was executed on a
throwaway branch and its result is reported in §3.

Evidence base: `docs/design/SOURCE_PROVIDER_V1_SELECTION.md` (the selection
and §5 sequencing), `docs/implementation/R2_PULL_PROMOTION_PLAN.md` (phases
and §5 gates), `docs/implementation/PYTH_PULL_PROFILE_R2.md` (the frozen model
contract and default-release STOPs), `research/source-profile-v1/`
(`DOSSIER.md`, `PROVENANCE.md`, `src/{spec_v2,crossing_v1,auth_v2}.rs`),
`GOAL.md:49-56` (queue items 1-2), `:636-653` (the E4 branch record),
`:749-757` (the codex resolution record), `CURRENT_TRUTH.md:136-148`
(authorization scope), `:300`/`:305` (matrix rows), `:391-397` (STOP 3),
`programs/clutch-sbf/program/src/instructions/source_ingest.rs` (the
fail-closed registry), `research/liveness-policy-profile/terminal_profile.py:255`
(`SOURCE.DEFAULT_REGISTRY_EMPTY`), and the parked
`fable/r2-runtime-capabilities` branch (`f9045a0`), log-and-diffed against
current main (`e5b0503`).

Headline findings, stated first:

1. **The E4 trial rebase is CLEAN — zero conflicts** — despite 153 commits of
   drift since the merge base and a +60/−4 drift in the one shared file
   (`lib.rs`). The rebased branch compiles against current main and its 42
   tests pass 42/42. The trial branch is left in place as
   `r2-caps-rebase-trial` (tip `01a004b`, parented on main `e5b0503`). §3.
2. **One §5 flip gate is already green:** the plan's "V3 successor branch
   coordination" gate (`R2_PULL_PROMOTION_PLAN.md:143-148`) resolved itself —
   `fb72b34` (the V3 staged lifecycle) is an ancestor of current main, so R2
   now simply rebases onto V3's sealed base, exactly the ordering the plan
   prescribed. §4.
3. **The identity freeze is not faucet-blocked.** Post-cutover pins are
   read-only chain-state and repository facts (program bytes, ProgramData,
   Config bytes, SDK release); none of it needs a funded key. The devnet
   drought (GOAL.md "no devnet SOL coming", 2026-08-20 ~10:50) blocks only
   public-cluster *exercise* of a flipped artifact, not the freeze or the
   local evidence ladder. §2, §4.3.
4. **A frozen SourceSpecV2 is cluster-specific by construction** — it binds
   the ProgramData deployment slot and the receiver Config full-body digest
   (`SOURCE_PROVIDER_V1_SELECTION.md:59-73`, `PYTH_PULL_PROFILE_R2.md:13-16`),
   both per-cluster state. The Phase-1 checklist
   (`R2_PULL_PROMOTION_PLAN.md:96-113`) never names which cluster it pins.
   That is a gap this report surfaces: the freeze act must state its cluster,
   and a devnet-pinned and a mainnet-pinned spec are different feed
   identities by construction. §2.3.
5. **The freeze embeds a trust decision, not just pinning.** Dossier §7 item 2
   ("Decide whether the protocol accepts Pyth's 3-of-5 router trust model",
   `research/source-profile-v1/DOSSIER.md:220`) has no other home: signing
   the freeze is accepting a 3-of-5 secp256k1 router quorum as the Terms
   trust floor, replacing 13-of-19 wormhole guardians
   (`SOURCE_PROVIDER_V1_SELECTION.md:161-163`, `:175-177`). §2.2.
6. **Aug 26 is a contested day on ember's calendar**: the cutover
   (16:00 UTC) shares its date with the perpetuals-RFC due date (G2), two
   days after the two Aug-24 CFTC filings (G1) and one day before the IAC
   statement date (G3) (register "Known hard dates"). E2 is "when after, not
   whether" — everything below is aimed at making the freeze act itself a
   short review-and-sign, not a work item, in a filing-loaded week. §2.

---

## 1. The decision cluster, stated

Four decisions plus one commissioning, in a forced partial order:

- **E1 — ratify the model close.** The codex convergence wave closed the six
  R2 model ambiguities: closing-boundary `CROSSING_V1` rule id 2 only,
  368-byte SourceSpecV2 under the distinct `dragons-clutch/feed/v2` domain,
  exact ProgramData/config pins, zero grid origin, decoded-body duplicate
  collapse, start-aware contiguity, named overflow refusals
  (`SOURCE_PROVIDER_V1_SELECTION.md:96-157` §4 + the §4.1 table;
  `PYTH_PULL_PROFILE_R2.md:9-38`). GOAL records it as "resolved-by-codex
  pending your ratification pass" (`GOAL.md:49-53`, `:749-757`). Research-only;
  ratifying authorizes nothing runtime.
- **E2 — the production identity freeze**, deliberately sequenced *after* the
  2026-08-26 16:00 UTC Pyth DAO receiver cutover (in-place upgrade, same ABI,
  13-of-19 wormhole → 3-of-5 router quorum). "A pre-cutover identity freeze
  is forbidden in this design" (`SOURCE_PROVIDER_V1_SELECTION.md:159-174`).
  Freezes: receiver program/ProgramData identity bytes, config byte digest,
  SDK release pin — and only then permits a registry entry.
- **E4 — merge the parked `fable/r2-runtime-capabilities` branch** (`f9045a0`:
  Upgradeable-Loader ProgramData + Instructions-sysvar decoders, 42
  adversarial tests, wired into nothing, `GOAL.md:636-653`). Content is done;
  the decision is merge *scheduling*, because any closure-byte change forks
  the sealed ELF identity and demands a reseal cycle (the commit message says
  so itself; MEMORY's R1 record agrees).
- **E3 — the registry flip.** Compiling a production source release into the
  default ELF's `release_registered`
  (`programs/clutch-sbf/program/src/instructions/source_ingest.rs:738-743`)
  is **the protocol's first value-admission authority** — the single decision
  that ends the empty-registry/`0x79` era. It is reserved to ember in every
  authorization statement (`GOAL.md:11-13`, CURRENT_TRUTH.md:143-145 "the
  production source-registry flip … remain[s] outside this authorization";
  `R2_PULL_PROMOTION_PLAN.md:149-151`). It deserves its own gravity and gets
  its own section (§4): until it happens, every Endow anywhere refuses value
  before owner allocation or Token-2022 CPI; after it, the refusal boundary
  *narrows to everything except one exact registered release* — it never
  disappears (`R2_PULL_PROMOTION_PLAN.md:118-124`). It also forces a full
  reseal cycle by construction (`:126-129`) and retires the
  `SOURCE.DEFAULT_REGISTRY_EMPTY` blocking id
  (`terminal_profile.py:255`; register C6).
- **E5 — commission the Pyth ToS legal lane** (ember+counsel), scoped in §5.

The forced order: E1 ≺ E2 ≺ E3 (each builds on the last; the plan's §5 gate
list makes E2 a hard prerequisite of E3). E4 is content-independent of all
three but couples to E3 through reseal economics (§3.2, §6). E5 gates E3
conservatively and gates any public frontend unconditionally (§5).

A standing falsifier overhangs the whole cluster and survives every
ratification: **one demonstrated double-witness boundary — two distinct
qualifying updates for one `T(k)` — reopens the provider selection entirely**
(`SOURCE_PROVIDER_V1_SELECTION.md:107-111`, `:197-199`). Nothing ember signs
here waives it.

## 2. Calendar analysis: before / at / after Aug 26

Register hard dates: **Aug 24** (two CFTC filings + John-packet
prerequisites), **Aug 26 16:00 UTC** (cutover; perpetuals RFC also due
Aug 26), **Aug 27** (IAC statement "should submit by"). The cutover is six
days from the register date; the freeze-vs-cutover geometry was designed
2026-08-19, when the design said "Freezing identity now pins a program seven
days from an in-place mutation" (`SOURCE_PROVIDER_V1_SELECTION.md:163-164`).

### 2.1 Before Aug 26 — preparable now, no identity bytes

Everything in this list is explicitly cutover-independent
(`SOURCE_PROVIDER_V1_SELECTION.md:167-172`; `R2_PULL_PROMOTION_PLAN.md:41-45`
"Everything below is buildable before 2026-08-26 because it pins no identity
bytes"):

- **E1's ratification itself** — the earlier the cleaner: "ratifying late
  means re-litigating under cutover pressure" (register E1 blocked-on note).
  Cost approximately zero (§8).
- **Phase 0 in full, on a dedicated runtime branch** (never sealed main):
  P0.1 layout (368-byte body, `InitSourceSpecV2` intent tag), P0.2 kernel
  port from `research/source-profile-v1` (swap `sha2` for
  `solana_sha256_hasher::hashv`, replace the two `MODEL_*` constants the
  research crate itself flags), P0.3 v2 authenticator trait
  (`LoaderStateV1 { linked_programdata, deployment_slot }` — the V1 two-body
  `deployment_generation() -> u64` shape is structurally wrong for pull),
  P0.4 remaining capabilities (signed-i64 Clock comparisons, external-body
  config digest — the other two P0.4 capabilities are the parked E4 branch),
  P0.5 account planes (Endow's state-role table hard-pins the 292-byte V1
  spec and must learn the 404-byte v2 account), P0.6 mock reshape, P0.7
  hostile SVM campaign, P0.8 error-granularity decision (27-variant research
  `AuthV2Error` vs one collapsed `SourceAdmissionFailed`), P0.9 registry
  mechanism (`release_registered` as a two-generation predicate; the six
  dispatch sites' hard-coded `::<MockParser, MockDeployment>` turbofish —
  visible at `source_ingest.rs:764,792,812,840,876` — cannot express even
  two registered releases) (`R2_PULL_PROMOTION_PLAN.md:41-92`).
- **Adopting the E4 rebase** as that branch's base (§3.2) — the decoders are
  P0.4's first half and P0.3's prerequisite ("R2 Phase-0.3's authenticator
  trait needs these decoders", register E4).
- **The Phase-1 runbook**: the checklist exists
  (`R2_PULL_PROMOTION_PLAN.md:96-113`); pre-staging means drafting the
  release-dossier template (the `PROVENANCE.md` successor section, checklist
  item 6), naming the target cluster (§2.3), and writing the exact commands /
  primary-source URLs so the post-cutover collection is a lane's afternoon,
  not a design session.
- **E5 commissioning** (the ask to counsel can be written now even if counsel
  bandwidth is contested until Aug 27 — §5).
- **What is already frozen and needs no act**: the model semantics
  (`CROSSING_V1` closing-only, zero origin, cursor/overflow, duplicate
  collapse), the parser/authentication model against the pinned `ec456fc`
  ABI — the cutover does not change the ABI
  (`SOURCE_PROVIDER_V1_SELECTION.md:167-172`), the hostile SVM test plan, and
  the archive semantic-owner rules.

What must NOT happen before Aug 26: any identity-byte pin, any registry
entry, any interim entry "to get ahead" — all three are forbidden by the
design in as many words (`:173-174`; GOAL.md:54-56 "the model does not
authorize an interim registry entry or value admission").

### 2.2 At/after Aug 26 — what the freeze needs from post-cutover chain state

The Phase-1 checklist, in order (`R2_PULL_PROMOTION_PLAN.md:96-113`), with
this report's annotations:

1. **Confirm the DAO cutover executed**; record post-cutover receiver program
   bytes' identity, ProgramData key, decoded deployment slot. *Annotation:*
   an in-place upgrade rewrites ProgramData — the deployment slot the spec
   pins is the *cutover's* slot, and E4's decoder is the tool that reads it.
   Its revoked-authority finding (bytes 13..45 of a revoked ProgramData still
   hold the former authority; the decoder proves it never reads them,
   `GOAL.md:645-650`) is exactly the class of hazard a naive post-cutover
   pin would inherit.
2. **Pin the post-cutover receiver Config full-body SHA-256.** The digest is
   the governance-generation pin: any later governance change (fee, sources,
   router address, `minimum_signatures`) is a new feed generation by
   construction (`SOURCE_PROVIDER_V1_SELECTION.md:61-67`). *Annotation:* this
   is also why freezing *too fast* is cheap to regret — a post-freeze
   governance touch-up by the DAO (plausible in a cutover's settling days)
   orphans the frozen generation. The failure mode is fail-closed, not
   unsound (the entry would refuse), but a burned generation costs a re-pin,
   and if already compiled in, a reseal.
3. **Pin the SDK/source release** — and here sits the named STOP: the
   migration guide says 1.2.0, the SDK manifest says 2.0.0 ("if it has not
   [resolved], record both and STOP"). `PROVENANCE.md` pinned the reviewed
   `pyth_solana_receiver_sdk/Cargo.toml` at version 2.0.0 with the
   `pro-compatible` feature; the discrepancy is upstream's to resolve, ours
   only to observe resolved.
4. **Set `activation_unix_timestamp`** at or after the cutover instant.
5. **Re-verify the 134-byte `PriceUpdateV2` layout and discriminator**
   against the deployed post-cutover program (expected unchanged: same ABI).
6. **Write the release dossier** into `PROVENANCE.md`'s successor section —
   every identity, checksum, URL, retrieval date.

Plus the two things the checklist implies but does not name:

7. **The trust-floor acceptance** (finding 5): signing the freeze is deciding
   dossier §7 item 2 — the protocol accepts the 3-of-5 router quorum plus
   pinned config generation as the Terms trust floor, with failure
   consequence stall-then-lapse, never substitution
   (`SOURCE_PROVIDER_V1_SELECTION.md:175-177`). A 3-of-5 secp256k1 quorum is
   a materially smaller signer set than 13-of-19 guardians; ember should
   accept that consciously, not as a checkbox.
8. **The cluster declaration** (finding 4): which cluster's receiver state
   the pins describe. §2.3.

**On "when after":** the register's E2 options are (1) freeze promptly, (2)
wait for stability evidence. The design gives no stability criterion. This
report recommends recording one rather than a calendar promise: **freeze when
(a) the SDK version discrepancy is observed resolved, and (b) the receiver
Config bytes have been observed unchanged across some named post-cutover
observation span** — then E2's date is evidence-triggered, ember's act stays
review-and-sign, and nobody pins bytes at 16:01 UTC on a filing-week
afternoon. The counter-pressure (every day unfrozen is a day E3 cannot even
be scheduled) is real but weak: E3's Phase-0 gates will not all be green in
cutover week anyway (§4, §8).

### 2.3 The cluster-scope gap (finding 4)

SourceSpecV2 binds the ProgramData deployment slot and the Config full-body
digest. Both are per-cluster chain state: the receiver program id is shared
across clusters, but each cluster's ProgramData was written at a different
slot and each cluster's Config is its own account. Therefore **a frozen spec
names one cluster**, and a registry release compiled from it admits value
only against that cluster's provider state. The Phase-1 checklist should gain
a line 0: "name the cluster (devnet / mainnet-beta); one dossier per cluster
if both are ever wanted." Given the authorized frame is Track C
devnet/testnet only (`CURRENT_TRUTH.md:136-148`), the natural V1 answer is a
devnet-state pin — with the explicit consequence that a later mainnet posture
is a second freeze act and second feed identity, not a reuse. (Whether the
DAO cutover lands on all clusters at the same instant is itself a Phase-1
item-1 observation to record, not to assume.)

### 2.4 The calendar, compressed

| When | What | Owner |
| --- | --- | --- |
| Now → Aug 25 | E1 ratification; Phase-0 branch (P0.1–P0.9) seeded from `r2-caps-rebase-trial`; Phase-1 runbook incl. cluster declaration + dossier template; E5 ask drafted | ember (E1, one line); lanes (rest) |
| Aug 24 | filings freeze/submit (G1) — the artifact they describe is the structurally value-refusing one; nothing in this cluster may change that before then (and nothing can: E3 ≻ E2 ≻ Aug 26) | ember |
| Aug 26 16:00 UTC | cutover lands; observation begins (record item-1 facts; start Config-stability watch) — no freeze act same-day required or recommended | lane (observation) |
| Aug 26 → freeze day | stability criterion runs; SDK discrepancy watched; Phase-0 gates finish going green | lanes |
| Freeze day (evidence-triggered) | E2: review dossier, accept trust floor, sign; pins recorded in `PROVENANCE.md` successor | **ember** |
| After E2 | E5 concluded (conservative path); Phase-2 compile + full reseal; **E3: ember's explicit go** | **ember** (+counsel for E5) |

## 3. The branch-merge sub-decision (E4)

### 3.1 Trial rebase: executed, clean, green

Performed on a throwaway branch, per the register's honest option to "assess
rebase cost against current main":

- Branch content: exactly one commit `f9045a0` — `loader_state.rs` (739
  lines), `instructions_sysvar.rs` (1,035 lines), +4 lines in `lib.rs` (two
  doc-table rows, two `pub mod` declarations). No touch on `genesis.rs`,
  `seeds.rs`, or any instruction family.
- Drift: merge base `7e4f3cd`, **153 commits behind current main**
  (`e5b0503`); the one shared file (`lib.rs`) drifted +60/−4 in that span;
  no file-name collision appeared on main.
- Result: `git rebase main` — **clean, zero conflicts**. The rebased tip is
  left in place as **`r2-caps-rebase-trial` (`01a004b`)** per instruction.
- Verification (narrowest possible, per house rule): the lib test target
  compiled against current main and the branch's own suites ran filtered —
  **42 passed, 0 failed** (168 non-branch lib tests filtered out; main's lib
  grew 157 → 168 tests since the branch parked, none colliding). Not run:
  any unfiltered suite, any SBF build, any bank campaign — the branch is
  routed by nothing, so lib-target compilation plus its own 42 tests is the
  full refutation surface for the *rebase*; the reseal cycle, when it
  happens, carries the rest.

So the register's "bit-rot risk against `genesis.rs`/`seeds.rs` shared-edit
churn" has not materialized: **rebase cost today is zero**. The real cost of
E4 was never the rebase — it is the reseal (a closure-byte change forks the
ELF identity; commit message and R1 record both say so).

### 3.2 Merge now vs ride the freeze wave

Three honest shapes:

- **(a) Merge to main now, reseal now.** Buys nothing E3 needs earlier and
  spends a full reseal cycle (final-LTO/stack audit, liveness re-measurement,
  100+-gate emission, manifest commit, Persvati attestation, hbox rebuild —
  `R2_PULL_PROMOTION_PLAN.md:126-129`) on two modules routed by nothing.
  Rejected by its own economics.
- **(b) Hold parked as-is.** Costs nothing today (proven above) but re-runs
  this assessment every wave, and P0.3 cannot start on top of a branch parked
  behind 153 commits.
- **(c) Adopt `r2-caps-rebase-trial` as the base of the R2 Phase-0 runtime
  branch now; merge rides the next forced reseal.** The plan already demands
  a dedicated runtime branch for Phase 0 (`:44-45`); the decoders are P0.4's
  first half and P0.3's prerequisite. Branch work forces no reseal — only
  the merge does — and the merge then rides whichever cycle comes first:
  the E3 flip cycle (one reseal total for capabilities + registry, the
  plan's Phase-2 cycle) or, if ember holds E3 long, an interim
  "resident, routed by nothing" reseal on the V3-residency precedent
  (`CURRENT_TRUTH.md:305` — the repo has already accepted sealed residency
  without promotion). Either way E4 stops being a standing decision and
  becomes ordinary branch content.

**Recommended: (c).** See §7.

## 4. The registry flip (E3): the full gate list

The flip is one function becoming true for exactly one spec:
`release_registered` (`source_ingest.rs:738-743`; the default arm returns
`false` unconditionally) sits in front of every source path — Endow refuses
`SourceReleaseUnavailable` before owner allocation or Token-2022 CPI
(`:285-286`; `CURRENT_TRUTH.md:300`), and init/genesis/open/append/seal all
gate on it (`:756,787,807,832,868`). Its gravity: **first value-admission
authority**; ends the era in which the default artifact is structurally
incapable of accepting value; forces a new ELF identity by construction;
narrows — never removes — the refusal boundary (the default campaign's
assertion changes from "Endow refuses 0x79 always" to "for every spec except
the exact registered release", `R2_PULL_PROMOTION_PLAN.md:118-124`).

### 4.1 The gates, consolidated

From the plan's §5 (`:135-151`) plus the profile's default-release STOPs
(`PYTH_PULL_PROFILE_R2.md:72-96`), deduplicated, with status:

| # | Gate | Status 2026-08-20 |
| --- | --- | --- |
| 1 | Every Phase-0 hostile suite green on the reshaped mock, both ELF profiles (default + `non-production-mock-source`) | not started except P0.4's first half (the E4 decoders, branch-resident) |
| 2 | Crossing-rule kernel falsifiers green **in-runtime**, not only in the research crate | research-only today |
| 3 | Phase-1 checklist complete with primary-source pins (§2.2's eight items incl. SDK-version resolution-or-STOP, trust-floor acceptance, cluster declaration) | blocked until ≥ Aug 26 by design |
| 4 | Official loader + Instructions-sysvar parsers replace the model projections (`PYTH_PULL_PROFILE_R2.md:78-80`) | half-landed: E4's two decoders are these, verified against published crate sources with real-serializer fixtures; the receiver-post ABI projection to `ImmediatePostV1` (checking the *adjacent instruction's data* names the pinned post, update, and config) is still open |
| 5 | Canonical Clock-sysvar `AccountInfo` decoder (exact key/owner/non-executable/canonical data; caller-supplied `ClockViewV1` never qualifies) (`:81-83`) | open (P0.4 second half) |
| 6 | Production SourceSpecV2 account codec, feed-domain registration, archive append adapter, compiled closed registry, reviewed together (`:84-86`) | open (P0.1/P0.2/P0.9) |
| 7 | Hostile real-bank tests: post/config/update substitution, set/post/restore, stale reuse, same-slot alternatives, ProgramData upgrade, Clock/cutover edges, missing/double witnesses, rollback/prefund (`:87-89`) | open (P0.7) |
| 8 | SBF stack/compute/account/rent/deployed-ELF evidence for create/append/seal + downstream resolution (`:90-91`) | open |
| 9 | Retention/recovery horizon measured, Terms trust floor stated, operational finality policy, provider/legal constraints accepted (`:92-93`) — the E5/C3 gate | open; legal half is §5 |
| 10 | V3 successor coordination — one seal cycle carries either R2 or V3 first (`:143-148`) | **GREEN**: `fb72b34` is an ancestor of main; R2 rebases onto V3's sealed base (finding 2) |
| 11 | Full reseal cycle budgeted for the flip itself (`:126-129`) | scheduling question (§3.2, §6) |
| 12 | **Ember's explicit go** — "not covered by standing swarm authorization" (`:149-151`; `GOAL.md:11-13`; `CURRENT_TRUTH.md:143-145`) | reserved; the decision this report exists for |
| — | Standing forever: the §4 double-witness falsifier — one demonstrated double-witness boundary reopens the provider selection | permanent, survives the go |

Every one of gates 1–11 is evidence a lane can assemble; gate 12 is the only
authority. The go should come **last**, against the assembled table, not as a
standing pre-authorization — the plan's own order ("any red stops the flip").

### 4.2 What the go is NOT

Not deployment (Track C authorizes devnet deployment separately and the
collector is faucet-starved — register F1); not promotion of any clearing
plane (D1/D2); not a market (a real Realm needs the A8 allowlist + a source
*together*, register A8); not mainnet, not real users, not filings language
(F6/G). The flip makes the *default artifact capable* of admitting value
against one exact provider release; every other refusal stands.

### 4.3 Devnet unavailable: what the local-validator ladder can and cannot substitute

The repo's local ladder is real and layered: litesvm/bank hostile campaigns →
`solana-test-validator` loopback with real signed transactions (the
22-transaction committed walk at `c05fe84`, refusing non-loopback operation —
`docs/implementation/COMMITTED_SBF_WALK.md`; the loopback bringup gates in
`SBF_BRINGUP.md:633,837`) → the in-flight Tier-2 signed validator walk
("the maximum devnet-free evidence class", GOAL.md:36-45).

**CAN substitute (and should be built into P0.7):**

- Hostile SVM campaigns against the reshaped mock, both ELF profiles —
  gates 1, 2, 7 in full. Devnet adds nothing to these.
- **Real post-cutover provider bytes without a funded key**: the receiver
  program, its ProgramData, and its Config are read-only fetches; cloned
  into a local validator/bank they give the Phase-1 pins *and* let the
  hostile campaign run against genuinely deployed bytes instead of only the
  mock reshape (the mock stays labeled non-production either way, plan §6).
  A real Hermes-signed post-cutover payload posted through the cloned
  receiver locally would exercise the actual 3-of-5 verification path
  end-to-end on loopback. This is strictly stronger than mock evidence and
  is available in the drought.
- SBF compute/stack/rent measurement for create/append/seal (gate 8) — bank
  measurement, same as every sealed row to date.
- The signed-loopback lifecycle join (blank-bank create/append/seal, plan §7
  route) — the walk precedent shows signed-validator evidence is attainable
  locally.

**CANNOT substitute:**

- Public-cluster inclusion, RPC-commitment, and finality *behavior over
  time* — the model deliberately has no `finalized` boolean and pushes
  commitment observation to operators (`PYTH_PULL_PROFILE_R2.md:58-62`);
  operator procedure can only be rehearsed against a real public cluster.
- **Provider liveness across real boundaries**: crossing cadence, migration
  gaps, the stall-then-lapse behavior under the provider's actual publishing
  rhythm. A cloned account is a snapshot; the falsifier watch (double
  witness) needs the living feed.
- Post-cutover *stability observation* itself (§2.2's criterion) — that is
  watching the real chain, though it needs reads, not SOL.
- Devnet deployment economics and the F1 identity choice; the "paces on a
  public cluster" evidence class that D1 option 3 names.

Consequence: the drought does **not** move E3's earliest technically-ready
date — gates 1–9 close locally — but it does mean the first *post-flip*
public evidence waits on the faucet. That is an argument for calm, not
delay-for-its-own-sake: the flip can be fully gated, executed, and resealed
locally, with public exercise following whenever Track C funding exists.

## 5. The Pyth ToS legal item (E5), scoped

Source: `SOURCE_PROVIDER_V1_SELECTION.md:179-187` (§6). Two facts route to
counsel; neither blocks §3-§4 engineering:

1. **ToS scope**: Pyth's terms prohibit bulk automated extraction and are
   silent on on-chain protocol usage. Questions for counsel: does an
   on-chain program authenticating receiver-posted updates constitute
   "usage" under the ToS at all; does the archive append pattern (one record
   per boundary, permanent) read as extraction; does *measuring the
   Benchmarks/Hermes retention horizon* (which §6 separately requires before
   any long-maturity market — and which C3's Variant A depends on) itself
   look like bulk extraction.
2. **The billed API key**: post-cutover historical payloads require a billed
   key whose secret cannot live in a static frontend. Consequences to scope:
   any client-side late-recovery feature is blocked pending a design that
   keeps the secret server-side or user-supplied; Terms must state the late
   recovery story honestly (who pays, whose key, what happens when nobody
   recovers — stall-then-lapse).

Deliverables: (a) a counsel memo on ToS applicability to on-chain usage and
to horizon measurement; (b) Terms language for late recovery and the trust
floor; (c) a go/no-go on any frontend referencing Hermes/Benchmarks.

Register options: run before E3 (conservative); run before any public
frontend; accept risk for devnet-only and defer. Timing reality: counsel
bandwidth is contested through Aug 27 (G1/G3). Since E3 cannot plausibly be
gate-complete before then anyway (§8), **"commission now, conclude before
E3, after the filing crunch" costs nothing** relative to the conservative
option. The devnet-only-defer option is livable for gates 1–8 but leaves
gate 9's "provider/legal constraints explicitly accepted" red — i.e., it
defers E3 itself, which is the opposite of what deferral is usually for.

## 6. Interactions

- **E1 → E2 → E3** is a strict chain; the only free scheduling is E1-early
  (recommended) and the E2 stability criterion (§2.2).
- **E4 ↔ E3 (reseal economics):** one cycle (capabilities + Phase 0 + flip
  compile, plan Phase 2) if ember's go comes reasonably after gate-green;
  two cycles (interim residency reseal, then flip) if E3 is held long. Shape
  (c) in §3.2 keeps both options open. Note the plan's cycle-exclusivity
  rule generalizes: whatever else wants a reseal (walk-evidence merges,
  F1-adjacent identity work) should coordinate one cycle at a time, second
  rebases onto the first's sealed base (`R2_PULL_PROMOTION_PLAN.md:143-148`).
- **E5 → E3** via gate 9; **E5 → C3** via the retention-horizon measurement
  (the R4 §8 Variant-A recommendation depends on R2 freezing a maximum
  admitted maturity — `TERMINAL_LIFECYCLE_RUNTIME_V1.md` §8 per register C3;
  the horizon measurement is the same work item as §5's question 1c).
- **E3 → C6:** retires `SOURCE.DEFAULT_REGISTRY_EMPTY`;
  `SOURCE.NO_TERMINAL_RELEASE` retires separately via E2/C3 (archive
  terminal release), not via the flip.
- **E3 → STOP 3/4:** the flip is the road to closing CURRENT_TRUTH STOP 3
  (authenticated source ingestion, `:391-397`) and shrinks the blank-bank
  injections four → one (`R2_PULL_PROMOTION_PLAN.md:130-133`).
- **G-cluster:** the Aug-24 filings freeze while the artifact is structurally
  value-refusing — factually stable, since E3 is impossible before Aug 26 by
  construction. But the register's warning stands the other way: once E3
  lands, "a value-admitting artifact changes the filings' factual posture"
  (register E3 interactions) — post-flip, any filing-adjacent statement
  quoting the value-refusing posture must be re-dated. One more reason the go
  is ember's and calendar-aware, not a lane's.
- **A8/F5:** a funded real market needs Realm admission (allowlist freeze +
  Token-2022 pin) *and* a source; E3 alone opens no market. Sequencing A8
  after E3 is fine; they meet only at the first real Realm.
- **F1:** if the faucet drought ends mid-cluster, the collector deploys the
  *sealed* identity — whichever seal is current. A post-E4-merge or post-E3
  seal changes what devnet would carry; F1's opt-3/opt-z choice should be
  made with that in mind, not discovered.

## 7. Recommendations per sub-decision, with counterarguments

- **E1 — ratify the model close now, before Aug 26.** It is research-only,
  authorizes nothing, and closes queue item 1 while the boundary semantics
  are fresh rather than under cutover pressure. The permanent double-witness
  falsifier survives ratification, so nothing is foreclosed.
  *Counterargument:* ratifying six codex-frozen choices in one act
  normalizes lane-resolved decisions ratified in bulk (the D3 concern). If
  that precedent bothers, ratify per-row against §4.1's table — the content
  outcome is identical and each row has a stated falsifier.
- **E2 — freeze post-cutover on an evidence trigger, not a date.** Adopt the
  two-condition criterion (§2.2: SDK discrepancy resolved + Config bytes
  stable over a named observation span), pre-stage the runbook with the
  cluster declaration and dossier template, and make the act itself
  review-and-sign. Explicitly include the trust-floor acceptance (3-of-5
  router) as a named line in the freeze record.
  *Counterargument (freeze promptly, option 1):* every unfrozen day delays
  the earliest E3 date and lengthens the window in which a DAO governance
  touch could force re-pinning anyway — waiting doesn't dodge that risk,
  only observes it. True; but Phase-0 gates won't be green in cutover week
  regardless, so prompt-freeze buys calendar time nothing downstream can
  use, while a hasty pin risks a burned generation (§2.2 item 2).
- **E4 — adopt shape (c):** make `r2-caps-rebase-trial` (01a004b, clean,
  42/42) the base of the R2 Phase-0 runtime branch now; the merge-to-main
  rides the next forced reseal (preferably E3's Phase-2 cycle; an interim
  residency reseal on the V3 precedent if E3 is held long). Do not spend a
  reseal on the decoders alone.
  *Counterargument (merge now):* an interim reseal would put the decoders
  under the manifest's protection immediately and end the parked-branch
  bookkeeping. But they are routed by nothing — the manifest would be
  attesting dead code at the price of a full cycle, and the trial rebase
  just demonstrated the parking cost is currently zero.
  *Counterargument (keep parked, don't even branch-adopt):* preserves
  perfect optionality. But P0.3 needs the decoders as its floor, so holding
  them parked blocks the very Phase-0 work §2.1 says to do now.
- **E3 — hold the go until the §4.1 table is green, then decide against the
  assembled dossier; do not pre-authorize.** The gravity argument: this is
  the first value-admission authority, every authorization statement
  reserves it, and its own design says any red stops the flip. Structure the
  eventual go as a dated record naming: the gate table at green, the freeze
  dossier it relies on, the E5 memo, and the reseal cycle that will carry
  it. The **hold** option (register option 2: default artifact remains
  structurally value-refusing) is a legitimate long-term posture for
  Track A/B — the flip is required for a *funded* lifecycle, not for the
  evidence program.
  *Counterargument (pre-authorize "go when green"):* it would let the swarm
  run gate-green → flip → reseal without a wait-state. But that converts the
  one reserved authority into a conditional standing authorization —
  precisely what GOAL.md:11-13 and CURRENT_TRUTH.md:143-145 exist to
  prevent. The wait-state is the point.
- **E5 — commission now, conclude after Aug 27 and before E3** (deliverables
  in §5). *Counterargument (devnet-only risk acceptance):* defensible for
  running gates 1–8, but it cannot green gate 9, so it defers E3 rather than
  unblocking it; and the retention-horizon measurement wants ToS cover
  regardless because C3 needs the number.

## 8. Execution costs

- **E1:** minutes. A dated ratification note retiring GOAL queue items 1–2
  into the register's vocabulary; no code, no reseal.
- **E2:** post-cutover, the checklist is roughly a lane-day of primary-source
  collection plus the observation span (calendar time, not work). Ember's
  own cost: one dossier review + sign. No reseal (pins live in
  `PROVENANCE.md`'s successor and the spec bytes; the registry compile is
  E3's).
- **E4 shape (c):** zero now beyond what this report already did (the trial
  branch exists at main's tip); the merge's true cost is amortized into
  whichever reseal cycle it rides.
- **Phase 0 (P0.1–P0.9):** the largest engineering block in the cluster —
  two novel capability suites remain (Clock, config digest), a kernel port,
  a trait generation, account-plane widening (9→12 / 8→10), both mock trios
  reshaped, the full hostile SVM campaign, and two design decisions (P0.8,
  P0.9). Multiple lane-days; all pre-Aug-26-startable; none of it needs
  devnet.
- **E3:** the go itself is a decision; its execution is the Phase-2 compile
  (small) plus **one full reseal cycle** — final-LTO/stack audit, liveness
  re-measurement, 100+-gate emission, manifest commit, post-commit check,
  fresh Persvati attestation, hbox rebuild ("Budget a full cycle; nothing
  may quote the old digests as current", `R2_PULL_PROMOTION_PLAN.md:126-129`)
  — plus the harness/docs flip surface (`expect_default_source_refusals` at
  `harness/src/main.rs:4986`, its call at `:7961`, both runner scripts, ten
  prose sites — the plan's Phase-2 enumeration).
- **E5:** counsel hours plus the horizon-measurement lane (an off-chain
  measurement with its own ToS question, hence sequenced behind the memo).
- **Not a cost of this cluster:** devnet SOL. Nothing above needs a funded
  key until the flipped, resealed artifact wants public exercise (F1).

---

*Report compiled 2026-08-20. Experiments run: the E4 trial rebase
(`r2-caps-rebase-trial` at `01a004b`, clean, 42/42 filtered tests green) —
no push, no merge, no other tree changes. Corrections belong in a dated
successor.*
