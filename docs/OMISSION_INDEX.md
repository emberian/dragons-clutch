# Omission and succession index

Status: working challenge ledger under active architectural reconsideration.
It is not a normative rejection list, release evidence, or deployment
evidence. A row records the current hypothesis; it does not make that
hypothesis permanent.

The 2026-08-25 review found that several earlier decisions may be safe
prototype waists rather than desirable protocol limits. In particular, fixed
execution roles, interpretation-only execution, categorical-only native
liabilities, exact-denominator-only receipts, non-nested wrappers, immutable
Dealer epochs, permanent RentCredit accounts, and current Token-2022 profiles
must be challenged for principled generalizations before they can be called
final omissions.

The successor does not surpass either predecessor merely by omitting its hard
parts. Every predecessor behavior is classified here as one of:

- **hard invariant**: the narrow accounting, authority, or evidence property
  that no implementation profile may weaken;
- **current safe profile**: a conservative executable choice with an explicit
  generalization path, not permanent ontology;
- **likely scar**: a restriction inherited from implementation convenience and
  scheduled for a stronger design;
- **open research**: a useful generalization whose authority or theorem is not
  yet sound enough to accept; or
- **unfinished**: still required by the project goal and never evidence of an
  intentional omission.

A rejected or superseded row is closed only by the cited accepted decision,
an explicit strongest-alternative analysis, and the named successor evidence.
Silence, missing code, a fixture, or an unimplemented frontend path cannot
close a row. Proposed omissions must first receive an architecture decision.
An existing decision may be reopened when it encodes an implementation
limitation rather than an economic, safety, or authority invariant.

## Invariants, profiles, and scars

