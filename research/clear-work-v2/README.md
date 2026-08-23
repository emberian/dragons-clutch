# ClearWork V2: active-width checkpoint model

This is an isolated format and equivalence model. It does not change the live
SBF ABI, account tag, PDA namespace, or `ClearWorkV1` implementation.

## Result

Active-width storage is a safe, high-leverage successor. Region-addressed
mutation is not the first step.

The existing 47,846-byte body has the exact dimensional form

```text
body(N,U,O) = 678 + 73N + 68U + 336O + 16NO + 16UO
account(N,U,O) = 158 + (2 + 32U) + body(N,U,O)
```

where `N` is the frozen live-order count, `U` is the frozen distinct-owner
count, and `O` is the immutable outcome count. At `(64,64,16)` this reproduces
the deployed V1 sizes exactly: 47,846 body bytes and 50,054 account bytes.

The size is not mainly a necessary 48 KiB checkpoint. It is a compact state
of this shape padded to all three maxima. An active `(4 orders, 3 owners, 2
outcomes)` checkpoint projects to 2,326 bytes and 17,079,840 rent lamports,
versus 50,054 bytes and 349,266,720 lamports today. That saves 47,728 bytes and
332,186,880 lamports of cold outlay per candidate. Terminal closure still
makes rent recoverable; the saving is participation capital and creation
work, not protocol revenue.

| O | N | U | account bytes | rent lamports | saving vs V1 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 2 | 0 | 0 | 1,510 | 11,400,480 | 337,866,240 |
| 2 | 1 | 1 | 1,747 | 13,050,000 | 336,216,720 |
| 2 | 4 | 3 | 2,326 | 17,079,840 | 332,186,880 |
| 4 | 16 | 8 | 5,686 | 40,465,440 | 308,801,280 |
| 8 | 32 | 16 | 13,606 | 95,588,640 | 253,678,080 |
| 16 | 64 | 64 | 50,054 | 349,266,720 | 0 |

Most ordinary shapes also fall below Solana's 10,240-byte per-instruction
account-growth ceiling. Those checkpoints need one creation rather than the
current first stage plus four grows. Wide shapes retain the staged protocol.

## What the prototype proves

[`src/lib.rs`](src/lib.rs) treats V1's explicit little-endian encoding as the
semantic oracle. It:

1. projects only active rows and columns in eight named semantic regions;
2. reconstructs omitted bytes from `ClearWorkV1::encode_idle_into`;
3. refuses projection unless every omitted byte was canonical padding;
4. decodes the reconstructed image through `ClearWorkV1::decode_into`; and
5. requires the caller's expected `(O,N,U)` to equal the encoded widths.

The test corpus contains every save/resume boundary of both two-pass and
three-pass reachable walks: idle, begin, every order push, and every pass end.
It also flips every byte of a reachable compact image, checks total decoding
and re-encoding closure, mutates every omitted V1 byte, exercises short/long
images, and refuses all single-coordinate width substitutions. The resulting
37 snapshots—including accepted, empty-book, relation-refused, poisoned-resume,
claims-disabled, and explicit-slice states—reconstruct byte-for-byte and decode
to the same V1 state.

Run it with:

```sh
cargo +1.93.1 test --manifest-path research/clear-work-v2/Cargo.toml --locked
```

This corpus is exact for the included reachable states and exhaustive over the
included byte mutations. It is not yet the live relation's full 322-point
equivalence gate; promotion must run that gate with save/resume at every push.

## Real SBF compute ownership

A detached, throwaway worktree at source commit `c6daa985e2788eb55820dc71d6880eac3cb6dda6`
instrumented `AdvanceClearWork` with `sol_remaining_compute_units`. The probe
used the real SBF ELF in ProgramTest and calibrated the remaining-CU syscall at
102 CU per adjacent call. The instrumented ELF SHA-256 was
`c7bcb84c5ca7ad9e68c354a875f4486302b938ffea6ee9cb96e05fd7f5326f58`.
The retained extracted rows are in [`evidence/cu-probe.txt`](evidence/cu-probe.txt).

| operation over the 47,846-byte body | calibrated CU range |
| --- | ---: |
| box/copy the idle in-memory checkpoint | 231 |
| `ClearWorkV1::decode_into` | 72,303–72,318 |
| `ClearWorkV1::encode_into` plus body window | 49,532–49,545 |
| decode + encode | **121,835–121,863** |

The current W1 small-book quotes are about 287–288k CU, so the codec walk is
roughly 42% of an advance. This is an isolated interval measurement, not a
subtraction across different ELFs and not a prediction that compact CU scales
perfectly with byte count. A promoted V2 requires the same probe around its
own implementation.

## Linked ELF ownership

The current local unstripped SBF artifact
`36af8903bc9b98d4bdd806e971fa2e91d8aea36da6ff2fe1277c7778698443e3`
contains these directly attributable final symbols:

| owner | `.text` bytes | `.rodata` bytes | direct total |
| --- | ---: | ---: | ---: |
| V1 encode/decode/idle wrapper | 36,112 | 0 | 36,112 |
| one `ClearWorkV1` idle static | 0 | 48,328 | 48,328 |
| **codec and static** | **36,112** | **48,328** | **84,440** |
| layout ClearWork framing/grow symbols | 12,592 | 0 | 12,592 |
| walk, creation, and close route symbols | 43,816 | 0 | 43,816 |

