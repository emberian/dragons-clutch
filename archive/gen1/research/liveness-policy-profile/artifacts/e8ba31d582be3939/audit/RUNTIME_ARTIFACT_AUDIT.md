# Dragon's Clutch current default-ELF convergence audit

Status: **artifact and first-party SBF stack audit PASS** for exact clean source
commit `2dbc9fc7a8dbbc6e20c23e2aa44069d2c180aa0c`. This is local build,
static artifact, and linked in-process-bank evidence. It is not a cross-host
reproducibility, deployment, release, RPC, cluster, formal-verification, or
production-source-provider claim.

## Source boundary — in place, per the build-path protocol amendment

- **Canonical build location:** `/Users/ember/dev/dragons-clutch` itself.
  Per `docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md` the ELF identity is
  same-path-reproducible only (Cargo folds the absolute workspace path into
  every path-dependency's `-C metadata`, and hash-sorted symbol ties order by
  those hashes), so the canonical identity is defined at the canonical
  checkout path, where every bank log's fixture binds. Before each build the
  tree was verified exactly HEAD: `git status --porcelain` empty and
  `git diff HEAD` empty. No detached worktree is used for the canonical
  build; the one cross-path build below is the relocation probe.
- Exact Git archive: 24,012,800 bytes, SHA-256
  `7aa7695740402d229c8dfacecb3d878a5adde44f66295e6e9518da10737f2b20`
- Declared SBF closure: **108 files**, SHA-256
  `f56ee103539fc5c45e88183a34a60b415c5958a04b4c1a37c1137bfe9c250353`
- `2dbc9fc` is `6e4702a` (the T2-8 entitlement/settlement merge, which
  followed the T2-7 selection merge `8fe5f9e`) plus three closure-neutral
  commits: the build-path root-cause note and protocol amendment
  (`e754a67`, docs only), one GOAL.md log commit (`5cbfbd8`), and the
  audit-gate syscall review below (`2dbc9fc`,
  `programs/clutch-sbf/audit/audit_artifact.sh` only — outside the declared
  closure). Nothing in the SBF closure differs from the T2-8 merge.
- The closure grows 106 → 108 files against the `d6929549…` seal's
  declaration: the two additions are exactly
  `programs/clutch-sbf/program/src/instructions/orders_batch/selection.rs`
  (T2-7) and
  `programs/clutch-sbf/program/src/instructions/orders_batch/entitlement.rs`
  (T2-8). The declared `source_paths` themselves are unchanged — no path
  entered or left the closure declaration.

## Toolchain and reproducibility

- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, SBF `rustc 1.89.0`
- Anza release commit `549805f3e85f345c9df98d59759691443eef57aa`
- Pass 1/pass 2 stripped ELF, both built in place with fresh
  `CARGO_TARGET_DIR`s: byte-identical SHA-256
  `e8ba31d582be3939c7ee41db3372af0068df7dafead1c779c9de1cfefdd2d9dc`,
  1,914,432 bytes
- Pass 1/pass 2 unstripped ELF: byte-identical SHA-256
  `d188ae6dbe7d3619c54374602d3141c1036e26b10e88c8ace47155c5cb0d84e4`,
  2,087,824 bytes

### The two relocation probes

**Cross-path build probe: disposition `PATH_TIED_SYMBOL_ORDER`, observed
byte-identical.** One build of the same commit in a detached worktree at
`/Users/ember/jobs/dragons-clutch-r1-2dbc9fc-xpath-worktree` (fresh target
directory, same recipe) produced a stripped ELF byte-identical to the
canonical `e8ba31d5…`. The path-dependence mechanism is present and
measurable — the *unstripped* cross-path ELF differs
(`e510689fdbb657a5cca0167d78da33aa00b260f52ad12274f889ece5a83ac40a` vs the
canonical `d188ae6d…`, the per-path `-C metadata` hash suffixes living in
the symbol table that stripping removes) — but no hash-sorted layout tie
survives in this artifact, so the stripped bytes coincide at this path
pair. This observation does **not** reinstate a path-independence claim:
per the root-cause note, a future tie would again make the stripped bytes
a function of the checkout path, so the disposition stays
`PATH_TIED_SYMBOL_ORDER` and the canonical identity stays defined at the
canonical path.

