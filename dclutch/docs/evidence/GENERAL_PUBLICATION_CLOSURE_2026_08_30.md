# General's publication closure, and the two walls between it and a live root

2026-08-30, GENPUB lane. WALL22 closed the last U-003(b) artifact gap and left
one line open: "the publication closure — three record labels in a General
release and the founding path that publishes them." This document records what
that closure is, that it now executes on a real chain, and exactly where a
General activation still stops.

**Headline: activated from a published release — NO.** The three records are
published and finalized on chain at the addresses the Trading activation seam
derives, but no General root was created, because the founding that must precede
it refuses. That refusal is **family-neutral and not General's**: a Direct
control run at the same commit, on the same checked release, refuses identically.

## 1. What "publication closure" turned out to be: three records, not four

The Trading activation frame borrows exactly three finalized records beyond the
ProgramSet and the config. `programs/dclutch-trading-sbf/src/outer.rs::
process_activation` reads an `AccountProfileV1` at `PROFILE_RAW`, an
`EffectProgramV2` at `EFFECT_RAW`, and a `CapabilityProgramV1` at
`SET_DESCRIPTOR_RAW`.

There is no fourth. `CapabilityProgramV1::decode` reads the activation
transition off the bytes *after* the descriptor header — the transition is
**embedded in the descriptor**, not a record of its own. Publishing it
separately would finalize a record the seam never reads and give the transition
two authors. Direct publishes the same three
(`direct_activation_{account_profile,effect,descriptor}_record`), which is the
precedent this follows rather than a coincidence.

## 2. The release now publishes them, and it cannot publish a set that cannot activate

`crates/dclutch-operator/src/general_selected_release_v1.rs` emits
`GeneralReleaseProfileV1::SettlementWithActivation` — eight entries, not seven.

The narrower profile is not a smaller release, it is an **unfoundable** one.
`authenticate_set_descriptor` admits only a descriptor stamped
`CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1`, and every action descriptor is
stamped `v4::SCHEMA_RELEASE_ID`; a Market founded on a seven-entry General set
can never create the capability root all seven of its actions execute against,
so every action it publishes is unreachable forever.

Nothing about the activation is authored in the release compiler.
`build_general_activation_bundle_v1` composes the triple on the family-neutral
template and runs the **real effect kernel** over the effect it has just built,
returning an error rather than a bundle if the projection is not
`general_root_creation_tail_v2` byte for byte. The set entry comes from
`build_general_activation_capable_program_set_v1`, which re-authenticates its own
bytes. So a release cannot hold an activation that would brick a root, and
cannot name one at a coordinate an ordinary controller request could reach.

The activation descriptor's identity joined the publication as a twelfth fixed
identity. Without it, the summary a Market binds describes a release nobody can
activate.

### What moved

The General `program_set_id`, `config_id` and `publication_id` all move: eight
entries change the set digest, which is a field of the config, whose digest is
the publication's. Nothing outside `dclutch-operator` and
`successor/general_market.rs` pins them, and no devnet General market exists to
desynchronize.

## 3. Executed on chain

One `run.py --through participant --seeds 1 --hold-after-participant` with
`DCLUTCH_MARKET_CAPABILITY=general`, on hbox, against a fresh checked release
built at `0e6bb66e` (13 SBF links, freshness PASS, `sbf_build_diagnostics_total=0`).

The compiled market input carried `family=general`, **68 publication records**
where it carried 65, an eight-entry 608-byte `CapabilityProgramSetV2`, and
manifest entry 3.

The record ladder finalized all 68. Read back from the run's own preserved
ledger — a validator restarted read-only on it, on its own port block — five of
them are Registry-owned at the PDA derived from `(schema, sha256(body))` and
**byte-identical** to what the release compiler published:

| record | address | bytes |
|---|---|---|
| `general_00_program_set_record` | `GpAb9UDdrWcoLZoXBbgmcsGAKjtD2KHWzy84MJhy1c5w` | 608 |
| `general_01_config_record` | `3acSHMt3SYi3statMqmdaiPg9z3j1syXazFC3G52NSVF` | 232 |
| `general_65_activation_account_profile_record` | `12CykdbGjLXwHm4qqHog2dEShRurV2QS69udqnZrbLEL` | 112 |
| `general_66_activation_effect_record` | `CKbgY4g8UTmXRCYjzX6R3rEcWvZsZTftSXtzS7TeecCL` | 128 |
| `general_67_activation_descriptor_record` | `EHn6EiPDBaEgqh97kVHk71o5GzVsoUZBj3svoUy9rgAS` | 368 |

