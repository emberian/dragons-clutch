# Structured across the selection membrane, and the first Structured Market

Lane MEMBRANE, 2026-08-30. Commits `2560230c`, `b46d9de7`, `4d37ec31`,
`f06cbf04`.

This continues the work `docs/evidence/SELECTED_CAPABILITY_SEAM_2026_08_29.md`
records. That document ends with the seam proven by four families and two of
them founded. This one adds the third founded family, and it is the one that
arrived at the membrane already market-free.

## 1. What existed, and what did not

STRUCT-SEL established (`9ceaa05f`, `5f83ebfa`) that Structured is trapped
through neither its config nor its `release_id`: the whole five-action
`program_set_id` is byte-identical across two Markets. `da8d3a85` then narrowed
the four builders to market-free V6 siblings, so a complete release could be
*constructed* before founding.

What did not exist: a publication type, a release compiler, a release
authenticator, a `structured_market.rs`, and a founded Market.

## 2. The release chain

**`authenticate_open_capability_release_v1`**
(`crates/dclutch-bearer-v2-operator/src/open_release_v1.rs`) admits one complete
five-action release from three content identities and a pile of untrusted bytes.
Each artifact is decoded under its own type before its digest is compared, so a
release carrying well-digested rubbish refuses.

Two things it does that the Rational template does not:

- **It pins the kind, and the kind travels in the selection.** Rational's
  admission hardcodes its family constant. This layer cannot: one set builder
  serves Bearer and Structured. So the selection carries all three identities a
  manifest entry actually names — kind, release, config — and the admission
  requires every descriptor to carry exactly that kind. The kind is a fact the
  *manifest* states and the admission refuses to contradict, not a conclusion a
  caller asserts. This closes a real hole: `validate_rational_open_capability_
  program_set_v3` reads no kind at all, which is how a placeholder
  `identity(0x10)` passed validation for months.
- **It reads the routed action out of the effect, not the entry.** A permuted
  set keeps every digest intact and every join green while dispatching each
  selector at another action's effect. The routed action is read from the
  effect's own route template.

**`structured_selected_release_v1`**
(`crates/dclutch-operator/src/structured_selected_release_v1.rs`) compiles the
five actions into one publishable release with a canonical publication
(`DCSTPB01`, 452 bytes, width derived from the field table with const asserts).
The compiled release is handed to the admission before return, and the
publication is built from the *admission's report* rather than the compiler's
own locals — so a fact the admission did not establish cannot reach a manifest.

Structured names **fewer** free facts than Rational: its kind and capacity
profile are not parameters at all, because they are `STRUCTURED_CAPABILITY_
KIND_ID_V2` and `STRUCTURED_CAPACITY_PROFILE_ID_V2`. A parameter for either
would offer a caller the chance to publish a Structured capability under
another family's identity.

## 3. The lifecycle policy, and the read that produced it

STRUCT-SEL left this named and undone: the open family's lifecycle policy was
"borrowed from the Rational fixture literal", with no claim made about whether
Structured's actions author state PDAs.

The read does not come out where the analogy would put it.

`StateLifecyclePolicyV5` selects plans by **exact equality** on the action tag
(`lifecycle_v3.rs:1173` and `:1187`). There is no wildcard. The open family's
fixture policy — present in both test fixtures — parks its single `Authenticate`
plan at `action: u32::MAX`, and the five open actions carry tags 1 through 5.
**That plan is selected by nothing the capability can dispatch.**

That shape is worse than an empty policy. It carries a real seed domain
(`dclutch/rational-open/dormant/v4`), a recipe and a plan, so it reads as a
design to anyone auditing the release, while being semantically identical to
empty. Shipping it would publish a dormant PDA derivation this family never
performs inside an artifact whose digest all five descriptors name.

`encode_open_capability_lifecycle_policy_v5` is therefore empty and
parameterless, and its header makes the argument rather than inheriting it: all
three open bundle builders encode exactly one effect route with role
`FixedRole::Claims` and each **refuses** a program shaped otherwise
(`open_selected_v3.rs:418`, `open_structured_v3.rs:524`, `hot_effect_v3.rs:279`).
There is no second role through which Trading-owned state could be reached, so
there is no such state for a policy to plan.

