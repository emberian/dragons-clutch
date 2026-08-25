# Compiled signed Direct with canonical state — 2026-08-25

## Result

Source commit `8e8e26631877cc2d63a083f7cfb05058d5f43e77` executes one
inline ordinary Direct fill from two independently signed compact intents. The
caller supplies no claim or custody plan. A controller authenticates the native
Ed25519 instruction and exact current instruction, builds a fixed register
frame from runtime-owned facts, runs Lean-generated transition bytecode, and
derives both child plans from the successful output registers.

Replay and claims no longer share a pair-specific projection. The physical
claim child mutates exactly four canonical owners:

- seller execution-profile/generation/maker replay root;
- buyer execution-profile/generation/maker replay root;
- seller execution-profile/maker/outcome Position; and
- buyer execution-profile/maker/outcome Position.

This is local real-SVM evidence for exact artifacts, not mainnet evidence and
not a claim that the Solana program is formally verified.

## ABI convergence addendum

Commit `fa46a7817e55f1ed4ac16917bdb4128d6d7040fb` replaces the
controller and harness's parallel handwritten intent/profile codecs with the
shared safe, `no_std`, `no_alloc` `dclutch-direct-codec`. Its encoders match four
Lean-emitted vectors byte-for-byte; round-trip and hostile width, magic,
version, and reserved-byte tests pass.

Rebuilding that commit produces a 55,728-byte controller ELF with SHA-256
`659be91dc9694e921986d4c2dbfdbcfbc931b9a582c9ed255bb08453c5c58937`.
Equivalent Loader V3 capitalization is 0.390212400 SOL, and the three-program
first-party total is 0.591398160 SOL. This removes 320 ELF bytes and one parallel
ABI truth. The post-convergence real-SVM measurements are 11,320 CU for wrong
replay, 14,487 for wrong Position, 17,031 for an out-of-limit price, 39,527 for
success, and 34,066 for late rollback; native signature tampering still refuses
before controller execution. All other artifact hashes in the table below are
unchanged.

## Canonical-authority successor addendum

Commit `88fb859840f9448e22472b600be91a3f6da1c61b` deletes the
experimental execution-profile account and ABI. Each maker's unchanged
136-byte intent now signs the canonical Market key. The 304-byte controller
instruction remains unchanged in length.

The controller now authenticates the actual authority graph used by founding:

- the canonical, open Market PDA and its exact root identity;
- the immutable Realm selected by that Market, including the collateral mint,
  token program, adapter release, and mint/freeze-authority policies;
- the finalized capability manifest selected by that Market;
- the unique Direct capability entry and its semantic release, capacity,
  child-schema, derivation, activation, dependency, and funding coordinates;
  and
- the finalized venue fee policy selected by that capability entry.

Replay and Position PDAs are now namespaced by the Market key. A hostile case
substituting a valid same-shaped manifest from another Market refuses without
mutation. Product identity remains bound transitively through the immutable
Market identity; the controller reads operational outcome width from the
Market, whose owning protocol program remains responsible for full economic
state validation. Static clients do not become authorities.

An initial implementation decoded all 15 supported `CategoricalMarket<N>`
types. Rust monomorphization grew the controller to 120,552 bytes. The accepted
implementation instead validates the canonical Market header and exact encoded
width, then decodes the width-independent `MarketRoot` slice. This removes the
duplicated generic decoders without weakening the Market PDA, phase, Realm, or
manifest bindings. The rebuilt controller is 79,680 bytes with SHA-256
`acc6bcbf1078ec48aefa298a748632e004b485f501cc41f1f4a489d5b869da9c`.
Equivalent Loader V3 capitalization is 0.556918320 SOL; the three first-party
programs total 0.758104080 SOL. Removing the profile also removes its
0.001837440 SOL mutable-state rent requirement. Market, Realm, manifest, and
policy accounts are existing shared protocol records, not per-fill creations.

The successor real-SVM measurements are:

