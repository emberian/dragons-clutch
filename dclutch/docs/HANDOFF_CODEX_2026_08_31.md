# Codex handoff — 2026-08-31 evening

The Fable/Opus swarm drains here. This doc is the queue for codex
passes, organized by track, each item pointing at its zero-research
source. GOAL.md is the day's board (read its top ~200 lines first);
WAVE.md carries the rulings that bind you. **Read
LETTER_TO_CODEX_2026_08_31.md first** — it is the judgment layer this
queue rests on: where we are, what the day taught, what aiming high
means here.

## House discipline (non-negotiable, all inherited)

- Commits via `bash tools/lane.sh commit "msg" -- <named paths>`,
  git-diff each named path first. NEVER `git add -A`. NEVER stash
  (other lanes share the tree).
- Filtered tests only — the narrowest thing that could refute you.
  No bare `-p <crate>` sweeps, here or on hbox. hbox builds go through
  `swarm-build`; hbox is co-tenant (spare ~/dev/datacake procs).
- Generated surfaces are REGENERATED through their own generators,
  never text-merged or hand-edited (TS registries before genref).
- Refusals name codes; hostiles red-proven by mutation; expectations
  pinned literally, never derived from the code under test.
- Seam findings: fix or hand-write a verdict with a reason; never
  `--write`.
- Zero SBF frame diagnostics (and note: the CI grep cannot see
  below-4096 growth — measure with tools/sbf-frame-sizes.py around
  frame-adjacent changes; `inline(never)` on request-embedding
  structs).
- Web and SDK are twins that sometimes differ: fix each side in its
  own file, run both suites TOGETHER at the end.
- NO devnet writes. Devnet writes belong to a designated steward lane
  on cut day only. No mainnet anything, ever.
- Rulings, vetoes, economics decisions, wire-shape choices not already
  ruled: STOP and leave a question in the report. WAVE.md's rulings
  are law (canonical-generation mandate c2eb4f63 especially: derive >
  generate-with-gate > root).

## In flight at handoff (harvest from GOAL.md before starting)

PUBLISH-8 (the flow's site cut), OPERATOR-BOOK (walkthroughs 1-2),
OPERATOR-FORMS (audit + spec + 2 consoles), CLOSE-DRIVER resumed for
its 10 seam fix-or-verdicts. Their reports land on GOAL.md; do not
duplicate their scopes until they close.

## Track A — the FLOWFUL redesign (spec: docs/design/FLOWFUL_IA_V1.md)

1. Market-page restructure + drawer discipline (the spec's phase 2;
   the mockup canvas "Flowful Clutch" shows the target).
2. The IA moves: delete the /direct byte-duplicate of /trade; fix the
   /resolution header; rehome /campaign + /population to the observer
   journey; the /live hard-coded scoreboard tiles become live or
   honest (all documented in the spec's drive-by findings).
3. The site maker flow (spec §4.5): author-an-offer in the browser —
   the board's empty state currently says "Nothing in this build
   authors an offer yet" on purpose; make the spec's copy true instead.
4. TicketBoard fetch-surface component test; board expiry screening
   should use the flow's own finalized slot over the relay's slotBasis.
5. Remaining five consoles onto OPERATOR-FORMS' vocabulary (landed:
   spec at docs/design/OPERATOR_FORMS_V1.md, seven typed fields in
   components/operator/, /product-v2 and /found converted as the
   pattern). Two audit-found BUGS ride this item: /liquidity's
   scaffold ships 38 accounts against a validator demanding 39, and
   /direct carries four `required` inputs nothing reads (two dead
   fields) — fix both during their consoles' conversion.

## Track B — protocol (all specs in docs/design/)

6. Bootstrap ceremony stage (~1.5d): PROFILE_SUCCESSION_HANDOFF §4 +
   PROFILE-3's report (genesis-planted Buffer + one real Upgrade tx is
   the cheap route; the StageV1 insertion has 3 named silent holes —
   read before touching). Must stay never-half-flipped.
7. §8.3 permit-refund arm driver (~0.5d): series_permit_expiry.rs:216
   calls authenticate_profile and NO program test constructs that
   route — PROFILE-3's finding.
8. One loopback close rehearsal: CLOSE-DRIVER's derive_coordinates RPC
   path is un-loopback-tested (its named residual risk); ride the
   bootstrap-stage world from item 6.
