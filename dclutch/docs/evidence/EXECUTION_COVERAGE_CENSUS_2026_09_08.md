# Execution coverage census — 2026-09-08

This is the dated authority for whole-protocol execution coverage at census
source revision `4ef5a4eb6a313459395434ec712361090e888981`. It keeps four facts
separate: current route inventory, finalized native instruction evidence,
historical binding claims, and ProgramTest. A compile, fixture, refusal, status
sentence, or binding without its transaction evidence is not an accepted route.

## Exact result

The repository-owned census enumerates **164 routes, 456 protocol refusal
codes, and zero unclassified dispatch positions** at the named revision. The
checked-in local-validator corpus folds **five accepted routes, zero
refusal-only routes, and 159 routes without a checked-in exact local-validator
observation**. Each admitted row has a successful signature and slot, the
native instruction bytes, the invoked program address from runtime logs, and a
selector that uniquely identifies the route.

| Substrate and proof | Accepted routes | Refusal-only | Meaning |
| --- | ---: | ---: | --- |
| checked-in exact local-validator corpus | 5 | 0 | One preserved Structured Claims CPI and four Dealer transactions |
| checked-in finalized devnet witnesses | 46 | not folded here | Historical public-chain route execution; read from existing witness files, with no new RPC |
| exact historical Agave union | 50 | — | Devnet plus local-validator, with the Claims route counted once |
| exact current/final-source local-validator replay | 0 | 0 | No campaign in this record ran ELFs built from `4ef5a4eb6`; this is the release-closing debt |

The five exact local-validator routes are:

| Route | Runtime source | Signature / slot | Accepted poststate |
| --- | --- | --- | --- |
| `claims/founding_v5::process` | Structured preserved validator; inventory capture `b5628fe1f` | `2U3PJ1SLaQ7Q5jwk7gf5oYKX4VjEjyFt2nAcPmRUXeQGQSJEDvwdUoKBw12f7wxuukJMJ8fkKa9KEtmc6i82mUKR` / 9,789 | Claims founding CPI accepted; route identity is native `DCLFDR05` sent to the captured Claims program |
| `trading/scoring_dealer_v1::found::process_dealer_found_v1` | runtime `7d5f920a4`, host `c04d95a0c` | `4DPottw5ENC3ezTjL6BKV2BRdpMkqjfqmX5TsakDDrPo9xTv6cDgUhWB88gE5abRb5dX8WXAVJPPQW8i8SqHPsbN` / 8,214 | Dealer record and deposited collateral read back |
| `trading/scoring_dealer_v1::quote::process_dealer_quote_v1` | runtime `7d5f920a4`, host `c04d95a0c` | `3Xc6iDeP6otxQgoLCcizYoq2dhSX6XisJnWtbpVvTtmceKUwzUY3jHRBZ1Cau5xsvSfwJzg7KrxzH9Nn2jGiY4bc` / 8,246 | Quote accepted against the founded Dealer |
| `trading/scoring_dealer_v1::fill::process_dealer_fill_v1` | runtime `7d5f920a4`, host `c04d95a0c` | `5juD5dkRCVfD3qKbwaWyE4W3dQ2p7BtXjoYmWzRY8arvmWBfKTTyXZqXCUGn39jd9EWDHmopbSejYzwWF2cNdYxh` / 8,408 | Nonzero fill: 27 Hoard atoms, 9 taker atoms and 18 Dealer atoms, with exact claim balances |
| `trading/scoring_dealer_v1::withdraw::process_dealer_withdraw_v1` | runtime `7d5f920a4`, host `c04d95a0c` | `2xtEfxu5UfHGRXiaEVsbAK3a9rgWvLeMEc5ykfZ7gVGr81FnKVsCgEfQ7svRS3J9pX4CHzJw7J1MUdRUf6jo6xzy` / 8,443 | Exact three-atom withdrawal |

The Dealer campaign carries founding and funding infrastructure. Only these
four Dealer instructions are bound here. It does not prove the accelerator
arm, settlement, resource retirement, or full Dealer lifecycle retirement.

## Why 119 is not the accepted total

Filtering every repository `bindings.json` to `outcome == "executed"`, joining
the declared substrate, and unioning the existing successful devnet witnesses
produces **119 historical route claims before Dealer** and **123 after the four
Dealer routes**. The current non-disjoint inputs are 46 devnet routes, 62
local-validator binding routes, and 71 ProgramTest binding routes. Choosing the
strongest declared substrate makes those 46 devnet, 36 local-validator, and 41
ProgramTest-only.

