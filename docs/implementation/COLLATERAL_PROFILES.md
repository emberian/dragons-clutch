# V1 Realm collateral profiles

Status: offline policy model and test corpus (2026-08-18). This is not a Solana
account parser, program adapter, deployment configuration, routeability claim,
or chain-readiness claim.

## Decision

Collateral is generic and immutable per Realm. A Realm commits to one canonical
profile before it can mint liabilities. DREGG uses the same profile type as any
other legacy SPL token; it is a house dogfood instance, never a required asset,
hard-coded program branch, global protocol fee token, or liveness dependency.

V1 supports two collateral program families:

1. legacy SPL Token, with no extension claims; and
2. Token-2022 base fungible mints, with no mint extensions and at most
   `ImmutableOwner` on the Hoard token account.

This is deliberately narrower than everything Token-2022 can express. The
collateral token is a liability backing asset, so accepting a mint means more
than being able to decode it. Dragon's Clutch must know that an atom deposited is
an atom credited, an atom remains transferable out, the Hoard cannot be frozen
or seized by an external authority, no unknown program executes during transfer,
and no opaque or withheld sub-balance defeats visible conservation.

## Canonical profile

`RealmCollateralProfile.canonical_bytes()` is exactly 266 bytes:

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `DCCOLP1` followed by one zero byte |
| 8 | 2 | schema version, little-endian `u16` (`1`) |
| 10 | 2 | strict-policy flags, little-endian `u16` |
| 12 | 66 | collateral currency reference |
| 78 | 66 | fee currency reference |
| 144 | 66 | liveness currency reference |
| 210 | 8 | maximum accepted mint supply in atoms, little-endian `u64` |
| 218 | 8 | allowed mint-extension bitset, little-endian `u64` |
| 226 | 8 | required mint-extension bitset, little-endian `u64` |
| 234 | 8 | allowed account-extension bitset, little-endian `u64` |
| 242 | 8 | required account-extension bitset, little-endian `u64` |
| 250 | 16 | zero reserved bytes |

Each 66-byte currency reference is `(kind: u8, token_program: [u8;32],
mint: [u8;32], decimals: u8)`. Native SOL has kind `0`, zero program/mint,
and nine decimals. An SPL token has kind `1` and names its accepted Token or
Token-2022 program as well as its mint. The profile digest is:

```text
SHA-256(ASCII "dragons-clutch/collateral-profile/v1" || 0x00 || canonical_profile_bytes)
```

Decoding requires exact length, known schema/flags/currency kinds, valid program
identities, legal bitsets, and zero reserved bytes, then re-encoding byte for
byte. A Realm can narrow the protocol extension ceiling but cannot expand it.
Unknown bit positions fail closed. A future schema does not silently inherit V1
semantics. V1 requires all strict authority/state flags, native SOL liveness,
and a fee currency equal to either collateral or native SOL. A separately
tokenized fee asset needs its own admission policy and therefore a later schema.

The generic Token-2022 golden vector is
`aafb22527b09935db83362d09eebb7cd875a7714fc9e3c3764a9e57c207c5c32`.
The offline DREGG example described below is
`ef63ccd0c5e1616c1570dd96a985ef9924f622d44c246f5aa88e1b9545f54343`.

## Authority, supply, and account policy

Before every transition that depends on collateral availability, a future
adapter must authenticate the mint and Hoard account and reconstruct a snapshot
for this policy. The offline model accepts only when:

- account owner program and mint exactly equal the profile;
- the mint and Hoard account are initialized;
- decimals exactly equal the profile;
- current supply is positive and no greater than the immutable ceiling;
- mint authority and freeze authority are absent;
- the Hoard account is initialized, not frozen, and owned by the expected Realm
  authority;
- delegate and close authority are absent; and
- every mint/account extension is known, in its correct location, and admitted
  by both the protocol ceiling and Realm bitset.

The supply ceiling is not a solvency proof. Clutch solvency comes from actual
Hoard atoms and the kernel invariant. The ceiling and absent mint authority are
asset-quality/admission constraints; supply may still fall when holders burn
their own tokens. The adapter must compare atom balances, never UI-scaled
amounts. A profile does not bless the collateral's market value.

## Token-2022 V1 matrix

