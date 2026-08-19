# Measured liveness and terminal-admission profile

Status: **R1 ARTIFACT SEALED / RESOLUTIONWORK MEASURED / DEFAULT VALUE
ADMISSION FAIL-CLOSED / PROTOCOL ECONOMIC ADMISSION STOP** (2026-08-19).

The executable profile is
[`research/liveness-policy-profile`](../../research/liveness-policy-profile).
It binds exact local-bank observations, fixed account widths, pinned default
rent, explicit policy inputs, runtime reward constants, and terminal account
ownership. It does not guarantee future Solana inclusion, quote a SOL price,
assume future volume or subscribers, debit Hoard collateral, or promote a
mainnet Realm.

## Result first

One important subsystem now closes honestly: routed occupation
`ResolutionWorkV1` decomposes resolution into bounded Begin, Fold, Finalize,
and expired Abort transactions; its measured routes meet the selected 25%
sample-headroom rule; its runtime rewards match the selected quote policy; and
Finalize/Abort return exact payer principal, pay keepers only from the prepaid
work compartment, send unsolicited lamports to the incinerator, and close both
Work and Reserve.

That result does **not** make the whole protocol economically admissible:

- the default program has no released source provider, so `Endow` now refuses
  before admitting collateral;
- live Direct V2 can freeze an empty epoch without a lapse, and its retained
  top-three `Select` reaches the 1.4M-CU limit and rolls back;
- most existing accounts do not record a rent payer or expose an authenticated
  close route;
- existing outcome mints have no `MintCloseAuthority`;
- Hoard donations, bearer-burn forfeiture, and fractional sub-lot residues have
  no selected terminal disposition.

Accordingly the profile publishes measured route quotes and bounded capital
requirements, but it does **not** emit a complete promoted `LivenessPolicy`
tuple. A STOP is not converted into a finite placeholder.

The sealed default ELF is `a5725a3d8e149b2b52605e1785f7ad29fdc6b2db1ed32ca83a31b41822d6b6a1`
(`1,228,192` bytes), built from runtime source `7e8f6b1`. Two ordinary clean
builds are byte-identical. A relocated Cargo-home build is explicitly
path-sensitive and therefore is not substituted as the canonical artifact.
The release/test ancestry is `316c620`; it changes no one of the 88 declared
runtime-source paths.

## Policy inputs, not measurements

The selected research policy is:

| input | value |
| --- | ---: |
| requested sample headroom | `5/4` |
| compute-envelope quantum | `50,000` CU |
| transaction ceiling | `1,400,000` CU |
| route base-fee cap | `10,000` lamports |
| priority-price cap | `1,000,000` micro-lamports/CU |
| keeper surplus | `100,000` lamports |

For measured transaction consumption `C`:

```text
required_CU = ceil(5*C/4)
limit_CU    = round_up(required_CU, 50,000)

if limit_CU > 1,400,000: STOP (no lamport quote)
otherwise:
  external_fee_cap = 10,000 + ceil(limit_CU * 1,000,000 / 1,000,000)
  keeper_reward    = external_fee_cap + 100,000
```

The largest raw sample that can pass is `1,120,000` CU. The fee and keeper
inputs are policy ceilings, not evidence of landing probability or keeper
participation. A finite reserve covers the named finite payout but cannot
defeat censorship or unbounded congestion.

## ResolutionWork V1

The final route campaign selected and executed explicit compute limits and the
same `1,000,000` micro-lamport/CU price used by the quote:

| route/shape | largest observed CU | selected limit | policy reward |
| --- | ---: | ---: | ---: |
| Begin | `810,992` | `1,050,000` | caller-paid `1,160,000` |
| Fold(1) | `804,616` | `1,050,000` | `1,160,000` |
| Fold(2) | `812,193` | `1,050,000` | `1,160,000` |
| Fold(3) | `813,128` | `1,050,000` | `1,160,000` |
| Fold(4) | `815,573` | `1,050,000` | `1,160,000` |
| Finalize | `1,094,832` | `1,400,000` | `1,510,000` |
| expired Abort | `587,197` | `750,000` | `860,000` |

