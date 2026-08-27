# Decision report — `realm-admission-allowlist` (register A8 + F5, one report)

Status: **ANALYSIS FOR EMBER / DECIDES NOTHING.** Standalone report of the
2026-08-20 decision-register fan-out
(`docs/decisions/DECISION_REGISTER_2026-08-20.md:195-213` A8, `:830-850` F5;
the register's report list item 9 at `:1052-1054` directs that "the collateral
matrix, the adapter's recorded divergence, and the Token-2022 pin belong in
one report"). Claim vocabulary of `CURRENT_TRUTH.md` §1 governs. Every fact
below is cited from the tree; nothing here changes a byte of consensus code,
relaxes a gate, or promotes a claim. The register was checked for a coupling
to `internal-venue-ownership` (A7): there is none — A7 is listed
paragraph-sufficient (`DECISION_REGISTER_2026-08-20.md:1059-1063`) and enters
this report only as shape precedent (§7).

---

## 1. The two decisions, stated

**A8 — `realm-admission-allowlist-freeze`.** Freeze the V1 collateral-profile
allowlist: which collateral mints, token-program families, and Token-2022
extension sets a Realm may admit, and demonstrate generic semantics with two
synthetic Realms (`docs/OPEN_QUESTIONS.md:40-46`, a P0 row). The register's
options: (1) freeze the conservative matrix as implemented; (2) widen to
plain-SPL; (3) freeze after the F5 pin so allowlist and pin are one act.

**F5 — `token2022-pinned-elf`.** TOKEN2022_PLAN open decision 7, explicitly
unresolved: select and pin the exact Token-2022 program artifact
(`docs/implementation/TOKEN2022_PLAN.md:745-749`: "a program id is not a
pin"). Riding along for ratification: the decisions already taken
in-direction — checked-mirror `collateral_atoms` (#3), ImmutableOwner-required
(#4, stricter than the matrix, named as a divergence at
`TOKEN2022_PLAN.md:733-741`), outcome-mint decimals-0/freeze-`None` (#5),
no-ATA (#6).

### 1.1 What the tree has already built, precisely

The "allowlist" is not a proposal; it is executing code with a frozen shape.
Stating it exactly is most of the decision:

- **Program family: Token-2022 only, three independent gates.** The policy
  *codec* admits two families — legacy SPL Token with all-zero extension
  bitsets, and Token-2022 (`programs/solana-layout/src/collateral.rs:116-125`,
  `:355-363`) — but the executable adapter drives exactly one:
  `token::require_drivable_collateral` refuses any non-Token-2022 collateral
  with `WrongTokenProgram` at market initialization
  (`programs/clutch-sbf/program/src/token.rs:1381-1398`, test `:1907-1914`;
  called from `admit_collateral`,
  `programs/clutch-sbf/program/src/instructions/market_init.rs:350`);
  `validate_token_program` requires the presented token-program account to be
  exactly `TOKEN_2022_PROGRAM_ID` on every collateral instruction
  (`programs/clutch-sbf/program/src/instructions/split.rs:760-765`); and every
  CPI constructor hard-codes that program id
  (`token.rs:926,965,1085,1110,1119`). Independently, a legacy policy must
  declare *zero* account extensions (`collateral.rs:355-363`) while
  `CreateMarket` refuses a Realm whose policy does not admit `ImmutableOwner`
  on the Hoard (`market_init.rs:352-355`,
  `ClutchError::TokenAccountNotAdmitted`) — so a legacy Realm is doubly
  unreachable.
- **Mint extensions: none, as a protocol constant.**
  `PROTOCOL_MINT_EXTENSION_CEILING = 0` (`collateral.rs:112`); a Realm bitset
  above a ceiling refuses at decode (`collateral.rs:350-354`). All twenty
  mint-side matrix rows are refused categorically.
- **Account extensions: `ImmutableOwner` only.**
  `PROTOCOL_ACCOUNT_EXTENSION_CEILING = EXTENSION_IMMUTABLE_OWNER`
  (`collateral.rs:114`); all other account-side rows refused.
- **Realms narrow, never widen.** The 266-byte policy carries four per-Realm
  bitsets (offsets 218-249, `docs/implementation/COLLATERAL_PROFILES.md:40-43`)
  that intersect under the ceilings; unknown bit positions and unknown TLV
  discriminants fail closed (`collateral.rs:103-108`, `token.rs:441`), and a
  mint extension found on an account (or vice versa) is refused as
  mislocated (`token.rs:276-310`, the compile-time partition assert at
  `:307-310`).
- **Authority and state policy is fixed, not menu-selectable.** The flag word
  must equal `COLLATERAL_POLICY_STRICT_FLAGS` — mint authority absent, freeze
  authority absent, nonzero supply, Hoard delegate and close authority absent
  (`collateral.rs:80-97`, `:325-327`). Decimals exact, supply within the
  immutable ceiling, fee currency ∈ {collateral, native SOL}, liveness native
  SOL (`collateral.rs:328-335`).
- **The divergence:** the adapter *requires* `ImmutableOwner` on the Hoard
  where the matrix merely *allows* it (matrix row 7,
  `COLLATERAL_PROFILES.md:108`; taken as open decision 4,
  `TOKEN2022_PLAN.md:733-741`; enforced at `market_init.rs:310-319,352-355`).
- **What the SVM evidence pins:** the Token-2022 build all bank evidence runs
  against is `spl_token_2022-10.0.0.so`, sha256
  `a794161408080f690dac00832f45b3c3e2b71f1339586667ad1f979cf91d5b68`, 506,896
  bytes, installed at genesis by `solana-program-test` 4.2.1 from
  `solana-program-binaries` (`TOKEN2022_PLAN.md:227-234`), pinned transitively
  by `--locked` lockfiles in both harness workspaces
  (`programs/clutch-sbf/svm-tests/Cargo.toml:75`,
  `toolchain/probes/token2022/Cargo.toml:45`).
- **What production admits today: nothing.** The sealed default ELF refuses
  `Endow` with `SourceReleaseUnavailable` (`0x79`) because the production
  source registry is empty, and no real Realm profile is authenticated,
  frozen, or released (`CURRENT_TRUTH.md:292`, the A8 "blocked on it" cell).
  On any cluster the program would bind only the token program's *address*
  (`TOKEN_2022_PROGRAM`, `collateral.rs:122-125`) — whatever build resides
  there.

So the honest form of both decisions is **ratify-or-amend what is built**,
the same shape as A7 and F2: the code has already selected an answer, and
`docs/OPEN_QUESTIONS.md:3-4` forbids leaving a P0 row "intentionally
unresolved" once it has.

---

## 2. The extension matrix: structural hostility, with the tree's own refusal evidence

The matrix (`COLLATERAL_PROFILES.md:91-137`, pinned to Token-2022 source
commit `426400f`, discriminants 0..=28) refuses 28 of 29 rows. The refusals
are not one undifferentiated "be conservative": they divide into five
structural classes against the two properties the adapter's solvency story
needs — **conservation** (an atom deposited is an atom credited, checked as
exact deltas, `token.rs:887-903`) and **custody** (only the program's PDA can
move Hoard value, probe scenario 5).

**Class 1 — breaks conservation of transferred atoms.**

| ext | why hostile | tree's refusal evidence |
| --- | --- | --- |
| 1 `TransferFeeConfig` / 2 `TransferFeeAmount` | recipient receives net atoms; withheld sub-balance is spendable by the mint's withdraw authority, not the Hoard | **measured**: 1,000,000 atoms sent, 990,000 credited, 10,000 withheld (probe `transfer_fee_mint_is_refused_and_would_break_conservation`, `toolchain/probes/token2022/evidence/probe_run.txt`; `TOKEN2022_PLAN.md:315-318,334-340`); refused at market init (host row, `market_init.rs:1965-1991`) and at instruction time on a bank (`Custom(26)`/`0x001a`, `programs/clutch-sbf/svm-tests/tests/token_leg.rs:523-563`); **falsifiable**: the widened-profile counterfactual admits it and demonstrates the 20,000-atom shortfall on a 2,000,000 deposit (`TOKEN2022_PLAN.md:301-302,341-345`) |
| 28 `PermissionedBurn` | external approval alters ordinary burn/fungibility semantics | matrix row (`COLLATERAL_PROFILES.md:129`); refused by ceiling; exhaustive-corpus coverage (below) |

**Class 2 — breaks custody or seizure-resistance.**

| ext | why hostile | tree's refusal evidence |
| --- | --- | --- |
| 12 `PermanentDelegate` | a third party can transfer or burn any balance, including Hoard collateral | host row `market_init.rs:1974`; matrix `:113` |
| 6 `DefaultAccountState` | new accounts can default frozen; freeze authority updates the setting | host row `:1972`; matrix `:107` |
| 26 `Pausable` / 27 `PausableAccount` | an authority can pause transfers, minting, burning — discretionary liveness seizure | host row `:1976`; matrix `:127-128` |
| 3 `MintCloseAuthority` | a zero-supply mint can be closed and reinitialized with a *different* extension set — a mint address is not a stable description of behaviour, defeating init-time-only admission | probe `mint_close_authority_mint_is_refused` (`TOKEN2022_PLAN.md:306-307`); instruction-time on a bank (`token_leg.rs:532`); this row is *why* the refusal re-runs at every token instruction (`TOKEN2022_PLAN.md:528-541`; `market_init.rs:296-297`) |
| 11 `CpiGuard` | owner-toggleable; can make the adapter's intended CPI path fail | matrix `:112`; refused by ceiling |

**Class 3 — breaks the transparency the conservation check needs.**
4/5 `ConfidentialTransfer{Mint,Account}`, 16/17
`ConfidentialTransferFee{Config,Amount}`, 24 `ConfidentialMintBurn`: opaque
balances and encrypted withheld amounts make the plaintext exact-delta
verification (§3.3 step 6, `TOKEN2022_PLAN.md:505-509`) unevaluable — the
delta check is the *backstop against extensions the refusal misses*
(evidence item E6, `TOKEN2022_PLAN.md:694-697`, still open), so admitting
opacity would remove the second line of defence, not just the first. Matrix
rows `:105-106,117-118,125`; refused by ceiling; no live-mint probe (E4
covers rows "that can be constructed on a live mint", `:669-676`).

**Class 4 — breaks unit semantics.** 10 `InterestBearingConfig`, 25
`ScaledUiAmount`: mutable UI-versus-atom conversion invites exactly the unit
confusion the adapter excludes by speaking atoms only
(`TOKEN2022_PLAN.md:592-594`; matrix `:111,:126`).

**Class 5 — breaks transferability or executes foreign code.** 9
`NonTransferable` / 13 `NonTransferableAccount`: collateral cannot cycle in
and out of the Hoard (host row `:1973`). 14 `TransferHook` / 15
`TransferHookAccount`: transfer invokes a configured external program — the
**single reentrancy exception in Token-2022**, refused for that reason
(`TOKEN2022_PLAN.md:647-650`; host row `:1975`). 8 `MemoTransfer`:
owner-toggleable inbound requirement can make Hoard credits fail. The
metadata/group family (18-23) and 0 `Uninitialized` are refused as parse
surface with no conservation content (matrix `:101,:119-124`) — "not
collateral policy" rather than actively hostile.

**The one admission — 7 `ImmutableOwner`, account-side, and it is required
on the Hoard.** The Hoard's custody story is that its owner authority is a
program address; `SetAuthority(AccountOwner)` is the instruction that would
break that; `ImmutableOwner` is the extension that forbids it
(`market_init.rs:310-319`). Probe scenario 5 measured the story end-to-end:
a Hoard-shaped account with `ImmutableOwner` admits deposits and refuses a
wallet-signed withdrawal with `TokenError::OwnerMismatch` `Custom(4)`
(`TOKEN2022_PLAN.md:311-314,346-351`; re-demonstrated on the real ELF,
`token_leg.rs:565`).

**Coverage machinery behind the table:** all 29 discriminants have
exhaustive positive/negative coverage in the offline corpus
(`research/collateral-profiles`, `COLLATERAL_PROFILES.md:172-179`) and in
the Rust port's tests (`collateral.rs` addendum,
`COLLATERAL_PROFILES.md:255-292`); the on-chain decoder fails closed on a
future discriminant 29+ (`token.rs:441`; `EXTENSION_KNOWN_MASK`,
`collateral.rs:103-108`) — a Token-2022 release adding an extension makes
this program refuse, not shrug (`TOKEN2022_PLAN.md:544-548`).

---

## 3. Allowlist shape options, at the admission-code seams

The admission pipeline has exactly three seams where an allowlist could
live, and the tree already occupies all three:

1. **Protocol constants** — `PROTOCOL_MINT_EXTENSION_CEILING`,
   `PROTOCOL_ACCOUNT_EXTENSION_CEILING`, `COLLATERAL_POLICY_STRICT_FLAGS`
   (`collateral.rs:96-114`), compiled into the layout crate and enforced at
   policy decode (`:319-363`).
2. **Per-Realm declaration** — the four bitsets inside the 266 canonical
   bytes, digest-bound to the Profile identity
   (`COLLATERAL_PROFILES.md:29-66,189-249`), recomputed-not-trusted at every
   consumer (`verify_profile_identity`, `market_init.rs:343-345`;
   `genesis.rs:65-67` — `InitRealm`/`InitProfile` recompute both identities
   from the sealed canonical policy PDA).
3. **Adapter enforcement** — `MintPolicy::collateral` reads the policy's
   bitsets rather than restating the matrix (`token.rs:600-611`), and the
   check runs at market init (`market_init.rs:366,475,500`), at `Split`
   (`split.rs:847,911,933,937`), at `Endow` (`genesis.rs:1251,1273,1277`),
   at `WithdrawCash` (`cash_exit.rs:171,200,204`), and at every other token
   instruction (`market_init.rs:296-297`).

**Option (a) — frozen protocol ceiling + per-Realm narrowing declaration
[the built answer].** The ceiling owns safety; the Realm bitsets are a
declared-narrowing affordance. Today the mint ceiling is zero, so the only
legal mint bitset is zero and the per-Realm mint declaration is pure
future-proofing; on the account side a Realm chooses between 0 and bit 7 —
and choosing 0 makes the Realm unmarketable (`market_init.rs:352-355`), so
the effective V1 choice space is a single point. That is not a defect: the
bitsets are the *schema seam* through which a later, explicitly-specified
widening would flow without changing the 266-byte layout
(`COLLATERAL_PROFILES.md:131-137`: a widening requires a new schema or an
equally explicit compatibility decision — "not a frontend toggle").

**Option (b) — per-Realm free declaration (no protocol ceiling).** Rejected
by construction ("A Realm can narrow the protocol extension ceiling but
cannot expand it", `COLLATERAL_PROFILES.md:57-58`), and the tree carries the
measured argument: the probe's counterfactual profile that wrongly admits
`TransferFeeConfig` is *accepted* by the predicate and demonstrably loses
20,000 atoms of Hoard coverage (`TOKEN2022_PLAN.md:341-345`). A per-Realm
free choice would make every Realm's solvency story an audit of that Realm's
bitset rather than a protocol invariant.

**Option (c) — refuse-all-extensions-v1 (delete or zero-fix the bitsets).**
For mints this is byte-for-byte what option (a) already enforces. As a
schema act it would buy ~32 bytes of layout and cost: the account side
cannot be refuse-all (the Hoard *requires* `ImmutableOwner`; holder accounts
*allow* it, `token.rs:643-652` — note the requirement is Hoard-only, holder
accounts are never required to carry it); and re-cutting the 266-byte layout
forks every golden digest and parent Profile identity
(`COLLATERAL_PROFILES.md:62-66,218-223`), i.e. every frozen Realm identity,
for zero admission-behaviour change.

**The family question (the real content of "widen to plain-SPL").** The
built answer is Token-2022-only (§1.1's three gates). Widening would need:
policy-driven program ids in five CPI constructors, a parametrized
`validate_token_program`, a no-TLV legacy admission branch, duplicated bank
evidence — and it has a structural hole: **legacy SPL Token has no
extensions, so `ImmutableOwner`-required cannot be satisfied on a legacy
Hoard.** The custody story would regress from "the token program refuses
`SetAuthority(AccountOwner)`" to "the program promises never to sign one" —
one notch weaker, resting on adapter behaviour instead of token-program
structure. The DREGG dogfood profile binds the legacy program
(`COLLATERAL_PROFILES.md:157-163`), so under the built answer DREGG's
existing mint is policy-encodable but **not executable** — a consequence to
name out loud, not discover (§7).

---

## 4. The pinned-ELF question: what genesis installs vs what mainnet runs

Four different things currently answer "which Token-2022":

| surface | what it binds | where |
| --- | --- | --- |
| SVM evidence (probe + svm-tests) | `spl_token_2022-10.0.0.so`, sha256 `a7941614…5b68`, 506,896 B, installed at test genesis under the upgradeable loader | `TOKEN2022_PLAN.md:227-234`; lockfile-enforced via `solana-program-test =4.2.1` (`svm-tests/Cargo.toml:75`, `probes/token2022/Cargo.toml:45`) |
| recorded fallback (unused) | litesvm 0.15.2 bundles `spl_token_2022-11.0.0.so`, sha256 `495e9d76…3cad`, 615,936 B | `TOKEN2022_PLAN.md:282-286` |
| client library | `spl-token-2022-interface =3.1.1`, matching both the 10.0.0 and 11.0.0 ELFs (`spl-token-2022` 11.0.0 declares `^3.0.0`) | `TOKEN2022_PLAN.md:235-239` |
| the program itself | only the **address** `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` | `collateral.rs:122-125`; `token.rs:96` |

The register's framing is exact: "the probe drove 10.0.0 via
solana-program-binaries, litesvm ships 11.0.0, clusters run what they run —
a program id is not a pin" (`DECISION_REGISTER_2026-08-20.md:832-835`;
drift-review queue item 3, `DRIFT_REVIEW_2026-08-19B.md:414-420`).

**The version-drift hazard, decomposed:**

1. **Evidence transfer.** Every conservation, refusal, rollback, and CU
   figure in the bank evidence is a statement about *this program against
   the 10.0.0 build*. A cluster running a later build voids none of it
   logically but weakens all of it evidentially — the honest description of
   a cluster deployment is "tested against 10.0.0, deployed against
   whatever the cluster's governance has installed."
2. **New extensions.** A cluster upgrade that adds discriminant 29+ hits the
   fail-closed mask (`token.rs:441`) and refuses. For *collateral* mints this
   is the designed response. For *outcome* mints there is no exposure: the
   program creates them itself as extension-free base mints
   (`token.rs:579-590`, `initialize_outcome_mint_instruction`
   `token.rs:1096-1114`), and mint extensions are fixed at initialization.
3. **Changed instruction semantics.** A build that alters `TransferChecked`
   / `MintTo` / `Burn` arithmetic is caught by the exact-delta checks
   (`token.rs:887-903`) at transaction time — the defence-in-depth argument
   of `TOKEN2022_PLAN.md:587-591` is precisely a drift argument.
4. **Enforceability.** An on-chain pin of the resident build is not
   practically available: verifying it would mean hashing a half-megabyte
   ProgramData account inside the transaction, orders of magnitude over any
   CU budget even on the syscall-hashed runtime. A pin can live only in
   release records, manifests, and an observed-identity protocol — never as
   a runtime refusal.

**Where the pin currently lives in the seal:** the baseline manifest carries
`sbf.token2022_program_test` (and the mock-profile variant) as executed
gates with pinned key-lines (`MANIFEST.baseline.json:1648,1671,3283-3311`),
and the lockfiles freeze the harness graph — but no manifest row names the
*Token-2022 ELF digest* itself; the `a7941614…` identity exists only in
`TOKEN2022_PLAN.md` prose. The pin is real but unnamed, which is exactly the
gap the drift review's manifest item flagged for the clutch ELF and applies
here too.

---

## 5. The out-of-band-burn DoS: measured, and what it forced

**The finding as landed** (`TOKEN2022_PLAN.md:108-122`; the register's cited
cutover argument, `GOAL.md:1029-1031`): `Burn` is permissionless for a
token's owner, so a holder can destroy outcome tokens outside this program.
Under the two-truth shadow, the mint's supply fell, the program's ledger did
not, and every subsequent seam instruction on that outcome refused with
`ShadowSupplyMismatch` (`0x001e`, `Custom(30)`) — with nothing in the
program able to repair the ledger. A denial-of-service any holder could
inflict for the cost of burning their own tokens, measured on a bank
(`a_supply_that_drifted_outside_the_program_is_refused`,
`programs/clutch-sbf/svm-tests/evidence/svm_run.txt:62`).

**What it forced — and this has already happened.** The DoS was the
argument *for* the single-truth cutover (open decision 3's real content),
and the cutover landed 2026-08-19
(`docs/implementation/TOKEN2022_EXTERNAL_TRUTH_V1.md`, "IMPLEMENTED AND
RUNTIME-PROMOTED WITHIN THE STATED SCOPE"): actual Token-2022 mint supply is
authoritative for external claims; `ExternalAccount` left every production
account list; `external_supply` survives only as a last-observed cache
(`TOKEN2022_EXTERNAL_TRUTH_V1.md:22-60`;
`programs/clutch-sbf/program/src/claim_truth.rs:1-13`). A *lower* observed
supply is synchronized as a safe liability donation — the market stays live,
demonstrated on a bank
(`a_direct_burn_is_synchronized_and_the_market_stays_live`,
`token_leg.rs:470-521`: burn 2 outside the program, the next transition
recognizes supply 5, continues, and the cache follows). A *higher* supply is
impossible without the program's mint-authority PDA and refuses —
`ShadowSupplyMismatch`'s one remaining, correct use
(`claim_truth.rs:209,439`). `CURRENT_TRUTH.md:301` seals the posture:
"Actual Token-2022 mint supply is authoritative; ordinary burns are
recognized as forfeiture."

The collateral-side mirror evolved in the same direction: strict equality
became coverage, `hoard_token.amount >= collateral_atoms`, because direct
token donations to the Hoard create unowned surplus
(`genesis.rs` module doc `:26-32`; `require_hoard_covers_collateral`,
`split.rs:946`).

**What it still forces:** the forfeiture/donation residue has **no selected
terminal disposition** (`CURRENT_TRUTH.md:301`); R4's Hoard vNext
(`donation_atoms`/`forfeiture_atoms` ledgers + burn-only `dispose_surplus`)
is the designed answer and is a Wave-2 reseal item in the R4 report
(`docs/decisions/REPORT_r4-terminal-ratification_2026-08-20.md:513-521`).
Two ledger-hygiene residues for F5's ratification act: riding decision 3
should be recorded as **overtaken** on the outcome side (the shadow was
deleted, not checked) and **amended** on the collateral side (mirror →
coverage); and the committed `svm_run.txt` still names the pre-cutover test,
so the evidence file is one regeneration stale against
`tests/token_leg.rs`.

---

## 6. Interactions

- **B4b — RevenuePolicy plane C lives under this same token plane.** The
  recommended treasury shape is an ordinary Position created by the treasury
  authority through the owner-signed Endow path, zero new account families
  (`DECISION_REGISTER_2026-08-20.md:313-318`). Consequence: the collateral
  admission matrix governs revenue custody *automatically* — treasury atoms
  enter through `genesis.rs:1251-1277` like anyone else's — and the policy's
  fee-currency constraint (fee ∈ {collateral, native SOL},
  `collateral.rs:328-332`; `COLLATERAL_PROFILES.md:57-61`) is load-bearing
  for the whole B4 cluster: a separately tokenized fee asset needs a new
  schema before any revenue plane can denominate in it. Freezing A8 before
  B4 lands means B4 inherits a decided custody surface instead of an open
  one.
- **R4 — mint-close-authority on new outcome mints.** The R4 ratification
  puts `MintCloseAuthority` on *new* outcome mints so terminal close can
  reclaim rent at authoritative zero — "`MintPolicy::outcome` admits exactly
  the close-authority extension bit"
  (`REPORT_r4-terminal-ratification_2026-08-20.md:518-520`; blocking-id row
  14 at `:560`; today `allowed_extensions: 0`, `token.rs:588`, and legacy
  extension-free mints are declared permanent infrastructure). No conflict
  with the collateral matrix — bit 3 is hostile on a *third-party* mint
  (identity instability) and wanted on a *program-created* mint whose close
  authority is the program's own PDA; the role split of
  `TOKEN2022_PLAN.md:381-394` is exactly the seam that keeps these separate,
  and the A8 freeze should state that the collateral ceiling never inherits
  the outcome-role admission. One test consequence: the instruction-time
  refusal scenario uses discriminant 3 on the outcome mint
  (`token_leg.rs:531-534`) and must move to a still-hostile discriminant
  when R4's wave lands. The `MintPolicy::outcome` change is a closure-byte
  change and forks the ELF identity — a reseal-cycle item (R4 report,
  `:521-529`).
- **E2/E3 — the devnet Realm and the Aug-26 calendar.** A real market needs
  Realm + source together (A8's interactions row); the R2 identity freeze is
  date-pinned to 2026-08-26 (`DECISION_REGISTER_2026-08-20.md:643-669`). A8
  at urgency rank 3 wants at least a provisional answer before a devnet
  Realm exists; the E-cluster's reseal cycle is the natural vehicle for any
  coupled byte changes.
- **F6 — mainnet gates.** The mainnet half of the pin (per-cluster observed
  identity of the resident Token-2022 build) is release-record work that
  belongs to the F6 prerequisite stack; nothing in F5 needs to wait on it,
  and nothing in F5 can substitute for it.
- **A7 — shape precedent only.** Ratify-what's-built with the row retired,
  exactly the act A8 needs; the register keeps A7 as its own paragraph
  entry.

---

## 7. Recommendations, with counterarguments

**A8 — recommend option (1) executed now, with the divergence resolved into
the matrix, and the family answer named.** Concretely, one act:

1. Ratify the built allowlist: Token-2022 base mints only; mint-extension
   ceiling zero; account ceiling `ImmutableOwner` only; strict authority
   flags fixed; unknown discriminants fail closed.
2. Amend `COLLATERAL_PROFILES.md` row 7 from Allow to
   **Required-on-Hoard / Allowed-on-holder-accounts**, closing the named
   divergence in the adapter's direction (the probe measured why:
   `Custom(4)` is the custody story working).
3. State the legacy-SPL family's V1 status explicitly:
   **policy-encodable, not executable** (`require_drivable_collateral` is
   deliberate), and therefore that the DREGG dogfood mint has no executable
   V1 profile — the dogfood instance either waits for a widening schema or
   uses a Token-2022 asset. Silence here is exactly the "convenient code
   path silently selecting an answer" the P0 preamble forbids.
4. Gate the freeze's demonstration on the two-synthetic-Realm walk the P0
   row itself demands (`OPEN_QUESTIONS.md:45-46`;
   `docs/ENGINEERING_PLAN.md:300`): two Realms, two different Token-2022
   collateral mints, byte-identical semantics, no DREGG branch (there is
   none — no `dregg` symbol exists in program source). This is the one
   evidence gap: the corpus has three golden *profiles*, but no bank walk
   in two Realms.

**F5 — recommend a two-level pin: name the evidence pin, record the
deployment identity per cluster at its promotion gate.** Concretely:

1. Keep 10.0.0 as the evidence pin (it is already lockfile-frozen) and
   **name it**: a manifest row carrying
   `a794161408080f690dac00832f45b3c3e2b71f1339586667ad1f979cf91d5b68` so the
   pin is a gate, not prose — the same repair the drift review demanded for
   the clutch ELF digest.
2. For any cluster deployment, record the *observed* resident build
   (ProgramData dump + sha256 + slot) in the release record at that
   cluster's promotion gate — devnet at the E3 flip, mainnet under F6. This
   is register option (1) executed lazily, with option (3)'s timing, and
   option (2)'s honesty about drift as a recorded assumption in between.
3. Ratify the riding decisions: #4 ImmutableOwner-required (subsumed by
   A8's act above), #5 decimals-0/freeze-`None` (unchanged by R4 — close
   authority is a different field), #6 no-ATA/validate-instead; record #3 as
   overtaken-and-amended per §5.

**Counterarguments, stated fairly:**

- *"Widen to plain-SPL — DREGG is legacy, and legacy has no extensions, so
  its admission story is simpler."* The simplicity is real but one-sided:
  the widening costs dual CPI plumbing, dual bank evidence, a reseal — and
  a structurally weaker Hoard (no `ImmutableOwner` exists on legacy, §3).
  The P0 row's own constraint is "DREGG must not create a special branch";
  widening an entire program family to admit the house dogfood asset is
  that branch wearing a generic coat. DREGG is "never a required asset"
  (`COLLATERAL_PROFILES.md:9-13`).
- *"Freeze allowlist and pin as one act at the mainnet gate (register
  option 3 taken late)."* That leaves the P0 row silently selected for the
  entire devnet period, which `OPEN_QUESTIONS.md:3-4` forbids, and it gains
  nothing: the mainnet half of F5 is release-record work that cannot change
  the allowlist's content. The two decisions share paperwork, not
  substance — do the paperwork together now (this report is that vehicle),
  leave only the per-cluster identity recording gated.
- *"Pin 10.0.0 for production too."* Unenforceable (§4 item 4) and false
  comfort: the cluster's governance upgrades the resident build regardless.
  A production "pin" that no mechanism checks is worse than a recorded
  observation, because it reads as a guarantee.
- *"Refuse-all-v1: delete the bitsets, simplest possible schema."* Zero
  admission-behaviour change, and it forks every golden digest and frozen
  Realm identity while deleting the only seam a specified future widening
  could use without a layout re-cut (§3, option c).
- *"The extension matrix is too strict — some refused rows are harmless."*
  The matrix itself already says so: "a V1 support decision, not a claim
  that refused SPL extensions are unsafe in general"
  (`COLLATERAL_PROFILES.md:131-133`). The widening path is specified
  (payload bytes, authority mutability, CPI behaviour, atom equations,
  adversarial tests) and deliberately expensive. Strictness that can be
  relaxed by explicit act is the right default for a liability backing
  asset.

---

## 8. Execution costs

| act | bytes touched | seal consequence | scale |
| --- | --- | --- | --- |
| A8 ratification text (matrix row 7 amendment, legacy-status statement, OPEN_QUESTIONS row to Decided, register retirement) | docs only | none | hours |
| Two-synthetic-Realm SVM walk | test-only (svm-tests) | manifest key-lines fork → manifest re-emit; **no ELF fork** | a focused lane; the fixtures (`fixture_terms`, `CollateralCase`) already exist |
| F5 manifest row naming the Token-2022 ELF digest | manifest generator | manifest re-emit | hours |
| Per-cluster observed-identity script + release-record slot | off-chain tool | none until a cluster promotion uses it | small; F6-gated for mainnet |
| Riding-decision ratification incl. #3's overtaken/amended record | docs only | none | hours |
| `svm_run.txt` regeneration (stale pre-cutover test name) | evidence file | rides any svm re-run | trivial |
| — if instead: legacy-SPL widening | 5 CPI constructors, `validate_token_program`, no-TLV admission branch, `collateral.rs:355-363` relaxation, new custody argument, dual bank evidence | **ELF identity fork → full reseal cycle** (R1 precedent: closure-byte changes fork the ELF) + schema decision for the Hoard story | the largest option on the table, for one dogfood asset |
| — if instead: refuse-all schema re-cut | 266-byte layout, all golden digests, parent identities, Rust+Python corpora | every frozen Realm identity forks; reseal | large, zero admission delta |
| (context) R4's `MintPolicy::outcome` close-authority bit | program source | already costed in the R4 report as a Wave-2 reseal item; sequence A8/F5 paperwork so any coupled byte changes ride that same cycle | — |

The compute figures cited from `TOKEN2022_PLAN.md` §0.5 (219,391 CU `Split`,
etc.) predate the 2026-08-19 syscall-hash reseal; the software-SHA-256
obligation named there was discharged (`sol_sha256` in production SBF,
`programs/solana-layout/src/lib.rs:415`, `stream.rs:558`;
`docs/reviews/COMPUTE_CEILING_REATTRIBUTION_2026-08-19.md`), making every
digest-bound instruction 3-8x cheaper — the raised-ceiling caveat for
`Split`/`Merge` no longer binds and imposes no cost on any option above.

---

*Report compiled 2026-08-20 in a read-only sweep; one file, no other tree
changes. Corrections belong in a dated successor or an amendment the
register's own rule prescribes.*
