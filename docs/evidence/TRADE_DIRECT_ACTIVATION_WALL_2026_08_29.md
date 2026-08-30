# The Direct capability-activation wall, measured on the live flagship

2026-08-29, TRADE lane. This document records why the first Direct trade on
the public devnet flagship `7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC`
is refused by the deployed protocol itself, with every refusal point read in
the sealed artifacts and the cohort-6 program source. Nothing here is a
client defect: the trade exterior, producers, and route manifest emitters all
work up to this wall.

## The chain of demands, each verified at its owner

1. **A trade needs the capability root.** The Trading Hot leg reads the root
   account's `CapabilityRootHeaderV1` and its exact 24-byte
   `DirectRootStateV1` tail before executing anything
   (`programs/dclutch-trading-sbf/src/hot_v3.rs` root prestate reads), and the
   operator's route authentication requires the root account Trading-owned at
   planning (`crates/dclutch-operator/src/direct_inline_route_v3.rs`,
   `authenticate_named_route_v3`). The root
   `Ht5td43wzV6wMmj3efghMeuWMzNscmnvhJr44gK3XQvu` does not exist on devnet
   (AccountNotFound at finalized commitment, 2026-08-29 ~20:20 EDT).

2. **Only activation creates the root.** Core's capability route
   (`programs/dclutch-core-sbf/src/capability.rs`,
   `CoreEffectActionV1::ActivateCapability`) CPIs Trading's outer
   (`programs/dclutch-trading-sbf/src/outer.rs`, `process_activation`), which
   writes `CapabilityRootHeaderV1 || <family tail>`. Nothing at founding
   creates it; no other instruction can.

3. **Activation selects a descriptor out of the family's ProgramSet and
   demands two facts of it:**
   - the selected descriptor's schema must be
     `CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1` (`outer.rs:948`,
     `TradingSbfError::UnsupportedContent`);
   - the paired effect must project exactly the root tail:
     `effect.request_bytes() == descriptor.root_state_bytes()`
     (`outer.rs:1442`, `TradingSbfError::Root`). For Direct that is
     `DIRECT_ROOT_STATE_BYTES_V1 = 24`.

4. **The sealed flagship ProgramSet has no such entry.** Decoded from the
   sealed `market17.json` (the exact bytes the founding published as
   `direct_program_set_record`, content `2e18e9ef…`): 248 bytes = header + 3
   entries:

   | selector | descriptor schema | descriptor `root_state_bytes` | effect `request_bytes` | activation verdict |
   |---|---|---|---|---|
   | `InlineOrdinary` | CapabilityProgramV4 | — | — | refused at `outer.rs:948` (not V1) |
   | `DIRECT_BEGIN_RETIRING_SELECTOR_V1` | V1 (`DCLTCPR1`) | 24 | **0** | refused at `outer.rs:1442` |
   | `DIRECT_NATIVE_CLOSE_SELECTOR_V1` | V1 (`DCLTCPR1`) | 24 | **0** | refused at `outer.rs:1442` |

   (Descriptor `root_state_bytes` read at `CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET
   = 272`; effect `request_bytes` at effect-kernel v2 offset 14. An earlier
   sweep misread the descriptors as declaring zero — they declare 24; it is
   the sealed *effects* that project nothing.)

5. **The gap is sealed into the market identity.** The manifest entry 0
   (kind `DIRECT_SUCCESSOR_KIND_ID_V3`, activation policy PrepaidLazy,
   zero dependencies) binds `release_id = sha256(program_set bytes)`, and the
   manifest digest is one of the Market PDA's own identity seeds. No record
   republication can add a fourth entry to THIS market.

6. **The deadline is real and enforced twice.** Entry 0's
   `activation_deadline_slot = 490330281` (about 16:30 EDT 2026-08-30) is
   checked at `capability.rs` (`current_slot > deadline` refuses) and inside
   `FundingLedgerV2::activate_in_place`
   (`Error::ActivationDeadlineElapsed`). When it lapses, the Trading funding
   ledger `2zQPU4Lq…` (Pending, 4,398,720 lamports = exact rent + the
   2,672,640-lamport root quote) can never activate: `7Mcu1ZT9` remains Open,
   admittable, and permanently untradeable.

