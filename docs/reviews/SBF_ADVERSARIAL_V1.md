# Dragon's Clutch SBF defensive correctness review V1

Status: **STOP for deployment or value-bearing use.** The adapter has a strong
hostile-account skeleton, and the recorded SVM runs establish several real
Token-2022 and rollback properties. They do not yet establish a permissionless,
source-authenticated, complete market lifecycle. In particular, deposited
free cash has no withdrawal instruction, the source plane is unauthenticated,
and the venue's reservation and settlement plane is incomplete. The
materialized-claim exit is now runtime-promoted only for the narrow bearer path
stated in SBF-V1-003.

Review date: 2026-08-18. The original review baseline is commit
`858f4086ff3ed76d71c10d39ce7240eeea2a9ad1`. The pooled-custody disposition was
re-audited independently at commit
`4e06710e40098df004c90a1da8e3e617b2171e92`. Bearer-token truth and
market-local construction were delta-reviewed through integrated commit
`fa166fb`. The signed committed-validator evidence at `aadc0cd` and its
promotion record at `3a0e45b` were then delta-reviewed; all evidence is accepted
only at the scope stated below. Later uncommitted harness changes are excluded.
This review used only local source, local tests, the checked-in local-bank
evidence, and the loopback harness. It used no public RPC, wallet, credential,
network target, deployment, or external action.

## 1. Severity and claim boundary

- **P0 / release stop**: a valid local transition can globally obstruct an
  economically required transition, select a payout without authenticated
  source facts, or leave a valid claimant without a redemption path.
- **P1 / construction stop**: the program cannot create or connect the state
  required for the documented lifecycle, or an implemented instruction is not
  joined to the value/reservation plane its name implies.
- **P2 / proof and hardening gate**: the present code fails closed or the field
  is non-load-bearing, but the state model, test oracle, or claim language must
  be repaired before promotion.

Equivalence is not correctness. Much of the SBF program deliberately mirrors
`clutch-solana-reference`; the differential proves agreement between those two
implementations. It cannot disprove a lifecycle omission shared by both, such
as the original absence of external redemption, or an environment assumption
shared by their fixtures, such as pre-created program-owned PDAs. The current
bearer cutover temporarily removes rather than replaces large differentials, so
green host counts now carry an even narrower claim.

## 2. Release-stop findings

### SBF-V1-001 — unsolicited Hoard inflow disabled collateral transitions

**Disposition:** **FIXED for direct-donation liveness at `4e06710`; preserve as
a regression.** The original P0 counterexample was confirmed by a real
Token-2022 program in the local Agave bank.

At the reviewed baseline, every collateral instruction requires exact equality:

```text
HoardAccount.collateral_atoms == hoard_token.amount
```

That equality is not stable for a permissionless SPL token account. A transfer
into a token account does not require the destination authority's signature.
Any holder of the Realm collateral can therefore increase the Hoard token
balance without changing `HoardAccount::collateral_atoms`. The next `Split`,
`Merge`, or `RedeemInternal` reaches `validate_collateral_leg` and refuses
`HoardMirrorMismatch`. No instruction reconciles or removes the surplus.

This is not hypothetical. The existing SVM regression
`no_wallet_signature_can_take_collateral_out_of_the_hoard` first transfers five
atoms directly into the Hoard, proves that a wallet cannot take them back out,
and then proves that the next protocol `Split` refuses `Custom(0x001d)`. The
authority check works; exact-equality liveness does not.

Commit `4e06710` replaces equality with the one-sided locked-backing check
`hoard_token.amount >= HoardAccount.collateral_atoms`. `Endow` is now the exact
inbound Token-2022 transfer; Split, Merge, and internal redemption are
token-neutral reclassifications. The real-bank test now transfers five atoms
directly into the Hoard, confirms that wallet-authorized outflow still refuses,
then successfully performs Split while the five atoms remain unowned surplus.
The same suite confirms undercoverage refuses.

