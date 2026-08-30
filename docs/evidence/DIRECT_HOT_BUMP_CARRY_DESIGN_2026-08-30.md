# Carrying PDA bumps across the Direct Hot CPI boundary — design, 2026-08-30

## What this is

The Direct Hot route's compute cost varies by tens of thousands of CU for
reasons that have nothing to do with what the trade does. This note says where
the remaining variance lives, which of it can be removed and how, and — for the
one carrier that a previous audit left as open design work — what the carrier
should be and why it is sound.

It is not CU evidence. The CU evidence is
`programs/dclutch-trading-sbf/program-test/tests/direct_hot_top_level_margin_gate.rs`
and `direct_hot_pda_depth_census.rs`. Every site below was read at `8d3ca1f9`,
not inferred.

## The measured picture, so the sizes below mean something

`Pubkey::find_program_address` walks bump 255 downward and pays about 1,500 CU
per candidate it rejects. Over 32 pinned fixture key draws the route's band is
entirely made of that: every gap between two distinct observations is a multiple
of 1,500. Decomposed from the runtime's own per-CPI accounting:

| program | band over 32 seeds |
|---|---:|
| Claims | 16,499 |
| Custody | 13,500 |
| Trading's own code | about 6,000 |
| Registry | 0 |

The Registry is zero because the only address it derives is seeded by the
release set, which no key draw moves. That is the shape of the whole problem: a
search costs variance only when a drawn key is among its seeds.

There is a second draw that is not the maker's keys, and it is documented under
`GATE_SEEDS` in the gate: `release_set_id` is a hash of the deployed ELF
digests, it seeds the Market identity transitively, and so **a rebuild redraws
every bump depth on the route with no source change at all**. Until the varying
searches are gone, a CU figure from one build is not comparable with one from
another build.

## 1. The Core Market PDA is searched three times from identical seeds

Verified sites, all deriving `MarketCoreStateSeedsV2::new(identity)` under the
same Core program id:

| program | site |
|---|---|
| Trading | `programs/dclutch-trading-sbf/src/hot_v3.rs`, `authenticate_market` |
| Claims | `programs/dclutch-claims-sbf/src/sparse_native_transfer_v1.rs:508` |
| Custody | `programs/dclutch-custody-sbf/src/lib.rs:347` |

Nine seeds each, all drawn from the Market identity, so all three vary with the
key draw and all three vary *together* — one address, one bump, three searches.

**The carry.** Trading already searches for this address and discards the bump.
Both child request codecs have zeroed reserved space enforced by `require_zero`,
so carrying one byte needs no wire-length change:

- `CustodyRequestV1` — 24 reserved bytes, `crates/dclutch-custody-contract/src/lib.rs:484`
- `SparseNativeTransferV1` — 5 reserved bytes at offset 11, `crates/dclutch-claims-svm/src/sparse_native_transfer_v1.rs:227`

The children then use `Pubkey::create_program_address` and compare to the market
account they were handed.

**Why a caller-supplied bump is not an authority here.** This is the argument
`borrow_finalized_record_at` in `hot_v3.rs` already makes and this tree already
accepted: the derivation IS the check. A wrong bump reproduces a different
address and refuses. The conjunction is not weakened, because for a non-canonical
bump to pass there would have to EXIST a Core-owned account, at the
non-canonical address, decoding to a valid state for that identity — and Core
creates market states only at the canonical bump. Canonicality is enforced where
the account is made, not where it is read.

**The churn to expect.** The child request digest is `hash(request_bytes)`
(`custody_composition_v3.rs:222`, `claims_composition_v3.rs:640`), so writing a
reserved byte changes the digest, which changes the caller-authority address
below it. That is not a correctness problem — it is symmetric, and the fixtures
recompute — but it moves addresses across the child fixtures and their tests.

## 2. The caller-authority bump: the carrier the earlier audit left open

`DIRECT_HOT_PDA_SYSCALL_AUDIT_2026-08-28.md` recorded these as "deliberately
unchanged … request-scoped. No authenticated persisted owner carries their
canonical bump." They are also duplicated across the CPI boundary: Trading
derives the child's caller authority, and the child derives the same address
again —

- Claims: `programs/dclutch-claims-sbf/src/sparse_native_transfer_v1.rs:371`
- Custody: `programs/dclutch-custody-sbf/src/lib.rs:277`