That 123-route number is an authored historical claim union. It is not the
exact ledger total. After the 50 routes with durable native Agave evidence are
removed, **73 claimed successes still lack checked exact evidence here**: 32
have a local-validator binding as their strongest remaining claim, and 41 are
ProgramTest-only. The dated JSON lists every route in both sets. The old 119
figure therefore cannot be promoted to accepted runtime coverage merely by
reading `status = witnessed`, and the four new Dealer rows do not repair its
missing evidence.

The generated `docs/reference/route-witnesses.md` still classifies any route
named by a binding as witnessed before consulting the binding outcome and
without consuming the folded ledger. It is useful as a binding register, but
it is not the acceptance census. The exact reporter is
`dclutch-route-census` over `out/ledger.json`.

## Current campaign disposition

| Family | Durable finding | Census consequence and owner |
| --- | --- | --- |
| Dealer | Four accepted local-validator transactions with native bytes and nonzero Fill/Withdraw poststates | Folded exactly. Final-source replay and remaining settlement/retirement stay with Dealer and gauntlet owners. |
| Direct | The runtime-567 validator journey accepted founding/Open, admission, nonzero trade and fees, resolution, payout and retirement | Historical source-split poststate evidence. Journey owner must replay with `--census` from final sources; the binding alone is not this ledger. |
| Claims | Six named real-ELF ProgramTest targets accepted their tested paths; one preserved Claims CPI has exact validator evidence | ProgramTest stays separate. Claims/gauntlet owners owe validator evidence for ProgramTest-only and unwired routes. |
| Custody economics | A 15-transaction validator campaign accepted Found, Propose, matured Apply, Withdraw, upkeep Found and nonzero Credit | Historical JSON predates native instruction capture, so it cannot be exact-folded. The runner now supports `--census`; economics owner must replay it. |
| Recovery | Exhaustion reached terminal admission and three refunds; the shorter fresh diagnostic reached Recovery selector 0 and a Core terminal poststate | Historical source-split evidence, without a checked accumulated ledger. Recovery/gauntlet owner must replay and fold the accepted route packets. |
| General | Fresh validator work accepted infrastructure, root activation, accelerator invocation and OpenBatch; PlaceOrder still exhausts compute | Nonempty acceptance remains pending. The `DCLTGMF3` outer route remains ambiguous because its predicate has additional length/decode facts. General owner must finish PlaceOrder and emit uniquely corroborated route evidence. |
| Series | ProgramTest expiry/constructor controls exist, while the two-occurrence test reused occurrence-zero child requests | Compile and fixture results do not count. Astra owns occurrence-aware child requests; Sol owns the evaluator bank correction. They owe two accepted validator occurrences folded against final sources. |
| Ensemble | Three captures accepted. Corrected native replay accepted the Source transition only after the deadline; the live Fold later refused `OutputState` | `process_ensemble_fold` and reclaim remain without accepted evidence. Source/Resolution owns the output producer, actual Fold and reclaim. |
| Structured | Claims operations accepted in real-ELF ProgramTest and the validator accepted a root-activation prefix; one Claims founding CPI is folded exactly | ProgramTest and prefix evidence do not close representation or retirement. Structured/Claims owner owes fresh validator receipt, representation, terminal and retirement poststates. |

## Routes with no historical successful claim

There are **41**, comprising 23 `status-report`, 7 `unwired`, 6 `structural`,
3 unrecorded and 2 repointing rows. There are zero refusal-only routes in this
historical claim comparison. The machine-readable store contains one owner row
per route; this table groups the same complete set for execution.

