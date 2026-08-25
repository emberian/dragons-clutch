# Multiprogram ownership and footprint experiment — 2026-08-25

Status: engineering decision input; not release evidence and not a succession
claim.

## Result

A capability/multiprogram architecture is a credible successor to the current
monolithic adapter, but **not** as one program per historical action and not by
letting each venue own its own claims.  The narrow waist should be a small set
of state-owning executors plus semantic controllers:

1. immutable Registry/Core: Realm, Product, Market identity and lifecycle,
   finalized records, capability manifest, and activated release bindings;
2. canonical Claims/Replay executor: the sole owner of Position claims and
   reusable replay coordinates across trading venues;
3. one data-driven Trading controller for Direct, Dealer, General, and later
   Product-defined venues, with family semantics expressed as checked programs
   or descriptors instead of `N = 2..16` Rust monomorphizations;
4. Resolution controller: provider authentication and terminal outcome
   admission;
5. Realm-selected Custody adapter: the small SPL/Token-2022 physical boundary;
   and
6. optional wrapper/mint controllers only where Token-2022 ownership makes a
   separate program an actual authority boundary.

Source, Series, Direct, Dealer, and General do not automatically deserve five
programs.  Their semantic programs and account profiles can be immutable data
consumed by the Trading or Resolution controller.  Split by persisted-state
ownership and syscall trust boundary, not by instruction count.

The existing controller → claim executor → custody adapter → SPL Token campaign
is already a minimal buildable prototype of this shape.  It executes exact SBF
ELFs, authenticates the controller by an `invoke_signed` PDA, derives rather
than accepts its physical plans, performs two real Token CPIs, fits its complete
signed fill in a 990-byte v0 transaction, and demonstrates transaction-wide rollback
by byte comparison after a late custody refusal.  Its main unfinished
architectural gate is multi-controller release admission: its claim states are
currently bound to one experimental controller identity rather than one
Market-authorized release set.

## Exact footprint experiment

`tools/sbf-footprint.py` hostile-parses the ELF64 section table, hashes the
complete file, and calculates equivalent Loader V3 capitalization without
depending on the host's `size` implementation.  Its named default profile is:

- `Rent::default()`-equivalent 6,960 lamports per allocated byte;
- 128 bytes of account-storage overhead and an 890,880-lamport floor;
- one 36-byte Loader V3 Program account; and
- one ProgramData allocation with 45 metadata bytes plus the complete ELF.

Run:

```sh
tools/sbf-footprint.py \
  target/deploy/dclutch_claims_proof_sbf.so \
  target/deploy/dclutch_controller_proof_sbf.so \
  target/deploy/dclutch_custody_proof_sbf.so
```

The pinned successor Direct artifacts are:

| Program | ELF bytes | `.text` bytes | SHA-256 | Loader V3 capitalization |
|---|---:|---:|---|---:|
| canonical claim executor | 3,432 | 2,544 | `5878343447df3e4c703b1047f0fd4f9df890c74a28c410c738bd10d1c5358468` | 0.026232240 SOL |
| signed semantic controller | 79,520 | 74,072 | `fc96b90929281f129d5e465f9323ea107f59d2b50e363f0b5a68779d5c6baf5f` | 0.555804720 SOL |
| real custody adapter | 24,800 | 21,712 | `c4f9a6ac223639158fb3f40d40b1e59ac1c1e369ff0c3c9c0667c1658f787796` | 0.174953520 SOL |
| first-party total | 107,752 | 98,328 | — | 0.756990480 SOL |

The embedded checked transition program is 568 bytes, or 0.714% of the
controller ELF.  It is not the dominant footprint.

During this experiment the actively rebuilt integrated artifact was observed
as follows:

| Artifact | ELF bytes | `.text` | `.rodata` | `.data.rel.ro` | `.rel.dyn` | SHA-256 | Loader V3 capitalization |
|---|---:|---:|---:|---:|---:|---|---:|
| integrated working-tree SBF | 9,420,048 | 9,303,280 | 11,048 | 6,088 | 97,904 | `dbfc527d7b67f944d757e78ad4364c7bbe9b03b30d1fdd7e30e56c95c57ac5c3` | 65.565879600 SOL |

This exact working-tree observation is **not** release evidence: parallel work
was live and the source tree was dirty when it was built.  Its hash makes the
measurement unambiguous; the checked-in footprint utility makes the next clean
build reproducible.  The earlier pinned 9,771,616-byte integrated artifact and
68.012792880-SOL measurement remain documented separately.

The three-program Direct slice is 87.42 times smaller by ELF bytes and 86.61
times smaller by equivalent capitalization than the observed integrated
working-tree artifact.  This is intentionally **not a feature-parity ratio**:
the integrated artifact contains every current family while the successor
executes one signed Direct fill.  It establishes that the split can be tiny; it
does not establish that all successor families will remain tiny.

Splitting itself has little fixed rent cost.  Under this profile each additional
Loader V3 Program/ProgramData pair adds 2,345,520 lamports (0.002345520 SOL)
before its ELF bytes.  Duplicated SDK adapters and duplicated semantics, not
the Loader metadata, are the aggregate-rent risk.

## Where the monolith's text comes from

The symbol-bearing build paired with the observation had 9,303,280 bytes of
`.text`.  Summing symbol sizes by Rust namespace gives the following diagnostic
attribution:

| Namespace attribution | Function-symbol bytes |
|---|---:|
| `general` adapter + `dclutch_general_contract` | 3,570,280 |
| `dealer` adapter + `dclutch_dealer_contract` | 1,981,160 |
| `structured` adapter + `dclutch_structured_contract` | 892,032 |
| `bearer` adapter + `dclutch_bearer_contract` | 723,085 |
| `position` adapter only | 630,560 |
| `direct` adapter + `dclutch_direct_contract` | 290,568 |
| `source` adapter + `dclutch_source_contract` | 168,616 |
| `series` adapter + `dclutch_series_contract` | 158,592 |

These are not linker-removal deltas: shared functions, inlining, and contract
symbols can serve more than one route.  They do identify the dominant mechanism.
General dispatch instantiates every supported width, as do Dealer, Structured,
Bearer, and core Position operations.  The largest functions occur repeatedly
for `N = 2..16`.  This is width-specialized Rust code generation, not debug
information (`.text` alone is 98.76% of the stripped ELF) and not the 568-byte
interpreter.

The first code-size attack should therefore be semantic-width erasure:

- validate exact account widths and bounded outcome counts once;
- expose fixed-layout runtime views over canonical bytes;
- execute bounded loops or checked semantic bytecode over those views; and
- retain specialized syscall adapters only where the account/CPI surface truly
  differs.

This can be measured inside one program before any ownership split.  A program
boundary should then be accepted only when it reduces the proof/release surface
or corresponds to distinct state ownership.

## Required cross-program commitment

Pinning `CLAIM_PROGRAM_ID` and `CUSTODY_PROGRAM_ID` in Rust is sufficient for
the experiment and insufficient for protocol succession.  A canonical
`ExecutionReleaseSetV1` (name provisional) must be a content-addressed semantic
object selected by the immutable capability manifest.  It needs to bind:

- semantic release, Product/Frame schema, Effect schema, and account-profile
  identities;
- controller, claim, and custody Program IDs;
- each Loader V3 ProgramData ID, admitted deployment slot, complete ELF digest,
  and upgrade policy;
- controller-PDA seed domain and derivation version;
- exact permitted child-program/release roles; and
- theorem/evidence digests for the admitted translation and artifact paths.

Activation should authenticate the full checked release once and persist a
small Registry-owned binding.  Every executor call then checks the immutable
Market/manifest/release-set identity, the Registry owner, the controller
program, the controller PDA signer, and the current ProgramData deployment slot.
An upgrade changes that slot and must make the old binding refuse until a new
release is explicitly admitted.  Hashing an 80 KiB ELF on every fill is neither
necessary nor desirable.

The effect envelope must also bind `market`, `generation`, capability kind,
semantic release, action, pre-state coordinates, and one transition commitment.
The claim and custody plans derived from one transition carry the same
commitment.  Claim/replay accounts remain owned by the common executor rather
than a venue controller, so Direct, Dealer, and General cannot mint parallel
claim truths.  Solana's transaction rollback supplies atomicity across the
children; replay fields prevent reuse after a committed execution.

## CPI and account cost

The current successor's top-level controller frame has 18 accounts.  Its claim
CPI passes five semantic accounts (plus the executable program account to the
runtime), and its custody CPI passes seven before the custody adapter performs
two Token transfers.  The earlier registered monolithic Direct ordinary frame
had 21 account metas.  Multiprogram composition therefore does not inherently
increase the transaction's unique account set; child program identities and
release evidence do add keys, while removal of staging, System, Rent, or venue-
specific accounts can offset them.

The complete current successor succeeds in 59,037 CU and demonstrates late
cross-program rollback in 58,076 CU.  Its 990-byte v0 wire leaves 242 bytes of
the 1,232-byte packet limit.  A historical minimal controller + claim-child
campaign consumed 3,810 CU versus 110 CU for the standalone specialized claim
path; that 3,700-CU difference is a useful upper-bound observation for that
old controller/PDA/CPI envelope, not a general Solana CPI price and not a
comparison to the current controller.  A feature-matched monolith versus split
benchmark remains required before accepting the final partition.

## Translation-validation direction

The path should not become “Lean prints a large Rust program.”  Lean should own
the semantic objects—schemas, admission relation, transition program, effect
plans, and refinement theorems—and emit compact data plus adversarial corpora.
The executable bridge should be checked in layers:

1. Verus (or an equivalent Rust verifier) proves the safe, fixed-memory Rust
   decoder/interpreter refines its executable specification for every input in
   its bounded domain;
2. independent property-space translation validation compares Lean evaluation
   with the actual Rust implementation over generated admitted and hostile
   corpora, including arithmetic boundaries;
3. qedsvm or a successor checks exact ELF paths and CU bounds; and
4. Solana runtime campaigns cover CPI, ownership, loader, rollback, and real
   Token behavior outside the pure-language theorem.

Verus would reduce the Rust-source boundary; it would not by itself prove
`rustc`, LLVM, the SBF loader, or Agave.  Likewise, direct Lean-to-SBF would add
a compiler/runtime trust boundary unless its output is translation-validated.
The small IR plus exact-account executors make all of those bridges more
tractable than a 9 MB monolithic CFG.

## Decision and next experiment

Proceed with the multiprogram successor, subject to two nonnegotiable gates:

1. replace the experimental single-controller claim binding with a
   Market-authorized `ExecutionReleaseSetV1` and one canonical claim/replay
   owner; and
2. implement the same signed Direct ordinary transition both monolithically and
   through the split release set, then record exact CU, account keys, packet
   bytes, ELF/rent totals, and hostile rollback from one clean source commit.

In parallel, erase `N` from one large current family (General is the highest-
value probe) without changing program ownership.  If the runtime-view/IR build
collapses its attributed megabytes while preserving the hostile campaign, that
is direct evidence that the data-driven architecture will scale beyond the one
Direct prototype.  Only after those two experiments should the repository
delete the corresponding monolithic route.