The descriptor record is finalized under `7fa4d378…` —
`CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1`, the one schema the activation seam
accepts and that no action descriptor carries. That is wall #22's own coordinate,
published.

Chain identity plus the offline triangle test is the complete argument: the
published bytes equal the compiler's, and
`the_three_activation_records_close_the_triangle_the_seam_authenticates`
requires the compiler's descriptor to name the digests of the compiler's other
two records. So the on-chain descriptor names the on-chain profile and effect.

## 4. The driver

`local-private-validator-general-capability-activation-v1` is the tree's second
capability-activation driver and its first non-devnet one. The Direct sibling
has created a real capability root on public devnet, so its account frame — 35
metas, one funding ledger, an ALT — is reused unchanged; only the sixteen family
request bytes differ.

The discipline is that the campaign report supplies **two routing coordinates**,
the Market address and the Trading funding-ledger address, and nothing else:

- the manifest record's address comes from the Market's own
  `capability_manifest` identity, and its bytes must hash to it;
- the General entry is **found** in that manifest by kind, never supplied;
- the ProgramSet record's address comes from that entry's `release_id`, and the
  set must authenticate as `SettlementWithActivation` — a seven-entry General
  release refuses here, with nothing signed;
- the activation descriptor is **selected** out of that set by
  `general_activation_request_v1`;
- the account-profile and effect record addresses come from the identities that
  **on-chain descriptor** carries.

The root address gets a second author (`general_capability_root_address_v3`).
The poststate gets two: `GeneralRootV2::decode` must accept the tail, which must
equal both `GeneralRootV2::active` and `general_root_creation_tail_v2`, and the
funding row must equal `FundingLedgerV2::activate_in_place` over the same
manifest and the slot the chain recorded.

The offline control nobody had is
`the_addresses_this_driver_borrows_are_the_ones_the_founding_finalizes`. The
founding publishes those three records under schemas the **release compiler**
names; this driver borrows them under schemas it derives **from chain**. Two
authorities for one address. If they drift, the failure is a `required_account`
on a vacant PDA, on a validator, with a founding already spent and nothing to say
why. The control compares all five coordinates in milliseconds; proven red by
pointing the effect record at the profile schema.

## 5. Wall one, found and fixed: the cache bump had no author

Every local founding in the tree had been refusing since `a40ef689` that
afternoon, at `activation cache progress: ReleaseSetSelectionMismatch`, after all
five role activations succeed.

`a40ef689` put the activation cache's own PDA bump at
`ACTIVATION_CACHE_BUMP_OFFSET_V1 = 12` — the first of four canonical reserved
bytes — and moved `RESERVED_OFFSET` 12 → 13. It did not move
`activation_cache_progress_v1`'s selection comparison, which still spanned
`0..ROLES_OFFSET (48)`, a span that now contains the bump.
`ActivatedExecutionReleaseSetV1` is a projection of the release set, has no field
for a fact about an account address, and leaves byte 12 zero;
`put_activation_cache_bump_v1` refuses to write zero. **A byte-exact compare
against a value with no author cannot succeed**, and every cache the Registry had
ever written refused against its own expected projection. The same full-body
compare in the operator made `RegistryActivationModeV1::Repeat` unreachable.

Fixed at `0e6bb66e`. The contract compares the spans its projection authors; the
operator, which derived the address and therefore holds the bump, now requires
the observed byte to be zero (an older body, that constant's documented
fallback) or exactly the derived bump — a check the tree did not have.

**Why the suite stayed green** is the transferable part: the operator fixture
staged its existing cache from `expected_cache.to_bytes()`, so its bump byte was
zero and its body was the one shape that could still pass. The fixture was
testing the projection against itself.

Confirmed in execution, not only in tests: the re-run's administration stage
reported "campaign: 33 transactions submitted this run" with all five roles
activated and no mismatch.

## 6. Wall two, open: `0x5182` at the atomic founding, and it is not General's

The founding then refused at the atomic DCLTGMF3 Open leg:

```
Instruction 3: custom program error 0x5182
  = ClaimsFoundingSbfErrorV5::Release
    ("Core caller authority or current release selection refused")
```

848,697 CU for the transaction, of which claims-sbf consumed 142,018 before
refusing. CPI order: Trading → Custody (`DCLTPCM01`) → Core (`DCLTGFA1`) →
Custody (`DCLTPCR01`) → **Claims**, which fails. The hostile probe immediately
before it *succeeded* — "DCLTGMF3 refuses a substituted Claims request and rolls
the whole founding back" — so the route is reachable and the genuine request is
what refuses.