These are lower-bound direct-symbol attributions, not promised deletion
savings: LTO shares callees and a feature build is the only valid artifact-size
answer. More importantly, an active-width wire format does **not** itself
remove the fixed 48,328-byte in-memory value or its static. A V2 adapter that
expands back into `ClearWorkV1` earns the rent saving and much of the wire-walk
CU saving, but not the ELF/heap saving. Removing those requires a native V2
engine with bounded active-region views or a generated capacity profile.

## Proposed live format

Use a new account version and PDA seed, for example
`dragons-clutch:clear-work:v2`. Keep the existing identity/status header facts:

- market, epoch, and candidate identities;
- open/bound/complete status;
- immutable order-set binding once pass one seals;
- the sealed consumed fold;
- physical page/slot/live-rank cursor; and
- exact body length and stored bump.

Do not let an instruction choose widths. Before body decoding, derive and
authenticate them from the already-frozen state:

```text
O = epoch.outcome_count = feed.outcome_count
N = window.live_order_count = feed.order_len
U = epoch.owner_count
expected_len = account_len(N,U,O)
```

Require `0 <= U <= N <= 64`, `1 <= O <= 16`, exact account length, V2 tag and
version, V2 PDA, and the current candidate/feed/epoch binding. Zero-width
order/owner tables are retained because an empty frozen book can reach an
early typed refusal and still needs a closable checkpoint. The dynamic owner
interner is `2 + 32U` bytes. Omitted rows and columns have exactly one meaning:
the V1 canonical idle image.

The first implementation should fully decode and re-encode the compact body.
That retains V1's typed hostile-byte validation and all three existing resume
layers:

1. later passes must reproduce pass one's fold or get
   `ResumeFoldMismatch`;
2. the layout header binds the immutable frozen order set; and
3. every bound resume compares the body's sealed fold with the header anchor.

Candidate identity remains the PDA/header/feed identity. Widths are added to
that binding; they never become mutable cursor state.

## Why V2 should not start with an incremental digest

The relation fold is deliberately not a cryptographic commitment. It cannot be
relabelled as a body authenticator.

Hashing only a changed region also does not authenticate untouched bytes. A
table of region hashes plus a root stored in the same program-owned account is
useful for accidental-corruption detection only after a fully validated
initialization; checking the root from stored leaves without hashing an
untouched region proves the hash table, not that region's bytes. Recomputing a
whole-body SHA-256 on every patch restores the claim but recreates a linear
scan. A Merkle path can authenticate a touched region if the old root is
trusted, but it changes the trust argument from "all bytes validated now" to
"the owner program was the only writer since a fully validated state."

That may be a sound V3 optimization, but it is not free and it is not needed
to capture the dominant rent win. If pursued later, require:

- a fixed region directory derived from immutable widths;
- domain-separated leaf preimages including codec version, region id, widths,
  generation, offset, and length;
- a root bound to market, epoch, candidate, order set, and consumed fold;
- atomic region write, generation increment, leaf replacement, and root
  replacement;
- full validation at construction/migration; and
- delayed verification of an untouched region before its first semantic use.

No digest may replace the fold-seal comparison, header continuation check,
candidate/feed binding, or codec validation.

## Versioning, migration, and closure

- V1 and V2 must have distinct tag/version and PDA seed namespaces. A
  variable-length V2 must never pass the V1 exact-length decoder.
- Do not migrate an in-progress walk. Existing V1 checkpoints finish and close
  under V1; newly submitted candidates select V2. The simplest safe launch has
  no migration instruction.
- If an idle-only migration is later justified, it must atomically close the
  V1 child, create the V2 child, preserve the recorded funding payer, and
  switch one persisted candidate child-version fact. It may not reset a fold,
  cursor, order-set anchor, or deadline.
- Candidate/root closure must treat full V1, growing V1, full V2, and growing
  V2 as exhaustive child variants. A candidate cannot close while any variant
  exists.
- V2 close verifies owner, exact versioned PDA, context-derived exact length,
  candidate/epoch binding, complete status, and the existing terminal
  conditions before returning lamports to the recorded payer.
- The grow-stage target is the context-derived V2 length. Shapes at or below
  10,240 bytes use one creation; larger shapes use monotone staged growth with
  the same target/step/replay refusals as V1.

## Promotion sequence

1. Add active-width codec code beside V1 in `clutch-batch`, still converting
   through a caller-owned V1 value for semantic equivalence.
2. Run the complete relation equivalence/resume and hostile-byte corpus across
   every supported `(O,N,U)` boundary, especially 1/2/15/16 outcomes and
   1/16/17/63/64 orders/owners.
3. Add layout V2 header/body/interner accessors and close/grow codecs with no
   dispatch route yet.
4. Add V2 create/advance/slice/complete/close routes under new intent tags and
   seed; keep V1 readable and closable.
5. Measure SBF CU around V2 decode and encode, creation CU on both sides of the
   10,240-byte threshold, frame diagnostics, and final ELF symbols.
6. Only then consider a native active-region engine that removes the fixed
   in-memory/static object and earns the ELF/heap half of the optimization.
