# The cohort lineage: which release set each cut left behind

Status: **complete.** All eight release sets devnet has ever activated are
recorded below with full 32-byte ids, their activation caches, their order, and
which one is current. Sections 1–5 are MIGRATE's original record; §§6–10 are the
LINEAGE-DEVNET lane's devnet recovery, 2026-08-31, which **closes §2's two
missing rows and corrects what they were looking for**.

Written by the MIGRATE lane, 2026-08-31, as the standing record behind
`docs/ledger/COHORT9_PLAN_REVIEW_2026_08_31.md` cut gate 6 (*"the cohort-8→9 predecessor
mapping recorded durably"*) and as the input list for the retroactive lineage
declarations of `RELEASE_LINEAGE_MIGRATION_V1.md` §4.5.

Every row carries its source with `file:line`. A row without one is not a row.

> **Read §6 before §2's table.** The two ids §2 recorded as `UNRECOVERED` —
> `d202e1f4…` and `97d49888…` — are **not release-set ids at all**. They are the
> cohorts' checked-upgrade *plan* digests. The real release-set ids were on
> chain the whole time and are in §6.

---

## 1. Why this file exists

A market's `selected_release_set` is seed component 6 of 9 in its own address
(`crates/dclutch-market-core-codec/src/physical.rs`, `as_slices`). It is the
market's *name*, so it is never rewritten, and every cut therefore leaves the
markets founded before it pinned to a set the world has moved off.

`ReleaseLineageV1` is the record that makes that followable: keyed by the
**predecessor**, naming the successor, so a reader holding nothing but a
market's founding pin can derive the address and read where the world went.
The codec, the `DeclareSuccessor` route and the forward walk all exist at HEAD
(`crates/dclutch-registry-contract/src/lineage.rs`, `.../lineage_walk.rs`,
`programs/dclutch-registry-sbf/src/lineage_v1.rs`,
`packages/dclutch-sdk/lib/releaseLineage.ts`).

**What does not exist is the declarations themselves**, and they cannot be
authored without the full 32-byte id of each endpoint. That is what this file
is: the input list, and an honest account of which entries are still missing.

---

## 2. What is recorded

| cohort | release-set id | activation cache | source |
|---|---|---|---|
| DEPLOY-1 | `e68f73651f97993110262bf5177029d7c31387b4cbcd67f4d96115db398a063b` | `Hz6BXyxyf66teABb6Pr6ev9jCZBJJpP5Q9p4sYJwJSkj` | `docs/evidence/DEPLOY_1.md:83-84` |
| cohort-5 | `094336271db1146f09f6ff419488af2d3174da762d3b2b468fac635754aa862d` | `77PrN82TY4rrQwUjyKBM14A1n3qxktHrN8vd2RcacovK` | `packages/dclutch-sdk/lib/releaseIdentity.ts:157-158` |
| cohort-6 | `d5aaadea2435978604d93c0e48af0e44547ec54b69681585f47f185ef530a2fa` | `69d1MKP4PaPVDFankLfnzeHBugoVBjPCDm7PEHParRF6` | `apps/dclutch-web/lib/deployments.ts:106,109` |
| **cohort-7** | `91dcbefd3f8d81b27236aeae535baffcb002210cffad680ba06feb7d7e2f90ae` | `GRDN2mbVNjshw3eonn85kJAddv67xKRWCjr18Hwn9kgu` | §6, recovered on devnet — **not** `d202e1f4…`, see §7 |
| **cohort-8** | `559f26e64683bd986d56575518daec2b65bfb66e20845d8615b13ead268378b4` | `DGqeJ2RU6vuQmhJSMz13bGyo9PGTNuaFN1nzb7QAyETb` | §6, recovered on devnet — **not** `97d49888…`, see §7 |

The last two rows were filled in from devnet by the LINEAGE-DEVNET lane; their
proofs are §6.1 and their cache addresses §6. The paragraph that follows is
MIGRATE's original note on the two truncations, kept because its search was
sound and its conclusion is still true of *those strings* — §7 explains why
finding them would not have helped:

> The two truncations were greped tree-wide across every `.md`, `.ts`, `.json`,
> `.rs`, `.sh` and `.py` file outside `node_modules`, `target` and the lane
> worktrees. `d202e1f4` has two hits, both in `docs/ledger/SESSION_STATE_2026-08-31.md`. `97d49888` has
> one, in `GOAL.md`. There is no deployment manifest, no cut JSON, no evidence
> file and no test fixture carrying either full id or either cache address.

**Cohort-6's row is also stale as a "current" hint**, and that is a separate
fact worth stating here so it is not rediscovered: `deployments.ts`'s
`activationCache` was last updated at `9955b80f` (*"manifest: the hint follows
cohort-6"*, 2026-08-29). Cohort-7 and cohort-8 both landed afterward and the
hint was never regenerated. Readers survive only because
`openReleaseBoundSessionV1` treats it as a hint and rediscovers the live cache.
It is recorded above as **cohort-6's set**, which is what it truthfully is —
not as the current one.

---

## 3. How the two missing ids are recovered

Neither is a judgement call; both are mechanical reads against public devnet.
The lane that runs them should land the results in the table above, in the same
commit as the declarations they enable.

**Route A — out of the markets themselves, and self-authenticating.** Every
market account carries its own `selected_release_set`, and because that field is
a PDA seed, an id read out of a market's bytes is proved by the market's own
address: substitute one byte and the account is at a different address. The two
cohort-8 markets are already named in the site's editorial registry
(`apps/dclutch-web/fixtures/market-registry.devnet.json`):

- market21 `5w24EmP7Q2Kkw9y9tjMPdixLPMdJHA1xsY7Wip3k5SDm`
- market22 `8Xky2yx3wBmDRXeNfKSuJigqiWDtwSvGvB75BSW6tPxK` — the first-ever-traded
  market, whose founding and trade signatures are pinned in
  `apps/dclutch-web/fixtures/public-cut.devnet.json`

`decodeMarketCoreStateV2` already returns the field as
`identity.selectedReleaseSetId`, and the site already renders it
(`apps/dclutch-web/components/MarketDetailWorkspace.tsx:351`). One
`getAccountInfo` per market yields cohort-8's id with its proof attached.

Cohort-7's id comes the same way from any cohort-7 market. `docs/ledger/SESSION_STATE_2026-08-31.md:663-679`
names market19 and records that the three older markets are 360-byte accounts
the current Core refuses on length — **a length refusal is not a read failure**:
the bytes are still there and `selected_release_set` is still at its offset.

**Route B — out of the Registry's own caches.** Superseded activation caches are
never deleted. Every 1288-byte Registry-owned account is one release set's cache
and carries its `execution_release_set_id()` at offset 16, which is exactly what
`discoverCurrentActivationCacheV1` already enumerates
(`packages/dclutch-sdk/lib/releaseIdentity.ts`). Route B additionally yields the
cache addresses that Route A does not, and the five per-role deployment slots
that `DeclareSuccessor` conjunct 5 compares.

Route B is the one the declarations actually need, because `DeclareSuccessor`
reads both endpoints out of activation-cache **accounts** and nothing off the
wire (§4.5). Route A is the cheaper cross-check, and the two must agree.

**A truncation cannot substitute for either.** The lineage PDA seeds on all 32
bytes, and both endpoints are derived from accounts rather than supplied, so an
eight-character prefix authors nothing and authenticates nothing. This is not a
formatting inconvenience; it is why cohort-7 and cohort-8 are blocked rather
than merely untidy.

---

## 4. The declarations this enables, and who may author them

The chain to author, once §3 is done, is one `DeclareSuccessor` per hop:

```
DEPLOY-1 → cohort-5 → cohort-6 → cohort-7 → cohort-8 → cohort-9
```

Each hop needs a signature from the upgrade authority of **every role whose
artifact moved across that hop**, read out of the successor's activation cache
(§4.3). A role whose binding is byte-identical on both sides needs no signature
and is asked for no consent — its slot must hold `system_program::ID` and must
not be a signer.

Hops whose endpoints are both already superseded are still declarable, and this
is the point the cut turns on. **Nothing in the record or the route reads a
clock.** §4.2 omitted `declared_at_slot` because no conjunct would read it, and
the consequence is that a hop declared today for two cohorts that superseded
each other weeks ago encodes to exactly the 248 bytes it would have encoded to
at the time. There is no stamp to backdate and no contemporaneity to
counterfeit. Both suites assert that equality directly
(`crates/dclutch-registry-contract/src/tests.rs`,
`a_hop_authored_long_after_the_fact_is_byte_identical_to_a_timely_one`;
`packages/dclutch-sdk/lib/releaseLineage.test.ts`, *"walks a retroactively
authored history exactly like a timely one"*).

Conjunct 5 — strictly advancing deployment slots — is satisfied by history for
every retroactive hop, since the slots being compared are the two endpoints' own
and never the current one.

**What retroactive authoring does not buy.** It does not migrate any market. A
market's `selected_release_set` is its name and stays put; re-pointing needs the
`active_release_set` field and `Core::MigrateMarket`, which are commits 4–6 and
are not in the tree. What the declarations buy is that a market's history
becomes **followable** — a reader in the cohort-9 world can walk from market22's
founding pin to the current set and show the path — which is a strictly smaller
claim than migration and the one available before the wire break.

---

## 5. What a reader can do the moment the declarations land

- `walk_lineage_to_head(origin, lookup)` — where the world went, for a reader
  holding only a founding pin.
- `walk_lineage_to(origin, destination, lookup)` — whether a market's history
  reaches the new world, and when it does not,
  `SuccessorUndeclared { at }` names the set that still owes a declaration.
- `followReleaseLineageV1(client, { registryProgram, origin })` — the same walk
  against a live cluster, returning the ordered `path` of sets traversed.

Until the declarations exist, every one of those calls correctly reports that
market22's chain stops at its founding set. **That is not a defect in the walk;
it is the walk reporting the state of the world accurately**, and it is what
this file exists to get changed.

---

## 6. The complete set, recovered from devnet

Recovered by the LINEAGE-DEVNET lane on public devnet
(`https://api.devnet.solana.com`) at finalized slot **491018122**, 2026-08-31.
Read-only: no transaction was built, signed or sent to produce this section.

§3's two routes were run and **they agree**, which is the point of running both.

Every 1288-byte account the Registry `Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj`
owns, ordered by the newest deployment slot the cache pins. There are **eight**,
not five:

| # | release-set id | activation cache | newest pinned slot | cohort |
|---|---|---|---|---|
| 1 | `e68f73651f97993110262bf5177029d7c31387b4cbcd67f4d96115db398a063b` | `Hz6BXyxyf66teABb6Pr6ev9jCZBJJpP5Q9p4sYJwJSkj` | 489100942 | DEPLOY-1 |
| 2 | `ff40e4fc657c0a7f5acad0cbef614897a68696fbb54e2aa43958cf48dbb3ce9b` | `DN4mTs5t5EiY7gPQ4z7kNM8RY6U4gNUiPdvoxfChngSk` | 489783964 | unlabelled |
| 3 | `730073d6534d0ad9fe0d412f17c9375451d2087b03bfe26939c306512a28bf17` | `AXPzXtfxs3QY6Pq5r1kvMcLGQU5wu8uwXFy7iqtzjLfv` | 489859176 | unlabelled |
| 4 | `63e8082c7b033f834c22aa61887105b4b8b68cb2abaf0a05d166445e1ef43c84` | `CzctbcqVKsG6vTBLjzfU9AEHXiWgemupwU4yjAGyYw1E` | 489910393 | unlabelled |
| 5 | `094336271db1146f09f6ff419488af2d3174da762d3b2b468fac635754aa862d` | `77PrN82TY4rrQwUjyKBM14A1n3qxktHrN8vd2RcacovK` | 489926024 | cohort-5 |
| 6 | `d5aaadea2435978604d93c0e48af0e44547ec54b69681585f47f185ef530a2fa` | `69d1MKP4PaPVDFankLfnzeHBugoVBjPCDm7PEHParRF6` | 490106442 | cohort-6 |
| 7 | `91dcbefd3f8d81b27236aeae535baffcb002210cffad680ba06feb7d7e2f90ae` | `GRDN2mbVNjshw3eonn85kJAddv67xKRWCjr18Hwn9kgu` | 490697521 | **cohort-7** |
| 8 | `559f26e64683bd986d56575518daec2b65bfb66e20845d8615b13ead268378b4` | `DGqeJ2RU6vuQmhJSMz13bGyo9PGTNuaFN1nzb7QAyETb` | 490849793 | **cohort-8 — CURRENT** |

Rows 2–4 are real activations that no document in this repository names. They are
recorded here with their ids rather than guessed at with cohort numbers, because
an unlabelled row with a true id is a row and a labelled row with a guessed one
is not. Their slots place them between DEPLOY-1 and cohort-5.

**Which one is current is measured, not asserted.** Row 8 is the only cache whose
five pinned deployment slots all equal the five live ProgramData slots read in
the same pass — core 490849793, claims 490826560, trading 490830840, resolution
490693331, custody 490814947. Every other cache is superseded on at least one
role, which is exactly the state `openReleaseBoundSessionV1` refuses on.

Note that cohort-8's resolution slot (490693331) is **cohort-7's** resolution
slot: that role did not move across the 7→8 hop. This is docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:288's
"resolution AlreadyCurrent", visible in the bytes.

### 6.1 The address-derivation proof

Route A was run against all six markets in the editorial registry
(`apps/dclutch-web/fixtures/market-registry.devnet.json`). For each, the nine
ordered seeds were read out of the account's own bytes — domain
`dclutch/market-core/state/v2`, then realm, product record, product id,
resolution policy, capability manifest, **selected release set**, registry
program, generation, contiguous from offset 48
(`packages/dclutch-sdk/lib/generated/coreFound.ts:15,41-48`) — and re-derived
under Core `HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N`.

**All six reproduced the address they were found at.** A market's
`selected_release_set` is seed component 6 of 9, so this is the proof §3
describes: substitute one byte of the recovered id and the derivation lands
somewhere else.

| market | bytes | selected release set | cohort |
|---|---|---|---|
| `8Xky2yx3wBmDRXeNfKSuJigqiWDtwSvGvB75BSW6tPxK` — market22, first ever traded | 368 | `559f26e6…378b4` | cohort-8 |
| `5w24EmP7Q2Kkw9y9tjMPdixLPMdJHA1xsY7Wip3k5SDm` — market21 | 368 | `559f26e6…378b4` | cohort-8 |
| `6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4` — market19, open for trading | 368 | `91dcbefd…f90ae` | cohort-7 |
| `9JwhTHyxGhaoVsvSyT9VsJxV7PoQcPcjyhMLuJtY38Uq` — closed by an upgrade | 360 | `d5aaadea…30a2fa` | cohort-6 |
| `7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC` — first public market | 360 | `d5aaadea…30a2fa` | cohort-6 |
| `CasyDFowGxqREDW5iWvKRgSMCgk5HnLQjnjegvRsSNPM` — never activated | 360 | `d5aaadea…30a2fa` | cohort-6 |

Each recovered id also appears in the §6 cache table, so Route A and Route B
agree on both cohort-7 and cohort-8.

§3 was right that a length refusal is not a read failure — the three 360-byte
markets read their release set out perfectly. It is worth recording that
`requireCurrentCoreStateWidth` refuses them *without* its explanatory sentence:
it tests `bytes.length === 352` for the legacy case
(`packages/dclutch-sdk/lib/marketCoreV2.ts:203`) and the stranded devnet markets
are **360** bytes, so the "This older devnet Market generation is incompatible"
help never fires for the accounts it was written for. A one-line fix for the
next cut; recorded here rather than fixed in an evidence lane.

---

## 7. What `d202e1f4…` and `97d49888…` actually are

Neither is a release-set id, and no amount of recovering would have made them
one. Both are the cohort's **`final_set_sha256`** — the digest of the *checked
upgrade plan*, an off-chain artifact of the release pipeline:

- `d202e1f4060454519165b8a086d3d6903449069884ea06ae1a02e9f94adc8d0f` —
  `~/jobs/dclutch-cohort7-20260830/plan/plan.json:140` (`final_set_sha256`) and
  `.../plan/prepare.log:3` (`checked_upgrade_set_final_sha256`).
- `97d49888686f71de3da87d918af8acabc7462d1505c2eea4491ef175d7d98985` —
  `~/jobs/dclutch-cohort8-20260831/plan/plan.json:140`,
  `.../upgrade/final-audit.json:149`, and `.../HELD_STATE.md:1281`
  (`FINAL SET SHA256`).

The `execution_release_set_id` a market pins is a different hash of a different
thing, and the two were never equal for any cohort — DEPLOY-1, cohort-5 and
cohort-6 all have both, and §2's three recorded rows are execution ids while
docs/ledger/SESSION_STATE_2026-08-31.md and GOAL.md were quoting plan digests.

**So §2's "not recoverable from this repository at all" was true, and its
diagnosis was wrong.** The ids were unrecoverable from the repo because a plan
digest is not on chain and is not in the repo — not because the release-set ids
were missing. Those were in the markets' own bytes the whole time, one
`getAccountInfo` away, exactly where §3's Route A said to look.

The lesson worth keeping is narrower than "check your greps": **a truncated hex
prefix carries no type.** `d202e1f4…` and `559f26e6…` are indistinguishable as
strings, and the prose around them called both "the set". Eight characters of
hex identify a value only if you already know which value you are holding.

---

## 8. Every hop, and what each would require

Composed from the eight caches. `moved` is
`before.artifact_release_id() != after.artifact_release_id()`, which is the
program's sole definition (`programs/dclutch-registry-sbf/src/lineage_v1.rs:272-277`).

| hop | roles that moved | conjunct 4 | conjunct 5 | signers conjunct 6 demands |
|---|---|---|---|---|
| 1 → 2 | all five | ok | ok | `4zrxtw5c…` |
| 2 → 3 | all five | ok | ok | `4zrxtw5c…` |
| 3 → 4 | all five | ok | ok | `4zrxtw5c…` |
| 4 → 5 | all five | ok | ok | `4zrxtw5c…` |
| 5 → 6 (cohort-5→6) | all five | ok | ok | `4zrxtw5c…` |
| 6 → 7 (cohort-6→7) | all five | ok | ok | `4zrxtw5c…` |
| 7 → 8 (cohort-7→8) | core, claims, trading, custody — **not resolution** | ok | ok | `4zrxtw5c…` |

- **Conjunct 4** (a hop may move a role's bytes, never its identity) holds on
  every hop: all five program ids are constant across all eight sets.
- **Conjunct 5** (a moved role's deployment slot strictly advances) holds on
  every hop, as §4 predicted it would for retroactive hops — the slots compared
  are the two endpoints' own.
- **Conjunct 6** is the binding one, and §4's framing needs one correction. It
  says a hop needs "the upgrade authority of every role whose artifact moved",
  which reads as though different hops might need different coalitions. On this
  cluster they do not: **all five roles in all eight sets bind the same upgrade
  authority**, `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`, the retained
  Loader deployer. Every hop needs exactly that one signature and no other.
- The 7→8 hop additionally requires resolution's consent slot to hold
  `system_program::ID` and **not** be a signer — an unmoved role makes no new
  claim, so nothing may stand where consent would go
  (`lineage_v1.rs:236-241`).

---

## 9. Why no declaration was authored, and what refuses

**No `DeclareSuccessor` was sent.** Nothing was written to devnet by this lane.

Conjunct 6 is what stops it. It requires the signature of the upgrade authority
bound in the successor's activation cache, and per §8 that is
`4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP` — the retained Loader deployer,
which is deliberately not among the keys any cohort lane holds:

> The retained Loader upgrade authority is **not** here; it is
> `/Users/ember/jobs/dragons-clutch-devnet-20260819/keys/deployer.json`
> — `~/jobs/dclutch-cohort7-20260830/README.md:33-35`

The cohort-7 and cohort-8 key inventories carry buffers, market keys,
participants and the campaign payer. They carry no upgrade authority, by
construction. Authoring any hop therefore needs a key held outside the lane
boundary and a decision above this lane's charter — so the record stops here
rather than improvising a Registry write.

Concretely, the refusal a lane would hit without that signer:
`RegistryError::ReleaseLineageAuthorityMissing`, from
`lineage_v1.rs:230-231` — `if !slot.is_signer || slot.key.to_bytes() != bound`.

### 9.1 A second blocker, structural: nothing can build the instruction

Independent of the key, **no tool in this repository can send a
`DeclareSuccessor`.** The route exists on chain and is reachable — the wire type
is `crates/dclutch-registry-svm/src/lineage_v1.rs:62-64`, the SBF route is
`programs/dclutch-registry-sbf/src/lineage_v1.rs`, and `lib.rs:301-303`
dispatches it under `DECLARE_SUCCESSOR_MAGIC_V1` — and it is deployed, because
cohort-8 was built from a tree containing `lineage_v1.rs`
(`~/jobs/dclutch-cohort8-20260831/src/programs/dclutch-registry-sbf/src/`).

But nothing on the host side constructs it. A sweep of `tools/`, `packages/` and
`apps/` for `DECLARE_SUCCESSOR|DeclareSuccessorV1` returns exactly one file,
`apps/dclutch-web/lib/generated/routeCensus.ts`, which merely *enumerates* the
route. `packages/dclutch-sdk/lib/releaseLineage.ts` is a **read** mirror: it
derives the PDA, decodes the record and walks the chain, and has no writer. The
operator binary that performs every other devnet write,
`tools/local-validator/bootstrap/successor`, has ~50 subcommands and none of
them is this one.

So authoring a hop today means writing a new Registry-write path — an 11-account
frame against a route with eight conjuncts — from scratch. That is precisely the
"improvisation on Registry writes" this lane was chartered not to do, and it is
a build task with its own review, not a step inside an evidence recovery.

### 9.2 What is ready

Everything except those two things. The endpoints exist as accounts, both caches
decode, conjuncts 1–5 and 7 are satisfied for all seven hops, the lineage PDAs
are pristine (§10), the route is live in the cohort-8 Registry, and the campaign
payer `GZQoAjVBaNh7KcGDSjjMaFBcTaJPbYxhkDYHudYb88ic` holds 2.645351216 SOL
against a measured rent-exempt minimum of **2,616,960 lamports** (0.00261696
SOL) for a 248-byte account — about 1000× the cost of all seven hops. Funding
was never the constraint.

A follow-on lane needs, in order: a host route that builds the instruction, a
decision about the deployer signature, and then seven transactions.

---

## 10. What the walk actually reports today

The shipped SDK walk (`packages/dclutch-sdk/lib/releaseLineage.ts`,
`followReleaseLineageV1`) was run unmodified against live devnet at the slot
above. Its output, and the correction it forces:

**market22 — the first-ever-traded market — is already followable, and needs no
declaration.** It was founded on `559f26e6…`, which is the *current* set. The
walk arrives:

```
origin      559f26e64683bd986d56575518daec2b65bfb66e20845d8615b13ead268378b4
lineage PDA B9MpfCAHSYftkZsBKsr4NebdywjXDkx8iSUmFGj1S37s
to head     status=arrived  hops=0  alreadyCurrent=true
to current  status=arrived  hops=0  alreadyCurrent=true
```

This is the walk reporting the state of the world accurately, exactly as §5
says. market22 does not span a cut yet: it was founded by cohort-8 and cohort-8
still runs. **It acquires a lineage requirement the moment cohort-9 activates,
and not before** — at which point the 8→9 hop, plus the six behind it, become
the input list §4 describes.

The markets that are behind today are the older ones:

```
market19 (cohort-7, 91dcbefd…)   to head    arrived, hops=0    <- head of ITS chain
                                  to current refused: successor-undeclared at 91dcbefd…
legacy markets (cohort-6, d5aaadea…)
                                  to current refused: successor-undeclared at d5aaadea…
```

Both refusals name the set that owes a declaration, which is the repair
instruction rather than a complaint — the behaviour §5 promises.

**The `to head` results are worth reading carefully.** market19 reports
`arrived, hops=0, alreadyCurrent=true` while sitting two cuts behind the world.
That is correct under the walk's own contract — with no destination the walk
runs to the head of the *declared chain*, and an undeclared set is trivially its
own head — but `alreadyCurrent: true` is a dangerous thing for a caller to read
off a stranded market. Until a chain exists, `walk_lineage_to_head` cannot
distinguish "current" from "unmapped", and only `walk_lineage_to(destination)`
tells the truth. Callers on the site and in tooling should use the destination
form. Recorded here as a live finding rather than a code change, because the
walk's contract is right and it is the *usage* that needs pinning.

### 10.1 The lineage PDAs, and that they are pristine

Derived under the Registry with `[dclutch:release-lineage:v1, predecessor]`.
Every one is vacant, so conjunct 7's no-fork check would admit each hop:

| predecessor | lineage PDA | state |
|---|---|---|
| cohort-6 `d5aaadea…` | `6BY8scYfpWVMWLXerZCUzCe5vPtZ7DQmehTLYxtHcASQ` | vacant |
| cohort-7 `91dcbefd…` | `D7PA8X22p85PkdgbYHfuLWUqoywnRiTkeg6hKCgzduLa` | vacant |
| cohort-8 `559f26e6…` | `B9MpfCAHSYftkZsBKsr4NebdywjXDkx8iSUmFGj1S37s` | vacant |

### 10.2 Cut gate 6

The mapping cut gate 6 asks for — cohort-8's predecessor is cohort-7 — is
recorded: `91dcbefd3f8d81b27236aeae535baffcb002210cffad680ba06feb7d7e2f90ae`
→ `559f26e64683bd986d56575518daec2b65bfb66e20845d8615b13ead268378b4`, with
core, claims, trading and custody moved and resolution unmoved. The gate asked
for the mapping recorded durably, and that is what this is; it did not ask for
the declaration, which §9 explains is one signature short.