9. The basis hoist (~4,500 CU recovery): TIERS attributed the margin
   move to authenticate_product_basis_v3's unconditional admission +
   decode probe running 4x/trade; the correction note at the foot of
   BASIS_ABI_UNIFICATION_V1.md names the fix (hoist to the founding
   caller). Wire-free, but it moves hot-path code: red-proof + both
   margin gates + the fee pair are the controls.
10. Economic error-collapse widening (WAVE b0e81f7c §2): the
    compaction route's inner refusals get named codes; additive,
    CEILINGS' exhaustive bands make it safe.
11. CANON's six emitter briefs (CANONICAL_CLIENT_EXPECTATIONS_V1):
    the twin-less SUSPECT class — resolution-cert V1 emitter, the seal
    contract's generated file, AccountProfile V2 header offsets, etc.
    Look FIRST at the private-const PDA seed domain in
    generate-direct-participant-v1 (an address authenticated by
    nothing on chain). The completed generator audit (CANON's child,
    reported at handoff): NO further convictions, 15/15 shared
    generators byte-identical web-SDK; four NO-ROUTE deletion
    questions (registered-direct module dead on chain; DCLTPAY2
    dormant per its own Lean; rational-terminal-hot's hot half
    unbound; the participant seed domain above); three structural
    risks to gate before they convict (runtime-v2-admission's "route"
    file binds none of its ids; general-successor emits V2 while
    ControllerRequestV3 is live; dealer-equity's V2 decoy preimage).
    Tooling caveat: route-binding.mjs needs an include!-aware hop for
    core-found and one extra re-export hop for the manipulation-floor
    id.

## Track C — smalls (one sitting each)

12. DONE by orchestrator (66edf88f): claims runner exits 2 pre-build;
    proven live in the tier (claims: DID NOT RUN).
13. DONE (66edf88f): tautology deleted.
14. FRAMEGUARD: a delta-ratchet gate over sbf-frame-sizes.py output
    (the 640-byte silent growth class from FRACCHECK-3).
15. ChainExplorer.tsx's 3 real TS inference errors under the BigInt
    noise; the tsc target question.
16. Mobile nav end-of-scroll affordance (DESIGN-2's declined tradeoff
    — a static edge fade was the rejected half-fix; do it properly or
    not at all).
17. DONE (66edf88f): accepted.rs dead items removed.
17b. DONE by orchestrator (f0d84523): the heap-inertness continuation
    floor re-pinned 103,589 → 104,366 by the test's own protocol
    (eleven seeds, +777 exactly, zero jitter, rung intact) — the last
    programs-row red; goes green at the next public cut, as does seam
    (verdicts f19e10e5 landed after PUBLISH-8's pin).
18. TWO SITE-PUBLISHED GUIDES TEACH COMMANDS THAT CANNOT RUN
    (OPERATOR-BOOK's finding, priority within this track):
    docs/guides/reader.md documents run.py without its five required
    args; docs/guides/trader.md documents `dclutch join`, which does
    not exist (the real act is dclutch-local-successor-bootstrap …
    user-position-admission-v1). The VERIFIED correct invocations are
    in docs/operators/{author-a-ticket,found-a-market}.md — fix by
    transcription from there, strike-five register.
19. tools/gauntlet run.sh --mode full is dead code after its own
    15-minute build (unconditional die at :631, advertised in the
    README as an entry point) — make it refuse BEFORE building, or
    retire the mode honestly.
20. Founding is unreachable from a cold machine: the checked release
    root demands --predecessor-profile (a 144-byte chain dump) with no
    committed fixture. Commit a devnet-derived fixture with provenance
    or a fetch step in the walkthrough — an onboarding wall.
21. Stale target/release dclutch binary confusion (0.1.0-devnet.1 vs
    .3 in source) — the walkthroughs build fresh; consider a version
    guard in run-local tooling.
    Also for the NEXT PUBLICATION CUT: wire docs/operators/ into
    render-site.mjs's guide list (~10-line branch; listing IS
    publishing there by design — "Unlinked is not unpublished").

## NOT for codex

- The cohort-9 CUT itself (devnet, deployer, ceremony — steward+ember
  day act; the checklist and every invocation live in GOAL.md/the
  reports, dry-run first).
- The donation-slice payee, the opener-shortfall economics, the
  RECORDS-MIGRATE split, Rent's deferral list (ember rulings).
- The upkeep vault (UPKEEP_VAULT_V0 is a sketch awaiting its
  adversarial review — cohort-10).
- General's runtime-dispatch unit (weeks-class; charter comes with
  cohort-10 planning, not a background pass).
