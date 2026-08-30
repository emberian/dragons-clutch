# SESSION STATE — 2026-08-30 ~16:50 EDT

Read this first. Written immediately before a `/compact`, so it assumes the
reader has no memory of the session. The wave board at
`/private/tmp/dclutch-wave2-board.md` is the full record (long); `GOAL.md` is
the done-log; `WAVE.md` carries the rulings.

## THE ONE THING THAT MATTERS

**The public Direct Hot route does not fit under the 1,400,000 CU ceiling for
arbitrary keys, and that is the only thing between here and the first trade
on a public dClutch market.**

Everything else is built: market18 is open on devnet, both participants are
admitted and funded, the first capability root in the protocol's history is
live, the heap wall is closed, the manifest producer exists, and the load
simulator is sustaining on live devnet. Seven of the first trade's eight
stages have finalized on chain. Only the eighth is blocked.

Measured on clean main (CUCUT, `ff9112c1`, all eight ELFs rebuilt, 32 seeds):
worst seed **1,393,616** against the ceiling — **6,384 CU of margin**, and the
checked-in gate is already red. Earlier estimates of 18,424 were optimistic.

## THE PLAN TO CLOSE IT (in flight)

The band is **100% bump-search depth** — every gap between observations is a
multiple of ~1,500 CU, the cost of one `find_program_address` attempt. One
transaction makes ~42 searches, ~16 of which vary with the key draw, worth
~63,000 CU above the unavoidable minimum. Nearly every survivor is *across a
CPI boundary*: Trading finds an address, discards the bump, and the child
searches for the same address again. The Market PDA alone is searched four
times from identical seeds.

- **CUCUT** (agent `ada700a9591280bf4`) — the ~16 key-varying searches: carry
  the Market bump Trading→children through the spare zeroed reserved bytes both
  child wires already have (5 in Claims, 24 in Custody — no wire-length change,
  just relaxing a `require_zero` to a typed read), plus stored bumps in Claims
  state, Custody state, CoreState. Owns `hot_v3.rs`, the trading program-test,
  and the margin gate (a **ratchet**: lower the constant as cost drops).
  Precedent in-tree: `borrow_finalized_record_at`.
- **BUMPREC** (agent `a2bb9fa1946bb506f`) — the 18 *constant* record searches
  (~27,000 CU of mean) via `authenticate_record_at`: authenticate a record at a
  supplied address instead of deriving one. Fenced out of `hot_v3.rs`.
- Landing both puts worst ≈1,338,000 + 15,000 tolerance ≈ **1,353,000 — bar
  met.** Then TRADE-2 cuts cohort-7 → market19 → activate → admit → **the first
  trade, with whatever keys the participants actually have.**
- **Still unspecified**: the caller-authority bump is circular (its seeds
  include `hash(request_bytes)`), so it needs a carrier outside the hashed
  region. CUCUT writes the design note; nobody builds it yet.

## THE RISK NOBODY HAS ROUTED YET — TELL TRADE-2 BEFORE THE CUT

`release_set_id` is a hash of the deployed ELF digests, and it seeds the
activation cache directly and the Market identity transitively — which seeds
the Claims market, positions, maker replays and every caller authority below
them. **A rebuild redraws every bump on the route with no source change.**
CUCUT measured a cache bump moving 254→255 from a build whose only difference
was caller-side: 7,500 CU across five searches, band 36,001→42,000. This is
almost certainly what an earlier lane logged as "codegen noise of ±20,000 CU
between builds."

**Consequence: cohort-7's ELFs are a fresh die roll on CU.** TRADE-2 must
MEASURE the actual cohort ELFs after building them and before relying on the
route, rather than assuming main's numbers carry. If the draw is bad, rebuild
is a legitimate remedy — but only if someone knows to look.

## LIVE LANES (resume with SendMessage to the agent id)

