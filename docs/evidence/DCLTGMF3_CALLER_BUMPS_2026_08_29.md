# DCLTGMF3 caller bumps and founding-frame contraction — 2026-08-29

## Result

`DCLTGMF3` replaces `DCLTGMF2` as the sole active generic-Market founding
outer. It preserves the atomic Lock → Found → Realize → Claims → Open rollback
domain while removing five repeated outer PDA bump searches and five redundant
child sysvar account occurrences.

The instruction is exactly 13 bytes: the eight-byte `DCLTGMF3` discriminator,
then the canonical Lock, Found, Realize, Claims, and Open bump bytes in that
order. Those bump bytes are invocation evidence, not persisted semantic truth.
For each child, Trading uses `create_program_address` over the complete existing
seed vector and requires the reproduced address to equal the account in the
frame. Custody, Core, and Claims still perform their own canonical
`find_program_address` search before honoring the PDA signature. A wrong bump
therefore refuses at the outer equality boundary, and a noncanonical address
cannot acquire child authority.

No PDA domain, seed component, request digest, role, or resulting address
changed. The operator tests reproduce each of the five canonical addresses with
`create_program_address`; the Trading hostile test changes every bump and proves
the wrong address or invalid point cannot pass the identity check.

## Runtime sysvars

The new frame removes these account occurrences and obtains the same runtime
facts through the Solana sysvar syscall boundary:

- Rent from Core's compact projected Found prefix;
- Clock from Core generic Found;
- Rent from Claims FoundingV5; and
- Clock and Rent from Core generic Open.

This is an ABI contraction, not a semantic relaxation. The ordinary
ProjectFound36 graph remains the independently owned global frame. Other Claims
routes that authenticate a caller-presented Rent profile still use their prior
account path. Only the generic founding children use runtime Rent/Clock here.

## Exact geometry

For the permanent Direct-profile founding with two physical FundingLedgerV2
accounts:

| Measure | DCLTGMF3 |
| --- | ---: |
| fixed account references before the funding tail | 125 |
| physical funding-tail references | 2 |
| Direct wrapper account references | 127 |
| complete compiled unique keys | 58 |
| static keys | 3 |
| loaded writable keys | 12 |
| loaded readonly keys | 43 |
| required signatures | 1 |
| base serialized v0 message bytes | 429 |
| message bytes after six new distinct keys | 441 |
| message bytes after seven new distinct keys | 443 |

The compiled complete-key census digest is
`8fb27f15c8509350a0702a1c6e3208ade60d6c16b48bb6d324cc721a08186561`.
The boundary tests prove 58 + 6 = 64 is admitted and 58 + 7 = 65 is refused
before any write or transaction planning. The frame itself contains no
transaction-level signer; the one signature is the distinct fee payer.

## Durable caller turnover

The public founding journal and its pre-send projection are now respectively:

- `dclutch-public-founding-submission-journal-v2`; and
- `dclutch-public-founding-pre-send-projection-v2`.

The operation is `dcltgmf3`, the exact success mutation label is
`found the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF3)`,
and the compute label is `founding-dcltgmf3`. Per-run output uses
`dcltgmf3_compute_units`; the twenty-seed summary owner uses
`dcltgmf3_compute_units_arithmetic_mean`.

The six-row ordered founding journal remains DCLTCFQ1, DCLTPCB2, DCLTGMF3,
Core funding Create, Resolution funding Activate, and Core funding Accept.
ProjectFound36, four-row AggregateRetirement, and durable SourceAbort changes
that landed ahead of this range remain intact.

## Focused tests

The final rebased source passed:

- founding operator: 4/4;
- Trading generic-founding hostiles: 7/7;
- Core generic-founding hostiles: 3/3;
- Claims FoundingV5 hostiles: 2/2;
- successor durable founding journal: 5/5;
- successor compiled lock census: 4/4;
- SDK generic-founding suite: 14/14, generated ABI check, and typecheck;
- web generic-founding suite: 14/14 and generated ABI check;
- decision-0012 dryplan: 8/8;
- private lifecycle runner: 37/37; and
- devnet activity harness: 30/30.

