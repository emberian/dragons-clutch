# Measured liveness policy profile

Status: **INTERMEDIATE LOCAL-BANK EVIDENCE / ARITHMETIC CANDIDATE / NOT
PROMOTABLE / RUNTIME INTEGRATION PROPOSED / FINAL ELF PENDING** (2026-08-19).

The executable evidence manifest is
[`research/liveness-policy-profile`](../../research/liveness-policy-profile).
It supplies measured and explicitly unmeasured inputs to the pure
[`clutch-liveness`](../../crates/clutch-liveness) arithmetic. It does not select
a Realm policy, debit or credit a Solana account, guarantee future inclusion,
establish a market fee, value a token in SOL, or change a release STOP.

## 1. Sealed evidence identity

Every real-SBF row below used:

| field | intermediate evidence value |
| --- | --- |
| historical source/test tree | `a29902b` (program boundary `3a81b38`; source/archive core `0b96a3a`) |
| copied intermediate SBF artifact | `research/liveness-policy-profile/artifacts/c8ff4ac7286004cb/clutch_sbf.so` |
| SHA-256 | `c8ff4ac7286004cb5d897cc92b05f7a9e386107d295cb1441adcd227e0b35138` |
| artifact size | `809824` bytes |
| execution environment | local Agave `ProgramTest` bank, not mainnet |

The checker pins every named test and layout source to its Git blob, hashes the
normalized capture, verifies the local ELF byte-for-byte, materializes the
historical tree, compiles the account probe there, and recomputes every integer
below. This matters because the default fixture under
`svm-tests/tests/fixtures` is an older ELF, while `target/deploy` is mutable
during parallel builds. Every replay sets `SBF_OUT_DIR` to the digest-named
copied artifact; otherwise a test can pass against the wrong program.

`--check-current` is intentionally a separate, strict gate. The sealed profile
remains reproducible after later source work, while the current-tree gate fails
until any ABI/ELF change receives a new bank campaign and evidence version.
This c8ff artifact is an intermediate KernelAccount-v2 profile. The final seal
remains mutable until occupation-v4 work either proves it does not change the
program artifact or supplies a new committed ELF and replay.

## 2. Exact measurements

All CU values are transaction metadata samples. They are not instruction-only
costs, maxima, future prices, or inclusion promises.

| route | exact samples (CU) | selected observed maximum |
| --- | ---: | ---: |
| funded `PlaceOrder` | `598889`, `596919` | `598889` |
| funded `CancelOrder` | `470322`, `474995` | `474995` |
| `SubmitDirectPage`, two orders | `1249371` | `1249371` |
| narrow `SettlePage`, two orders | `862075` | `862075` |
| focused native Resolve d1/d2/d3 | `1092204`, `1130463`, `1165736` | `1165736` |
| exact native retry d1/d2/d3 | `938556`, `976815`, `1012088` | `1012088` |
| focused internal redeem d1/d2/d3 | `708298`, `705798`, `705473` | `708298` |
| focused bearer redeem d1/d2/d3 | `788047`, `785347`, `784552` | `788047` |
| focused `WithdrawCash` | `229755` | `229755` |

The native campaign passed all seven real-SBF tests, including non-point and
conflicting retry refusal, hostile account-role/mint-vector refusal, exact-lot
redemption, sub-lot refusal, and late-CPI rollback.

Blank-bank market creation produced these two exact samples:

| immutable Terms mode | resolution ABI | sample CU |
| --- | ---: | ---: |
| categorical degree 0 | v2, `165` bytes | `914280` |
| native degree 1 | v3, `319` bytes | `915284` |

`ProgramTest` creates a random payer, and the payer-dependent PDA syscall can
take a different number of iterations from run to run. These are therefore
samples, not measured maxima. The arithmetic candidate charges the full
`1400000`-CU transaction ceiling for `CreateMarket`; it does not promote either
sample as a bound. `InitRealm` and `InitProfile` have no isolated CU samples and
receive the same full-ceiling fallback.

The joined blank-bank lifecycle additionally executed every degree one through
degree three from creation to zero Hoard:

| degree | CreateMarket | Resolve | bearer redeem | internal redeem | WithdrawCash |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | `1081504` | `1076536` | `773047` | `697790` | `294703` |
| 2 | `1072104` | `1107963` | `762847` | `687790` | `294703` |
| 3 | `1074689` | `1144736` | `763552` | `688965` | `294703` |