Begin has `11` accounts, Fold `8`, Abort `8`, and Finalize `15 +
outcome_count`. The Work account is `1,296` bytes and the zero-data Reserve is
`0` bytes. At the pinned `Rent::default()` minima their principals are
`9,911,040` and `890,880` lamports.

Runtime V1 pays per successful Fold call, not per record:

```text
fold_base_reward   = 1,160,000
fold_record_reward = 0
finalize_reward    = 1,510,000
abort_reward       =   860,000
```

Every width `1..=4` is accepted, so the reserve prices the most expensive
reachable partition rather than assuming the largest Fold. With positive equal
per-call rewards the worst partition uses singleton Folds:

```text
rent = 9,911,040 + 890,880 = 10,801,920
success(n) = n*1,160,000 + 1,510,000
abort(n)   = n*1,160,000 +   860,000
deposit(n) = rent + max(success(n), abort(n))
           = 12,311,920 + n*1,160,000
```

For the hard archive maximum `n=32`:

```text
persistent Work+Reserve deposit = 49,431,920 lamports
caller-paid Begin quote         =  1,160,000 lamports
maximum cold payer outlay       = 50,591,920 lamports
```

The reserve is a maximum path, not the sum of mutually exclusive Finalize and
Abort. After `k` singleton Folds, Abort pays `k*1,160,000 + 860,000` and returns
the exact remaining payer reserve and both rent principals; the terminal split
sends only the monotone donation compartment to the neutral sink.

## Source/value admission

`CreateMarket` may still create economically inert research infrastructure.
`Endow` is the sole collateral-value gate. Its ABI appends read-only Terms and
SourceSpec accounts and authenticates their full owner, PDA, bump, body,
digest, feed, adapter, and deployment-generation bindings before owner-plane
writes or Token-2022 CPI.

The default release registry is intentionally empty. The canonical 15-account
Endow therefore refuses `SourceReleaseUnavailable` (`0x79`) and the bank test
proves exact byte and lamport rollback, including hostile one-lamport prefunds.
This repairs the old “accept value into a market with no compiled source
release” counterexample by disabling value admission; it does not prove future
provider uptime, Feed/archive construction, source cleanup, projection-buffer
availability, or source-work capitalization.

The mock-provider success path is a different, explicitly non-production ELF.
Its CU may be reported as auxiliary feasibility evidence only and never mixed
into the default artifact's admission projection.

## Native and occupation resolution boundary

The same sealed default ELF and the corrected committed fixture at `161f530`
pass all `15/15` native-resolution bank tests. The fixture-only correction
replaces a stale source-version literal (`1`) with the canonical version (`7`);
it does not change the program ELF. Point-v3 Resolve is admitted by the chosen
sample-headroom policy:

| degree | Resolve CU | retry CU | internal redeem CU | bearer redeem CU |
| ---: | ---: | ---: | ---: | ---: |
| 1 | `1,088,267` | `934,622` | `775,978` | `785,073` |
| 2 | `1,096,640` | `942,995` | `778,021` | `786,873` |
| 3 | `1,103,534` | `949,889` | `776,411` | `784,578` |

Each point-v3 Resolve rounds to the `1,400,000`-CU policy limit and a
`1,510,000`-lamport keeper quote. The old fixture run's `Custom(80)` refusals
are retained in the upstream audit as superseded negative evidence of fixture
drift, not characterized as a runtime regression.

Monolithic occupation-v4 initial Resolve remains a policy STOP. Its same-ELF
degree-1 through degree-3 samples are `1,243,529`, `1,254,699`, and
`1,268,630` CU, all above the `1,120,000` raw-CU headroom boundary. Their
retries pass that boundary, but a retry cannot make an inadmissible initial
route admissible. The live action is the measured staged ResolutionWork route
or fail-closed refusal; there is no extrapolated span/degree allowance.

## Direct selection

