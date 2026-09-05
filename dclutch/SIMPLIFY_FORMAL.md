# SIMPLIFY-FORMAL -- `formal/dclutch-semantics`

Lane: SIMPLIFY-FORMAL, branch `simplify/formal`, worktree off `main` at
`330bbfaba`, 2026-09-04. Every figure below is measured on this worktree at
its HEAD; every deletion names its control.

## Counts

| | before (`main` 330bbfaba) | after |
|---|---:|---:|
| library modules (`DClutchSemantics/*.lean`) | 143 | 138 |
| library lines | 60,311 | 59,353 |
| emitters (`Emit*.lean`) | 105 | 95 |
| emitter lines | 8,917 | 8,551 |
| emitter helper definitions deleted for `RustEmit`'s | -- | 158, from 76 emitters |
| `lakefile.toml` | 423 lines, 105 `[[lean_exe]]` | 11 lines, one library |
| root module | hand-kept list of 133 of 143 | generated, all 138 |
| Lean-emitted vectors | 6 (2 with no consumer) | 4, each naming its consumer |
| guards comparing raw emitter stdout | 10 | 1 (a binary corpus) |
| generated files with a guard | 101 of 101 (census stale on `main`) | 101 of 101, census regenerated |
| `formal/` directories that are not the library | 3 (`qedsvm-*`) | 0 (under `docs/evidence/`) |
| package `.lean` lines in total | 69,362 | 68,049 |

## Modules deleted (6), with the control

| module | why | control |
|---|---|---|
| `ClaimSbfProfile`, `SbfProfile` + `EmitClaimSbfProfile` | printed `generated_profile.rs` for the DCLTCAT1 claims-proof program, banished in `11ca28bab` | no file in `crates/`, `programs/`, `tools/`, `apps/` names any emitted symbol; `SbfProfile` had one importer, `ClaimSbfProfile`, which had one, the emitter |
| `ProductPayoff`, `ProductPayoffAbi`, `ProductPayoffExamples` | the V1 payoff record `DCLTPAY1`; every crate speaks `DCLTPAY2`/`DCLTPAY3` | `grep DCLTPAY crates programs` finds only `PAY2`/`PAY3`; nothing imports the V1 module but its own ABI and examples; `LiabilityBasisV2` imports `ProductPayoffV2`, which stays |
| `AccountProfileV2Profile13` | fifty-six lines saying four numbers are zero, "mirroring" a Rust decoder it printed nothing for | zero importers; Rust owns the check (`fractional-claim-operator/artifacts_v4.rs`) |

## Emitters and vectors deleted (3 + 2)

`EmitClaimVector`, `EmitCustodyVector` and the 72/40-byte hex vectors: their
only Rust consumer (`dclutch-direct-contract`) was banished 2026-08-27, which
`vectors/MANIFEST.md` already said. The theorems about those plans are in
`Physical`, imported by six modules, untouched.

## Emitters unified: 13 -> 6, one per record

Each prints every target its record has, selected by argument; `rust` is the
default so a guard that names no target keeps its meaning.

| emitter | targets | replaced |
|---|---|---|
| `EmitRealmPositionAbi.lean` | rust, ts | `...AbiRust`, `...AbiTs` |
| `EmitRefusalBandsV1.lean` | rust, ts | `...V1Rust`, `...V1Ts` |
| `EmitRationalTerminalHotV3.lean` | rust, ts | `...V3Rust`, `...V3Ts` |
| `EmitCapabilityManifestV1Abi.lean` | rust, ts | `...AbiRust`, `...AbiTs` |
| `EmitProtocolInfrastructureProfileAbi.lean` | rust, ts | `...ProfileAbiRust`, `EmitProtocolInfrastructureTs` |
| `EmitRegisteredDirect.lean` | lifecycle, controller, ts | `EmitDirectLifecycleAbiRust`, `EmitRegisteredControllerAbiRust`, `EmitRegisteredDirectTs` |