This closes the direct-donation denial of service. It does **not** close pooled
cash ownership or exit: the one-sided check names only locked backing, not the
sum of every position's free and reserved cash. That remaining stop is
SBF-V1-006.

**Regression gate:** preserve the current real-bank donation-then-Split case and
extend it through Withdraw and both redemption paths. The surplus must remain
unowned, and no exit may consume another position's free cash, reserved cash,
or locked principal.

### SBF-V1-002 — an ordinary holder burn globally desynchronizes one outcome

**Disposition:** **FIXED for demonstrated direct-burn liveness at `054b2f6`;
preserve and broaden the regression.** The original P0 market-liveness failure
was confirmed by a real Token-2022 program in the local Agave bank. Commits
`e67c315` and `e55ff2f` implement the whole-vector repair across the seam,
Resolve, and internal/external redemption paths; `054b2f6` migrates the real-SVM
token plane and proves the direct-burn case live.

At the original baseline, outcome mints are ordinary extension-free Token-2022
mints, so an account owner can burn a materialized Egg without going through
Dragon's Clutch. The mint supply falls while
`SupplyLedgerAccount::external_supply` and the kernel aggregate do not. The next
materialize or dematerialize instruction for that outcome refuses
`ShadowSupplyMismatch`, and no repair transition exists.

The existing SVM regression
`a_supply_that_drifted_outside_the_program_is_refused` demonstrates the exact
state at the custody baseline. It contradicted the project-level statement that
direct holder burns are safe liability donations; the interim cutover is the
first code that implements that statement.

The selected policy in `e67c315` is to make authenticated Token-2022 mint supply
authoritative. The old external field becomes a last-observed supply cache. A
current supply above cache refuses; a current supply below cache is recognized
as irreversible holder forfeiture, lowering that outcome's recognized claim
liability and kernel total while leaving Hoard token amount and locked backing
unchanged. The difference is conservative overcollateralization with no sweep
right. Whole-vector synchronization prevents a stale positive-weight outcome
from being omitted when liability is computed. This accounting direction is
sound: the burn cannot increase required collateral or create owner cash.

Commit `e55ff2f` makes Resolve and internal redemption consume the same
authoritative whole-mint vector. At `054b2f6`, the real bank materializes an
Egg, burns it directly through Token-2022, synchronizes the lower supply on an
unrelated Materialize, and proves the market proceeds. It also refuses an
incomplete mint vector without movement and retains hostile-extension
refusals. This closes the observed liveness counterexample.

Commit `fa166fb` restores meaningful host boundary coverage for maximum outcome
count, incomplete suffix, malformed mint, wrong runtime owner, executable mint,
wrong authority, swapped suffix, and exact mutability. Historical owner-shadow
state-transition differentials remain quarantined, and not every replacement
case runs through the real bank. That residual is a P2 promotion gap, not a
reason to keep the original P0 marked open.

**Regression gate:** materialize and directly burn one Egg through the real
Token-2022 program, then successfully Split/Merge/materialize/dematerialize,
resolve, and execute both redemption paths as applicable. Prove actual supply,
observed cache, internal aggregate, kernel total, required collateral, locked
backing, and Hoard amount obey the documented equations; prove an actual supply
increase refuses atomically. Restore malformed-account and differential coverage
for the new complete mint-vector plane rather than counting disabled legacy
tests.

### SBF-V1-003 — materialized claims lacked a redemption path

**Disposition:** **FIXED and runtime-promoted for the demonstrated transferred-
holder, one-hot finite-preset path at `aadc0cd`; preserve broader bearer
hardening as P2.** P0 claimant liveness at the original baseline. Commit
`e67c315` routes a positionless `RedeemExternal`, `e55ff2f` joins authoritative
bearer supply to Resolve/RedeemInternal, and the signed committed-validator walk
now executes the exit against the exact audited ELF.

At the original baseline, the program implements `RedeemInternal` only and has
no `RedeemExternal`. `Dematerialize` runs through the active-market kernel path
and is unavailable after resolution. A winning Egg that remains materialized
when the market resolves therefore cannot be burned for its collateral.