The discriminants and mint/account location below are pinned to official
Token-2022 source commit
[`426400f`](https://github.com/solana-program/token-2022/blob/426400f29d5f1e299be8b353fdf13f22358fbd68/interface/src/extension/mod.rs).
Every extension not explicitly allowed is refused, including future unknown
discriminants.

| ID | Extension | Location | V1 | Reason / risk |
| ---: | --- | --- | --- | --- |
| 0 | `Uninitialized` | padding | Refuse | Padding or unknown semantic state is not collateral policy. |
| 1 | `TransferFeeConfig` | mint | Refuse | Recipient receives net atoms; fees are withheld separately and authorities can alter/withdraw them. |
| 2 | `TransferFeeAmount` | account | Refuse | Adds withheld atoms outside the ordinary spendable balance. |
| 3 | `MintCloseAuthority` | mint | Refuse | A zero-supply mint can be closed and reinitialized with different extensions. |
| 4 | `ConfidentialTransferMint` | mint | Refuse | Opaque balances and proof flow are outside transparent V1 accounting. |
| 5 | `ConfidentialTransferAccount` | account | Refuse | Hoard conservation cannot use a plaintext-only balance check. |
| 6 | `DefaultAccountState` | mint | Refuse | New accounts can default frozen; freeze authority can update the setting. |
| 7 | `ImmutableOwner` | account | **Allow** | Narrows owner mutation and adds no amount arithmetic or external CPI. |
| 8 | `MemoTransfer` | account | Refuse | Adds mutable incoming-transfer instruction requirements. |
| 9 | `NonTransferable` | mint | Refuse | Collateral cannot enter and leave the Hoard normally. |
| 10 | `InterestBearingConfig` | mint | Refuse | UI conversion changes over time; V1 exposes atom-only semantics. |
| 11 | `CpiGuard` | account | Refuse | Can make the intended adapter CPI path fail and is owner-toggleable. |
| 12 | `PermanentDelegate` | mint | Refuse | A third party can transfer or burn any ordinary token balance, including Hoard collateral. |
| 13 | `NonTransferableAccount` | account | Refuse | Account belongs to non-transferable collateral. |
| 14 | `TransferHook` | mint | Refuse | Transfer invokes a configured external program and resolves extra accounts; self-transfer behavior also differs. |
| 15 | `TransferHookAccount` | account | Refuse | Account belongs to transfer-hook collateral. |
| 16 | `ConfidentialTransferFeeConfig` | mint | Refuse | Encrypted fee state compounds opaque-balance and withheld-fee accounting. |
| 17 | `ConfidentialTransferFeeAmount` | account | Refuse | Encrypted withheld amounts are outside transparent V1. |
| 18 | `MetadataPointer` | mint | Refuse | Mutable external pointer is unnecessary in minimal collateral semantics. |
| 19 | `TokenMetadata` | mint | Refuse | Variable-length mutable metadata expands parsing without backing value. |
| 20 | `GroupPointer` | mint | Refuse | Mutable external pointer is unnecessary in minimal collateral semantics. |
| 21 | `TokenGroup` | mint | Refuse | Group state is unrelated to collateral conservation. |
| 22 | `GroupMemberPointer` | mint | Refuse | Mutable external pointer is unnecessary in minimal collateral semantics. |
| 23 | `TokenGroupMember` | mint | Refuse | Group membership is unrelated to collateral conservation. |
| 24 | `ConfidentialMintBurn` | mint | Refuse | Opaque supply changes are outside transparent V1. |
| 25 | `ScaledUiAmount` | mint | Refuse | Mutable UI scaling invites unit confusion; V1 uses exact atoms only. |
| 26 | `Pausable` | mint | Refuse | An authority can pause transfers, minting, and burning. |
| 27 | `PausableAccount` | account | Refuse | Account belongs to pausable collateral. |
| 28 | `PermissionedBurn` | mint | Refuse | External approval changes ordinary burn/fungibility semantics. |

The list is a V1 support decision, not a claim that refused SPL extensions are
unsafe in general. A later schema can admit one only after specifying exact
payload bytes, authority mutability, CPI and extra-account behavior, received
versus sent atom equations, withheld/opaque balances, migration behavior,
compute budget, and adversarial adapter tests. Changing this table requires a
new profile schema or an equally explicit compatibility decision; it is not a
frontend toggle.

## Three currencies, separate accounting identities

The profile names collateral, fee, and liveness currencies independently:

- **collateral currency** backs complete-set liabilities and lives in the Hoard;
- **fee currency** denominates protocol or batch fees and lives in a separate
  protected accounting pool, even when it is the same mint as collateral; and
- **liveness currency** funds transaction/rent/keeper execution and lives in a
  separately capitalized liveness pool.

The V1 dogfood shape uses collateral-denominated fees and native SOL for
liveness; the other V1 fee choice is native SOL. This does not permit fee
balances to back claims, collateral principal to pay fees, or future fees to
capitalize liveness. Sharing a mint never merges accounting identities. If
native SOL fees differ from collateral, no implicit swap, oracle conversion, or
route exists; someone must fund the named balance.

## DREGG dogfood profile

`dregg_dogfood_profile(decimals, max_supply_atoms)` binds the user-supplied DREGG
mint `XkeTXo1125vz5H9svJpGiw4JvLbN8VmMu9cmMvspump` to the legacy SPL Token
program, uses that same token as the explicit fee currency, and uses native SOL
as the explicit liveness currency. It is only a constructor around
`RealmCollateralProfile`.

The golden test calls it with six decimals and a `10^15`-atom ceiling solely to
freeze an offline vector. This research did **not** query an RPC or authenticate
DREGG's current token program, decimals, supply, mint authority, freeze
authority, or account state. Those values are not chain facts and the golden
digest is not a deployment manifest. Promotion must replace the test assumption
with an authenticated, reproducible mint snapshot and human-reviewed release
record.

## Evidence and unverified boundary

`research/collateral-profiles` contains 266-byte encode/decode round trips,
golden digests, exhaustive coverage of all 29 pinned extension discriminants,
and adversarial checks for unknown/duplicate/mislocated extensions, authorities,
decimals, supply, frozen state, delegate, close authority, program, mint, length,
magic, and reserved bytes. The checked JSON corpus is deterministic and the
implementation uses only Python's standard library.

A future Solana adapter remains responsible for hostile TLV parsing, duplicate
extension detection, account owner/executable checks, exact Token/Token-2022
program pinning, PDA derivation, alias prevention, reload-before-write, CPI
construction and return checks, balance deltas, rent/close behavior, runtime
upgrades, and proof that all required checks occur atomically. These offline
tests do not establish any of those properties and do not authorize an RPC
read, key access, signing, submission, deployment, or use of real collateral.

## Addendum 2026-08-18: parent Realm Profile identity (P1-G join)

Status: MODEL/PROPOSED. Python side implemented; on the Rust side the reserved
field landed in the same wave and the derivation/binding has not.

`ADVERSARIAL_REVIEW_V0.md` §P1-G recorded two unjoined profile-digest
algorithms and asked for a decision. **Decided: the 266-byte collateral-policy
digest is not the Realm's Profile ID. It is one domain-separated subfield inside
a broader parent Profile.** The full rationale, layout table, cross-language
expectations, and coordination notes for the `solana-layout` lane are in
[the resolution evidence plan](RESOLUTION_EVIDENCE_PLAN.md) §3.

Neither existing digest rule changed. The child rule is still
`SHA-256("dragons-clutch/collateral-profile/v1" || 0x00 || 266 bytes)` and the
two golden child digests above are byte-identical to before. The Rust
`canonical_profile_hash` rule is still
`SHA-256("dragons-clutch/profile/v1" || profile_bytes)`. What the decision
freezes is *which bytes the parent rule consumes*: exactly 64, laid out as

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `DCPROF1` followed by one zero byte |
| 8 | 2 | parent schema version, little-endian `u16` (`1`) |
| 10 | 2 | parent flags, little-endian `u16` (zero in V1) |
| 12 | 2 | subfield tag, little-endian `u16` (`1` = collateral policy) |
| 14 | 2 | subfield schema version, little-endian `u16` |
| 16 | 32 | the collateral-policy digest |
| 48 | 16 | zero reserved bytes |

The generic Token-2022 parent identity is
`8180f42830d90ef060ec2e4d91c6c19145db9cd9e2dbfd759045770930831688` and the
offline DREGG example is
`31cd82668ac7846bbf6bf38d25107d0301bc468d40816bf9a565ac93766f93b3`. Both remain
offline research values; the DREGG row is still built from assumed decimals and
an assumed ceiling and is not a chain fact.

What landed in `research/collateral-profiles`:

- `ProfileIdentity` with 64-byte canonical encode/decode, the composition rule,
  `binds()`, and `verify_profile_identity()`;
- `identity_vectors.json`, a second checked corpus with a derivation manifest:
  3 positive vectors, 9 decode refusals, 3 binding refusals, and 4
  domain-separation confusions, all recomputed from `model.py` by the tests;
- 9 new tests, for 28 total (the 19 existing tests are unchanged); and
- `run_lab.py` now also prints both parent identities.

The load-bearing negative is the binding refusal: a well-formed parent profile
is not evidence of the right subfield. `ProfileIdentity.from_canonical_bytes`
accepts a parent carrying another Realm's collateral digest without complaint;
only recomputing the child digest from the actual policy and comparing rejects
it. An adapter that merely decodes has checked nothing.

On the Rust side, the concurrent `solana-layout` lane landed
`ProfileAccount.collateral_policy_digest` at byte offset 66 (100-byte account,
`account_version::PROFILE = 2`), zero until frozen and tied to
`PROFILE_FLAG_POLICY_FROZEN`, and deliberately added no derivation function. The
algorithm above is the one those 32 bytes are for. Still owed there: a 64-byte
length requirement on `canonical_profile_hash`, the parent encoder/decoder, a
checked binding rule that recomputes the child digest rather than merely
decoding, and the cross-language golden tests. See
[RESOLUTION_EVIDENCE_PLAN.md](RESOLUTION_EVIDENCE_PLAN.md) §3.4.

Unchanged by this addendum: an admitted layout Profile still does not imply
admission by the collateral model. That requires a future adapter to
authenticate a real mint and Hoard token account, which no offline crate can do.