Each joined row also recorded `Endow=294841`, `Split=276413`, and
`Materialize=154683` CU. Its SourceArchive is canonical and sealed but installed
at genesis; it is lifecycle evidence, not a provider-production claim. The
policy uses the larger sample across the focused and joined campaigns for each
selected work family.

### Artifact upload and seal

Only successful mandatory transactions are capitalized. Refusal-path CUs in
the resumability trace remain evidence but are not charged as work that must
succeed.

| artifact | successful transaction CU sequence | sample total CU |
| --- | --- | ---: |
| policy | `13373, 9297, 8138, 94175` | `124983` |
| grid | `17135, 15319, 12631, 9943, 9577, 107539` | `172144` |
| Terms | `23109, 28757, 26069, 23381, 20693, 18005, 15317, 12629, 9941, 8184, 18048` | `204133` |

The Terms campaign rehydrated a partially uploaded stage into a new bank,
finished it, and sealed atomically. The raw trace also retains early-seal,
duplicate-write, repeated upload, and idempotent-seal observations so the
normalizer's semantic indices are auditable.

### Source and archive seam

The source/archive host campaign passed `4/4`. Native Resolve authenticates and
consumes the sealed archive, so that read/authentication cost is already inside
the Resolve and retry samples. There is no routed production instruction for
standalone source-spec creation, archive creation, append, seal, or cleanup.
Those CUs are **UNMEASURED**. Consequently this profile does not manufacture a
`SharedFeedPair` cap from the Resolve sample.

The later native occupation-window preflight (`f69b2ee`) is also excluded: its
proposed v4 record is `383` bytes, but it has no production route, bank CU,
rent, or ELF evidence and live promotion refuses. V4 cannot silently replace
the measured v2/v3 rows.

## 3. Account widths and default rent

The probe uses `solana-rent = 4.3.0` and `Rent::default()`:

```text
minimum = (data_bytes + 128) * 6960 lamports
```

This is exact evidence for that pinned default, not a promise that a future
cluster will retain the same rent parameters. A runtime adapter must read and
validate the live Rent sysvar and refuse if a frozen reserve is insufficient.

| account | bytes | default minimum lamports |
| --- | ---: | ---: |
| policy final / stage | `266` / `402` | `2742240` / `3688800` |
| grid final / stage | `589` / `725` | `4990320` / `5936880` |
| Terms final / stage | `1656` / `1792` | `12416640` / `13363200` |
| Realm / Profile | `70` / `100` | `1378080` / `1586880` |
| Market / Hoard / Position | `726` / `108` / `220` | `5943840` / `1642560` / `2422080` |
| Kernel / Replay / Supply ledger | `1255` / `84` / `333` | `9625680` / `1475520` / `3208560` |
| Resolution v2 / v3 | `165` / `319` | `2039280` / `3111120` |
| outcome mint / immutable-owner Hoard token | `82` / `170` | `1461600` / `2074080` |
| order page / reservation | `4012` / `570` | `28814400` / `4858080` |
| candidate / candidate feed / receipt | `305` / `6266` / `217` | `3013680` / `44502240` / `2401200` |
| SourceSpec / SourceArchive | `292` / `2560` | `2923200` / `18708480` |

For the measured two-outcome v3 market constructor, its seven state PDAs, two
outcome mints, and Hoard token require `32426640` lamports at the pinned default
rent. The arithmetic candidate additionally includes:

- all three immutable artifact finals: `20149200` lamports;
- the conservative simultaneous maximum of all three stages: `22988880`;
- Realm and Profile: `2964960`.

The resulting founding storage candidate is `78529680` lamports. Charging both
all stages and all finals is deliberately conservative across seal order. It
does not imply those accounts share one production escrow or that Realm/Profile
must ultimately be charged to each market.

## 4. Unmeasured policy inputs and derivation

The following are policy inputs, not empirical claims:

| input | candidate value |
| --- | ---: |
| requested sample headroom | `5/4` (25%) |
| envelope rounding quantum | `50000` CU |
| per-transaction ceiling | `1400000` CU |
| base transaction fee cap | `10000` lamports |
| priority price cap | `1000000` micro-lamports/CU (1 lamport/CU) |
| keeper tip | `100000` lamports/transaction |