The lifecycle harness deliberately terminates with three materialized winning
claims and three collateral atoms still in the Hoard. It calls that an exact
coverage identity, which is true as a solvency statement, but it is not a
closed lifecycle: the holder has no instruction that consumes those claims and
releases those atoms. This violates the product sentence "After a frozen
observation program identifies the realized cell, that Egg redeems" and the E3
engineering gate that calls for both external and internal redemption.

Transferability makes the omission wider. The custody-baseline SVM suite
intentionally transfers outcome tokens to a second wallet while leaving the
original position's external shadow unchanged. A position-bound external
redemption would not be sufficient; redemption must be authorized by possession
of the actual token account, work for arbitrary holders, burn exactly the
redeemed quantity, update market-wide supply exactly once, and pay the token
holder's collateral account under the Hoard PDA.

The new account plane is directionally correct: claimant signature plus
current source-token ownership authorizes an exact burn; no Position or
originating-owner shadow participates; the Hoard PDA transfers the immutable
payout to a claimant-owned collateral account; and token consumption supplies
replay safety. It constructs the kernel redemption as a finite-preset market,
which matches the current SBF account codec. Any future derived-basis codec must
cut this path over explicitly rather than inheriting that assumption.

The committed walk supplies the missing runtime disposition. It materializes a
winning Egg, transfers it through Token-2022 to an independent holder with no
Clutch Position or Replay account, then resolves the market. A transaction
containing two identical external redemptions refuses the duplicate with
`Custom(0x001c)` and restores all eighteen watched accounts byte-for-byte. A
subsequent valid redemption burns the holder's remaining three Eggs, pays three
collateral atoms, and exactly reloads the source and destination token accounts,
outcome mint, Hoard state and token account, SupplyLedger, kernel, and unrelated
second-owner state. This closes the original stranded-bearer counterexample for
that path; it does not constitute a hostile-role matrix, an independent
state-transition differential, or evidence for a future derived payout mode.

**Regression gate:** preserve the signed transferred-holder redemption and
duplicate-exit rollback. Broaden it across every admitted payout mode and mutate
every suffix mint/order, claimant/source/destination binding,
program/owner/extension role, payout lot, and late-CPI failure; every refusal
must restore all watched bytes. Any derived-basis codec must earn a new runtime
gate rather than inheriting the finite-preset result.

### SBF-V1-004 — deterministic resolution is not source-authenticated

**Severity:** P0 resolution authority if made reachable; currently also a P1
reachability stop.

`FeedAdvance` accepts a page from any signer. It validates byte shape,
contiguity, interval arithmetic, feed identity, and replay, but no deployed
source program, source account, publisher/quorum proof, cluster clock, or
canonical record. `FeedAccount::summary` is caller-supplied, recorded verbatim,
and never read.

`Resolve` then folds a separate caller-supplied window buffer. The buffer is
checked against the terms' domain and the feed's cursor, but not against the
pages that advanced that cursor or a cryptographic commitment to their accepted
records. Its `window_id` is a nonzero caller label, not a recomputed identity.
Consequently, the current plane proves that a claimed record sequence is
internally well formed and mature relative to a cursor; it does not prove that
the records came from the market's frozen external source.

The committed `program/src/source.rs` correctly says this itself: it defines a
useful future source-admission relation, has no production parser, and is not
called by `FeedAdvance`. That module is design progress, not a live authority
boundary.

**Required join:** immutable terms must bind a concrete audited adapter release,
parser release, source program and account, deployment generation, asset
orientation/scale, clock/finality/freshness policy, confidence policy, grid, and
canonical record-selection rule. `FeedAdvance` must authenticate those runtime
facts and persist a commitment that `Resolve` actually consumes. Maturity must
come from authenticated cluster/source time, not from a cursor that the same
unauthenticated page stream can advance.