| ID | Predecessor behavior or tempting design | Classification | Accepted treatment | Closure or revisit condition |
| --- | --- | --- | --- | --- |
| O-001 | Eager universal Market foundation containing every venue, wrapper, resolution, recovery, and funding account | current safe profile | Keep thin Core, but allow eager atomic creation of the exact selected capability subset | [Decision 0001](decisions/0001-thin-market-core.md); mandatory universal unused state remains unjustified |
| O-002 | Cumulative historical action namespace and compatibility for experimental wire/account layouts | current safe profile | Pre-release wires have no compatibility privilege; released schemas may gain explicit version decoders and one-way migrations | A release must state its compatibility and migration policy |
| O-003 | One deployed Program per family or Market, a dynamic capability-to-Program map, or an onchain code-stamping factory | current safe profile | Five state-owning roles are V1; the same descriptor may later use a Registry-authenticated stateless interpreter or translation-validated AOT accelerator | [Decision 0003](decisions/0003-fixed-role-capability-execution.md); a sixth state owner still needs an exclusive authority boundary and new profile |
| O-004 | Closed Rust family dispatch, permanent family enum/bit positions, and `N = 2..16` control-flow monomorphization | likely scar | Descriptor semantics are canonical; interpretation and checked AOT are alternative strategies | First lift with the same Direct descriptor executed both ways and compare acceptance, refusal, CU, packet, and rent |
| O-005 | Parallel old/new decoders, authority paths, DTO truths, or fallback executors | hard invariant, narrowly stated | One live writer and one persisted truth per fact; read decoders, migrations, differential references, and non-authoritative measurement ELFs are allowed | Release must prove only one writer is authorized and identify every retained non-authoritative reference |
| O-006 | Hard-coded DREGG token behavior or mandatory house collateral | hard invariant | Immutable Realm/profile identity selects collateral; token names never select semantics | No token-name branch |
| O-007 | Mock provider authority, client/index authority, or caller-authored resolution truth | hard invariant | Mocks are test-only; release state plus provider-authenticated evidence owns truth, while clients may submit untrusted witnesses | Additional real providers remain unfinished breadth |
| O-008 | Hoard principal, future fees, or unclassified donations funding rent, work, liveness, liquidity, bounty, reserve, or treasury | hard invariant | Hoard and anticipated revenue never capitalize liveness; a donation enters only through an explicit typed classification transition | No implicit lifting path |
| O-009 | A cross-asset `total_principal`, implicit conversion, or exchange rate between lamports and Realm collateral | hard invariant | Conserve assets independently; an optional valued-risk capability must name oracle, units, timing, rounding, and failure behavior | Never infer a conversion from arithmetic convenience |
| O-010 | Undercollateralized claims, uncovered Dealer inventory, protocol credit promises, or leverage against anticipated revenue | likely scar after splitting the row | Native claims must remain solvent and anticipated revenue is not capital; Dealer may generalize from gross coverage to exact terminal-scenario coverage and separately capitalized margin | True defaulting credit remains open research with its own failure instrument |
| O-011 | Dealer sponsor RFQ compatibility, mutable administrator repricing, or configuration changes that can revalue incumbent LPs | likely scar | Reject unilateral repricing, not evolution: use consent-bound capital tranches, deterministic accepted pricing policies, or quiescent epoch transitions | Prove LP consent, value transfer, scenario solvency, and withdrawal behavior |
| O-012 | Fractional native liabilities, residual credits, remainder ledgers, or hidden redemption rounding | likely scar after splitting the row | Hidden rounding remains forbidden; exact claim shards and explicit Token-owned remainder/change instruments are valid | Implement exact denomination and conservation before expanding Structured |
| O-013 | Native polynomial, B-spline, ramp, or tent liabilities inside the elementary claim basis | likely scar | Categorical is the optimized `Q=1` basis; add certified nonnegative integer partition-of-unity bases | First slice is a capped ramp plus exact complement with totality, partition, solvency, and redemption proofs |
| O-014 | Wrapper nesting, recursive payout authority, wrapper-specific collateral truth, or wrapper-owned resolution | likely scar after splitting the row | Keep one native liability/resolution truth; admit acyclic compositional recipes with canonical flattening and a bounded representation DAG | Arbitrary live wrapper-on-wrapper custody remains open research |
| O-015 | Bespoke protocol holder ledger, privileged wrapper transfer, transfer hook, permanent delegate, or silent reconciliation of token supply | likely scar after splitting the row | No shadow supply or silent reconciliation; Token behavior is selected by a versioned profile rather than one forever-fixed extension set | Lift display decimals and inert metadata first; risky extensions require separate proofs and rollback campaigns |
| O-016 | A caller or static client supplying executable Program identity, physical effect plans, semantic IDs, or trusted account projections | hard invariant | Callers may supply hints, witnesses, candidates, and physical accounts; release-selected state verifies every authoritative identity and derived effect | No caller input becomes authority by inclusion |
| O-017 | Calling the selected General result “optimal clearing” without a checked optimality certificate | hard invariant | Say “best valid submitted candidate” unless a descriptor admits and verifies a named optimality certificate | A checked certificate may lift the vocabulary restriction |
| O-018 | Instructions-sysvar adjacency as the authority joining provider publication to Source consumption | hard invariant, narrowly stated | Adjacency alone is not authority; a release-bound parser may use authenticated instruction evidence, while same-instruction CPI remains the conservative Pyth profile | [The Pyth compost decision](compost/PYTH_LOCAL_UPGRADED_2026_08_22.md) covers only its selected transport profile |

## Current physical profile restrictions

These are not mathematical or product omissions.

| ID | Current profile | Classification | Required lifting path |
| --- | --- | --- | --- |
| P-001 | Product V1 admits at most sixteen outcomes | current safe profile | First erase remaining width dispatch and measure contiguous runtime views; page only where packet/account/CU evidence requires it |
| P-002 | Capability manifest profile 1 admits at most sixteen entries and sixteen dependencies per entry | current safe profile | Finalized ordered paged graph with one aggregate commitment, unique kinds, and checked acyclicity |
| P-003 | Current checked transition program has a finite instruction bound | current safe profile | Wider profiles, staged computation certificates, and AOT are valid; final economic commit remains bounded and atomic |
| P-004 | Bearer/Structured V1 uses zero-decimal, narrowly admitted Token-2022 Mint and Account layouts | likely scar | Keep it as the strict default profile and add separately versioned Token behavior profiles |
| P-005 | RentCredit V1 is permanent and has no close, drain, migration, or caller-selected redirect | likely scar | Lifecycle-scoped refund sink closes to its immutable wallet only after its authenticated producer subtree closes |