| lane | agent id | doing |
|---|---|---|
| TRADE-2 | `a7c1ba28ecbf894d9` | caller sweep (SDK/web byte-identical pair + tests, devnet driver's 6 wire sites + count-of-3 gate, `preflight.py` grep anchor + geometry table, 2 stale `rpc.rs` doc comments), then market19/activate/admit/trade. **Owns**: the cohort-7 cut, all devnet writes, `tools/release`, the public-cut fixture, `OPEN_LABEL`, and the ONE authorized whole-tree refusals regeneration (at the cut, on a quiet tree, announced first). |
| CUCUT | `ada700a9591280bf4` | child-bump plan, above |
| BUMPREC | `a2bb9fa1946bb506f` | the 18 constant record searches |
| CI-2 | `a8abf0f1f1f6b761a` | wiring the four gates that exist and do not run |
| MEMBRANE | `a5e9b10376d59fbf3` | Structured publication/authenticator/seam module; Rational founded market via `DCLUTCH_RATIONAL_COLLATERAL_MINT`; General hot commit half. No report yet. |
| STORY-2 | `ae1b54b8aaee446db` | graduation wall (Core `0x3003`, identity linkage, evidence `/tank/dregg-build/story-walk3/run.log` on hbox); relayer public submission run; story-page truthfulness. No report yet. |

## PENDING EMBER DECISIONS

Four ADRs written today, each with evidence, options and a recommendation, in
`docs/decisions/`:
- **0014 the fee rate** — three rulings: (D1) keep per-venue `fee_recipient`,
  take **no protocol cut** (the protocol has no income; market founders do —
  say it out loud); (D2) `MAX_FEE_BPS = 500`, no lower bound, which
  **overrides a deliberate prior decision** and says so; (D3) unpin the release
  const so the demo can show rate diversity.
- **0015 the four dead markets** — they are **untradeable, not unredeemable**.
  Rule C now (they are filed under "open", the one untrue thing on the site);
  hold A (leave them standing as witnesses); refuse D; keep B available.
- **0016** a checked release binds three identities, one author each.
- **0017** the reentrancy answer was never ratified; its enforcement is
  subtractive.

Also open and unowned: **what does the Registry outer composition buy?** The
continuation route is over the ceiling on 3/12 seeds, ~32,900 CU *more*
expensive than top-level, and `hot_heap_frame_is_inert` is red on clean main.
Answer that before deciding fix-or-retire; "which route is production" is a
live lever, not an inheritance.

## OPERATIONAL RULES THAT COST REAL DAMAGE TODAY

- **`tools/lane.sh commit`** — the enforced `--only` rail, in the repo the
  whole time. `git add <files> && git commit` commits the WHOLE SHARED INDEX;
  it swept another lane's files twice and left `main` uncompilable once.
  `git commit -- <paths>` is the manual form but does **not** cover untracked
  files.
- **Multi-file breaking changes go in a worktree** until they compile. The
  shared tree is a build input for a dozen lanes.
- **Never run whole-tree generators** at this lane count — `tools/genref/generate.sh`
  swept eighteen lanes' refusal codes into one reference.
- **Cite by symbol; line numbers decay within the hour** (a citation went stale
  in 60 minutes when an unrelated commit drifted the region 60 lines).
- **32 seeds, never 12** — twelve understated a worst draw by 7,659 CU.
- **A gate that cannot fail is decoration** — prove it red before trusting green.
- **An impossibility is a refusal; a size is an estimate.** "Needs an ABI
  change" is a cost in the grammar of an impossibility. (Ember caught the
  orchestrator doing this too — see below.)
- **Disk**: the volume hit 100% twice and stopped every lane. Root cause was
  the simulator's O(N²) census (now bounded to a constant 3,716,160 B) plus
  ~373G of stale lane scratch. Clean up worktrees and target dirs.

## THE ORCHESTRATOR'S OWN ERROR, RECORDED

Ember caught me ruling that TRADE-2 should **select maker keys** landing in the
cheap half of the CU band so the first trade would succeed, and label them
"selected for CU" — rigging the demo and labelling the rig, one hour after
telling another lane that a size is not a refusal. Reversed. The standing test
is ember's: **does it make the DEMO work, or the PRODUCT work?** A stranger
draws their keys once and does not get to draw again.

## COMPLETED TODAY (short form; `GOAL.md` has the full done-log)

Claim-check compaction shipped whole (14 commits, one ELF) — a terminal market
now retires past a sleeping holder who is still paid, to the atom, what
redeeming on time would have paid; redeem costs 13,399 CU on 7 market-free
accounts. R3 **narrowed, not closed** (native yes, fractional no). · Four
cohort-critical security welds, incl. a permissionless verb that let anyone end
every holder's redemption for one fee. · Dealer R4 closed by making the bad
state unrepresentable. · Seam-audit gate green and `--write` made unable to
read the working tree. · Simulator restored, storage bounded, death made
self-honest, a third Helius-key leak site found and fixed (verified zero in
both repos' history, the live site, and the work dir). · The site got names,
questions, clocks, odds, share cards, sparklines, live updates, and the compost
poster. · Lineage migration design + commits 1–3 (4–7 held for after the cut).
· Basis-ABI unification ruling + its five wire-neutral commits.