Control: each of the thirteen emissions, rustfmt-normalised where its guard
normalises, is byte-identical to the file committed on `main` once that
file's provenance line is rewritten to the new emitter name; the nineteen
generated files were re-emitted and differ from `main` in that line only.
Guards rewired (four `check-generated.sh`, one cargo test, `lean-emit.mjs` in
both packages, twelve `abi:*` package scripts): four shell guards PASS, twelve
`npm run abi:*:verify` PASS.

## Promotion: `DCGREQ02` gets a Lean author

`GeneralControllerRequestV2.lean` was a schema nothing printed while
`general-codec/successor_request_v2.rs` hand-stated the same magic, version,
width and seven offsets. `EmitGeneralControllerRequestV3Rust.lean` now prints
both generations into the module it already owned, and the V2 codec reads
every one of those facts from it (the five `pub const CONTROLLER_REQUEST_*_V2`
names other crates import survive as aliases). Control: the V3 half of the
emission is byte-identical; `cargo check --tests` in `dclutch-general-codec`
green. The map's §4 item 3 (unify the two wires) is the General maker's; this
is the half that has one author either way.

## Restated theorems

- `AccountProfileV2Profile14.profile_coordinates_are_exact` -- seven literal
  definitions restated as a conjunction and closed by `native_decide`: deleted.
  The emission is the pin.
- `GeneralControllerRequestV3.selector_and_settlement_prefix_match_v2` --
  compared V3's offsets to the literals 10/11/16/56/60; now compares them to
  V2's layout, which is the claim it was named for.

A heuristic scan (a theorem whose every conjunct equates a literal-valued
definition to its own literal) found only the first. The `coordinates layout
= [...]` witnesses in the ABI modules are not restatements: the left side is
computed by `specialize` and the right side is the intended public ABI.

## Structure