| Case | Result | CU |
|---|---|---:|
| direct controller-PDA impersonation | refused | 7 |
| valid signatures, wrong replay bump | refused without mutation | 30,806 |
| valid signatures, wrong Position bump | refused without mutation | 33,979 |
| same-shaped manifest from another Market | refused without mutation | 22,191 |
| matcher price below signed seller limit | refused without mutation | 36,545 |
| signed fee-rate byte tampered after signing | native Ed25519 refusal before controller | 0 |
| admitted compiled fill | committed | 59,067 |
| frozen fee destination after first Token CPI | full rollback | 58,106 |

The two-instruction signed fill serializes to 1,326 bytes as a legacy
transaction, above Solana's 1,232-byte packet limit. A table containing every
eligible account compresses the message to 804 bytes. The reusable production
shape stores only 12 Market-stable keys—controller, journal, child programs,
Market, Realm, policy, manifest, mint, fee destination, token program, and
instruction sysvar—and produces a 990-byte transaction. Maker replay roots,
Positions, and collateral accounts remain explicit in each message.

The operator canonically orders table contents, derives official create and
bounded extend instructions, proves each extension packet fits, decodes exact
finalized table bytes, refuses same-slot additions and deactivating or duplicate
tables, compiles an exact packet-safe v0 message, and plans authority-checked
deactivation, conservative cooldown, and close. Tables remain semantically
inert routing projections: the controller reauthenticates every loaded account.

The real-SVM campaign also executes that lifecycle through the official address
lookup table program. It creates the table at 10,517 CU, extends all 12
addresses at 9,304 CU, advances one slot for activation, commits the physical
Direct fill as an actual signed 990-byte v0 transaction, deactivates at 3,151
CU, advances the full 512-slot SlotHashes cooldown, and closes at 2,158 CU. The
table account is absent afterward. This is real ProgramTest/SVM evidence, not
an external `solana-test-validator`, devnet, or mainnet execution.

## External-validator transport addendum

Commit `3c695725361261af62353eef1978adcb9b50cadd` takes the same
canonical Direct transaction across a separate validator process and JSON-RPC
transport. The ignored harness test launches `solana-test-validator 4.0.2`,
loads the exact controller, claim, and custody ELFs as genesis SBF programs,
and uses the validator's canonical SPL Token and address lookup table programs.
The Rust RPC client is pinned to 4.2.1.

The test derives fixture state with the same Rust constructors used by the
ProgramTest campaign and imports those accounts at local genesis. Seller and
buyer test signers come from fixed test-only seeds. The transaction fee payer
is ephemeral and exists only in memory; its public account is capitalized in
the temporary local genesis. No wallet file, public RPC, devnet account, or
external faucet is read or used.

Over RPC, the campaign:

- reads the validator's actual `SlotHashes` sysvar and creates the official
  lookup table from a valid recent coordinate;
- extends it with the 12 canonical Market-stable keys and waits until those
  additions are active;
- signs and submits the exact 990-byte v0 Direct transaction;
- observes the journal and both replay roots advance from 0 to 1;
- observes seller and buyer claims move from 5,000/200 to 3,000/2,200;
- observes canonical SPL-token balances move from 2,000/100/20 to
  998/1,100/22; and
- deactivates the lookup table through the official program.

The temporary validator ledger is deleted when the child process exits. The
external-process campaign therefore does not wait roughly 205 seconds at the
default validator clock merely to close that disposable table. The independent
ProgramTest campaign above executes and checks the complete 512-slot cooldown
and close route.

Run the transport campaign explicitly:

```sh
SBF_OUT_DIR=$PWD/target/deploy \
SOLANA_TEST_VALIDATOR=/path/to/solana-test-validator \
cargo test --manifest-path crates/dclutch-svm-harness/Cargo.toml \
  --test physical_direct_composition \
  compiled_direct_crosses_the_local_validator_rpc_boundary \
  -- --ignored --nocapture
```

This is local validator transport and execution evidence. Genesis-imported
fixture accounts are not evidence for account-creation workflows, a checked
deployment, devnet behavior, mainnet behavior, or a complete Direct lifecycle.

