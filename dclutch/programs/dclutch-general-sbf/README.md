# General successor physical boundary

This program is the thin Solana adapter for the Lean-owned, data-defined
General controller ABI. It imports neither the legacy General program nor any
outcome-count-specialized dispatcher. Candidate pages are borrowed and checked
one at a time; collect and distribute consume exactly one execution row per
instruction.

## Stable request and state wires

Every instruction is the 64-byte `ControllerRequestV1` generated from
`DClutchSemantics.GeneralControllerAbi`. `pageIndex` is active for `Consider`,
`Collect`, and `Distribute`; `executionIndex` is active only for `Collect` and
`Distribute`. All other coordinates are canonical zero.

Program-owned fixed-layout state is:

- selection cursor: 128 bytes, PDA seeds
  `["dclutch:general-selection:v1", market, batch]`;
- verification cursor: 960 bytes, PDA seeds
  `["dclutch:general-verification:v1", market, candidate]`;
- verified certificate: 416 bytes, PDA seeds
  `["dclutch:general-certificate:v1", market, candidate]`;
- settlement cursor: 208 bytes, PDA seeds
  `["dclutch:general-settlement:v1", market, candidate]`;
- candidate header: 256 bytes, PDA seeds
  `["dclutch:general-candidate:v1", market, candidate]`;
- selection policy: 64 bytes, PDA seeds
  `["dclutch:general-policy:v1", market, policy]`;
- candidate page: 11,840 bytes, PDA seeds
  `["dclutch:general-page:v1", market, candidate, pageIndexLE]`;
- child-signing authority: PDA seeds
  `["dclutch:general-authority:v1", market, releaseSet]`.

The selection and verification accounts may be zero-filled only at their
explicit initialization transition. Certificates and settlement cursors are
written only after verification/freeze. Candidate, policy, and page accounts
are immutable General-owned input records at the PDAs above. Their bytes are
the sole semantic facts; the adapter does not persist a second projection.

## Common release prefix

Every route starts with these exact accounts:

| Index | Account | Privilege |
| ---: | --- | --- |
| 0 | Market identity | readonly |
| 1 | Registry activation cache | readonly |
| 2 | Registry program | readonly, executable |
| 3 | selected Trading program (this program) | readonly, executable |
| 4 | selected Trading ProgramData | readonly |

Before reading or changing controller state, the adapter invokes Registry
`Reauthenticate(Trading)` with accounts 1, 3, and 4, then requires return-data
producer 2 and the exact 144-byte role receipt. The receipt must match the
activation cache's release-set, Trading program, artifact release, and semantic
release coordinates.

## Action frames

`Consider` has exactly 12 accounts:

| Index | Account | Privilege |
| ---: | --- | --- |
| 5 | selection cursor | writable |
| 6 | verification cursor | writable |
| 7 | candidate certificate | writable |
| 8 | candidate header | readonly |
| 9 | selection policy | readonly |
| 10 | request's next candidate page | readonly |
| 11 | incumbent certificate, or Market account 0 when selection is empty | readonly |

Each call consumes one page and advances the 960-byte verifier atomically. The
last page performs candidate-wide per-order rounding once, writes the 416-byte
certificate, compares immutable policy criteria with mandatory candidate-ID
tie-break, and updates selection.

`Freeze` has exactly 6 accounts: common prefix plus writable selection at 5.

`InitializeSettlement` has exactly 9 accounts:

| Index | Account | Privilege |
| ---: | --- | --- |
| 5 | frozen selection | readonly |
| 6 | settlement cursor | writable, initially zero |
| 7 | selected candidate certificate | readonly |
| 8 | candidate header | readonly |

`Collect`, `Materialize`, `Distribute`, and `Close` have exactly 24 accounts:

| Index | Account | Privilege |
| ---: | --- | --- |
| 5 | selected Claims program | readonly, executable |
| 6 | selected Claims ProgramData | readonly |
| 7 | selected Custody program | readonly, executable |
| 8 | selected Custody ProgramData | readonly |
| 9 | release-pinned General authority PDA | readonly outer; child signer |
| 10 | settlement cursor | writable |
| 11 | selected candidate certificate | readonly |
| 12 | candidate header | readonly |
| 13 | request page, or Market account 0 for Materialize/Close | readonly |
| 14 | Claims root | writable when the Claims action requires it |
| 15 | row owner Claims position, or Market sentinel | writable when active |
| 16 | settlement Claims position | writable when active |
| 17 | Claims effect replay | writable when active |
| 18 | Realm/custody binding | readonly |
| 19 | collateral mint | readonly |
| 20 | source collateral account | writable when active |
| 21 | destination collateral account | writable when active |
| 22 | Custody authority PDA | readonly |
| 23 | selected token program | readonly, executable |

The adapter reauthenticates Claims and Custody independently before their CPI.
It signs child requests only with account 9. Each child must independently
reauthenticate Trading before trusting that signer. The outer adapter consumes
each return-data receipt immediately, before another CPI can overwrite it, and
commits settlement bytes only after every child accepts.

## Canonical child boundary

This slice intentionally defines no General-private Claims, Custody, or receipt
wire. `SettlementChildrenV1` states only the semantic requirements and passes
General-owned replay context: release set, Market, candidate, owner/order/nonce,
settlement revision, and page/row coordinates. Claims owns its runtime-tail
effect plan and request digest. Custody owns its compartment-transfer plan.
Each role owns its own receipt and post-revision commitment.

The physical adapter must bind the canonical child request digest back to the
General replay context, consume the canonical return-data receipt immediately,
and require the expected child program as producer. Claims and Custody state
mutation remain separately deployed role programs; this boundary deliberately
does not reuse the experimental Direct proof children.