The generated reference document
`docs/reference/abi/genericFoundingV1.md` is intentionally not regenerated in
this lane. The release-reference owner freezes all ABI sources first and runs
the repository-wide generator and its check exactly once.

## Exact SBF and frame evidence

The code range is `8ed1f1a8^..172f3420`, rebased on main `6d9d4b36`. An exact
`git archive` of `172f3420` was expanded on hbox at
`/tank/dregg-build/dclutch-dcltgmf3-172f3420/source`. Its Git tree is
`591d92991a695552dd4dc37681d88b9ad9d05efa`, equal to the local source tree.

Builds used `swarm-build`, `cargo-build-sbf 4.0.0`, platform-tools v1.53, and
SBF rustc 1.89.0. Plain optimized links and independent
`-Zemit-stack-sizes --emit=obj,link` measurement builds both emitted zero
`overwrites values in the frame` diagnostics for every changed program.

| Program | ELF bytes | ELF SHA-256 | measured frames | deepest frame | spare |
| --- | ---: | --- | ---: | ---: | ---: |
| Trading | 1,915,752 | `9595c5856fc48fdc22f6b12520070993e65d4fde6b2a9c0b01bfad08a356e5f6` | 803 | 4,032 | 64 |
| Core | 1,047,240 | `0591bb399771a2fcf0df7fde56f9ffa97f916075242c398f64945af7073f1259` | 235 | 3,968 | 128 |
| Claims | 1,162,696 | `0549a8b373dabc9ed5ddf186f1bcbbac55af8ed6a954093ffe45beed4e812f46` | 202 | 3,904 | 192 |

The Core link's deepest measured frame is
`generic_founding_v1::authenticate_claims_and_custody` at 3,968 bytes. The
Trading link's deepest frame is an unrelated Direct replay setup function at
4,032 bytes; DCLTGMF3 did not become the link's stack owner.

The linked ELF imports confirm the intended runtime boundaries. Trading imports
both `sol_create_program_address` and `sol_try_find_program_address`: the first
is the DCLTGMF3 fast reproduction path, while the second remains reachable from
other Trading routes. Core imports `sol_get_clock_sysvar` and
`sol_get_rent_sysvar`; Claims imports `sol_get_rent_sysvar`. The presence of a
link-wide canonical-search import is therefore not evidence that the five
DCLTGMF3 outer searches survived.

The build logs and frame reports live under the same hbox evidence root. Their
SHA-256 values are:

| Artifact | SHA-256 |
| --- | --- |
| Trading build log | `aaa88e9881ede5007846bf1b32e607c7063dc2f1aee62f742cf00f9631137f86` |
| Trading frame-build log | `3926de4f03da4bf97cc2eea044b114d33b7e584a37b323b515f3956bfab7d84e` |
| Trading frame report | `46ac8fead32117fe42cd5babd05220f465929f10687aec407f5040ed5a395def` |
| Core build log | `fcb635af014fc28df401c22a981b840a28c861e9370783d161050d15d96edfa2` |
| Core frame-build log | `f7f5526611b12e5af07722185baea0c4946cd253113ea282f50d5049a8edd317` |
| Core frame report | `8e70e542c3e3ee95721e60342681a0eb0642cc25c98407b2b1377010fb77eafc` |
| Claims build log | `dddb0470852cada6cf69be97f2d6a84a1bea21f98f4e441cb796fe90033f8536` |
| Claims frame-build log | `fd1a8dcf87fe18bcac5ba783117312fc5743cbebfac540c6166776d85cb51de2` |
| Claims frame report | `e38ac4eb13f4d40b9340905b94652be1544703c913dec9b596cebd30de4e90fc` |

These builds are source-bound SBF and static-frame evidence. This lane did not
run the final lifecycle twenty-seed campaign, so it makes no M-61 compute-margin
claim. A margin requires the exact final ELF's pass count and 20-seed arithmetic
mean; no single diagnostic execution is substituted for that result.
