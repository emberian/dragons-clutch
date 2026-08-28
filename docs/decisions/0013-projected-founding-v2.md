# Decision 0013: projected founding v2 carries the source cap and leaves lock headroom

Date: 2026-08-28  
Status: adopted for the next devnet generation

## Decision

The next devnet generation makes a coordinated wire break across source
policy, projected founding, capability funding, and the atomic generic founding
route. Existing devnet Markets and projected prestates are not compatible with
this generation and will not be migrated.

The five economically mutating stages remain one transaction:

```text
Lock -> Found -> Realize -> Claims -> Open
```

The earlier projected-Custody preparation remains non-economic. It may
authenticate immutable records and carry their exact result into the atomic
transaction, provided the result is canonical, one-shot, release-bound, and
consumed by the same Market founding.

This decision adopts five linked changes:

1. `SourceMaterialV3` is the root of an acyclic source-policy graph. The build
   order and authenticated edges are:

   ```text
   CapacityProfile(kappa)
     -> SourceSpec(capacity_profile_id)
     -> ManipulationFloor(source_spec_id)
     -> SourceMaterialV3(primary_source_spec_id, manipulation_floor_id,
                         principal_policy)
   ```

   The capacity profile does not point back to the floor. That proposed edge
   required a SHA-256 fixed point and is rejected.

2. `ProjectFound` authenticates the complete source-policy graph and converts
   the atom-denominated source cap to complete-set units at the Market's one
   named rounding boundary. `ProjectedCustodyStateV2` carries the resulting
   `principal_cap_sets: u64`, using an explicit canonical unbounded value. A V1
   projected state refuses; it never acquires a cap by interpretation.

3. `CoreState` is the sole runtime owner of `principal_cap_sets`. Found checks
   the initial quantity before committing it. Every complete-set split checks
   the resulting aggregate credit against the same persisted cap before any
   mutation. Claims reads the authenticated Core Market; its founding request
   does not duplicate the cap as a caller-authored field.

4. The projected generic Found frame stops re-reading facts already
   authenticated by canonical projections:

   - the complete Registry activation cache owns the exact activated execution
     release projection, so the execution-release raw/staging pair leaves the
     Found frame;
   - `ProjectedCustodyStateV2` supplies the authenticated Realm/collateral,
     source identity, and principal cap, so the Realm and SourceMaterial
     raw/staging pairs leave only the projected generic Found frame;
   - ordinary `ProjectFound` retains and authenticates the source graph and all
     records needed to produce that state;
   - ProgramData accounts, deployment slots, Registry/Rent artifact records,
     and staging-vacancy finality witnesses remain checked.

5. Per-entry `FundingStateV1` accounts become canonical controller-subset
   `FundingLedgerV2` accounts. Each ledger is keyed by controller program,
   Market, generation, manifest identity, and a nonzero selected-entry mask.
   Its fixed-layout rows are the selected manifest indices in ascending order.
   The ledgers presented to founding must be controller-homogeneous, pairwise
   disjoint, and cover the exact required entry set. They authenticate the
   immutable manifest before deriving `released = quote - remaining`.

   A Direct-capable Market therefore presents two ledgers: one Resolution-owned
   ledger for the three Resolution entries and one Trading-owned ledger for the
   Direct entry. This preserves each controller's authority and independent
   close/rent lifecycle without adding a Resolution-to-Trading CPI boundary.

No fee rate or fee destination changes in this decision.

## Exact account budget

The deployed DCLTGMF1 instruction has 138 account references for its three
funding entries. Those references deduplicate to 64 frame keys. The bounded v0
transaction adds two keys that the old guard omitted:

- the fee payer;
- the one Compute Budget program used by limit, price, and heap-frame
  instructions.

The actual current transaction therefore locks 66 keys against devnet's limit
of 64. Address lookup tables change message size, not this count.

The selected changes have this Direct-capable budget:

```text
current four-entry transaction                    67
two subset ledgers replace four physical states   -2
activation cache replaces execution-release pair  -2
projected state replaces Realm pair                -2
projected state replaces SourceMaterial pair       -2
-----------------------------------------------------
Direct-capable projected founding v2 transaction  59
```

A Resolution-only three-entry Market uses one ledger and compiles to 58 keys.
Adding Direct adds its distinct Trading-owned subset ledger, producing 59.
Five keys of devnet headroom remain.
The shipped census must compile the actual bounded v0 message and assert its
complete `account_keys` length; counting instruction metas is not sufficient.

## Source-cap units and rounding

The immutable source policy remains expressed in collateral atoms. Runtime
split routes operate in complete-set units and do not receive the basis scale.
The projection therefore computes one canonical set cap:

```text
cap_sets = floor(cap_atoms / basis_scale)
```

The implementation must name overflow, saturation, zero-scale refusal, and the
unbounded sentinel explicitly. A Lean theorem and generated Rust implementation
must agree on this projection before it can mint a runtime cap. The proof does
not cover Solana account loading, Registry publication, CPI, or adapter/runtime
behavior; those remain separately tested trust boundaries.

## Finality and semantic ownership

Registry raw ownership is not finality. A fully appended raw record can still
have a live staging cursor. Consumers continue to require the canonical vacant
staging PDA unless a future Registry wire version introduces another checked
finalization authority.

The activation cache is not a second execution-release truth. Its declared
role is the exact completed release projection, derived at its canonical PDA
and inert until every required role has activated.

The projected state is not a second Market truth. It is a one-shot prestate
owned by Custody, created only after a checked Core receipt, bound to the exact
Market request, release generation, roles, source, collateral, expiry, and cap.
The atomic founding consumes it and commits `CoreState` as the runtime owner.

## Refusals and adversarial coverage

The coordinated change must demonstrate at least:

- a content-valid record with a live staging cursor still refuses;
- a substituted floor, SourceSpec, capacity profile, adapter configuration, or
  collateral unit refuses before cap projection;
- V1, wrong-owner, wrong-PDA, stale, cross-release, wrong-Realm,
  wrong-collateral, and wrong-source projected states refuse;
- partial or cross-release activation caches and moved ProgramData slots
  refuse;
- initial principal above the projected cap refuses with no account mutation;
- every supported split path refuses cap growth before mutation;
- a four-entry Direct founding compiles to at most 64 complete message keys;
- the hostile late-stage case still rolls back every earlier stage;
- all shipped SBF links pass the frame-diagnostic script;
- compute evidence is reported as pass count and 20-seed mean.

## Deployment and exterior acceptance

The seven devnet program addresses may be retained through decision 0012
Upgrade when that is the shortest safe route. Existing V1 Markets, projected
prestates, releases, and activation caches are disposable. The release driver
must be resumable for exact partial publication and activation, must refuse
authority or Loader-owner drift, and must bind the checked raw ELF to any
zero-padded live ProgramData payload before a write occurs.

This decision is not complete when the repository is green or a Market merely
reaches Founding. Acceptance requires a fresh Direct-capable Market that is
discoverable from a cold public client and is exercised through wallet
submission, resolution, terminal redemption, and public-ledger reconciliation
on devnet. Devnet execution is not mainnet evidence.

## Rejected alternatives

- Adding three source-policy raw/staging pairs to DCLTGMF1 produces 72 locks
  before Direct and does not fit devnet.
- Storing `floor_content_id` on the capacity profile creates a content-address
  cycle.
- Shrinking each FundingState saves rent but no transaction locks.
- Removing staging accounts without a new finality authority admits complete
  but unfinalized records.
- Payer/founder aliasing and caller-PDA sharing can reach the limit but leave no
  Direct headroom.
- A tree-wide sealed-Registry migration can save more locks, but it is broader
  than the existing authenticated projection seam requires.