**Regression gate:** two syntactically valid observation sets for the same
window must not both be admissible. Mutating source account, owner program,
deployment generation, parser version, sequence, publish slot/time, finality,
confidence, grid, or archived-page commitment must refuse before feed state or
resolution state changes.

## 3. Construction-stop findings

### SBF-V1-005 — the green harness depends on state no Solana instruction creates

**Disposition:** **PARTIALLY FIXED at `d67f5af`.** Market-local core construction
and a second wallet's generation-zero Position/Replay are real-SBF executed.
The full blank-bank lifecycle remains a P1 end-to-end reachability and harness
claim-boundary stop.

At the original baseline, `CreateMarket` creates the outcome mints and Hoard
token account but requires eight canonical state PDAs to arrive already
allocated, program-owned, correctly sized, writable, and all-zero. A wallet
cannot create a PDA account because it cannot sign for the PDA; only this
program can do so.

Commit `d67f5af` closes that gap for the accepted bearer-token ABI. One
`CreateMarket` now System-CPI-creates seven absent canonical targets—Market,
Hoard, founding Position, kernel, replay, supply, and resolution—then creates
the Token-2022 plane. There is no legacy ExternalAccount. A real in-process bank
committed the transaction at 888,587 CU, checked runtime ownership, rent
exemption, and codec readback, proved idempotence, and proved that a late
occupied-mint refusal rolls all earlier state and token construction back to
absence.

The first backed `Endow` for a second authenticated wallet likewise creates its
absent generation-zero Position/Replay pair. The real bank proves unauthorized
creation refuses before allocation, a late token overdraw rolls both System
CPIs back to absence, and the admitted deposit commits at 248,131 CU. No later
generation/reopen ABI exists yet.

The same construction gap remains elsewhere:

- no instruction creates a Feed head or an Epoch;
- the resolution and feed-page buffers are required to be program-owned, but no
  instruction creates or populates them;
- `InitOrderPage` exists, but it requires an Epoch that cannot be initialized;
- candidate, checkpoint, and related settlement state have no public lifecycle;
  and
- Realm/Profile initialization is host-tested only, while PriceGrid and Terms
  require caller-supplied artifact accounts that an ordinary wallet cannot
  produce under the current wire. The Terms body exceeds one Solana packet and
  no staged upload/finalization protocol exists.

The original local harness succeeds by installing these accounts directly in
validator genesis. `LIFECYCLE_WALK.md` records 195 injected genesis accounts,
including 131 program-owned accounts, and uses a different market
nonce/prestate for most steps because `simulateTransaction` does not commit.
The new narrow SVM cases supersede that claim for the market-local core only;
they do not make the old whole lifecycle permissionless.

The signed committed-bank runner now executes rather than merely generates its
same-address plan. At `aadc0cd`, it commits twenty sequential signed and
confirmed transactions against one market identity, exercises absent-state
CreateMarket, ordinary Token-2022 account construction and transfer, second-
owner Position/Replay construction, and the positionless external exit. It
reloads eighteen watched accounts, observes two exact expected refusals, and a
deliberately corrupted terminal expectation makes the fresh rerun fail. This is
meaningfully stronger lifecycle evidence, but the plan still genesis-injects
eleven Realm/Profile/Terms/feed/policy/buffer/page prerequisites, uses separate
advance-feed and matured resolve-feed fixtures, omits SettlePage, and has no
Withdraw. Its honest label remains **GENESIS-ASSISTED / NOT END-TO-END**.

**Required gate:** start a blank local bank containing only the program, system
programs/sysvars, token program, a funded payer, and chosen collateral mint.
Using only public Dragon's Clutch instructions, commit one sequential lifecycle:
Realm, Profile, grid, terms, feed, market, at least two Position triples, epoch,
pages, deposits, trading, observation, resolution, internal redemption, and
external redemption. No program-owned account may be injected at genesis.

### SBF-V1-006 — backed `Endow` landed, but owned pooled cash has no exit