This is the deferred work the repository already names:
`docs/OMISSION_INDEX.md` ("Still required for a live release: an eighth
`CapabilityProgramSetV2` entry naming the activation descriptor"),
`tools/gauntlet/blocked.json`, and the web route census
(`UNSELECTED_ENTRY_ROUTES`). No capability of ANY family has ever been
activated on any cluster; the Core/Trading activation route is itself
first-execution territory.

## What unblocks the first trade, exactly

1. **Author the Direct activation artifact triple** in
   `crates/dclutch-direct-codec` (a sibling of `begin_retiring_bundle_v1` /
   `native_close_bundle_v1`): a V1 `CapabilityProgramV1` descriptor with
   `root_state_bytes = 24`, an `AccountProfileV1`, and an effect-kernel-v2
   `EffectProgramV2` whose projected request buffer is the exact initial
   `DirectRootStateV1` (`DCLTDRT1`, version 1, phase Open, count 0 — 24
   bytes). This is protocol-artifact authoring and deserves adversarial
   review; a wrong effect activates a wrong root permanently.
2. **Add the fourth ProgramSet entry** naming it
   (`crates/dclutch-direct-codec/src/program_set_v4.rs`,
   `ordinary_lifecycle_entries`), at a non-colliding selector.
3. **Found a successor flagship** from the regenerated market input (the
   cohort-6 PROGRAMS are data-driven and need no redeploy; the founding,
   admission, and trade drivers are all proven or landed).
4. **Drive the activation** with the frame mapped below, then the trade.

## The complete activation frame (mapped for the driver)

Single Core instruction, permissionless (fee payer only; the Core caller
authority is a PDA Core itself signs via CPI — it must NOT be a top-level
signer). Instruction data:
`Request::administrative(ActivateCapability, generation, market)` (72 B) ‖
`CoreEffectEnvelopeV1` (280 B; binds sha256 of the live 360-byte Market
account and sha256 of the role request) ‖ role request
(`CapabilityExecutionSelectionV1` 144 B with record bumps ZERO ‖
`CapabilityFundingHeaderV2` 16 B — for the flagship shape:
physical=1, logical=1, mask=0b0001 ‖ family request ≥16 B with the u32 set
selector at offset 12).

35 accounts for the Direct P=1 shape (Market W, realm record pair, manifest
record pair, the Trading funding ledger W, the root W, activation cache,
core/trading/resolution program+programdata, registry program, rent sysvar,
caller-authority PDA, then the child tail: program-set pair, config pair,
activation profile pair, activation effect pair, seven exact aliases of the
prefix accounts, system program, activation descriptor pair). Exceeds the
1,232-byte packet uncompressed — needs one ALT. Root PDA seeds:
`dclutch:capability-root:v1`, market, generation LE, manifest, entry index
LE, kind, capability release, config — under Trading. Funding ledger PDA:
`dclutch/cap-funding-ledger/v2`, controller program, market, generation LE,
manifest, mask LE — under the controller. The closest working template is
the Core-level CloseCapability builder in
`programs/dclutch-core-sbf/tests/capability_close_alias_program_test.rs`.

## What the TRADE lane landed against this wall

- `devnet-checked-execution-release-v1` and
  `devnet-direct-hot-route-manifest-v3` (commit `1fe37fa4`): the named
  manifest-producer gap is closed at the emit side, gated on a session's
  frozen lookup table with bracketed refusal codes; the first live-set
  checked multiprogram was produced from the chain's activation cache
  (execution set `d5aaadea…`, checked id `bdf54c41…`).
- Four first-execution walls in `devnet-direct-trade-produce-v1` fixed at
  their owners (`8dfd2a41`, `1bbb8cdf`): transaction-row identity,
  checkpoint-scalar capability root, devnet market-record resolution,
  Open-Market generation.
- Bootstrap tickets, session runner, SDK verification script, and the
  devnet load-simulator configuration staged; all of it runs the moment the
  activation entry exists on a successor market.

## Walls #26 to #28, and the two corrections that were worth more than the walls

Three walls stood between a fixed activation and a public Direct trade. All
three are recorded here because the second and third were found only by
executing the route, and the corrections below were found only by distrusting
the measurements that closed them.

**Wall #26 — the route refused itself.** `TradingSbfError::Release` was raised
one statement before commit, at the end of an otherwise successful execution,
because the Direct crosscheck unwrapped a child-programs value that only a
Registry continuation ever populated. Every public caller — SDK, CLI, web
panel, devnet driver — sends a bare Hot instruction straight to Trading, so the
public route refused 100% of the time on a deployed program while two devnet
simulations burned ~880,000 CU each proving everything else about the trade was
correct. Fixed in `6c84b33a`, red/green at the exact refusal code, and 4,466 CU
cheaper than the code it replaced.

**Wall #27 — the route then died on the heap.** With #26 fixed the path ran
230,000 CU further than it ever had and hit `memory allocation failed, out of
memory` in a finalization it had never reached. The cause is the route, not the
tail W2p closed: invoking Trading directly means two Registry reauthentication
CPIs a continuation never makes, held against a bump allocator that never
frees. Closed in `8ee544e4` by putting Hot on
`declares_extended_heap_profile_v1` and granting 65,536 bytes, with
`TradingSbfError::HeapFrame` (0x4008) refusing BY NAME — checked before the two
CPIs — when a caller omits the grant. An out-of-memory abort names nothing:
not the route, not the budget, not the instruction the caller left out.

**Wall #28 — the compute margin — was ruled ACCEPT on a number that was wrong
twice before it was right.**

### Correction one: a single-seed bisect that would have shipped a wrong culprit

The continuation route was found exceeding the 1,400,000 ceiling. A bisect
across the intervening commits identified `df404c56` as the cause: at its
parent the route consumed 1,383,710 CU and at the commit it exceeded the
ceiling. That commit describes itself as "the record type only, no route and no
program change yet" and is entirely truthful — it changed two SHARED contract
crates, and Core's own consumption grew 149,945 to 157,465, a measured +7,520
CU, because a private constant-foldable `role_index` became a cross-crate
public method in the hot authentication path.

The bisect was run at seed 0 alone, and the control refuted it. At the parent
commit the continuation ALREADY failed 4 of 12 seeds. `df404c56` shifted a
boundary and flipped one seed; it did not create the problem. The route had
been chronically over the ceiling for 30+ commits and nobody had noticed
because nobody runs it. A single-seed bisect would have shipped a confident,
specific, wrong culprit — and the +7,520 CU would have made it look proven.

### Correction two: the gate that refuted the ruling it was built to enforce

Wall #28 was accepted on twelve seeds, worst draw 1,373,917 CU. Building the
regression gate meant widening the sweep to thirty-two, which put the worst
draw at 1,381,576 — seed 15, a legitimate key draw. The gate as first written
would have been born red on a regression that never happened.

Ledger M-61 exists for exactly this and it was violated twice in one day by the
same lane, in both directions: once by trusting one seed, once by trusting
twelve. The rule that survives both is narrower than "sweep seeds": **a sample
that has not stopped moving is not a bound.**

### What the honest numbers are

Measured at `fd8cad39`, all five ELFs built from that commit, 32 pinned seeds:

| Route | Result | Range | Mean | Worst margin |
|---|---|---|---|---|
| Public top-level Direct | 32/32 pass | 1,341,077 – 1,381,576 | 1,360,206 | 18,424 |
| Registry continuation | 3/12 OVER ceiling | — | — | negative |

Two consequences, both the opposite of what was assumed going in:

1. **The public top-level route is CHEAPER than the continuation**, by a
   consistent ~32,900 CU per seed on the same ELFs at the same seeds. The two
   reauthentication CPIs cost less than the Registry outer invocation a
   continuation pays instead. Wall #26's option B was "route the public path
   through the continuation"; taking it would have put the public trade on the
   costlier route that is also the one over the ceiling.
2. **The cross-seed band is 40,499 CU against a worst margin of 18,424** — the
   band is more than twice the margin. By `tools/gauntlet/CU_BUDGETS.json`'s own
   tolerance formula, `roundup(40499, 10000) + 10000` is a 60,000 CU tolerance
   and `1,381,576 + 60,000` is past the ceiling. That file states a budget above
   the ceiling is how it "says out loud that a transaction has stopped fitting."
   By this project's own standard, Direct Hot has stopped fitting for arbitrary
   keys. Structural CU reduction is the real remedy and is tracked separately;
   it is not what the first trade waits on.

### How the first trade handles the band: selected keys, said out loud

The band is a property of the maker keys, and for the first trade those keys
are ours — disposable participants we generate. So they are SELECTED rather
than rolled: candidate key sets are measured before submission and one landing
in the cheap half of the band is used.

This is recorded here because the recording is what makes it honest. An
unlabelled lucky draw published as a first trade would be the dishonest
version; a chosen draw, labelled as chosen with its measured cost, is
engineering. What it demonstrates is exactly this much and no more: that the
route executes end to end on real programs with real collateral. It is NOT
evidence that the route fits for arbitrary keys, and this document says so in
the section above rather than leaving the reader to infer it.

The stopping rule is part of the protocol, not a caveat on it: if key selection
cannot reach a real margin, the trade does not go out and the number gets
posted instead.

The gate landed in `883a077b` as a deterministic 32-pinned-seed tripwire at
1,390,000 CU. It catches code-cost erosion — which is the risk `df404c56`
demonstrated is real and invisible — and it explicitly does not bound what a
real maker's keys cost. Nothing currently executes it automatically:
`.github/workflows/checks.yml` builds no SBF, and no pre-push hook is installed.