## Admitted-frame refinement addendum

Commit `a4509f649884b96f4aec1d99203e4f5d193803a3` proves the Lean
theorem `DClutch.DirectProgram.admitted_program_refines`. For every `FillFrame`
with a witness of `DClutch.Direct.Admissible frame`, executing the generated
35-operation program in Lean's `DClutch.TransitionVM` succeeds and returns
exactly:

- the seller's next nonce plus one;
- the buyer's next nonce plus one;
- the frame's exact gross quote; and
- the frame's named floor fee.

`Admissible` supplies the phase, slot, side, Market/generation/outcome,
lifecycle, replay, price, fee-rate, exact-integer quote, balance, and `u64`
bounds used by the theorem. The output slots begin at zero; gross, fee, and
successor nonces are derived by the program rather than trusted as caller
registers. The proof factors the unchanged program into setup, admission,
replay, pricing, and balance stages and composes their execution theorems.

Commit `1d62b03e15516133d7337d04c166dfcaa1869c72` composes that result
with the multiprogram physical semantics. The new
`DClutch.Direct.CompiledPhysical.compilePhysicalPlan` materializes child plans
only after successful transition-program execution. It takes successor nonces,
gross, and fee from the output registers; only fill and selected outcome remain
frame inputs. The theorem
`admitted_compilation_refines_physical_transition` proves that every admitted
compilation yields the existing canonical four-effect claim plan and
two-transfer custody plan, both abstract child interpreters reach their specified
projections, and `atomicCommit` rejoins them to the high-level Direct
`postState`.

Commit `a50df9065989074195b721d93b4ba4268914885f` adds hostile Lean V1
decoders and general bounded round-trip theorems for Effect and custody plans.
`admitted_physical_wire_round_trip` instantiates both on the plans selected by
compiled Direct execution. Its additional premise names the physical V1 `u32`
outcome-coordinate bound; the semantic Product remains width-unbounded. Lean
also checks concrete refusals for truncation, excess count, reserved-byte
corruption, and a noncanonical non-claim coordinate.

The source builds with Lean 4.30.0. Regeneration still produces exactly 568
bytes with SHA-256
`72cc0faa6a9768b766a3003c8ff6f38889f564f49005ce68b2187c98349bff5c`,
and `lake exe emit-direct-program-rust` still reproduces the checked-in Rust
array byte-for-byte. The independent safe Rust VM's four vector, boundary,
hostile-program, hostile-frame, and rollback tests pass.

These theorems are high-level admission-to-abstract-VM evidence. They are not a
machine-checked refinement of physical account/register decoding, native
signature verification, the Rust VM, the controller or child SBF ELFs, CPI, or
Solana runtime behavior. They also do not prove the reverse implication that
every accepted abstract register frame came from the full `Admissible`
predicate. The physical composition theorem models typed plans and an abstract
atomic envelope. The new Lean parsers close its canonical typed-plan byte
round-trip, but do not prove refinement of the separate safe Rust or SBF
parsers, account ownership, CPI, or runtime rollback.

## Generated register and physical-plan ABI addendum

Commit `1d6d5741c599eb7264d7c4873754558517f8f06c` replaces Direct's
parallel numeric register vocabulary with typed Lean `ScalarSlot` and
`IdentitySlot` schemas. Lean proves that their explicit wire indices equal
constructor order, that their complete index lists are canonical ranges, and
that their generated Rust names are unique. The Rust generator now emits all
41 scalar and four identity indices alongside the transition program. The
controller consumes named output registers and ties its 34-value input prefix
length to the first Lean-owned output slot, so a changed input/output partition
cannot compile silently.

Commit `c744110f4694441721a1550bb93fc3ef85b7784f` also generates the
named scalar and identity input-frame macros from those schemas. The adapter
now binds each runtime fact as `seller_limit`, `buyer_next_nonce`,
`venue_collateral`, and so on; the generated macro alone owns their array
order. Changing, inserting, or reordering a semantic input changes the macro
contract and makes the handwritten adapter call fail compilation. Macro
expansion rebuilds the exact same controller ELF.