**The control**: the same commit, the same checked release root, the same
validator recipe, the only delta being `DCLUTCH_MARKET_CAPABILITY=general` versus
unset. Direct refuses **identically** — same code, same instruction index, same
hostile-succeeds-then-genuine-fails shape. General reached it at slot 11834,
Direct at slot 6776. The capability family is not a variable and the publication
closure is not implicated.

### Two suspects eliminated from chain state

A validator restarted read-only on the failed run's own ledger says the
activation cache is in perfect shape: `DCLTACT1`, schema 1, profile 1, 1288
bytes, Registry-owned, all five 248-byte role blocks written with nonzero
artifact release ids, reserved bytes canonical — and byte 12 carries bump 254,
with `create_program_address(["dclutch:release-activation:v1", release_set_id],
registry, 254)` reproducing the cache address exactly. So
`authenticate_activation_cache_bump_v1` passes and `a40ef689`'s carried-bump
reader is **not** the refusing conjunct, which was the leading suspect on CU
grounds.

`require_readonly_frame`'s transaction-level writability merge — the class named
in `80b78181` — is also eliminated: the failing transaction has exactly one
protocol instruction (three ComputeBudget invokes, then Trading at index 3), so
no second instruction can merge a writable privilege onto those accounts.

### What is left

Everything raising `0x5182` lives in
`programs/dclutch-claims-sbf/src/founding_v5.rs`:

| site | conjunct |
|---|---|
| 459, 462 | caller-authority seeds and address |
| 480, 487, 495 | permit owner/width, decode, address vs `intent.bump()` |
| 569–597 | `authenticate_permit_body`, a nineteen-field intent-vs-request join |
| 673, 687, 691 | the activation-cache role loop — cache half eliminated above; the **per-role deployment authentication** is not |

The 142k CU still points at the role loop, and its deployment half is the only
part not eliminated from account state alone.

**Context worth carrying**: General founding succeeded on 2026-08-29 with 65
records. Between then and now, four bump-carrier commits landed in one afternoon
into the single transaction that composes all five programs — `e93fe5e9`
(CoreState carries the founding's bumps), `395210c9` (the capability seal),
`e0a2fd25` (the aggregate and position), `a40ef689` (the activation cache). One
defect of exactly that class has now been found and fixed. This looks like a
fifth of the same shape.

## 7. What this lane did NOT verify

- **No General root was activated anywhere.** The driver has never executed its
  `--execute` path on any cluster. Its offline controls pass and its instruction
  frame is the Direct route's, which has created a real root on devnet — but
  that is inheritance, not execution, and it is named as such.
- **The founding wall is characterized, not fixed.** Two suspects are eliminated
  and the remaining candidate list is short, but I did not convict a conjunct.
- **Zero devnet writes.** Everything here is loopback on hbox.
- **No CU measurement of the activation.** Nothing of mine ran on chain beyond
  record publication; the 142,018 CU figure is the refusing Claims leg, not an
  activation cost.
- **The activation records are published but never READ by the seam.** Their
  addresses are proven to be the ones the driver derives and the ones the
  founding finalizes; no transaction has yet handed them to
  `process_activation`.
- **Rational, Structured and Fractional are untouched.** Rational and Structured
  still have no capability root tail; Fractional's remains impossible. This lane
  closed General's line only.

## 8. Reproducing

Both ledgers are preserved on hbox and restart read-only on their own port
block: `/var/tmp/dclutch-genpub-probe2` (General) and
`/var/tmp/dclutch-genpub-direct` (Direct), 2.9 GB each. A fresh checked release
at `0e6bb66e` — freshness PASS, zero SBF diagnostics — is at
`/tank/dregg-build/dclutch-genpub-release2`, so the next lane need not spend an
hour rebuilding one.

```sh
SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=6 DCLUTCH_MARKET_CAPABILITY=general \
  swarm-build python3 tools/release/private-validator-lifecycle/run.py \
  --repo <clean clone at the gate commit> \
  --release-root /tank/dregg-build/dclutch-genpub-release2 \
  --validator .../solana-test-validator --solana .../solana \
  --work /var/tmp/<fresh> --through participant --seeds 1 \
  --hold-after-participant
```

Keep `--work` on ext4, not ZFS. At the hold, the activation runs against the
held validator:

```sh
dclutch-local-successor-bootstrap \
  local-private-validator-general-capability-activation-v1 \
  --rpc-url <the handoff's rpc_url> \
  --plan <run>/mutable/plan.json \
  --campaign-report <run>/founding.json \
  --payer-keypair <run>/mutable/keys/campaign-payer.json \
  --output <fresh>.json --execute
```