**Relocated-Cargo-home probe: byte-identical (INDEPENDENT) at this seal.**
The relocated-home build reproduced exact `e8ba31d5…`. This supersedes the
`d6929549…` seal's PATH_SENSITIVE finding for *that* artifact (three
registry-crate panic `Location` strings rendered as absolute paths under a
relocated home); the present artifact's codegen embeds no registry path
string under either home, evidenced by byte-identity. As before, this is a
single-host observation, not a cross-host claim.

The build ran locally on an Apple M2 Max under macOS 26.6.1. GPU was
unused. No network, RPC, signing, deployment, submission, or
external-state mutation occurred.

## Final-LTO and direct-frame gate

- 36 backend diagnostic lines naming 28 unique symbols (same counts as the
  `d6929549…` seal; a different symbol-hash set at the new metadata).
- Zero diagnostic symbols are first-party `clutch_sbf` symbols.
- Zero diagnosed symbols survive final LTO.
- Foreign diagnostics by lines/symbols: `clutch_batch` 11/8,
  `clutch_batch_policy_identity` 18/13, `clutch_solana_layout` 2/2, and
  `clutch_solana_reference` 5/5. Every one is nonresident.
- 976 resident text symbols at 974 addresses; all 974 addresses were
  disassembled.
- 58,099 direct `r10` references; maximum offset 4,096; zero invalid
  positive, zero, or greater-than-4,096 references. The deepest direct
  reference sits in `claim_truth::observe_outcome_mints`.
- 883 first-party resident function regions are enumerated with their exact
  direct-reference count and maximum in `first-party-frame-audit.txt`
  (retained evidence).

**T2-7/T2-8 handler frame verification.** The six new intents (tags
54–59) land with boxed single-decode `#[inline(never)]` helpers by design;
their measured maxima in this exact artifact:

| function | max direct `r10` | references |
| --- | ---: | ---: |
| `orders_batch::selection::submit_candidate` (tag 54) | 4,096 | 115 |
| `orders_batch::selection::write_candidate_feed` (tag 55) | 768 | 58 |
| `orders_batch::selection::seal_candidate` (tag 56) | 4,096 | 233 |
| `orders_batch::selection::finalize_selection` (tag 57) | 4,096 | 172 |
| `orders_batch::entitlement::freeze_entitlement` (tag 58) | 4,096 | 166 |
| `orders_batch::entitlement::entitle_slice` (tag 59) | 4,096 | 169 |
| `entitlement::entitle_single_slice` | 4,096 | 84 |
| `entitlement::entitle_portfolio_pair` | 4,096 | 163 |
| `entitlement::settle_portfolio_pair` (SettlePage, pair half) | 4,096 | 308 |
| `settlement::prepare_entitled_direct_slice` | 112 | 63 |
| `selection::seal/selection scoring helpers` (`candidate_selection_score`) | 4,096 | 107 |
| `selection::supersede_displaced` | 4,096 | 72 |
| `entitlement::rebuild_entitlement` | 4,096 | 108 |
| `entitlement::create_receipt` | 4,096 | 60 |
| `orders_batch::decode_candidate_boxed` / `decode_receipt_boxed` | 344 / 224 | 2 / 2 |
| boxed zero-receipt/zero-post constructors | 0 | 0 |