Commit `d07b0d732ba2f0686dc0cb9f60b7e68344500e2a` moves the physical
child-wire constants into the same generated artifact. Lean encodes the
complete 72-byte claim and 40-byte custody zero templates. Typed `ClaimPatch`
and `CustodyPatch` schemas name the dynamic spans; Lean proves that every span
is in bounds, that all spans are pairwise disjoint, and that their Rust names
are unique. The controller now patches only successor nonces, outcome, fill,
gross, and fee. It no longer owns duplicate plan magic, version, count, opcode,
party, resource, reserved-byte, or record-layout literals.

`example_materialization_matches_encoding` checks that applying the complete
patch sequence to the canonical example equals the ordinary typed Lean
encoders. This is not yet a general patch-materialization theorem. The Rust
patch function and each adapter expression's correctness remain unverified
boundaries, but register ordering is no longer a parallel Rust authority.

The generated transition program remains exactly 568 bytes with SHA-256
`72cc0faa6a9768b766a3003c8ff6f38889f564f49005ce68b2187c98349bff5c`.
The first register-only change reproduced the prior 79,680-byte controller ELF
byte-for-byte. The generated-plan change remains 79,680 bytes and produces
SHA-256
`56e1f1bbdc1335a6bc328edc0b0cb9d08231ab4fef44db5ca285f966f78720b5`.
Its ProgramTest measurements are unchanged: 59,067 CU for success and 58,106
CU for late rollback, with every named hostile refusal still passing. The same
990-byte v0 transaction also committed across a separate
`solana-test-validator` 4.0.2 JSON-RPC process. The result removes semantic
duplication; it does not claim a size or execution-cost reduction.

This addendum supersedes the execution-profile architecture and current
controller measurements below. Earlier tables remain evidence for their named
historical artifacts.

## Semantic and generated material

Lean 4.30.0 owns:

- the Direct admission relation, exact quote equation, cumulative floor-fee
  boundary, replay progression, limits, lifecycle, and conservation theorems;
- the 35-instruction, 568-byte `DCTV` admission/derivation program;
- the exact 136-byte signed intent and 304-byte controller-instruction
  encodings and length theorems; and
- the loader-v1 offsets, roles, state tags, and ordered effects for the
  five-account claim child.

The transition-program bytes have SHA-256
`72cc0faa6a9768b766a3003c8ff6f38889f564f49005ce68b2187c98349bff5c`.
`lake exe emit-direct-program-rust` exactly reproduces the embedded Rust array,
and `lake exe emit-claim-sbf-profile` exactly reproduces the claim child's Rust
profile constants.

The canonical Market identity binds the manifest-selected Direct semantic
release and finalized fee policy. Both makers also sign the exact accepted fee
rate. Removing the duplicated fee-policy identifier reduced the program from
37 instructions / 600 bytes to 35 instructions / 568 bytes without weakening
fee-rate admission.

## Build and artifacts

The first-party artifacts were rebuilt from the source commit with:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-claims-proof-sbf/Cargo.toml \
  --lto --optimize-size --sbf-out-dir target/deploy
cargo build-sbf \
  --manifest-path programs/dclutch-controller-proof-sbf/Cargo.toml \
  --lto --optimize-size --sbf-out-dir target/deploy