## Required work that is not an omission

The following rows remain part of the surpass-both goal. They may not be moved
to the accepted table merely because implementation is difficult.

| ID | Required successor capability | Present status | Completion evidence |
| --- | --- | --- | --- |
| U-001 | Generic Trading execution for Direct, General, Dealer, and Series | unfinished | One admitted descriptor path per family, interpreted V1 composition, and classification of standalone family artifacts as deleted authority or retained non-authoritative AOT evidence |
| U-002 | Direct inline and registered/reserved order lifecycle | unfinished successor convergence | Chain-derived operator packets, claim/custody effects, cancellation/retirement, hostile rollback, packet and CU evidence |
| U-003 | General batch collection, candidate selection, materialization, distribution, expiry, and closure | unfinished physical convergence, with two named defects | Runtime-width Trading path with best-valid-candidate semantics and full terminal campaign. Landed 2026-08-27: the ALT-backed v0 packet witness for all seven N=258 actions (`docs/evidence/GENERAL_ALT_PACKET_WITNESS_2026_08_27.md`), and the foreign-descriptor refusal. **(a) CLOSED 2026-08-27** — the root-lifecycle conjunct is two instructions in the shared transition prelude plus one `ProjectDataU8` in the canonical AccountProfile operation list, so all seven actions refuse a `Retiring`, `Retired` or unwritten lifecycle; fail-closed if an artifact omits the projection, because no request profile may project the two new scalars. Measured cost 16 CU per action, no account, packet or page moved (`docs/evidence/GENERAL_ACCELERATOR_CAMPAIGN_2026_08_27.md`). **(b) the seam is OPEN, General's release content is not.** The two blockers that made a V3 capability root unreachable are both resolved and both have real-ELF control: the activation seam wrote an all-zero family tail no family root could be decoded out of, and it authenticated the record at `selection.capability_release()` as a `CapabilityProgramV1` while `hot_v3` authenticates the same field as a `CapabilityProgramSetV2`. The tail is now the effect program's projected request buffer and a `CapabilityProgramSetV2` release activates through its selected descriptor. **General's activation artifacts EXIST (GEN-ART, 2026-08-27).** All three generations now have a public typed encoder in their owning crate — `dclutch-transition-vm::v2::encode`, `dclutch-effect-kernel::v2::encode`, `dclutch-account-profile-contract::encode_v1` — each a total function that hostile-decodes its own candidate, with the transition and profile encoders pinned byte-for-byte against Lean-emitted artifacts. `programs/dclutch-trading-sbf/program-test/tests/activation.rs` (`Campaign::General`) creates a real `GeneralRootV2` through the seam on a validator, and `activate_general_owned_v3` — run over the same manifest, config, funding prestate, custody observation and slot — agrees byte for byte on both the root tail and the FundingState poststate. **Still required for a live release:** an eighth `CapabilityProgramSetV2` entry naming the activation descriptor, and the relaxation of `authenticate_general_program_set_v3`'s exactly-seven-`CapabilityProgramV4` rule (the activation selector must sit at the set's declared `selector_offset` without colliding with an action byte). That is a second batched artifact regeneration; the first cost 16 CU per action and moved no account, packet or page. **(c) the zombie refusal is executed against a real root but not through the ELF.** `crates/dclutch-general-adapter-contract/tests/root_lifecycle_projection_v3.rs` runs the real `AccountProfileV2` projection over a real composite capability root and requires the root's own lifecycle byte to reach `scalar::ROOT_LIFECYCLE_OBSERVATION`, then requires every action's emitted transition to refuse the projected `Retiring`/`Retired` bank — with the discriminator that the projected banks differ at exactly one register. Not yet run through `hot_v3::process_hot_execution_v3` on a real ELF: the conjunct evaluates at phase 6 of 10, so a refusal is reachable ahead of the heap wall, but a General Hot bundle in `programs/dclutch-trading-sbf/program-test` (Hot38 frame, capability seal, ALT, five ELFs) does not exist — the Direct analogue is ~4,000 lines of fixture support in `program-test/direct-hot` |
| U-004 | Multi-LP Dealer liquidity, then scenario-solvent capital generalization | unfinished physical convergence and expansion | Real V1 custody lifecycle plus exact scenario-reserve/netting kernel, consented policy evolution, value, and rollback evidence |
| U-005 | Recurring Series founding | unfinished physical convergence | Ticket funding redistribution, Core founding, Claims initialization, selected capabilities, terminal closure, and replay evidence |
| U-006 | Source/Resolution creation, recovery, terminal admission, and funding closure | unfinished convergence | Fixed-role Resolution Core-effect campaign, real provider execution, repaired native fixtures, and exact three-ledger closure |
| U-007 | Claims initialization, native Positions, bearer representation, and terminal redemption | unfinished convergence | Real Token-2022 and Custody positive/late-refusal rollback campaigns and complete initialization/closure |
| U-008 | Structured and useful Fractional successor products | unfinished | Exact-denominator fast path plus exact claim shards/remainders, wrap/transfer/unwrap/redeem/retire, real Token-2022/Custody, and operator/UI support |
| U-009 | Providers beyond the first Pyth profile | unfinished optional breadth | One release-bound adapter at a time with real ABI/crypto and recovery evidence; no mock fallback |
| U-010 | Chain-derived creation, trading, liquidity, resolution, redemption, and retirement UI/operator workflows | unfinished | Unsigned transaction construction from hostile-decoded live chain state and user-visible refusal/status reporting |
| U-011 | Current integrated local-validator, hbox, and persvati evidence | unfinished | Exact committed-source archives, verifier-clean artifacts, rent/CU/packet/rollback evidence, and resolved cross-host reproducibility boundary |
| U-012 | Checked release, obsolete-path deletion, documentation, and devnet preparation | unfinished | Checked manifest plus deletion ledger and current documentation; devnet submission still requires explicit current authorization |
| U-013 | Certified nonnegative liability bases beyond categorical | pure theorem, kernel, and translation corpus landed; the physical Market/Claims layout slice is unfinished | Two-claim ramp/complement theorem and kernel, exact partition sum, generalized solvency, terminal payout, and translation corpus. Landed: `DClutchSemantics.LiabilityBasisV2` (evaluator totality, exact partition sum, split/merge/transfer/redemption preservation, `H >= Q*peak(T)` proved exact for both admitted families, and the sole apportionment boundary's rounding direction), `crates/dclutch-liability-basis-v2-kernel` (`plan_transition_v2`, `maximum_liability_v2`, `plan_claim_transfer_v2`), and the Lean-emitted 16 agreement / 19 refusal / 24 transition corpus. Still required: a Market and Claims layout carrying basis width, payout scale, evaluator release, certificate schema, and capacity profile, with a founding, trading, resolution, and redemption campaign |
| U-014 | Interpreted and stateless-AOT execution of one semantic descriptor | unfinished expansion | Exact Direct equivalence/certificate, Registry-bound artifact/toolchain, refusal equivalence, rollback, CU, packet, and rent comparison |
| U-015 | Acyclic representation composition, Token behavior profiles, lifecycle refund closure, and measured width lifting | unfinished expansion | One typed DAG, first lifted Token profile, complete producer-subtree refund closure, and contiguous/paged profile evidence |

## Maintenance rule

Every capability comparison or predecessor compost review must update this file
in the same commit that accepts a rejection, successor, or profile restriction.
Moving an item from `unfinished` requires exact evidence links. A release check
must refuse if any required row lacks evidence or if a rejected path remains an
executable fallback. The concrete expansion program is tracked in
[Expansion frontier after the omission review](research/EXPANSION_FRONTIER_2026_08_25.md).
