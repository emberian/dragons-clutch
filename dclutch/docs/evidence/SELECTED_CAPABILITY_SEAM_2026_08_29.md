# The capability-neutral selection seam, and the first non-Direct-selected Market

2026-08-29, SEL-SEAM lane. **Evidence class**: executed on a fresh local
private validator against the real checked-release ELFs (gate
`fcabe578…` at `7b183c53`, `sbf_build_diagnostics_total=0`), driven by the
successor bootstrap through `tools/release/private-validator-lifecycle/run.py`.
No devnet, mainnet, or wallet claim is made anywhere in this document.

## What the seam is

A founded Market binds ONE selected trade capability beside its three
same-release Resolution companions. Before today the founding driver spoke
Direct-named facts; now the selection flows through one neutral waist:

- `tools/local-validator/bootstrap/successor/src/selected_capability.rs`
  (8936664c): the manifest entry is DERIVED from the release's own selected
  descriptor, program-set bytes, and config bytes — the publication is the
  single author of every capability fact; merge into the canonical
  three-entry Resolution base; validation re-derives and compares. The
  one-selected-capability-per-Market answer is read off the manifest CODEC:
  canonical encoding demands strictly ascending kinds, so a second entry of
  one kind is unencodable (test:
  `the_manifest_codec_itself_refuses_two_entries_of_one_kind`).
- Direct is the seam's first consumer (same commit): its attach/validate/
  entry paths delegate; its kind is now derived from its own descriptor
  rather than restated.
- The founding pipeline is capability-neutral (e424f4b8): `MarketRunInput`
  carries a family-neutral `selected_capability` payload (exactly one of it
  and `direct_capability`); validation, record publication, the
  controller-mask census (`selected_founding_controller_masks_v1`), and the
  founding evidence all branch through the seam. The root selection, funding
  ledgers, and checkpoints already consumed only the entry index and
  manifest bytes — the seam really was one waist.
- General is the second consumer (bb4e83ca, e424f4b8):
  `general_market.rs` re-shapes GEN-REL's `general_selected_release_v1`
  into the closure with **zero seam changes** — the capability-neutrality
  test. `demo_general_market_input` compiles the demo market graph with
  General selected; `local-private-validator-market-v1` selects it with
  `DCLUTCH_MARKET_CAPABILITY=general` (default remains Direct, unchanged).

## The executed proof

One pipeline run (`--through participant --hold-after-participant`,
`DCLUTCH_MARKET_CAPABILITY=general`) founded, on a fresh local validator:

**Market `NVR3SSokuGYdew2b2odchEoV5WNFXriSnyHy5y2Y2JS` — the first Market in
the tree's history whose selected trade capability is not Direct.**

The chain did all of it:

- 65 General publication records finalized (program set, `GeneralConfigV3`,
  and nine artifacts for each of the seven actions), schemas read off the
  release's own artifacts; then the market record graph, funding ledgers,
  and checkpoints — all through the unchanged capability-neutral ladder.
- Five durable founding legs finalized: 583,727 / 880,831 / 1,052,645 /
  332,919 CU, then the genuine composed DCLTGMF3 as a v0+ALT packet
  (58-key lock census), signature `3soyZCdv8sLrmdHBcrwJDLJB7bz2QsRJ2VRUGbBA
  7sAV5d13NRmLhjeSGfUW1MbpvgZzun2EUhajFsnzUVeAPkZZ`, finalized slot 11673,
  `err: None` — after the substituted-Claims hostile refused and rolled the
  whole founding back at slot 11572, exactly as on the Direct route.