| Routes | Count | Queue owner and next evidence |
| --- | ---: | --- |
| `accelerator/dealer::process`; `accelerator/dealer::process_scoring_row_v1` | 2 | Dealer accelerator program owner plus gauntlet evidence owner: accepted accelerator-arm packets and exact fold |
| `accelerator/series::evaluate_selected_and_publish#accepted`; `accelerator/series::process`; `claims/series_founding_transport_v1::process`; `core/series_open::process`; `core/series_permit_expiry::process`; `core/series_permit_expiry_precommit_v1::process` | 6 | Astra recurrence owner and Sol evaluator owner, then Series validator campaign owner: two real occurrences and native census fold |
| `accelerator/set_return_data#ChunkedBankV2`; `accelerator/set_return_data#OutputPageV3` | 2 | Accelerator program evidence emission plus gauntlet runner/bindings owner |
| `claims/fractional_atomic_v3::process`; `claims/market_closure_v1::process`; `claims/process_open#WholeUnwrap`; `claims/process_terminal#TerminalZeroBurn` | 4 | Claims program owner plus gauntlet owner: wire existing ProgramTest executions, then run closure on a validator |
| `core/activate_capability_child#ActivateCapability`; `core/capability::process#ActivateCapability`; `core/close_capability_child#CloseCapability`; `core/open_market::process#OpenMarket`; `custody/abort_open_and_close#AbortOpenAndClose`; `custody/lock_hoard#LockHoard`; `custody/refund_and_close#RefundAndClose`; `registry/continuation_v1::process` | 8 | W1d/cycle-2 Open-chain owner: accepted Open/capability/close sequence with exact poststates |
| `core/infrastructure_v2::process_initialize_v2`; `core/process_instruction#else`; `core/resolution::process#CloseFund`; `core/retire_v1::process#Retire` | 4 | Release/devnet, malformed-action hostile, and retirement owners. `CloseFund` is a recorded repointing and carries no execution debt until its composed caller exists. |
| `registry/hot_continuation_v2::process`; `trading/hot_v3::process_capability_seal_close_v1` | 2 | Direct-Hot gauntlet owner and devnet seal steward: accepted validator continuation and real seal close |
| `resolution/core_effect::process_close#Retired`; `resolution/derived_transport_v1::process_derived_settle_v1`; `resolution/process_admit#AdmitTerminal`; `resolution/process_close#CloseFund`; `resolution/process_ensemble_fold#EnsembleFold`; `resolution/process_reclaim#magic`; `resolution/process_reclaim_member_seat#ReclaimMemberSeat`; `resolution/provider_instruction_v3::process_provider_resolution_v3#count` | 8 | Source/Resolution/provider owners: compose close when live, finish Ensemble output/Fold/reclaim, and add a counted-provider accepted case |
| `trading/generic_founding_stages_v1::process_generic_found_and_permit_v1`; `trading/generic_founding_stages_v1::process_generic_market_open_v1` | 2 | Tier-1 shape owner: bind actual accepted founding-stage packets |
| `trading/outer::process_capability_lifecycle#else` | 1 | W1d/W2d. This is a repointing row; no acceptance debt is asserted until the route is retained as intended behavior. |
| `trading/user_position_admission_v1::process_user_position_admission_v1#Admit`; `trading/user_position_admission_v1::process_user_position_admission_v1#Close` | 2 | Trading evidence emission plus gauntlet runner/bindings owner |

## Reproduction and final-source closure

Run the dated offline verifier from the repository root:

```sh
docs/evidence/execution-coverage-2026-09-08/verify.sh \
  /private/tmp/dclutch-execution-coverage-20260908 \
  4ef5a4eb6a313459395434ec712361090e888981
```

It exports that commit, enumerates the inventory, folds both checked-in native
captures, renders `CENSUS.md`, checks the exact five-route ledger, and writes
`HISTORICAL_BINDING_CLAIMS.json`. The accepted run produced exact-ledger SHA-256
`f5f1d29961ccf7f4cbd0b0ce2c1bffaff0e37d789bbf3906dda9511861a0b145`.
The Dealer evidence subset is
`0e7b5eb80b5ef780696de621c7d897a18945adae7e03e42af300774efe4b534c`.

Final-source replay uses the same authority rather than translating results
into another report:

1. Commit the complete final source and run `tools/gate census --commit REV
   --work ABS_WORK` once.
2. Build all eight ELFs from that exact commit and start a fresh checked local
   validator. Run each family campaign with its census switch and
   `DCLUTCH_GAUNTLET_WORK=ABS_WORK`. Journey, tier 1, ladder, relayed, lineage,
   General, Dealer and economics already have a census path; missing family
   runners are the owners listed above.
3. Preserve each campaign's program map, native transaction evidence,
   poststate report and source/artifact hashes. `census observe` must accept the
   binding against the final inventory; a binding file by itself contributes
   zero exact coverage.
4. Render `tools/gate census --commit REV --work ABS_WORK --no-tests`. Closure
   requires every intended route to read `EXECUTED` from accepted finalized
   instructions. Keep ProgramTest, devnet, historical source-split and
   refusal-only totals separate.

The dated verifier deliberately asserts the 164-route shape above. If final
source adds or removes a route, it refuses, forcing a new dated inventory and
owner delta instead of silently changing this record.

## Stored artifacts

| File | Purpose |
| --- | --- |
| `docs/evidence/execution-coverage-2026-09-08/exact-local-ledger.json` | Five exact admitted local-validator observations |
| `docs/evidence/execution-coverage-2026-09-08/historical-binding-claims.json` | Reproducible 119-before-Dealer / 123-after-Dealer comparison, exact-evidence split, and all 41 owner rows |
| `docs/evidence/execution-coverage-2026-09-08/dealer-evidence.json` | Four successful Dealer transactions with native instructions |
| `docs/evidence/execution-coverage-2026-09-08/dealer-programs.json` | Program identities from that run |
| `docs/evidence/execution-coverage-2026-09-08/classify.py` | Dated successful-outcome and substrate classifier |
| `docs/evidence/execution-coverage-2026-09-08/verify.sh` | Clean-commit inventory, exact fold and assertions |
