# Folding the founding pre-fund into the founding transaction — feasibility census and ready-to-land change list

ECON lane, 2026-08-29. Status: **design complete, deliberately unlanded** — every
edit below sits in `tools/local-validator/bootstrap/successor/src/market.rs` on
the fenced founding path during the cohort-6 freeze window. Queue for the
cohort-7 window, owner: whoever holds the founding path after the freeze.

## What exists at HEAD (and what LEDGER's finding predates)

Since d60fbfb9 (2026-08-29 13:08) the five founding accounts — Market, permit,
aggregate, position, admission — are pre-funded by
`prefund_founding_accounts_v1` (market.rs:9926): five explicit
`solana_system_interface::instruction::transfer` instructions from the campaign
payer, sent as ONE observed campaign transaction (label `pre-fund the
founding's five program-allocated accounts`, ~900 CU measured), with
`TransactionEvidence`, a stage-log line, and a counted fee. The
"bare off-chain System transfer, no instruction performs it" wording of the
original finding describes the pre-d60fbfb9 shape.

What remains true: the pre-fund is a SEPARATE transaction from the founding
that writes the receipts. The on-chain program never performs the funding —
core only asserts the balances (`programs/dclutch-core-sbf/src/generic_founding_v1.rs:838-841`)
— so no protocol receipt describes the founding's largest inflow, and L7
evaluated over the founding transaction alone still cannot speak.

## The design question, answered

**Can the five transfers ride inside the founding transaction itself, client-side only?**
Yes — mechanically. And it is NOT cheap. Both halves with evidence:

### Why it works

- All five accounts are already writable metas in both routes' frames, and
  `system_program` is already a readonly meta AND an ALT address:
  composed `build_generic_market_founding_v3` (market.rs:8564-8807, 127 metas);
  split stage-1 `build_generic_found_and_permit_v3` (market.rs:8825-9037).
  Five `transfer(payer -> each)` instructions add **zero new account keys**.
- Wire: the founding v0 packet is ~460 bytes of the 1232 limit (derived from
  the pinned census; nothing records the real size). Five transfers cost
  ~+85 bytes, plus ~+31 for the static promotion below.
- Compute: +~900 CU against the 51,253-CU margin `tools/gauntlet/CU_BUDGETS.json`
  calls "below the ceiling ... AND SHRINKING". Affordable, from a scarce budget.
- Nothing on-chain censuses the instruction list: core/claims/custody/registry
  have zero instruction-sysvar reads on this path; trading's only reader is the
  heap-frame parser (`admitted_heap_frame_bytes_from_sysvar_v1`,
  entrypoint_adapter :1094-1145), which is position-blind and refuses only a
  SECOND heap grant. The core assert at generic_founding_v1.rs:838 reads
  post-transfer balances within the same transaction and is satisfied.
- The repo already lands a System transfer prepended to this exact frame, ALT
  and 256 KiB heap included: the fee-only-rollback hostile probe
  (market.rs:10312-10316 composed, :10667-10671 split) — strictly harder than
  this change (it adds a new recipient key; this adds none).

### Why it is not cheap — every pin that notices, with file:line

1. **Static-key promotion.** Making `system_program` an INVOKED program id
   forces it out of the ALT into the static keys (`solana-message` 3.1.0
   `compiled_keys.rs:149,153` extracts a lookup only for `!meta.is_invoked`).
   `static` 3 -> 4, `loaded_readonly` 43 -> 42 (composed) / 42 -> 41 (split):
   - lock-census pins: market.rs:8103-8118 (composed), :9177-9192 (split);
   - hard-coded census digest: market.rs:13443-13446
     (`generic_founding_final_compiler_census_pins_the_58_key_shape`);
   - split-vs-composed delta test: market.rs:12841-12891.
   `complete_keys` stays 58/57; the devnet 64-lock limit keeps its 6-key
   headroom (boundary probes market.rs:10176-10199 unaffected).