- The successor's own finalized read reported `phase=Open
  readiness=Consumed`.

**Verified from chain state, not driver say-so** (validator restarted on the
run's own ledger — the probe13 pattern): the account at the derived Open
Market address is Core-owned, magic `DCLTCOR3`, 360 bytes, 3,396,480
lamports; its `CoreState` binds the SHA-256 of the General-selected manifest
at the identity's capability-manifest coordinate (offset 176); manifest
entry 3 carries the General publication's `kind_id` / `program_set_id` /
`config_id` / `capacity_profile` at offsets +0/+32/+64/+96 — cross-checked
byte-for-byte against the publication, whose `program_set_id` and
`config_id` are themselves the digests of the program-set and config bytes.

So the composed GMF3 route and the whole record/funding/checkpoint ladder
are capability-neutral **on-chain in fact**: a General manifest entry rode
every wall the day cleared for Direct, and only a client-side classifier
noticed the difference (wall #10 below).

### Lab facts, labeled as lab facts

The General selection's derived facts come from the market graph itself
(capacity profile from the carried source-capacity body, claim basis from
the carried linked-basis record, outcome width from the cuts, price scale
from the collateral decimals, generation from the founding lane) and from
the same finalized loopback snapshot Direct quotes (activation deadline;
root Rent for the descriptor-named General root width). The policy windows
and external account widths are the executed accelerator campaign's; the
four deployment identities (accelerator artifact release, compiler,
toolchain, translation validation) are domain-separated projections of the
plan's release-set identity, because no local accelerator deployment exists
to observe — the same labeling discipline as the demo market's synthetic
Pyth release. A devnet General market replaces exactly those inputs.

## Wall #10 (client-side, the wall-#9 class)

After GMF3 finalized, the campaign died at `funding-readiness account
states were mixed or did not select one adjacent route: phase=Open
readiness=Consumed create=false activate=None accept=false` — the
funding-readiness classifier could not name the ConsumedByFounding
poststate, so the journal never recorded the finalization the chain had
already reached. The fix landed at its owner as ef3bbea4 ("the readiness
suffix believes the founding it just verified") from the parallel lane that
hit the same wall on the Direct route minutes earlier. A note for anyone
resuming a founding across a validator restart: a restart-from-snapshot can
prune transaction history, and the journal resume authenticates every
finalized signature — resume on the original process, or expect
"persisted finalized founding transaction disappeared".

## The Fractional fixed point (protocol finding)

The seam's design invariant, found by attempting the Fractional consumer
first (board 12:54): **a selectable capability's config must be derivable
before the Market exists.** The Market PDA derives from `MarketIdentity`
seeds that include the manifest digest; the manifest entry must name the
selection config (`require_entry_identity`, trading `dispatch.rs:448`);
Fractional's config IS the exposure terms (`artifacts_v4.rs:241`, and
Claims pins `header.selection().config() == request.input().terms` at
`fractional_atomic_v3.rs:224`); and the terms bind the Market PDA
(`bind_terms`; `logical_market == core.identity.market_id`,
`founding_v5.rs`). manifest ⊃ SHA-256(terms) ⊃ terms.market =
PDA(seeds ⊇ SHA-256(manifest)) is a SHA-256 fixed point no author can
construct: **no Fractional capability can ever be selected by a founded
Market under the shipped contracts.** The executed pin is
`a_fractional_selection_cannot_precede_the_market_it_binds`
(`fractional_market.rs`, da299fa3), which derives the Market PDA exactly as
founding does and shows the iteration diverging; the executed control shows
the same closure compiling whole for a Market that already exists, and the
seam consuming it with no family special case. General does not share the
defect (`GeneralConfigV3` is market-free — pinned by
`a_general_selection_precedes_the_market_it_will_bind`), and Direct's
config is price-scale + fee. The recommended fix (a market-free
`FractionalSelectionConfigV1` as the manifest-named config, the terms
joined to it at runtime) is on the board awaiting a ruling; until it lands,
any wiring that reaches the wall gets the one named refusal sentence
(`FRACTIONAL_FOUNDING_SELECTION_WALL_V1`).

## The second family, and the first executed action (evening addendum)

By evening the seam had three consumers in code (Direct, General, RAT-SEL's
Rational at 86c249a8) and two families founded through it in fact:

- **Rational-selected markets founded twice**, completing the FULL
  six-mutation success order both times (walls #10–#12 dead from a second
  route): market `9eCkwxBMFiYs9Pgb1EGh9REYWhJPMtefFXP5PTJzpGbd` at 502162b6
  and market `HYKunhUNmsJmuwMyp2SbfcRJxzpaGpcewQ9ehSAaC5xs` at the
  ea9a3e0c graft (= 502162b6 + the wall-#13 run.py fix). RAT-SEL's
  `DCLUTCH_RATIONAL_COLLATERAL_MINT` ordering constraint dissolves with no
  driver change: run.py's forge seed is deterministic
  (sha256(SEED_DOMAIN+"seed-01")), so the collateral mint is the same every
  run and can be named before the closure compiles while the founding
  itself forges it.
- **Wall #13, the participant stage's first blood** (no probe of any
  capability had ever reached stage 07): the admission snapshot requires
  the position owner funded, and nothing anywhere funded the wallet. Fixed
  at the owner (ca68ef5a): run.py funds the participant 0.02 SOL from the
  genesis source as its own named stage.
- **Wall #14**: the admission packet does not fit a legacy message and had
  never been compiled; the `--routing-table` flag existed with no caller.
  Proof of the shape, executed: the admission routed through the founding's
  OWN frozen DCLTGMF3 lookup table (66 keys, read back off the founded
  ledger) compiles, executes, and finalizes. Passing all five founding
  tables refuses `DuplicateAddress` — one table, the frozen one, is the
  contract.
- **THE FIRST EXECUTED ACTION AGAINST A NON-DIRECT-SELECTED MARKET**: the
  participant admission on the Rational market `HYKunhUN…` — admission
  signature `44PeT1AHWh3mTXCT2KynVTa7NxUEWJsCiZRBkXbWre2W5SxWxQKm5s8YpMs74c
  afNojtEWRX9S5gXCPKMaRsdnCy`, finalized slot 8930, 268,172 CU through the
  real Trading→Claims chain, plus the finalized Token-2022 collateral
  transfer (slot 8966). On-chain poststate: the Claims-owned Position
  (160 B) and admission (512 B) accounts exist at their derived PDAs; the
  Market's identity binds the Rational-selected manifest (entry 3 carries
  the DCRLPB01 publication's kind/release/config/capacity, byte-checked).
  The held validator for this ledger runs at `http://127.0.0.1:25544/`
  (`/private/tmp/selseam-hold-11/runs/seed-01/ledger`); note a
  restart-from-snapshot prunes pre-snapshot transaction history — account
  state is the authoritative read there.

Named residue: run.py does not yet pass `--routing-table` (the founding
campaign should record the frozen table's address in its evidence and
run.py should forward it — the admission-lane work at 8f10beb9/623a8783
owns that seam); and something in 502162b6..ca68ef5a re-broke the
readiness suffix for fresh runs (both families refused the six-mutation
pin there while the 502162b6 graft passes) — the window carries the landed
Fractional config split (7c569ac1, 4630ad77: the fixed point's recommended
fix, implemented) among other mid-flight protocol changes, and whoever
diagnoses the suffix should start from that diff.

## What is not claimed

- No devnet or public-cluster action of any kind.
- No General action has executed through Trading's hot commit half; the
  capability root is not activated (PrepaidLazy activation is first-use,
  and the accelerator is not deployed locally). Founding-with-selection was
  the missing shared infrastructure GEN-SER named; the commit half is now
  unblocked, not done.
- The first run held before the participant stage (wall #10 fell after the
  founding finalized); the re-run at the fixed revision is the pipeline's
  ordinary path and its evidence supersedes this section when it lands.
- The lab deployment identities above authenticate nothing about a real
  accelerator build; they are placeholders a devnet market must replace.