For a measured sample `C`:

```text
requested_CU = ceil(C * 5 / 4)
envelope_CU  = min(1400000, round_up(requested_CU, 50000))
work_SOL_atoms = 10000 + ceil(envelope_CU * 1000000 / 1000000) + 100000
```

The names “cap” and “tip” do not make these rates adequate. They have no landing
probability, congestion, elasticity, or keeper-participation evidence. The
candidate never converts them through a SOL/USD or token/SOL price.

| route maximum | requested CU | candidate envelope | 25% gate | work lamports |
| --- | ---: | ---: | --- | ---: |
| Place | `748612` | `750000` | pass | `860000` |
| Cancel | `593744` | `600000` | pass | `710000` |
| SubmitDirectPage | `1561714` | `1400000` | **fail** | `1510000` |
| narrow SettlePage | `1077594` | `1100000` | pass | `1210000` |
| native Resolve | `1457170` | `1400000` | **fail** | `1510000` |
| native retry | `1265110` | `1300000` | pass | `1410000` |
| internal redeem | `885373` | `900000` | pass | `1010000` |
| bearer redeem | `985059` | `1000000` | pass | `1110000` |
| WithdrawCash (joined maximum) | `368379` | `400000` | pass | `510000` |

Applying the formula separately to every artifact transaction yields policy,
grid, and Terms work candidates of `740000`, `1060000`, and `1760000`
lamports. Adding full-ceiling work for `InitRealm`, `InitProfile`, and
`CreateMarket` gives `8090000` market-work lamports.

The per-order clear candidate is the greater of one Cancel (`710000`) and half
of the two-order Submit envelope (`ceil(1510000/2) = 755000`). The per-order
settle candidate is half of the two-order Settle envelope (`605000`). This split
is arithmetic attribution only; it is not evidence that a different page size,
partial page, hostile order mix, or future candidate algorithm has the same
cost.

## 5. Candidate projected through `clutch-liveness`

The historical probe constructs this exact `LivenessPolicy` and asks the safe
`no_std` kernel for its market and order quotes:

| compartment | arithmetic candidate (lamports) |
| --- | ---: |
| market work | `8090000` |
| market storage | `78529680` |
| resolution | `1510000` |
| **market total** | **`88129680`** |
| per-order clear | `755000` |
| per-order settle | `605000` |
| **order work total** | **`1360000`** |

This is intentionally labeled
`INTERMEDIATE_ARITHMETIC_CANDIDATE_NOT_PROMOTABLE`. It proves
only that the checked integers fit the kernel and conserve its compartments.
It does not satisfy the requested headroom for Submit or Resolve.

## 6. Promotion blockers

1. `SubmitDirectPage` and native d3 Resolve exceed the requested 25% CU
   headroom beneath the current transaction ceiling. They need decomposition,
   a smaller admitted shape, or a deliberately weaker reviewed margin—not a
   false “maximum” label.
2. Source/archive construction, append, seal, cleanup, and terminal-failure
   routes have no production bank CU measurements. A shared source/archive cap
   therefore remains unset.
3. `InitRealm` and `InitProfile` have no isolated CU measurements. Full-ceiling
   placeholders are safe arithmetic but not measured maxima.
4. The current `OrderEndowment` models clear and settle work only. It cannot own
   the `4858080`-lamport reservation account or any order-page, candidate-feed,
   candidate, or receipt storage obligation. Runtime integration must first
   give per-order storage one canonical semantic owner.
5. No acceptable production neutral-failure sink has been selected. The probe
   uses a nonzero dummy identity only to exercise `LivenessPolicy::validate`;
   it is not a destination proposal.
6. A priority-price ceiling and keeper tip need measured landing and
   participation evidence. Finite prepaid lamports cover a finite payment but
   cannot guarantee inclusion under censorship or unbounded congestion.
7. Occupation-v4 integration is still changing the program after the c8ff
   baseline. This profile is intentionally intermediate. Any ABI, account
   width, SBF source, test, or ELF change requires a fresh artifact and replay
   before the identity can be called final.

Until all applicable blockers close, production adapter work remains
**PROPOSED**. No Hoard debit, venue revenue, future subscriber, future volume,
token price, or token-to-SOL conversion participates in this profile.
