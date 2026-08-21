# R2 Phase-0 runbook — the pre-cutover checklist

Status: **RUNBOOK / PRE-CUTOVER PREPARATION.** This document authorizes
nothing, promotes nothing, and pins no identity bytes. It is the operational
form of the calendar in
[`REPORT_r2-cutover-and-registry-flip_2026-08-20.md`](../decisions/REPORT_r2-cutover-and-registry-flip_2026-08-20.md)
§2 ("Calendar analysis: before / at / after Aug 26") and the report's E4
recommendation, shape (c) (§3.2, §7). The default ELF's source registry stays
empty and Endow keeps refusing `SourceReleaseUnavailable` (`0x79`) throughout
everything below (`CURRENT_TRUTH.md:300`;
[`SOURCE_PROVIDER_V1_SELECTION.md`](../design/SOURCE_PROVIDER_V1_SELECTION.md)
§5).

Its one job: make **E2** (the production identity freeze) a short
review-and-sign act on an evidence trigger, in a filing-loaded week, instead
of a design session at 16:01 UTC on 2026-08-26 (report §2, §2.2).

Two documents remain authoritative and are cited, never copied:

* [`R2_PULL_PROMOTION_PLAN.md`](R2_PULL_PROMOTION_PLAN.md) — phases P0.1–P0.9
  (§2), the Phase-1 checklist (§3), Phase-2 (§4), the promotion gates (§5).
* the R2 report — the decision cluster, the calendar, the E4 trial-rebase
  result, and the consolidated **12-gate E3 table** (§4.1).

---

## 1. The Phase-0 branch

### 1.1 Where it is

`r2-caps-rebase-trial` is a branch of **this repository**
(`/Users/ember/dev/dragons-clutch`), not of `degg-research`. Tip
`01a004bea59a36de5556bc83805755fdc723ae6a`, parented on main
`e5b0503d6b55b649a449e6b994ec8135ac1f5aa4`. It is the trial rebase the R2
report ran and left in place (report finding 1, §3.1); it is *not* the
Phase-0 branch itself, it is the **base** the Phase-0 branch is seeded from
(report §3.2 shape (c), §7).

### 1.2 What it contains

Exactly one commit — "Add the two R2 pull-profile runtime capabilities" —
being the rebase of the parked `fable/r2-runtime-capabilities` (`f9045a0`)
onto main:

| file | lines | what |
| --- | --- | --- |
| `programs/clutch-sbf/program/src/loader_state.rs` | 739 | Upgradeable Loader **ProgramData** decoder |
| `programs/clutch-sbf/program/src/instructions_sysvar.rs` | 1,035 | **Instructions-sysvar** decoder |
| `programs/clutch-sbf/program/src/lib.rs` | +4 | two doc-table rows, two `pub mod` declarations |

No touch on `genesis.rs`, `seeds.rs`, or any instruction family — the
shared-edit churn the plan's §5 V3-coordination gate worried about never
reached these files (report §3.1).

Provenance and evidence, as recorded at `GOAL.md:784-801`:

* both decoders verified against **pinned published crate sources** —
  `loader-v3-interface` 8.0.1, `instructions-sysvar` 3.0.1, layout
  byte-identical across 2.2.2 / 3.0.1 / 4.0.0;
* fixtures captured from the **real serializers**, not hand-written tables;
* **42 adversarial tests** — truncation at every byte boundary, an
  off-by-one sweep on the current instruction, non-adjacent post
  unreachable — over **24 refusal variants**; clippy and fmt clean; exact
  `+42` lib-test delta;
* **wired into nothing.** No dispatch site calls either decoder.

Two findings the decoders exist to carry forward, both load-bearing for
Phase 1:

1. A **revoked** upgrade authority serializes to 13 bytes, but the loader's
   metadata region is fixed at 45 — bytes `[13..45)` still hold the
   *previous* authority. A naive decoder reports a live authority on an
   immutable program; this one never reads them and proves it. This is
   exactly the hazard class a naive post-cutover pin would inherit, since an
   in-place upgrade rewrites ProgramData (report §2.2 item 1).