**Why the obvious carrier is circular, verified.** `CallerAuthoritySeedsV1`
(`crates/dclutch-release-set-contract/src/lib.rs:248`) includes
`role_request_digest`, and that digest is `hash(request_bytes)`. So a bump
written into the request changes the request bytes, which changes the digest,
which changes the address, which changes the bump. There is no cheap fixed
point, and this is exactly why the market bump in §1 is fine and this one is
not: the market bump does not appear in its own seeds.

**The carrier that works: a suffix outside the hashed prefix.** The digest
covers the encoded request struct, NOT the whole child instruction data. So
Trading can append the caller-authority bump after the request bytes, where it
is not hashed, and the child reads it from there and calls
`create_program_address`. The address it must reproduce is still seeded by the
digest of the unmodified request, so the byte is genuinely outside the fixed
point.

Soundness is the same as §1 and no weaker than today: a wrong suffix byte
reproduces a different address, which is compared against the caller authority
the child was given, and refuses.

**The invariant this depends on, which must be pinned by a test.** The whole
design rests on the digest covering exactly the request prefix. A later change
that hashes the entire instruction data — an entirely reasonable-looking
hardening — would silently reintroduce the circularity and turn a sound carrier
into an unsatisfiable one. That is a trap worth a named test: assert that
appending bytes to a child instruction does NOT change its
`role_request_digest`, so the day someone widens the digest, they meet a red row
that explains what it breaks rather than an unexplainable derivation failure.

## 3. What is NOT cheap: a bump field in CoreState

Storing the Market bump in `CoreState` would remove all three searches in §1
rather than two, and would need no wire carry at all, because every reader
already decodes that state.

It is out of reach of an ordinary Rust change. `CoreState` is generated:

```
// @generated by formal/dclutch-semantics/EmitMarketCoreRust.lean; do not edit.
pub const STATE_BYTES: usize = 360;
```

So it is a change to the formal specification plus regeneration plus whatever
proofs reference the layout, and then an account-format migration. Worth doing
on its own terms and with its own lane; it should not be smuggled into a
CU-reduction patch.

## 4. The alternative shape: each program stores the bump in the account it owns

Four of the remaining varying searches are for accounts whose OWN program
creates them, and which could therefore record their bump at creation and read
it back — no wire change and no cross-program coordination:

- Claims aggregate — `affine_batch_v2.rs:497`, seeds `[LIABILITY_BASIS_MARKET_SEED_V2, logical_market]` under Claims
- Claims seller/buyer Positions — two searches
- Custody replay — `lib.rs:629`

The earlier audit deferred exactly these: "Core Market, Claims
aggregate/Positions, Custody replay, Direct root, and seal layouts do not
currently own bumps. Migrating them would be an ABI/release change."

Compared with §1 this removes MORE variance (four searches against two) and
touches no wire, but it needs an account-format migration story where §1 needs
none. They are independent and can be done in either order.

## 5. What each step costs, by counting rather than by feel

The reserved bytes exist, so none of this is a wire-LENGTH change. The cost is
in the construction sites, because both request types are plain structs with
public fields and no builder, no `Default`, and no `#[non_exhaustive]` — so one
new field breaks every literal:

| carry | struct-literal sites | files |
|---|---:|---:|
| Claims, `SparseNativeTransferInputV1` | 15 | — |
| Custody, `CustodyRequestV1` | 107 | 48 |

That asymmetry is the scheduling fact. It also argues against doing these one at
a time: each carry on its own removes ONE varying search of about sixteen, which
is worth roughly 1,500–3,000 CU and a sixteenth of the band, in exchange for a
wire change to a shared contract crate that many lanes build from. The
arithmetic only becomes attractive when the carries land together with §4, and
the band collapses far enough for the gate to ratchet.

If any of this is taken piecemeal, take Claims first: fifteen sites against a
hundred and seven, for the same shape of change and the same size of win.

A cheaper-looking option that is NOT available, checked so nobody re-checks it:
Custody searches the same replay seeds twice, at `lib.rs:629` and `lib.rs:678`,
but the second is inside `initialize_replay` and the hot Transfer path does not
reach it. It is not a hot-route duplicate.

## 6. Recommended order

1. §1, the Market bump carry. Two varying searches, no migration, reserved bytes
   already exist for it.
2. §2, the caller-authority suffix, together with the digest-width test that
   keeps it sound. Two more varying searches.
3. §4, own-account bumps. Four more, and the migration story.
4. §3, CoreState, in the formal layer, if the last search is worth the lane.

The prize is not the CU. It is that once the key-varying searches are gone the
route's cost stops being a property of a hash and becomes a property of the
code — and only then does ratcheting the gate down mean anything, because only
then is a measured worst case a bound rather than a sample.