**Disposition:** the original unbacked-credit defect is **FIXED at `4e06710`**.
Cash withdrawal and reservation discipline remain a **P0 value-exit release
stop**.

At the original baseline, the four-account `Endow` let a Position owner increase
`cash_atoms` without moving collateral. Commit `4e06710` replaced it with an
eleven-account instruction that authenticates the Market, Hoard, Profile,
content-bound collateral policy, pinned Token-2022 program, collateral mint,
owner source account, and canonical Hoard destination. It computes the ledger
post-state before CPI, transfers exactly `q` from the signer-owned source to the
Hoard, checks exact debit and credit, and only then writes Position cash and
replay. A real-bank insufficient-funds case leaves both ledger accounts and both
token accounts byte-identical. The Hoard authority field is now initialized to
the actual dedicated signing PDA. Commit `d67f5af` extends the plane to thirteen
accounts with System/Rent roles so a first deposit can create an absent owner
plane atomically; existing-owner deposits use the same fixed list.

The accepted economic equation is now:

```text
C = L + F + R_cash + U

C       actual Hoard Token-2022 balance
L       locked complete-set collateral
F       sum of positions' unreserved free cash
R_cash  sum of positions' reserved cash
U       unsolicited unowned surplus, U >= 0
```

The current layout stores only `L` market-wide. `F` and `R_cash` live in
individual positions, and `U` is an inferred residual. That can be an inductive
model only if **every** cash-changing path is closed and independently tested.
Today it is not: no routed `Withdraw` returns unreserved free cash, placement
reserves nothing, cancellation releases nothing, and settlement remains
unimplemented. A user may make a real deposit that the program cannot return.

**Required gate:** implement exact Hoard-to-owner `Withdraw` for unreserved
cash, define reservation and release before order settlement becomes reachable,
and prove that Split, Merge, internal redemption, reservation, release,
withdrawal, and external redemption preserve the equation without consuming
`U` as owned funds. The blank-bank committed lifecycle must end with no stranded
free or reserved cash.

There is also an important host-oracle boundary. The public Rust helper
`genesis::apply_endow` mutates only Position/replay bytes; it cannot observe or
move Token-2022 collateral. It is useful for testing the ledger half but is
**not an Endow value oracle** and must never be cited as backing evidence. Only
the routed thirteen-account `genesis::process` path plus real-bank token pre/post
observations establish a deposit. Host differentials must label `apply_endow`
as ledger-only or compare the full instruction against an independent value
model.

### SBF-V1-007 — order admission is not joined to assets and settlement is a stub

**Severity:** P1 venue completeness.

`PlaceOrder` takes only actor, Epoch, price grid, and order page. It authenticates
the actor against the record and validates the page/grid, but carries no
Position, cash/claim reservation, generation account, Hoard, or token account.
Thus the recorded order is not evidence that its owner can settle. `CancelOrder`
correctly retires that record, while `SettlePage` always returns
`NotYetImplemented`.

No value loss is reachable through the stub, which is the correct fail-closed
behavior. The venue is nevertheless not yet an executable trading venue, and
the host batch model cannot supply authorization that the SBF account plane
never persisted.

**Required gate:** placement atomically reserves exact owner cash/claims under a
per-owner generation; candidate verification binds the exact frozen page set;
settlement consumes paired buy/sell fill allowances at most once, transfers
exact consideration and claims, and releases only unused reservation. Every
late failure must be rollback-tested in the real bank.

### SBF-V1-008 — global resolution consumes one owner's replay sequence

**Severity:** P1 replay-domain mismatch.

Resolution is permissionless and market-global, but its inherited evidence
plane still includes one Position/replay pair after the external-shadow cutover.
Any signer with valid evidence can select a valid owner's pair and increment that owner's replay
sequence even though resolution does not change the owner's balances. This can
invalidate that owner's pending instruction and makes a global terminal event
depend on an unrelated per-owner nonce domain.