Live Direct V2 remains a measured STOP. Its successful top-three Select reaches
exactly `1,400,000` CU and rolls back. Empty frozen epochs lack a lapse, and
Candidate, Window, receipt, pot, page, and Reservation V1 accounts lack exact
rent-owner cleanup.

The executable Direct V3 research model at `ef32495` supplies the intended
repair: staged Begin/Verify/Finalize, empty/pre/post lapse, at most three live
Candidates, a 64-tick lifetime replay bitmap, exact payer principals, monotone
donations, typed reservation transitions, and a separate WorkBudget. It remains
**MODEL / DESIGN ONLY**: no live ABI, SBF path, account allocation, CU/rent
campaign, deployment identity, or pre-Freeze abort has been promoted.

When measured, its reserve must be a path maximum:

```text
selected success = Begin + 3*Verify + Finalize + Settle
unselected lapse = Begin + 3*Verify + Lapse
selected lapse   = Begin + 3*Verify + Finalize + Lapse
empty lapse      = Lapse
reserve          = max(all reachable paths)
```

Candidate submission is optional caller-paid work. Nothing is divided between
two orders without an explicit contribution owner.

## Storage and terminal truth

The sealed account probe prints 37 actual account classes plus exact default
rent. The executable terminal inventory classifies each as:

- `PERMANENT_INFRA`: deliberately capitalized, bounded in its semantic scope,
  and not described as refundable;
- `REFUNDABLE_TRANSIENT`: immutable payer, exact principal, neutral donation,
  expiry/reaper, physical close, and rollback evidence all present;
- `UNCLASSIFIED_STOP`: one or more of those properties is absent.

Only ResolutionWork and its Reserve currently satisfy
`REFUNDABLE_TRANSIENT`. Artifact stages close, but their legacy rule gives all
prefunds to the recorded funder rather than separating neutral donation, so
they remain a profile STOP. Position/Replay, source state, order/direct state,
and legacy clearing state do not have complete refund ownership and terminal
routes. Existing outcome mints are explicitly permanent; this profile never
calls their rent refundable.

The terminal checker also refuses any claim that all value is recoverable.
Required but unreachable routes are STOPs, not omitted rows. Hoard value never
counts as liveness capital.

## Evidence boundary

The R1 evidence seal pins:

- one exact default ELF digest and byte length;
- two byte-identical ordinary builds, one explicitly path-sensitive relocated
  build, and stack/final-LTO diagnostics;
- each local-bank capture to that same ELF;
- every measurement's exact test blob and runtime source tree;
- the account probe's own Cargo manifest, lockfile, and Rust source;
- the normalized capture and every retained build/bank log by SHA-256.

The historical probe executes only the bytes archived at the runtime-source
ref; test fixtures are pinned separately at the release/evidence ref. It never
copies a later probe into an older runtime tree. Any source, ABI, test, probe,
artifact, or capture drift requires a new profile.

The upstream artifact report and checksum ledger are themselves copied into
the profile. Their SHA-256 digests are
`46de5a07ccb2aed018cd8c88a762e6bd5d737d488fe2e992c7abfb420fe4bf1d`
and `baaa5ee5ac3e6372faf9fe82cd60e31ed53e93d62b37b8314ef9f6f1634d4ac0`.
The audit found zero diagnosed first-party final-LTO stack survivors; all
`40,389` direct `r10` references are at or below `4,096` bytes.

## Promotion state

Current overall state is **MEASURED_RUNTIME / ECONOMIC_ADMISSION_STOP**.
Promotion requires, at minimum:

1. a real released source provider plus funded, bounded source/archive work and
   terminal sharing/cleanup;
2. a live staged direct lifecycle (or disabling Freeze) with measured path
   rewards and exact account close/refund evidence;
3. terminal disposition for Hoard surplus/burn forfeiture and fractional
   claimant residues;
4. an explicit permanent/refundable decision for every remaining account;
5. a selected operator policy for fee/priority caps, followed by landing and
   keeper-participation evidence.

No Hoard debit, venue revenue, future subscriber, future volume, reward token,
token price, or token-to-SOL assumption appears in an admission equation.