2. The current-instruction index lives in a **2-byte trailer outside the
   documented layout**, so every body read is bounded by `len - 2`.

### 1.3 Rebase state — measured 2026-08-21, read-only

Re-measured against current main `a310df28e5957549df8f551f99402c8aeade759e`
(the report's numbers were taken at `e5b0503`):

* `git merge-tree --write-tree main 01a004b` → **clean, zero conflicts**
  (result tree `1bfbc176191ceec06da571b36d68f79ef20f74a9`, no conflict
  section). Read-only probe; no branch was created or moved.
* `git diff e5b0503..main -- programs/clutch-sbf/program/src/lib.rs` →
  **empty**. The one shared file has not drifted at all since the trial.
* Branch is **60 commits behind** main, 1 ahead. All 60 are documentation,
  decision reports, evidence pages, and canon; **none touches the sealed
  109-file SBF source closure** — the changed-file set of `d77d670..a310df2`
  and
  `research/liveness-policy-profile/artifacts/4fded7a67a2d8994/audit/source-files.txt`
  have an empty intersection.

Consequence, stated plainly: the report's "rebase cost today is zero"
(§3.1) still holds at `a310df2`, and current HEAD is **closure-neutral**
against the sealed runtime ancestry `d77d670` — so the sealed identity
`4fded7a6…` / 1,979,512 bytes remains the identity of HEAD's source, built
at the canonical path.

### 1.4 Seeding the Phase-0 branch

The plan requires a dedicated runtime branch, never sealed main
(`R2_PULL_PROMOTION_PLAN.md:41-45`). Seed it, do not re-derive it:

```sh
git branch r2-phase0 r2-caps-rebase-trial
git rebase main r2-phase0          # expected clean per §1.3
```

Then re-establish the branch's own floor before adding P0 content — the
narrowest thing that could refute the rebase, per house rule, and exactly
what the report ran (§3.1):

```sh
cargo test --manifest-path programs/clutch-sbf/Cargo.toml -p clutch-sbf \
  -- loader_state instructions_sysvar          # expect 42 passed, 0 failed
cargo clippy --manifest-path programs/clutch-sbf/Cargo.toml -p clutch-sbf \
  --all-targets -- -D warnings
```

Not run, deliberately: any unfiltered suite, any SBF build, any bank
campaign. The decoders are routed by nothing, so lib-target compilation plus
their own 42 tests is the complete refutation surface **for the rebase**;
the reseal cycle carries the rest (report §3.1).

Leave `r2-caps-rebase-trial` in place as the report's cited artifact.

### 1.5 The merge rides E3's reseal

**Branch work forces no reseal; only the merge does.** A closure-byte change
forks the sealed ELF identity by construction — the R1 record, the E4 commit
message, and `R2_PULL_PROMOTION_PLAN.md:126-129` all say so, and the audit's
build-path protocol
([`BUILD_PATH_IDENTITY_2026-08-20.md`](../reviews/BUILD_PATH_IDENTITY_2026-08-20.md))
fixes what a fork costs to re-establish.

Per report §3.2 shape (c) and §7, the merge-to-main rides **whichever forced
reseal comes first**:

* **preferred** — E3's Phase-2 cycle (`R2_PULL_PROMOTION_PLAN.md` §4): one
  reseal total for capabilities + Phase-0 content + the registry compile; or
* **fallback**, if ember holds E3 long — an interim "resident, routed by
  nothing" reseal on the V3-residency precedent (`CURRENT_TRUTH.md:305`, the
  repo has already accepted sealed residency without promotion).