`the_fixture_policys_only_plan_is_unreachable_from_every_real_action` encodes
the exact decorated fixture shape and asserts `u32::MAX` selects one plan while
all five real tags select zero. If the codec ever grows a wildcard tag, that
test goes red and names the assumption.

## 4. The seam module, and one helper that moved

`tools/local-validator/bootstrap/successor/src/structured_market.rs` is the
fourth family through the seam with **zero seam changes**.
`DCLUTCH_MARKET_CAPABILITY=structured` plus
`DCLUTCH_STRUCTURED_COLLATERAL_MINT` selects it.

`market_realm_identity_v1` **moved** from `rational_market.rs` into
`selected_capability.rs`. Structured's config is the same type Rational's is, so
it inherits the same Mint-before-closure ordering — and a second consumer is
what proves a helper was never the first family's.

**The one number this family chooses differently.** Structured's representation
width `K` is *not* the market's outcome count. The demo graph has four outcomes;
the open RequestProfile V1 artifact bounds `K` at three. Reaching for the
market's count would compile a release that refuses at first dispatch. The bound
is now published as `STRUCTURED_MAXIMUM_REPRESENTATION_WIDTH_V1` from the
compiler that enforces it.

**A lab fact left labelled rather than dressed up.** The per-coordinate item
width is 64, which every fixture uses and which coincides with the
Lean-generated `dclutch-claims-representation-codec` `STATE_BYTES`. That crate
has **zero consumers** anywhere in the workspace, so the coincidence is recorded
as a coincidence rather than wired up as a derivation.

**The deadness pin.** Nine call sites pass `item_data_lengths: [64, 82, 165,
165]`; `open_structured_v3.rs:610` marks rows 1..=3 opaque and line 621 forces
their width to zero, so three of those four literals never reach a byte. The
compiler passes `[item_state_bytes, 0, 0, 0]` and
`the_three_dead_item_rows_really_are_dead` compiles both ways requiring
byte-identical output, with a positive control that row 0 *does* move the
artifacts.

## 5. The founded Market

**`HEanNZ1enV6RteCUtr1iZkyK3CAY5Mpau5Vc4Biuo2Xg`** — local validator on hbox,
gate `7978b736…` at source revision `4d37ec31`. All 37 Structured records
published and finalized; 207 transactions; `DCLTGMF3` executed: *"the Market is
OPEN, with the Claims liability aggregate, the founder Position, the admission
record, and a Hoard holding the exact collateral"*.

Capability manifest record `FszNgoYSEh27MdmQGEvYi3h9Ds8pWcBafywsbJodqbZF`,
2128 bytes, 4 entries, Structured at entry 3.

### Verified from chain state by derivation

Every line below is a recomputation, produced by
`tools/local-validator/verify-selected-capability-binding.py` (`f06cbf04`):

- the manifest record's **address** re-derived from the Market's own field —
  `b"dclutch-raw-record-v1"` + the manifest schema id + the digest at offset 176
  — lands on `FszNgoYS…` at bump 255, and its body digests back to that field;
- entry 3 `release_id` == SHA-256(ProgramSet bytes); `config_id` ==
  SHA-256(config bytes);
- entry 3 kind, capacity, child schema and child derivation all equal the
  coordinates read off the descriptor at the `CapabilityProgramV4` offsets;
- entry 3 `kind_id` **is** `STRUCTURED_CAPABILITY_KIND_ID_V2`, and the constant
  was itself recomputed from its domain preimage rather than trusted:
  `sha256(b"dclutch/capability-kind/structured-receipt-v2|depth2-representation-dag|shard-backed-receipt")`
  = `8747f721…ae8aed` = what the chain holds. Same for the capacity profile.
  These two Lean-generated constants had never been on any chain before;
- the `DCSTPB01` publication agrees with the chain entry on all four identities;
- the config binds the Market's own realm;
- and the **Market address itself** re-derives from its nine seeds — realm,
  product record, product id, resolution policy, **capability manifest digest**,
  selected release set, registry, generation — to `HEanNZ1en…` at bump 254.

That last one is the seam invariant stated as a fact about the chain rather than
a rule layered above it: the manifest digest is a *seed of the Market's name*, so
a manifest entry that depended on the Market address could not have produced that
address. A market-dependent entry is not merely wrong; it is unconstructible.