The resolution PDA and lifecycle already make the global transition
idempotent. Resolve should use a market-scoped sequence if an additional nonce
is needed; it should not consume a Position nonce. Internal redemption remains
an owner-scoped action and should keep owner authorization and replay.

## 4. Non-load-bearing inconsistencies to close

### SBF-V1-009 — baseline Hoard authority field named the wrong PDA

**Disposition:** **FIXED at `4e06710`; preserve as an initialization and
instruction-time binding regression.**

At the reviewed baseline, market initialization stores the Hoard state PDA's
address in `HoardAccount.authority`, while the real Hoard token account is owned
by the separate `hoard_authority_pda`. Collateral CPIs correctly derive and use
the latter, and do not trust the stored field, so this is not presently an
authority bypass. It is nevertheless two meanings for one persisted word.
Commit `4e06710` stores the dedicated signing PDA and Endow checks the stored
value against its recomputation before admitting the Hoard token account.

### SBF-V1-010 — clock and provenance fields are placeholders

`MarketAccount::created_slot` and `ResolutionAccount::resolved_slot` are written
as zero. The comments correctly forbid treating them as time, but a later
consumer could easily do so. Source finality and market deadlines require a
real Clock boundary before these fields or any slot-based policy become
load-bearing.

The current refusal-code fallbacks also map future unmatched codec/kernel
variants to catch-all ordinals. This fails closed, but the release manifest
should require an exhaustive refusal mapping for the pinned dependency digest.

## 5. Checks that held under review

No signer/owner/PDA/program-id substitution was found in the implemented seam
and internal-redemption paths:

- exact account counts and pairwise key distinctness run before state mutation;
- program-owned state roles check owner, executable bit, exact writability, and
  exact length;
- state addresses are recomputed from canonical seeds and stored bumps are
  compared where the frozen layout carries them;
- Split/Merge/Materialize/Dematerialize bind actor signature, Position owner,
  intent market/owner, canonical bearer-token address where applicable, and
  the complete ordered mint suffix;
- Token-2022's program key and executable bit are pinned;
- collateral policy bytes are content-bound to the frozen Profile before they
  name a mint or extension set;
- collateral mints/accounts and outcome mints/accounts are re-admitted from
  runtime owner and bytes at instruction time;
- outcome mint/burn CPIs and the Endow collateral transfer use the expected
  actor or canonical PDA authority and require exact observed deltas;
- pooled-custody Split, Merge, and internal redemption require exact zero token
  deltas while changing only their documented ledger terms;
- the market-wide two-term supply ledger closes against kernel supply before
  and after seam/internal-redemption transitions;
- replay increments are checked and overflow-refusing; and
- the real-bank refused-Endow regression proves that insufficient token funds
  cannot credit cash or consume replay. It is a pre-write refusal-atomicity
  test, not a general proof of post-CPI rollback.

These are meaningful properties. They should be preserved while changing the
economic state model; replacing exact equality with a coverage relation, for
example, must not weaken exact CPI deltas or canonical authority checks.

## 6. Reproduction record

The original/custody-baseline committed evidence records the following relevant
cases (the current bearer ABI needs replacements, as recorded below):

```sh
# Builds the ELF and runs against the real Token-2022 program in a local bank.
programs/clutch-sbf/svm-tests/run_svm_tests.sh \
  no_wallet_signature_can_take_collateral_out_of_the_hoard

programs/clutch-sbf/svm-tests/run_svm_tests.sh \
  a_supply_that_drifted_outside_the_program_is_refused

# Reproducible ELF, loopback validator differential, and lifecycle narrative.
programs/clutch-sbf/scripts/run_bringup.sh
```

At commit `858f408`, the checked-in evidence and handoff record both SVM gates
green, including 15 real-bank token tests, 10 accepting loopback transactions,
22 expected refusals, and an 11-step lifecycle narrative. Read those results
with the claim boundaries above: the validator genesis supplies the otherwise
uncreatable state, most lifecycle steps are simulated on oracle-generated
prestates rather than committed sequentially, and the terminal state still has
unredeemable external claims.

