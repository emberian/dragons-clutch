# Dealer policy SBF vertical slice

Status: **REAL LOCAL-SVM POLICY CATALOG / EXPLICITLY NON-PRODUCTION / NO
LIQUIDITY OR TRADING CAPABILITY**.

This is the first executable adapter boundary for the frozen covered-dealer
contract. It persists one exact `DealerPolicyV1` through a signed, resumable,
content-addressed SBF path. It does not claim a funded Dealer facility, a
market admission, liquidity, trading, a price certificate, or settlement.

The separate profile identity is:

```text
dragons-clutch/capability-profile/non-production-dealer-policy-catalog-lab/v1
cb8025ae72a0bc8666d9319be6fb678282d5a912969e6a10dfcddd8406237d72
```

Every production profile rejects the four allocated Dealer coordinates before
account inspection. The laboratory profile rejects every legacy intent and
enables only Dealer family 76, version 1, local actions 1 through 4.

## Wire and account contract

| action | payload | replay owner |
| --- | ---: | --- |
| `BeginPolicy` (1) | policy ID 32 + neutral sink 32 + expiry slot 8 | outer sequence exactly zero and absent stage PDA |
| `WritePolicy` (2) | policy ID 32 + cursor 2 + active length 2 + padded chunk 192 | outer sequence, payload cursor, and stored cursor equal |
| `SealPolicy` (3) | policy ID 32 | sequence exactly 1,148 and stored cursor complete |
| `AbortPolicy` (4) | policy ID 32 | sequence equals stored cursor |

The stage is `0x7d/1`, exactly `140 + 1,148 = 1,288` bytes, at:

```text
[b"dc-dealer-policy-stage-v1", funder, policy_id]
```

Its header owns the full policy ID, funder, neutral sink, stored bump, strict
cursor, exact length, creation/expiry slots, full refundable rent principal,
and hostile creation prefund. The body is zero-initialized and only the next
strict 192-byte chunk (or final 188 bytes) may be written. Inactive chunk bytes
must be zero.

The immutable catalog is `0x7e/1`, exactly `56 + 1,148 = 1,204` bytes, at the
pure contract's canonical PDA:

```text
[b"dc-dealer-policy-v1", policy_id]
```

Its adapter header owns the stored bump, catalog funder, permanently locked
rent principal, and creation-time final-PDA donation. The body remains the one
semantic truth. Seal hostile-decodes `DealerPolicyV1`, validates its local
semantics, recomputes
`SHA256("dragons-clutch/dealer-runtime/policy/v1\0" || body)`, and requires
the result to equal the request, stage, and PDA identity.

## Ordered account metas

- Begin: funder signer/writable; stage writable; System executable/read-only;
  Rent read-only; Clock read-only.
- Write: funder signer/read-only; stage writable; Clock read-only.
- Seal: funder signer/writable; stage writable; final writable; neutral sink
  writable; System executable/read-only; Rent read-only; Clock read-only.
- Abort: caller signer/read-only; stage writable; stored funder writable;
  stored neutral sink writable; Clock read-only. Before expiry caller must be
  the funder; after expiry any non-conflicting signer may reap.

Payer, stage, final, and sink identities are authenticated and forbidden alias
classes are checked explicitly. Program ownership, executable bits,
writability, exact lengths, tags, versions, padding, stored bump, PDA, and
Clock/Rent/System identities all fail closed.

## Rent and hostile prefunds

Stage and final creation use full-principal funding. A one-lamport predictable
PDA squat never discounts the funder's principal.

- Stage balance after Begin is `hostile_prefund + exact_stage_rent`. At Seal or
  Abort the funder receives exactly `exact_stage_rent`; the creation prefund
  and every later surplus go to the stored policy neutral sink.
- A prefunded final PDA is PDA-signed back to zero and credited to that sink
  before allocation. The funder then supplies the full final rent minimum.
  The immutable catalog retains exactly that principal.

Hoard principal, collateral, LP assets, fees, future fee revenue, and any
economic Dealer budget are never rent sources.

## Evidence and remaining blockers

The real-ELF SVM campaign covers successful Begin/Write/Seal/Abort, strict
replay, full account-image invariance on refusal, transaction-wide rollback of
a prior System transfer, wrong-Rent and owner/sink identity substitutions,
one-lamport stage and final squats, exact owner/PDA/body/creation rent, sink
routing, and duplicate Seal refusal. It also submits a valid legacy Split to
prove the laboratory profile refuses it before accounts. The production ELF
separately proves pre-account rejection of the same allocated Dealer request.
No mock-source account, feature, parser, fixture, or dependency participates
in this route.

Facility initialization remains blocked. The pure contract now owns canonical
`DealerFacilityGenesisV1`, `DealerFacilityPositionV1`, and
`FacilityPositionBindingV1` bodies, including the full policy/Market,
collateral/token, Position content/account/Replay, exact asset accounting, and
exact DealerState authority joins, but this catalog profile does not persist
or authenticate them. A 276-byte `DealerRootTombstoneV1` now fixes the pure
terminal evidence and rent split, but likewise has no global tag or shrink
handler. `FeeBudgetV1` and `LivenessBudgetV1` remain exact ledgers,
not token custody or a measured presently funded schedule. The protocol still
lacks the corresponding SBF admission/root-shrink route, budget asset owner,
complete Position/Replay transfer transition,
price-quantization certificate, and maximum-width liveness derivation.
Therefore all 22 economic `DealerRuntimeActionV1` variants remain disabled,
including `Initialize`, contribution, activation, selection, collection,
delivery, resolution, claims, and retirement.

Run the real-bank laboratory with:

```sh
programs/clutch-sbf/svm-tests/run_svm_tests.sh \
  --profile-non-production-dealer-policy-catalog-lab \
  real_sbf_catalog_is_resumable_rent_exact_and_replay_safe
```

The same checkout's production refusal is:

```sh
programs/clutch-sbf/svm-tests/run_svm_tests.sh \
  production_profile_refuses_allocated_dealer_action_before_accounts
```