- `RustEmit.lean` is the Rust twin of `TsEmit`: `rustByte`, `rustBytes`,
  `emitBytes`, `emitBytesSkip`, `emitBytesRows`, `emitRustBytes`, `emitSlice`,
  `emitSliceSkip`, `emitConst`. 158 hand-copied helper definitions in 76
  emitters were deleted where the body matched a canonical one exactly, or a
  variant that rustfmt normalisation makes equivalent (every Rust guard now
  normalises); three call-site renames distinguish the `#[rustfmt::skip]`
  shapes; each emitter opens only the names it uses. It imports nothing and
  `Codec` imports it, so every guard -- including the two that build one
  module in a clean archive -- has it built; the one migrated emitter whose
  module does not reach `Codec` (`EmitSeriesTicketStateV3Rust`) keeps its
  local helpers, and so does `EmitProductRepresentationV3AbiRust`, whose
  108-column magic line rustfmt leaves alone in single-line form and reflows
  in three-line form. `Codec.hexDigit`/`byteHex` delegate to it: one author of
  the hex rendering.
  Control: a harness (in this lane's scratchpad; `tools/ci/run.sh emission`
  is the tree's) ran every emitter for every target against its committed
  file -- rustfmt-normalised for Rust, raw for TypeScript and the vectors --
  and ran the three corpus emitters: **108 of 108 rows identical**, after the
  first run found eleven (nine ambiguity errors from a blanket `open`, one
  `hexByte` reference, the product_v3 line above) and each was repaired.
- `lakefile.toml`: the 105 `[[lean_exe]]` stanzas deleted. No guard, script
  or test outside this package ran `lake exe`; all run
  `lake env lean --run Emit<X>.lean`. `vectors/MANIFEST.md` now states the
  working command.
- `DClutchSemantics.lean`: the alphabetical glob of the directory with the
  one-liner that regenerates it; ten modules `main` did not list are listed.
- `README.md`: 154 lines of accreted narrative ("the current 22,584-byte
  claim-owner ELF") replaced by 66 lines of what is here and how a record is
  owned.
- `docs/evidence/qedsvm-*`: the three captured lifts moved out of `formal/`
  (map §1.6); the two seam-audit baseline rows keyed by the old path go with
  them; `seam_audit.py` exits 0.

## Guards

Nine raw guards normalise (six cargo tests, custody-sbf, fractional-claim,
protocol-parameters -- the last had arrived on `main` raw with a committed
file rustfmt does not leave alone; it is formatted and `--fixpoint` reads
hazards=0). The tenth, `tests/capability_funding_header_v2.rs`, normalises
too -- ba2c0d6b1's message says it never ran the emitter, which was wrong: the
freshness test sits at line 139, past where that commit stopped reading; the
duplicate shell row it added was withdrawn in 156566066. The eleventh raw
row, `direct-translation-validator/check.sh`, compares a binary corpus and
stays raw.

## Left deliberately, and why

- **`SeriesEscrowV3`** (91 lines, zero importers): a revision table
  `series-v3-kernel/src/escrow.rs` states by hand for six effects while the
  Lean has seven (`consumeIntoHoard`, which Rust has never had). Deleted, then
  restored: the map's §1.7 keeps every `Series*.lean`. Recommendation for the
  Series owner: either the Rust gains the effect and the table is emitted, or
  the module goes.
- **`DealerScenarioCollateral`** (202 lines, zero importers): the 2026-08-25
  scenario model; `DealerScenarioSolvency` has the same `Scenario` plus
  obligations, the same coverage and conservation theorems, and a corpus the
  kernel replays. Restored for the same reason (the `DealerScenario*` chain
  waits on ember's batch-spine ruling). Delete with that ruling.
- **`ProductBasisV3Agreement`**: read as a candidate; it is a genuine bridge
  theorem between two Lean evaluators, not a restatement. Kept.
- **`EconomicKernel`/`EconomicExamples`/`EconomicCodec`/`EmitEconomicVectors`
  and `vectors/economic-kernel-v1.txt`**: the map (§1.4c) finds
  `dclutch-economic-kernel` has no consumer. `EconomicKernel.lean` stays
  regardless (`DealerLiquidity` imports it); the vector and its emitter go
  with the crate if the crates maker deletes it.
- **Emitted constants nobody reads** (census in this lane, symbols of each
  generated file joined against every `.rs`/`.ts`/`.sh`/`.py` outside
  `generated/`): `generated_runtime_wire_v2.rs` 90 of 183 (the whole
  `CANDIDATE_*` layout), `generated_registered_fill_v4.rs` 23 of 138,
  `generated_layout.rs` 21 of 34, `generated.rs` (market-core) 17 of 92,
  `generated_relayed_abi.rs` 14 of 134, `generated_direct_program.rs` 12 of
  60 (the `CLAIM_*`/`CUSTODY_*` plan templates of the vectors deleted here).
  No generated FILE has zero consumers. Trimming these changes files Rust
  lanes are editing today; left for the convergence lane with the numbers.
- **One emitter per record family across generations** where the generations
  are separate generated files (`AccountProfile` x3, `CapabilityProgram` x5,
  `DirectCodec` x7, `MarketCore` x6, `Source` x8, `LiabilityBasisV2` x3, ...):
  the file names are pinned by the Rust makers; merging means merging the
  crates' `mod` lists. Stated, not done.
- **`ScoringRuleV1`'s two `sorry`s** stay, stated with reasons.
- **The map's five "records with separate Rust and TS emitters"** lists
  `DirectProgram`; there is no TS `DirectProgram` emitter. The sixth pair
  found by reading was `ProtocolInfrastructure`, and `RegisteredDirect` was a
  triple.

## Not touched

No `*Abi.lean` field, magic, width or reserved span changed (cohort-17's).
No mechanism module changed. Nothing in §1.7's survive list was deleted.