### The ordering constraint, confirmed independently

`DCLUTCH_STRUCTURED_COLLATERAL_MINT=2oC9JMSj18W5zF1JoHLvMJtogXWhvo127jWiPWsuJ8wn`
was named **before** the closure compiled, taken from a Rational run on a
different revision on a different day. The mint the run's own forge produced was
byte-identical. The forge is deterministic across families, revisions and days,
so any family whose config binds a Realm can name the mint pre-compile with no
driver change.

## 6. Two findings that are not Structured's

**The founding "flake" is ZFS.** Three runs with `--work` under
`/tank/dregg-build` (ZFS) all died with `did not reach finalized transaction
history` — at different points, with no program error code and no CU wall,
including one on a completely quiet box. Moving the work root to hbox's ext4
NVMe changed nothing else and the run published all 37 records and founded the
Market with zero finalization failures. A validator's rocksdb/accounts write
pattern is close to the worst case for copy-on-write; it does not error, it
roots slots late, so it presents as a flake. Keep the repo, target dirs and gate
output on `/tank`; put the run **work root** on ext4.

**Wall #22 is family-wide, not Direct-specific.** Activation is gated twice in
`programs/dclutch-trading-sbf/src/outer.rs`: the selected descriptor must carry
`CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1` (`:948`), and its effect must project
exactly `root_state_bytes` (`:1442`). The two schema ids are distinct constants
with distinct preimages (V1 `7fa4d378…`, V4 `2d85b221…`), and every family's
ProgramSet — General's seven entries, the open layer's five, Rational's four —
stamps the **V4** id. So no family's release contains an activatable descriptor,
and General's "hot commit half" is blocked behind this rather than merely
unstarted. TRADE's `b45d3a2c` (`direct_activation_bundle_v1`) is the template
for four families, not one; its brick-safety gate — running the real effect
kernel and asserting the projected request decodes as the exact initial root
state — is the part that generalises hardest.

## 7. Reproduction

```sh
# gate (hbox, from a clean checkout at the revision)
SWARM_MEM_MAX=32G swarm-build tools/release/checked-release-candidate.sh \
  --repo <SRC> --work <GATE_WORK> --commit <REV>

# founding: note the work root is on ext4, not ZFS
DCLUTCH_MARKET_CAPABILITY=structured \
DCLUTCH_STRUCTURED_COLLATERAL_MINT=2oC9JMSj18W5zF1JoHLvMJtogXWhvo127jWiPWsuJ8wn \
python3 tools/release/private-validator-lifecycle/run.py \
  --repo <SRC> --release-root <GATE_WORK> \
  --validator $(which solana-test-validator) --solana $(which solana) \
  --work /var/tmp/<lane>/hold-NN --seeds 1 --through participant \
  --hold-after-participant

# resume the preserved ledger and verify from chain state
tools/gauntlet/frontend/resume-validator.sh <WORK>/runs/seed-01/ledger 29300
tools/local-validator/verify-selected-capability-binding.py \
  http://127.0.0.1:29300/ <MARKET> <WORK>/runs/seed-01/market.json
```

## 8. Named debt

- **The run refuses after the chain is correct.** `run.py` returns "founding
  campaign changed its exact six-mutation success order" — the campaign reported
  21 completed mutations against a pin expecting six. This is SEL-SEAM's named
  readiness-suffix regression, unchanged and not Structured's; the Market is Open
  on chain regardless, exactly as SPINE-2's devnet founding was. `run.py` lives
  under `tools/release`, fenced for this lane.
- **No Structured action has been executed** against the founded Market. The
  admission path needs the routing-table wiring SEL-SEAM specified (wall #14),
  and any *capability* action needs wall #22 above.
- The two open-family test fixtures still encode the dead-plan lifecycle policy.
  They are not wrong — they test artifact plumbing — but a reader will take the
  domain literal for a design. Point them at the new encoder.
- `crates/dclutch-operator` `series_projected_v2::admitted_effect_owns_child_
  request_and_receipt_commitments` is **red on committed main**, verified red on
  a clean tree both at `4d37ec31` and at its branch point `6c84b33a`. Not this
  lane's; flagged because `-p dclutch-operator --lib` is 147 tests and exactly
  one is red, so a filtered run reports green honestly and says nothing.