Every T2-7/T2-8 handler and helper is at or under the 4,096-byte line and
the backend emits no diagnostic for any of them. The widened `settle_page`
(tag 43, now carrying the entitled direct-slice and portfolio full-pair
shapes) measures 4,096/185. The T2-6 walk handlers persist under the line
(`general_epoch::init_epoch` 4,096/393, `freeze_epoch` 1,264/336,
`advance_clear_work` 4,096/247, `advance_clear_slices` 4,096/64,
`complete_clear_work` 4,096/91), as do the Tier 0 restructurings of the
ten former opt-z overflowers (`place_order` 4,096/422, `recorded_redeem`
4,096/364, `prepare_direct_v4_economics` 4,096/236,
`authenticate_settlement` 1,432/423, `prepare_selection_commit` 4,096/200,
`resolve_global` 4,096/620, `apply_native_market_resolution` 4,096/270,
`commit_observed_supplies` 1,080/67, `apply_legacy_market_resolution`
4,096/261, `settle_page` above) and the T2-3 staged-creation handlers
(`create_first_stage` 4,096/206, `init_clear_work` 4,096/53,
`finish_creation` 4,096/8, `write_grow_stage` 4,096/5, `grow_clear_work`
328/58).

The backend-survivor check is authoritative alongside these direct
offsets; an offset at or below 4,096 alone is not evidence that a
nested-call warning is safe.

## ELF shape and the reviewed tenth import

ELF shape passes: three load segments, no writable-executable segment,
1,738,176-byte `.text`, entrypoint `0xEB9D0`, and exactly ten undefined
imports: `abort`, `sol_invoke_signed_rust`, `sol_log_`, `sol_memcmp_`,
`sol_memcpy_`, `sol_memmove_`, `sol_memset_`, `sol_panic_`, `sol_sha256`,
and `sol_try_find_program_address`.

**`sol_memmove_` is the T2-8 wave's one addition to the syscall surface,
and it was refused before it was admitted.** The prior nine-symbol pin in
`audit_artifact.sh` failed the first audit run of this cycle exactly as
designed (the refusing console is retained as
`audit-console-refused-sol_memmove.log`). Review, then admission
(`2dbc9fc`): no first-party source calls it — LLVM lowers the potentially
overlapping copies of the portfolio full-pair seam to the `memmove`
intrinsic, with exactly two resident callers in the final disassembly
(`clutch_solana_layout::portfolio_settlement::prepare_full_pair` and
`orders_batch::entitlement::settle_portfolio_pair`), and the resident
4-instruction hidden `memmove` shim is the platform-tools
compiler-builtins wrapper over the syscall — the identical provenance of
the long-admitted `memcpy`/`memset`/`memcmp` shims, covered by the pinned
platform-tools release identity. `.dynstr`/`.dynsym` grow by exactly this
symbol (150 → 163 / 288 → 312 bytes); `.shstrtab` is unchanged.

The custom-heap entry region persists from T2-6: the entrypoint symbol
moved (`0xD9670` → `0xEB9D0`) with the wave's code growth and no further
undefined symbol entered with it. The walk suite re-measures the
`request_heap_frame(262144)` surcharge at exactly 150 CU on an otherwise
identical transaction (450 vs 300).

Loader-v3 Program/Buffer/ProgramData sizing is 36/1,914,469/1,914,477
bytes, with 8,571,283 bytes of data-length headroom.

## Exact comparison with the superseded `d6929549…` seal

This is a **materially different artifact**. The stripped ELF grows from
1,785,904 to 1,914,432 bytes (+128,528) and 851,048 byte positions differ
over the common prefix.

- `.text`: **different**, 1,614,088 → 1,738,176 bytes
- `.rodata`: **different**, 107,121 → 107,673 bytes
- `.data.rel.ro`: **different**, 18,848 → 19,976; `.rel.dyn`:
  **different**, 44,288 → 47,008; `.dynamic`: **different**
- `.dynstr` and `.dynsym`: **different** — the reviewed `sol_memmove_`
  addition above; `.shstrtab` identical
- Stripped-ELF instruction disassembly: **different**, 197,551 → 212,779
  instruction lines (both measured on the stripped artifacts here)