The exact custody commit `4e06710` was checked in an isolated detached worktree,
not against later shared-tree edits:

```sh
cargo test --manifest-path programs/clutch-sbf/Cargo.toml -p clutch-sbf --lib
programs/clutch-sbf/svm-tests/run_svm_tests.sh
```

The result was 156/156 SBF unit tests, 10/10 collateral-plane real-bank tests,
and 6/6 outcome-token real-bank tests. The latter directly covered exact backed
Endow, insufficient-funds refusal atomicity, donation-then-Split liveness,
undercoverage refusal, token-neutral Split/Merge/internal redemption, exact
outcome mint/burn, hostile extensions, and out-of-band outcome-supply drift.
The locally built ELF was
`9d4812607dc5a678905c29f3c5a766571b3fc2c06b23c4b4f7bcf9aa3c038ba1`.

The SBF compiler still emitted stack-frame diagnostics in dependency functions
during that build. A prior artifact audit showed how to prove whether such
diagnosed functions survive final LTO. That audit has now been rerun at
`fa166fb`: fixed-Cargo-home builds produced deterministic ELF
`98cac8a1e48f629f15d0efbf6295b2c96df5296f6acf6cec28ca76491da4b391`
(549,000 bytes); all nine backend stack diagnostics/eight named symbols had zero
final-LTO survivors; final ELF shape, reviewed syscall surface, loader headroom,
and deepest valid `r10` offset of exactly 4,096 passed. Relocating the Cargo-home
path still changes the hash because dependency source paths are embedded, so
this is fixed-path reproducibility, not path-independent or cross-machine
reproducibility.

At integrated commit `d67f5af`, this reviewer separately ran the SBF host suite:

```text
118 passed; 0 failed
```

That smaller count is not equivalent to the 156-test custody baseline: the
legacy owner-shadow seam and evidence differentials are deliberately compiled
out while replacement bearer-plane differentials are unwritten. The new
claim-truth suffix tests are useful but do not replace the removed matrix.

Commit `fa166fb` raises the host suite to 120/120 and supplies the explicit
bearer mint-vector boundary mutations listed in SBF-V1-002. This materially
narrows the gap, but remains host boundary testing rather than an independent
full state-transition oracle.

Immediately after `d67f5af`, the full real-SVM command built the ELF but the test
crate failed to compile because `tests/token_leg.rs` still imported removed
owner-shadow symbols. Commit `054b2f6` replaces that stale plane. This reviewer
reran:

```sh
programs/clutch-sbf/svm-tests/run_svm_tests.sh
```

It built ELF
`9b7f3bc6d052cb8778dc17ab2fccc6d08c6f3837fe1873535aed179e0f5d29e7`,
then passed 12/12 collateral-plane and 6/6 bearer-token tests. This validates
the direct-burn liveness and core-construction cases described above. It does
not itself exercise `RedeemExternal`, the full hostile bearer account matrix,
or a replacement reference differential. The separate signed-validator walk
below supplies the narrow external-exit runtime evidence; the broader two gaps
remain P2 hardening work.

The exact-current `fa166fb` artifact was then rerun through the same complete
suite: 12/12 collateral and 6/6 bearer-token tests passed with ELF `98cac8a1…`;
the direct-burn synchronization plus unrelated live transition used 91,624 CU.
The focused extra `fa166fb` hostile-vector cases remain host-level.

Commit `aadc0cd` drives that exact audited ELF through the loopback-only signed
committed runner:

```sh
programs/clutch-sbf/scripts/run_committed.sh
```

The clean-source run commits twenty signed and confirmed same-market
transactions, observes two exact refusals, and reloads eighteen watched
accounts. Its terminal sequence transfers the winning Egg to a holder with no
Position/Replay, proves a duplicated `RedeemExternal` rolls all watched bytes
back, then executes an exact positionless bearer redemption. Corrupting one
terminal expected byte makes a fresh run fail, establishing basic harness
falsifiability. The run remains genesis-assisted by eleven prerequisite
accounts and retains 61 plus 6 atoms as owner cash because Withdraw is absent;
it is not a blank-bank or complete venue lifecycle.