Rejected: merging to main now and spending a full reseal cycle on two
modules routed by nothing (report §3.2 option (a), "rejected by its own
economics").

**Ride-along cargo for that same reseal wave: the rustdoc backlog.** The E4
lane surfaced two `private_intra_doc_links` warnings (`GOAL.md:798-801`).
That count is now stale — the cycle-E manifest run of `cargo_doc.clutch_sbf`
captured **13**, being 11 private-intra-doc links plus two unresolved links,
across 12 doc sites in 9 files (all paths relative to
`programs/clutch-sbf/program/src/`):

| site | link | closure line |
| --- | --- | --- |
| `lib.rs:70` | ``[`bpf`]`` (unresolved) | 69 |
| `instructions/direct_selection_v3.rs:4` | ``[`freeze_abort`]``, ``[`staged`]``, ``[`terminal`]`` | 46 |
| `instructions/observe_resolve.rs:52` | ``[`apply_recorded_redemption`]`` (private `fn`, `:2033`) | 56 |
| `instructions/orders_batch.rs:16,552,1536` | ``[`settlement`]``, ``[`entitlement::settle_portfolio_pair`]`` ×2 | 57 |
| `instructions/orders_batch/clear_walk.rs:21` | ``[`ReservationAccount`]`` (unresolved) | 58 |
| `instructions/orders_batch/clear_work.rs:18,67` | ``[`create_first_stage`]``, ``[`create_candidate_feed_account`]`` | 59 |
| `instructions/orders_batch/entitlement.rs:48` | ``[`settle_portfolio_pair`]`` | 60 |
| `instructions/orders_batch/selection.rs:66` | ``[`super::clear_walk::recompute_tie_digest`]`` | 63 |
| `instructions/split.rs:54` | ``[`require_internal_bound`]`` (private `fn`, `:738`) | 68 |

**Every one of the nine files is inside the sealed source closure**
(`…/artifacts/4fded7a67a2d8994/audit/source-files.txt`, the lines in the
right column), so a doc-comment byte is not free: the precedent is `9c371fe`,
whose rustdoc-link fix on the closure file `resolution.rs` forked the default
ELF from `a5725a3d…` to `bd20711b…` and forced a full reseal
(`GOAL.md:980-984`). They must therefore be repaired **inside** a reseal
wave, never as standalone housekeeping. Until they are, the
`cargo_doc.clutch_sbf` manifest gate cannot take the
`RUSTDOCFLAGS='-D warnings'` that `cargo_doc.claim_algebra_model`,
`cargo_doc.liquidity_policy_model`, and `cargo_doc.source_profile_v1` already
carry.

---

## 2. Phase-0 content — pointer, not a copy

P0.1 layout · P0.2 kernel port · P0.3 v2 authenticator trait · P0.4 remaining
capabilities · P0.5 account planes · P0.6 mock reshape · P0.7 hostile SVM
campaign · P0.8 error granularity · P0.9 registry mechanism — all specified
at `R2_PULL_PROMOTION_PLAN.md:41-92`, all buildable now because none pins an
identity byte, none needs devnet.

The only ordering fact this runbook adds: **P0.3 sits on the decoders.** The
v2 authenticator trait returns `LoaderStateV1 { linked_programdata,
deployment_slot }`, which is `loader_state.rs`'s output — so §1.4 is a
prerequisite of P0.3, and holding the branch parked blocks it (report §7,
E4 counterargument).

Devnet substitution for P0.7 is settled in the report's §4.3 and is not
re-litigated here: hostile campaigns on both ELF profiles, real post-cutover
provider bytes cloned into a local bank, SBF measurement, and the signed
loopback join all substitute; public-cluster inclusion behaviour, provider
liveness across real boundaries, and the stability observation itself do not.

---

## 3. The E2 evidence trigger

The register's E2 options are *freeze promptly* or *wait for stability
evidence*; the design states no stability criterion. The report recommends
**recording one** so E2's date is evidence-triggered rather than a calendar
promise (§2.2 "On 'when after'", §7).

Both conditions must be **written down before the cutover**. A span chosen
after seeing the data is not a criterion.

### 3.1 Condition A — the SDK version discrepancy observed resolved

The named STOP: the migration guide says **1.2.0**, the SDK manifest says
**2.0.0** (`R2_PULL_PROMOTION_PLAN.md:104-106`: *"if it has not [resolved],
record both and STOP"*). `PROVENANCE.md` pinned the reviewed
`pyth_solana_receiver_sdk/Cargo.toml` at 2.0.0 with the `pro-compatible`
feature (`research/source-profile-v1/PROVENANCE.md`, Pyth table last row,
raw-file SHA-256 `31cb23af…`).

This is **upstream's to resolve and ours only to observe resolved** (report
§2.2 item 3). Fill in, do not decide:

| field | value |
| --- | --- |
| migration-guide version, retrieval date, raw-file SHA-256 | |
| SDK manifest version, retrieval date, raw-file SHA-256 | |
| upstream revision at observation (successor to `ec456fc`) | |
| resolved? (yes → record the single version / no → **STOP**) | |

### 3.2 Condition B — receiver `Config` bytes stable over a named span

The `Config` full-body SHA-256 is the **governance-generation pin**: any
later governance change — fee, `valid_data_sources`, router address,
`minimum_signatures` — is a new feed generation by construction
(`SOURCE_PROVIDER_V1_SELECTION.md:59-73`). Freezing too fast is cheap to
regret: a post-freeze governance touch-up in a cutover's settling days
orphans the frozen generation. The failure is fail-closed, not unsound, but
a burned generation costs a re-pin and, if already compiled in, a reseal
(report §2.2 item 2).

Name the span **before** 2026-08-26 16:00 UTC:

| parameter | value to fix now |
| --- | --- |
| observation start (≥ cutover instant) | |
| observation end / minimum duration | |
| sampling cadence | |
| RPC commitment required per sample | |
| what counts as a break (any byte delta in the full body) | |
| where samples are logged (append-only, in-tree) | |

Condition B is green only when every sample in the declared span carries the
identical full-body digest. Reads only — no funded key is needed, so the
faucet drought does not block this (report finding 3).

### 3.3 The cluster — Phase-1 line 0

**A frozen SourceSpecV2 is cluster-specific by construction** (report
finding 4, §2.3). It binds the ProgramData deployment slot and the `Config`
full-body digest; the receiver program id is shared across clusters, but
each cluster's ProgramData was written at a different slot and each
cluster's `Config` is its own account. The Phase-1 checklist
(`R2_PULL_PROMOTION_PLAN.md:96-113`) never names which cluster it pins —
that is the gap.

So the checklist gains a **line 0**, and this runbook is where it is
answered:

> **Cluster pinned by this freeze: `______________`** (devnet /
> mainnet-beta), one dossier per cluster if both are ever wanted.

Given the authorized frame is Track C devnet/testnet only
(`CURRENT_TRUTH.md:136-148`), the natural V1 answer is a **devnet-state
pin**, with the explicit consequence that a later mainnet posture is a
**second freeze act and a second feed identity**, not a reuse. Whether the
DAO cutover lands on all clusters at the same instant is itself a Phase-1
item-1 *observation to record*, never an assumption.

### 3.4 The trust floor — a named line in the freeze record

Signing the freeze decides dossier §7 item 2
(`research/source-profile-v1/DOSSIER.md:218`; the report cites `:220`): the
protocol accepts a
**3-of-5 secp256k1 router quorum** plus the pinned config generation as the
Terms trust floor, replacing 13-of-19 wormhole guardians, with failure
consequence **stall-then-lapse, never substitution**
(`SOURCE_PROVIDER_V1_SELECTION.md:161-163`, `:175-177`). A materially
smaller signer set — accept it consciously, as its own line, not as a
checkbox (report finding 5, §2.2 item 7, §7).

---

## 4. Pre-staging the Phase-1 collection

Everything here is written **now** so the post-cutover collection is a
lane's afternoon, not a design session (report §2.1, last bullet).

### 4.1 Addresses are not yet recorded in-tree — record them

Neither the receiver program id nor the `Config` PDA appears anywhere in
`docs/`, `research/source-profile-v1/`, or `PYTH_PULL_PROFILE_R2.md`. The
primary source is already digest-pinned:
`target_chains/solana/sdk/js/pyth_solana_receiver/src/address.ts` at
`ec456fc`, raw-file SHA-256 `40b52e3f…`, which carries the default and
`pro-compatible` receiver / Wormhole / push program ids
(`research/source-profile-v1/PROVENANCE.md`).

Transcribing them into the dossier skeleton is **identity-byte-free and
cutover-independent**: pre-cutover ids are addresses, not pins. Do it now.

### 4.2 The collection commands

Read-only, no key, no submission. `<cluster>` is §3.3's answer; `<receiver>`
and `<config>` come from §4.1.

```sh
solana -u <cluster> program show <receiver>            # ProgramData key, authority, slot
solana -u <cluster> account <programdata> --output json --commitment finalized
solana -u <cluster> account <config>      --output json --commitment finalized
```

Decode the ProgramData body with `loader_state.rs` (§1.2) rather than by
eye — that is what it exists for, and its finding 1 is precisely why the
`[13..45)` bytes of a revoked-authority account must not be read as a live
authority. Digest the `Config` body **whole**:

```sh
python3 -c 'import base64,hashlib,json,sys; \
d=json.load(open(sys.argv[1]))["account"]["data"][0]; \
print(hashlib.sha256(base64.b64decode(d)).hexdigest())' config.json
```

### 4.3 The Phase-1 checklist, annotated

The plan's six items (`R2_PULL_PROMOTION_PLAN.md:96-113`), plus the two the
report adds (§2.2), plus line 0 (§3.3):

| # | item | annotation |
| --- | --- | --- |
| 0 | **Name the cluster** | report §2.3; this runbook §3.3 |
| 1 | Confirm the cutover executed; record receiver program bytes' identity, ProgramData key, decoded deployment slot | an in-place upgrade rewrites ProgramData — the pinned slot is the *cutover's* slot; decode with `loader_state.rs` |
| 2 | Pin the post-cutover `Config` full-body SHA-256 | the governance-generation pin; §3.2's span is what makes it safe to pin |
| 3 | Pin the SDK/source release | the named STOP; §3.1 |
| 4 | Set `activation_unix_timestamp` at or after the cutover instant | |
| 5 | Re-verify the 134-byte `PriceUpdateV2` layout and discriminator against the deployed program | expected unchanged: the cutover does not change the ABI (`SOURCE_PROVIDER_V1_SELECTION.md:167-172`) |
| 6 | Write the release dossier into `PROVENANCE.md`'s successor section | template at §4.4 |
| 7 | **Trust-floor acceptance** | §3.4 |
| 8 | **Cluster declaration** carried into the freeze record | §3.3 |

### 4.4 Release-dossier template

A successor section to `research/source-profile-v1/PROVENANCE.md`, in that
file's own idiom (review date, revision, per-file raw SHA-256, retrieval
dates, primary sources only, no public RPC beyond the read-only account
fetches §4.2 names).