Exact section digests and both disassemblies are retained evidence
(`comparison-d692-vs-e8ba.txt`). No CU row, stack row, frame row, or
ELF-shape row from the `d6929549…` seal is carried forward; every current
row in the liveness profile was remeasured against exact `e8ba31d5…`.

## Dependency and same-ELF execution linkage

The closed graph remains 42 packages: 11 first-party, 30 verified
crates.io archives/unpacked trees, and one vendored package.
`dependencies.tsv` and `registry-source-verification.tsv` are
byte-identical to the `d6929549…` seal's — the T2-7/T2-8 wave changed no
external pin.

The staged bank fixture was verified as exact `e8ba31d5…` before every
suite. Current `2dbc9fc` tests pass — 15 suites, 77 tests: artifact 6/6,
blank-bank 2/2, funded orders 2/2, ResolutionWork 4/4, batched folds 2/2,
collateral 13/13, DirectSelectionV2 2/2, prefund/source gate 5/5,
source-archive host tests 9/9, native resolution 15/15, the three T2-6
walk suites general-epoch 3/3, clear-walk 3/3, and clear-lifecycle 2/2,
plus the two new suites: T2-7 candidate-selection 5/5 and T2-8
entitled-clearing 4/4 (the headline conservation test settles one entitled
direct slice and one portfolio full pair with position bytes equal to the
implied allocation and exact total cash/Egg conservation). Exact fixture
hashes per suite are recorded in `same-elf-bank-linkage.txt` (retained
evidence).

CU drift against the `d6929549…` seal is small on almost every measured
route: ResolutionWork rows moved +37 to +44 CU (at most +0.09%, on Abort),
batched folds +76 to +456 CU (at most +0.05%, at FoldBatch(12) =
929,561 CU), native/occupation rows −1 to +2 CU, Direct V2 rows −1 to
−4 CU, order placement +21/+21 and cancel +2/+2 CU, withdraw −1 CU. No
selected limit moves a quantum and no admission flips. **Three families
exceed the ±1% window and are noted honestly:**

- blank-bank `create_market` moved to 195,057/214,728/213,333 CU
  (−7.1%/+0.005%/+1.4% for v2/v3/v4). The creation flow has been the
  drift-heavy family since the custom-heap wave (+10.1% on v2 at the
  `d6929549…` seal; most of that reversed here with the same
  dispatch/genesis code motion); the byte-exactness and rollback
  assertions of the same suite gate its semantics unchanged, and no
  projection quote derives from the create_market rows.
- general-epoch `PlaceOrder (single)` measured 190,534 CU (−1.5%); the
  portfolio placement (+0.8%), InitEpoch (+0.4%), and every FreezeEpoch
  row (+0.2% to +0.4%) stay inside the window. The general-epoch family is
  UNPROMOTED evidence (below).
- clear-walk pass-1 slot observations moved up to +3.1% at the hottest
  slot (388,267 → 400,428 CU on the 11-reservation slot); the walk binds
  the same verdict and the family is UNPROMOTED evidence (below).

Two account rows genuinely moved or entered, re-derived by the sealed
offline probe at `2dbc9fc`:

- `epoch.window` is 231 bytes (rent 2,498,640 lamports): **EpochWindow v2**
  (T2-7) appends the candidate-window deadline, the frozen set's exact
  live cardinality, the bounded 3-slot retained-candidate registry, and
  the selection result to the v1 deadline pair — the format revision the
  v1 doc promised, not a second family. Nothing decodes v1 frames; no
  deployment ever wrote one.