```

The build used cargo-build-sbf 4.0.0, platform-tools v1.53, SBF rustc 1.89.0,
and emitted no verifier diagnostic. Host tests used Rust 1.97.1 and
solana-program-test 4.2.1.

| Program | ELF bytes | SHA-256 | Equivalent Loader V3 capitalization |
|---|---:|---|---:|
| canonical claim executor | 3,432 | `5878343447df3e4c703b1047f0fd4f9df890c74a28c410c738bd10d1c5358468` | 0.026232240 SOL |
| signed compiled controller | 56,048 | `b960725cb5d151e30046b66fad1627bfe44c479f199a41fbcbb4b62b6b5cc1f8` | 0.392439600 SOL |
| real custody adapter | 24,800 | `c4f9a6ac223639158fb3f40d40b1e59ac1c1e369ff0c3c9c0667c1658f787796` | 0.174953520 SOL |
| first-party total | 84,280 | — | 0.593625360 SOL |
| official SPL Token 9.0.0 | 93,056 | `c85ce043abbfcb0363b5c724245caa9d9201d2a9b669c02a5c2770512b65d78f` | 0.650015280 SOL |

Capitalization uses `Rent::default()`, one 36-byte Loader V3 Program account
and 45 bytes of ProgramData metadata per program. The canonical legacy token
program is already deployed; its number is an equivalent local measurement,
not dClutch deployment capital. Transient buffers and transaction fees are
excluded.

The experimental mutable-state rent minima are:

| State | Count | Bytes each | Rent each | Total |
|---|---:|---:|---:|---:|
| maker replay root | 2 | 48 | 0.001224960 SOL | 0.002449920 SOL |
| maker/outcome Position | 2 | 56 | 0.001280640 SOL | 0.002561280 SOL |
| execution profile | 1 | 136 | 0.001837440 SOL | 0.001837440 SOL |
| controller journal | 1 | 16 | 0.001002240 SOL | 0.001002240 SOL |
| total | 6 | 360 | — | 0.007850880 SOL |

Replay roots are reusable across fills for one maker and profile generation;
Positions are reusable across counterparties. The table must not be interpreted
as a per-fill cost.

## Real-SVM campaign

The exact controller, claim, custody, and official SPL Token ELFs ran under
solana-program-test with SBF preferred. No native protocol processor or mock
token program was registered. The native Ed25519 precompile and runtime sysvars
provided signature and instruction evidence.

```sh
SBF_OUT_DIR=$PWD/target/deploy cargo test \
  --manifest-path crates/dclutch-svm-harness/Cargo.toml \
  --test physical_direct_composition -- --nocapture
```

| Case | Result | CU |
|---|---|---:|
| direct controller-PDA impersonation | refused | 7 |
| valid signatures, wrong replay bump | refused without mutation | 11,286 |
| valid signatures, wrong Position bump | refused without mutation | 14,457 |
| matcher price below signed seller limit | refused without mutation | 17,001 |
| signed fee-rate byte tampered after signing | native Ed25519 refusal before controller | 0 |
| admitted compiled fill | committed | 39,496 |
| frozen fee destination after first Token CPI | full rollback | 34,035 |

The committed fill advances both replay roots from 0 to 1, moves 2,000 selected
claims from seller to buyer, transfers 1,000 collateral units from buyer to
seller, transfers a floor fee of 2 to the venue, and clears the exact delegate
allowance. The late-refusal log contains a successful first official Token CPI;
the journal, both replay roots, both Positions, source, seller destination, and
venue destination nevertheless equal their pre-transaction bytes afterward.

## Boundary and next gates

The controller is now larger than the specialized children. The 568-byte
interpreter program is not the size problem: current Solana account, sysvar,
Ed25519-evidence, token-state, and CPI adapter code dominates the 56,048-byte
ELF. This measurement motivates generated codecs and descriptors, shared thin
adapters, and possibly a deliberately split authentication/execution program;
it does not justify reintroducing caller-authored plans or combined state.

Still open:

- immutable checked-release admission for the execution profile and controller
  artifact;
- Realm-derived collateral selection rather than the experimental profile's
  direct token bindings;
- generated safe-Rust and TypeScript codecs plus parser/refinement evidence;
- canonical prepaid account-creation and closure workflows;
- a new qedsvm lift for the canonical-owner claim artifact and broader
  machine-code coverage;
- Direct cancel, expiry, partial replay, retirement, and closure routes; and
- specialization of the same IR across the other capability families.

The earlier combined-projection claim artifact and its single-path qedsvm
theorem remain historical evidence only. They do not cover these bytes.