2. **The census must describe the real transaction.**
   `compiled_complete_lock_census_v1` (market.rs:7945) takes a single
   `&Instruction`; prepending only at the send site would leave the pins green
   while `resolved_accounts_sha256` (fed from the census digest, market.rs:10396)
   becomes a false witness. The census must be taught the full instruction list.
3. **Prestate authenticators invert.** `authenticate_founding_prefunding_v1`
   (market.rs:10991-11026) requires the five present-system-owned-empty-exactly-
   rent-funded BEFORE the founding; `untouched` (market.rs:10268-10284, re-run
   :10332) and `prestate_intact` (:10613-10622, re-run :10645, :10686) read the
   same prestate. All must learn vacant-and-empty. The durable-journal prestate
   digest (market.rs:1050, re-checked before signing at :620-628) tolerates
   absence (hashes a 0 byte) and needs no edit.
4. **Resume semantics are the real hazard.** Today's three-way branch
   (market.rs:9947-9977): all-absent -> send; all-exactly-funded -> skip;
   partial -> refuse. With in-tx transfers a mixed-generation ledger (an old
   run's pre-fund landed, then this driver resumes) would double-fund and
   refuse at 2x rent inside core (generic_founding_v1.rs:838,
   `CoreSbfError::Reference`). Keep the probe: read the five, include transfer
   instructions only for accounts at zero, refuse on any nonzero-but-not-exact
   (partial funding is already a refusal today). Compensating gain: when the
   founding tx is the only funder, a dropped founding rolls the transfers back
   too — the partial-prefund state becomes unreachable for fresh ledgers.
5. **Gauntlet binding.** `tools/gauntlet/tier1/bindings.json:264-269` pins the
   pre-fund label; a campaign without that transaction refuses as a stale
   binding (`tools/gauntlet/census/src/ledger.rs:455-461`). Delete in the same
   change. (private_activity.rs:824's allowlist entry goes dead but harmless;
   run.py's founding gate filters to FOUNDING_SUCCESS_MUTATIONS and never sees
   the pre-fund label.)
6. **Durable journal (composed route).** `exact_unique_accounts()` = 58 for
   Dcltgmf3 passes unchanged; `authenticate_current_founding_intent_v1`
   (market.rs:443-497) recompiles the instruction list, so a PRE-EXISTING
   journal from an old-shaped run refuses (correct behavior; fresh runs fine).
7. **Devnet delivery, the recorded hazard.** The founding is the campaign's
   largest send; its +1-key rollback-probe variant would not land on devnet for
   six blockhash lifetimes while everything smaller landed in seconds
   (market.rs:10300-10311). Doctrine (board archive 2026-08-27): send one
   preflighted probe of the NEW shape before committing the driver to it.

## What the change buys

- **Atomic funding**: transfers land iff the founding lands; one fewer
  transaction (and fee) per founding; the partial-prefund refusal dead-end
  becomes unreachable for fresh ledgers.
- **Visibility**: the transaction record that carries the founding receipt also
  carries the inflow instructions, so L7's payer-delta law can speak over the
  founding record itself while history is retained.
- What it does NOT buy: purge-surviving on-chain description. After blockstore
  purge both shapes are equally invisible; a receipt ACCOUNT describing the
  funding is a program-side change (cohort-7 scope, separate decision).

## Net change list

Transfers into both routes' instruction lists at build time (gated by the
resume probe); `compiled_complete_lock_census_v1` over the full list; census
constants static 3->4 / loaded_readonly -1 at market.rs:8105-8109 and
:9177-9181; the pinned digest at :13443; the delta test at :12886-12889;
`authenticate_founding_prefunding_v1` repointed post-founding or deleted;
`untouched`/`prestate_intact` taught vacant-and-empty;
`tools/gauntlet/tier1/bindings.json:264-269` removed; one devnet shape-probe
before the cut.