```markdown
## Pyth — post-cutover release, <cluster>

Cluster: <devnet | mainnet-beta>.  Collection performed <date>.
Cutover observed executed at <slot> / <UTC instant>.

### Chain state

| pin | value | how observed |
| --- | --- | --- |
| receiver program id | | address.ts @ ec456fc (SHA-256 40b52e3f…) |
| receiver program account SHA-256 | | `solana account`, finalized |
| ProgramData key | | decoded, loader_state.rs |
| deployment slot | | decoded, loader_state.rs |
| upgrade authority disposition | | decoded; `[13..45)` NOT read |
| Config PDA key | | |
| Config full-body SHA-256 | | §4.2 digest command |
| Config stability span | | §3.2 table; every sample identical |
| provider feed id (32 bytes) | | |
| activation_unix_timestamp | | ≥ cutover instant |

### Upstream revision

Repository revision: pyth-network/pyth-crosschain@<rev> (commit time <ts>).

| Path | Fact used | SHA-256 of reviewed raw file |
| --- | --- | --- |
| … | … | … |

### SDK version resolution

<the §3.1 table, filled>

### Trust floor accepted

3-of-5 secp256k1 router quorum + pinned config generation; failure
consequence stall-then-lapse, never substitution.  Accepted by <ember>
on <date>.  (SOURCE_PROVIDER_V1_SELECTION.md:175-177)

### Standing falsifier

One demonstrated double-witness boundary — two distinct qualifying updates
for one T(k) — reopens the provider selection entirely.  Survives this
freeze.  (SOURCE_PROVIDER_V1_SELECTION.md:107-111, :197-199)
```