## 7. Minimum next gate, in dependency order

1. Implement unreserved cash withdrawal and define exact cash/claim reservation
   and release while preserving
   `C = L + F + R_cash + U`; keep `U` unowned.
2. Preserve the green direct-burn and positionless-redemption runtime cases;
   complete the replacement bearer-plane differential and hostile-account
   campaign, and give every future derived payout mode its own runtime gate.
3. Define any later Position generation/reopen lifecycle separately.
4. Add public artifact transport plus Feed/Epoch/page/candidate/checkpoint
   constructors and remove genesis injection from the promotion gate.
5. Wire a concrete authenticated source adapter and Clock into FeedAdvance;
   bind Resolve to the persisted accepted-history commitment.
6. Split market-global resolution replay from owner-global redemption replay.
7. Join order placement to reservations and implement streaming settlement.
8. Run one committed, multi-owner, blank-genesis lifecycle through process
   restart, then re-run malformed-account, duplicate-role, late-CPI rollback,
   replay, direct-transfer, direct-burn, and source-substitution campaigns.

Only after those gates pass should the SBF program be called a deployable
protocol rather than a valuable processor and runtime bring-up.

## 8. Custody re-audit disposition

Commit `4e06710` is internally coherent for the transitions it implements:

```text
Endow(q):       actor token -q, Hoard token +q, free cash +q
Split(q):       free cash -q, locked backing +q, token delta 0
Merge(q):       locked backing -q, free cash +q, token delta 0
RedeemInternal: winning internal claim -q, locked backing -p,
                free cash +p, token delta 0
donation(q):    Hoard token +q, unowned surplus +q
```

The original donation grief and unbacked Endow defects are closed by code and
real-bank regressions. The Hoard authority mismatch is also corrected. The
remaining custody STOP is not a subtle counterexample: the protocol deliberately
implements the inbound half before the outbound half. `Withdraw` does not exist,
reservation is not joined to orders, and the full market-wide cash equation is
not locally represented.

Two truth-boundary cautions remain until later lanes close them:

1. `genesis::apply_endow` is a host-only ledger transition even though its name
   sounds like the full instruction. It can produce cash bytes with no token
   transfer. Only the routed thirteen-account SBF path is value-bearing
   evidence.
2. Legacy compiled-out seam/evidence fixtures and some historical prose still
   use owner-shadow vocabulary. `054b2f6` migrates the executable `token_leg`
   suite and `0a01b0a` gives its live tests bearer-truth names. Replacement
   evidence must continue to describe authoritative mint supply rather than
   citing deleted per-owner state.

The real-bank Endow evidence covers the existing-owner admitted path and
insufficient-funds refusal, plus second-owner creation, unauthorized creation,
and late-overdraw rollback after two System CPIs. The thirteen-role
hostile-account matrix is still inferred from reviewed shared validators and
analogous seam tests, not independently mutated role-by-role through Endow in
the bank. Promotion should add exact owner, signer, PDA, program-id, mint,
token-account authority, extension,
writability, duplication, and policy-substitution refusals with byte-identical
pre/post assertions.

Account construction is narrowly executed as described in SBF-V1-005. Bearer
truth now has real-SVM direct-burn evidence, closing SBF-V1-002's original
liveness counterexample. The signed committed-validator walk closes
SBF-V1-003's original positionless-exit counterexample for the demonstrated
transferred-holder, one-hot finite-preset path, including late duplicate-exit
rollback. A full hostile bearer matrix and independent replacement
state-transition differential do not yet exist, although focused host
account-boundary mutations do. Those P2 evidence gaps—and the independent
Withdraw, source-authentication, genesis-assistance, resolution-replay, and
order-reservation/settlement STOPs—remain stronger release criteria than raw
host test count or committed-run plan length.
