# SIMPLIFY-PROGRAMS — the twelve programs other than Trading

Branch `simplify/programs`, worktree of `/Users/ember/dev/dclutch` at `330bbfaba`.
Every number below was measured in that worktree; the census control for each
deletion is in the commit that made it.

## Programs: 12 → 8

| program | disposition | control |
| --- | --- | --- |
| `dclutch-registry-sbf` | keep | 11 routes, 2 devnet + 7 local-validator witnessed |
| `dclutch-core-sbf` | keep | 10 devnet-witnessed routes; Rent fold not done here (below) |
| `dclutch-claims-sbf` | keep; one producer-missing route deleted | 4 devnet + 18 program-test routes |
| `dclutch-custody-sbf` | keep | 7 devnet-witnessed routes |
| `dclutch-resolution-proof-sbf` | keep | 3 devnet + 12 local-validator + 10 program-test routes |
| `dclutch-rent-sbf` | keep, **named as a Core arm the fold cannot take this swarm** | 4 devnet-witnessed routes; see "Rent" |
| `dclutch-general-accelerator-sbf` | **merged** → `dclutch-accelerator-sbf` (`src/general.rs`), band 0xC and every code unchanged | devnet-deployed beside the seven in cohorts 14/15 |
| `dclutch-dealer-accelerator-sbf` | **merged** → `dclutch-accelerator-sbf` (`src/dealer.rs`), refusals moved to sub-band 0xC100 | never on any chain (`blocked.json` out-of-release-set); its 12,062-line program-test is the only driver |
| `dclutch-series-shadow-sbf` | **merged** → `dclutch-accelerator-sbf` (`src/series/`), refusals moved to sub-band 0xC200 | never on any chain; decision 0029 item 1 keeps the Series family, and the shadow is an evaluator with the accelerator's contract, not a program id |
| `dclutch-direct-aot-sbf` | **deleted** | `SHIPPED_LINKS: false`, in no cohort; evaluates the superseded Direct V2 descriptor (DIRECT_HOT_AOT_MEASUREMENT 2026-08-31 §5: "retire or re-point"); no Cargo.toml outside its own tree and campaign depends on it |
| `dclutch-product-runtime-v2-sbf` | **deleted** | `SHIPPED_LINKS: false`, in no cohort; its receipt had zero on-chain readers (Claims' optional recheck had one caller, passing `None`); Core and Claims re-derive its checks through `dclutch-product-runtime-v2-svm-reader` |
| `dclutch-trading-sbf` | not mine | — |

Refusal bands: 12 program bands → 8 (`RefusalBandsV1.lean` rows removed for 9,
10, 11, 13; retired list `[7, 9, 10, 11, 13, 14, 15, 16]`; band 12 renamed
`accelerator` / `ACCELERATOR_REFUSAL_BASE`). The emitted Rust and TS mirrors
are hand-applied to what the emitter prints for that table; the convergence
`lake build` + `abi:refusal-bands:verify` are the check.

Frameguard links: 12 → 8 in both halves of the pinned count; baseline rows for
the six departed packages removed. **The merged accelerator link is
unbaselined** — its frames are the union of three links and must be captured
once at convergence (`tools/frameguard/run.sh --at <commit>`); the ratchet is
red for that one link until then, by design (§3.4 of the architect's map).

## Commits on the branch

1. `eae0ba120` **one author for the refusal-band pin.** Forty copies of the
   same sixty lines (ALL array, `ordinal()` match, const loop, the fifteen-line
   "why a list" comment, `From<Enum> for ProgramError`) → one macro in the
   crate that owns the bands, `dclutch_refusal_registry::pin_refusal_band!`.
   The enum stays a plain item with literal discriminants (the census reads
   that; a macro-wrapped enum would be invisible to it) and the weld survives
   (a variant absent from the list is a non-exhaustive match). Converted:
   nine program crates, Claims' twelve sub-band enums, thirteen test callers.
   Not converted (other lanes' files): Trading's `TradingSbfError` and
   `SeriesAccountErrorV3`, the svm-harness caller. −2,647 lines. No
   discriminant moved; no ELF byte can move.
2. `2418e6173` **Claims: delete the Core-effect route nothing produces.**
   `process_core_effect` + the foundational split (−510 lines, six blocked
   census rows, one `blocked.json` entry). Control: zero producers of a
   `CoreEffectEnvelopeV1` targeting `Role::Claims` anywhere in crates,
   programs, tools, tests; Core never builds `InitializeClaims`/`SplitClaims`/
   `RedeemClaims`. Also dropped: Claims' dead optional admission-receipt
   parameter (its one caller passed `None`) and two stale module docs in
   Core/Claims `product_runtime_v2.rs` that said the modules were undispatched.
   Note: `MECHANISM_BATCH_SPINE` §4's table lists this route as SURVIVE — that
   row assumed a producer; nothing else in the spine note moves.
3. `c9b41990f` **delete direct-aot and product-runtime-v2** with the sweep:
   workspace members + `Cargo.lock` (−36), Lean rows + retired-list theorem,
   generated Rust/TS band mirrors, `tools/gauntlet/direct/` (the direct-aot
   campaign), substrates row, `blocked.json` row, census TARGETS, SHIPPED_LINKS
   in both the release tool and the successor, ci comment, frameguard 12 → 10.
4. `5b4fe2313` **the one accelerator.** `programs/dclutch-accelerator-sbf`
   with `src/lib.rs` (the guard chain + shared allocator + entrypoint),
   `src/general.rs::process`, `src/dealer.rs::process`,
   `src/series/{mod,evaluator,release}.rs` (`series::process`), the Series
   `build.rs` + `generator/`, three program-tests (`program-test/`,
   `dealer-program-test/`, `series-program-test/`) and two test callers.
   The selector reads the arm from the fact each arm already authenticates —
   the Shadow magic on the instruction data, else the family magic of the
   top-level Trading instruction read out of the raw Instructions sysvar at the
   one coordinate both admitted frames share (allocation-free; three host
   tests). Sweep: workspace member, Lean band rows, mirrors, census TARGETS,
   `general` bindings ids, `programs.json` labels, substrates producers, the
   general/dealer-checkpoint/general-hot runners, release roles in three
   tools, successor SHIPPED_LINKS, ci suite rows, frameguard 10 → 8. Plus in
   the same commit: the resolution alias `slot_pinned_deployment_observation`
   (a pure rename of `cached_deployment_observation`) collapsed; the Series
   arm's duplicate `funding_count` collapsed onto the evaluator's; one stale
   General unit test (it asserted the inline transport refuses, a rule the code
   had already dropped) corrected; deliberation narration in custody and the
   General arm trimmed to the invariants.

## Rent: merge into Core is right, and it is cohort-17's, not this branch's

The architect's map folds Rent into Core (the credit is Market-generation
scoped; Core's `found.rs` `authenticate_rent_credit` has to check the credit's
owner is *another* program — the seam the fold deletes). I read the seams and
did not do it, for one reason that is structural, not budget: **Rent is a slot
of the infrastructure profile.** `ProtocolInfrastructureProfileV1/V2` bind
`registry` and `rent` as two `ExecutionRoleBindingV1`s; the genesis ceremony
(`infrastructure.rs`, local-validator witnessed) and the succession ceremony
(`infrastructure_v2.rs`, `InfrastructureIdentityMoved`) refuse an aliased or
Core-valued rent binding by name, and `dclutch-release-set-contract` refuses
"Registry and Rent named an aliased program". Removing the rent slot is a Lean
ABI change (`EmitProtocolInfrastructureProfileAbiRust/Ts.lean`) on a wire the
genesis route carries, which §2.5 of the map reserves from the Lean lane this
swarm; keeping the slot and pointing it at Core is a dead field kept alive by
relaxing three refusals, which is worse than the seam it removes. So Rent stays
a program on this branch. The fold's exact edit list for cohort-17: (1) the
profile ABI loses `rent` (Lean-first, both emitters, the web decoder); (2)
`LifecycleRentInstructionV2` becomes a Core magic family, the credit PDA
derived under Core, `authenticate_rent_credit`'s owner check deleted; (3)
every reader of the credit (Core found/retire/generic_founding/series_open,
Claims founding_v5, Custody, Resolution) re-points its PDA program; (4) band
0x2 retired; (5) the successor's `rent-credit` subcommand becomes a `core`
one; (6) cohort manifests drop the role.

## Left deliberately, and why

- **`dclutch-custody-sbf`'s five projected/abort routes and Core's Series
  routes** are blocked on the Series family tier — decision 0029 item 1 (BUILD)
  keeps them; deleting them reverses a confirmed ruling.
- **Registry `continuation_v1` vs `hot_continuation_v2`** are two routes, not
  two generations: the generic role continuation has producers
  (`dclutch-operator`, the market-open/retirement operators, the successor's
  terminal sequence); the transparent Hot continuation is the Direct-Hot path.
  Both blocked for evidence wiring only.
- **Core `infrastructure` (V1 genesis) and `infrastructure_v2` (succession)**
  are two ceremonies with one width; V2 is the only repair for a Registry/Rent
  upgrade under a live cohort. Kept.
- **The five finalized-record authenticators** (Core `records.rs`, Registry,
  Resolution, the Series arm, Claims `terminal_settlement_v3`) restate one
  fact with a real divergence: Registry and Resolution refuse a staging PDA
  with nonzero lamports, Core/Claims/Series/the svm-reader accept dust (Core
  documents why). The zero-lamport form is griefable — anyone can dust a
  staging PDA before activation. One author (`dclutch-product-runtime-v2-svm-reader`'s
  private `authenticate_record`, made public) is the fix; it changes the
  Registry and Resolution ELFs on a witnessed route, so it is named here for
  the crates lane rather than done blind in the last hour of this branch.
- **`map_err(|_| Coarse)`: 2,737 sites in my domain.** I carried the cause
  where I rewrote (the Dealer arm already maps Trading's four accelerator
  conjuncts by name) and did not sweep; a sweep without a route behind each
  site is the "verification theater" the memory warns about.
- **The clients' `productRuntimeV2Admission.ts` (SDK + web), its generator,
  scripts and Studio step 03** scrape the deleted program's `lib.rs` and
  compose an instruction for a program no chain has. The map assigns
  `apps/` and `packages/` to the clients lane; their `abi:product-runtime-v2-admission:verify`
  is red on the converged tree until that module is deleted. Owed, named.
- **`tools/gauntlet/aot-cu`, `tools/direct-translation-validator`,
  `dclutch-direct-aot-{,v3-}contract`** — the map deletes them with direct-aot
  (generation-deletion / crates lanes); the aot-cu harness still names the
  deleted ELF as a comparison anchor until then.
- **The successor driver's `--general-accelerator-*` flags and
  `general_accelerator.*` manifest fields** name the role the accelerator plays
  for General markets and still deploy exactly one accelerator; the ELF it
  builds is `dclutch_accelerator_sbf.so` through the release tool's roles
  table. Renaming the vocabulary is the successor lane's.
- **`docs/reference/**`** is generated; regenerated at convergence with
  `--converge`.

## Wire

- No devnet- or local-validator-witnessed route moves: every General
  accelerator code, magic and frame coordinate is byte-identical; Core, Claims,
  Custody, Registry, Resolution, Rent dispatch unchanged.
- ELF bytes that move: Claims (a dead arm leaves), Resolution (an alias
  collapses — a rename, likely byte-identical, to be shown by sha256 at
  convergence), Custody (comment only — byte-identical expected), and the
  accelerator (a new link). Cohort-16 carries the Claims and accelerator
  redeploys; the accelerator's Registry pin (`general_accelerator.*` in the
  cohort manifest) is re-derived from the new ELF digest.
- Refusal codes that move: the Dealer arm's `0xD000..0xD00A` → `0xC100..0xC10A`
  and the Series arm's `0xB000..0xB004` → `0xC200..0xC204`; neither was ever
  observed on any chain, and both program-tests assert through the enum.

## Counts

- Program crates 12 → 8; program bands 12 → 8; frameguard links 12 → 8.
- Source lines in my domain (programs/*/src, non-Trading): 84,949 → 82,083
  before the fold's tooling; the folded accelerator is 4,517 against
  2,050 + 564 + 2,034 = 4,648 standalone.
- Routes (`dclutch-route-census inventory --check-unique` on the branch):
  164 → 156 — minus the six Claims core-effect rows, direct-aot's one,
  product-runtime-v2's one, and the six standalone-accelerator rows, plus
  the accelerator's five (`accelerator/process_instruction`, the General
  entry the `general` campaign binds; `accelerator/series::process` and
  `accelerator/series::evaluate_selected_and_publish#accepted`, blocked on
  the Series tier; `accelerator/set_return_data#{ChunkedBankV2,OutputPageV3}`,
  the Dealer arm, `unwired`). Refusal codes 357 → 344, exactly the two
  retired bands' 13. Unclassified positions: 0.
- Commit 4 and this note are unsigned: 1Password refused the signing prompt
  unattended, which is the honest signal that the lane worked while ember was
  away.