---

## 5. The E3 gate table — by reference

The consolidated **12-gate table** lives at report §4.1
([`REPORT_r2-cutover-and-registry-flip_2026-08-20.md`](../decisions/REPORT_r2-cutover-and-registry-flip_2026-08-20.md)),
deduplicated from `R2_PULL_PROMOTION_PLAN.md:135-151` and the profile's
default-release STOPs (`PYTH_PULL_PROFILE_R2.md:72-96`).

**It is not reproduced here, and must not be.** A second copy drifts, and
the flip's own rule is "any red stops the flip" — a stale duplicate is the
one way to get a false green. Read it there.

What this runbook is responsible for, against that table:

* gates **1, 2, 5, 6, 7, 8** are Phase-0 work (§2);
* gate **4** is half-landed — the two decoders in §1.2 *are* the official
  loader and Instructions-sysvar parsers replacing the model projections;
  the receiver-post ABI projection to `ImmediatePostV1` remains open;
* gate **3** is §3 and §4 of this document, and is blocked until ≥ Aug 26 by
  design;
* gate **9**'s legal half is E5 (report §5) — commission now, conclude
  before E3;
* gate **10** is already **GREEN** (`fb72b34` is an ancestor of main; R2
  rebases onto V3's sealed base — report finding 2);
* gate **11** is the reseal scheduling question §1.5 answers;
* gate **12** is **ember's, reserved, and last** — against the assembled
  table, never as a standing pre-authorization (`GOAL.md:11-13`;
  `CURRENT_TRUTH.md:143-145`; `R2_PULL_PROMOTION_PLAN.md:149-151`).

---

## 6. What must not happen before 2026-08-26

Forbidden by the design in as many words
(`SOURCE_PROVIDER_V1_SELECTION.md:173-174`; `GOAL.md:54-56`):

* **no identity-byte pin** of any kind;
* **no registry entry**;
* **no interim entry "to get ahead"** — the model does not authorize an
  interim registry entry or value admission.

And not by this runbook at any time: no weakening of any refusal to make a
campaign pass, and no claim that a green mock-reshaped campaign is
production-provider evidence — the mock stays labeled non-production even in
v2 shape (`R2_PULL_PROMOTION_PLAN.md` §6).

---

## 7. The pre-cutover tick list

Everything below is doable today; none of it pins a byte.

- [ ] **E1 ratified** — the six model-close rows, per-row against report
      §4.1's model table if bulk ratification is unwelcome (report §7).
- [ ] Phase-0 branch seeded from `r2-caps-rebase-trial` and re-based on
      current main; 42/42 filtered tests green (§1.4).
- [ ] P0.1–P0.9 started, in the order §2's P0.3 note implies.
- [ ] **Cluster named** in §3.3.
- [ ] **Config-stability span named** in §3.2, in writing, before 16:00 UTC.
- [ ] **SDK-discrepancy watch** set up per §3.1, with the STOP branch
      understood.
- [ ] Receiver program id and `Config` PDA transcribed from the pinned
      `address.ts` into the dossier skeleton (§4.1).
- [ ] Collection commands rehearsed against **pre**-cutover state — a dry
      run that pins nothing and proves the pipeline (§4.2).
- [ ] Dossier template instantiated as an empty successor section (§4.4).
- [ ] Trust-floor line drafted for ember's signature (§3.4).
- [ ] **E5 ask drafted** to counsel (report §5); conclusion expected after
      Aug 27 and before E3.
- [ ] Reseal wave for the E4 merge identified, with the 13-warning rustdoc
      backlog (§1.5) attached to it as ride-along cargo.

On the day: record item-1 facts, start the Config-stability watch. **No
freeze act same-day is required or recommended** (report §2.4).

---

*Written 2026-08-21 against report `REPORT_r2-cutover-and-registry-flip_2026-08-20.md`
and plan `R2_PULL_PROMOTION_PLAN.md`. The branch measurements in §1.3 were
taken read-only at main `a310df2`; no branch was created, moved, or merged.
Corrections belong in a dated successor.*