- The T2-8 entitlement freeze creates two **new persistent general-plane
  families** at walk-plane PDAs, both classified post-probe with byte pins
  from the layout crate (the sealed probe enumerates the direct-plane
  shapes only): `epoch.final_pot`, one 262-byte `FinalPotAccount` per
  general epoch at `seeds::pot_pda` created `POT_PHASE_CLOSED` by
  `FreezeEntitlement` (tag 58) with provably zero scalars, and
  `epoch.receipt`, one 217-byte `SettlementReceiptAccount` per
  `(candidate, slice)` at `seeds::receipt_pda` created by `EntitleSlice`
  (tag 59), at most `MAX_SLICES = 416` per selected candidate. **No
  handler closes either family** — consumption stamps the receipt
  exhausted and archives the reservations `CONSUMED` in place —
  so both rows stand `UNCLASSIFIED_STOP` on the same TerminalClosure
  blocker family as the checkpoint and feed
  (`SETTLEMENT_BLOCKERS` in `orders_batch/settlement.rs`, which also
  records what T2-7/T2-8 retired: `CandidateWindowClosure` and
  `EntitlementFreeze`, alongside T2-5/T2-6's rows; `PartialFillLedger`,
  `VirtualPot`, and `TerminalClosure` stand).
- The T2-7 `CandidateFeedStage` prefix (account tag 25) is judged **not**
  a new inventory row: it is the same 6,266-byte account as the feed
  (tag 18) while its content is being written — a stage prefix instead of
  a feed header, sealed one-way by `SealCandidate` — so the
  `legacy.candidate_feed` row covers it.

The terminal projection grows to a 47-row inventory, same 14 blocking ids.

**Selection and entitlement are SBF-executed evidence only, deliberately
unpromoted, exactly like the walk.** The two new measurement families
(`candidate_selection`, `entitled_clearing`) seal the T2-7/T2-8 CU
evidence — SubmitCandidate 31,105–35,605; WriteCandidateFeed chunks
6,653–9,894; SealCandidate 43,426–64,170 (the displacing seal is the
64,170 four-candidate case); FinalizeSelection 49,230 (3 retained, 2
verified), 39,462 (digest tie), 20,695 (honest lapse at 0 verified);
FreezeEntitlement 100,052; EntitleSlice 204,577 (single) and 246,173
(portfolio pair, 2 receipts); SettlePage 54,834 (entitled direct slice)
and 225,739 (entitled portfolio full pair) — with their bank logs
(`candidate_selection.log`, `entitled_clearing.log`). No admission,
quote, or reward row is derived for any tag-49–59 route, no live flag
moves, and the reference adapter still refuses tags 49–59
(`UnsupportedIntent`): admission-policy treatment of the general clearing
plane is a decision for ember, not this seal. The `general_epoch` and
`clear_walk` families keep the same standing.

Default Endow still refuses with `0x79` and byte/lamport-exact rollback
because no production source release is registered. No CU value is
invented for that log, and no mock-feature ELF evidence is mixed here.

The Direct V3 lifecycle remains classified-but-unpromoted in the terminal
inventory: still no measured CU row and no sealed close/rollback bank
capture, so the V3 rows keep their `DIRECT.V3_CLOSE_EVIDENCE_UNSEALED`
and rent-persistence STOPs exactly as the V3 terminal-classification
merge recorded them.

## Retained evidence

- Artifact/build/audit/comparison package:
  `/Users/ember/jobs/dragons-clutch-r1-2dbc9fc-reseal-evidence/artifact-e8ba31d582be3939c7ee41db3372af0068df7dafead1c779c9de1cfefdd2d9dc`
- Audit console:
  `/Users/ember/jobs/dragons-clutch-r1-2dbc9fc-reseal-evidence/authoritative-audit-console.log`
  (and the first run's refusing console,
  `audit-console-refused-sol_memmove.log`)
- Source archive:
  `/Users/ember/jobs/dragons-clutch-r1-2dbc9fc-reseal-evidence/source-2dbc9fc-7aa7695740402d229c8dfacecb3d878a5adde44f66295e6e9518da10737f2b20.tar`
- First-party frame table: `first-party-frame-audit.txt`; environment and
  same-ELF linkage: `environment.txt` and `same-elf-bank-linkage.txt`;
  cross-path build log: `sbf-build-crosspath.log` (sealed in-root)
- Evidence checksum ledger: `SHA256SUMS` (mirrored here as
  `upstream-SHA256SUMS`)
