# Devnet real-source snapshot — 2026-08-22

Status: **provisional read-only engineering evidence**. This is not a source
registry flip, deployment manifest, release identity, or authorization to sign
or submit a transaction.

## Purpose

The prior R2 success campaign used a no-op laboratory receiver and a
host-installed `PriceUpdateV2`. This snapshot establishes a real public-devnet
account shape that can be cloned into a local validator while funding is
unavailable. It does not make the laboratory fixture production evidence.

All RPC requests were read-only, used finalized commitment, and went to the
exact endpoint `https://api.devnet.solana.com`. No wallet, key file, faucet,
transaction, or deployment was used.

Retrieval window: 2026-08-22 20:43–20:46 UTC.

The checked machine-readable form of this observation is
[`devnet-real-source-snapshot-2026-08-22.json`](../../programs/clutch-sbf/source-profiles/devnet-real-source-snapshot-2026-08-22.json).
Its dependency-free
[`check_provisional_snapshot.py`](../../programs/clutch-sbf/source-profiles/check_provisional_snapshot.py)
compares the record with this review and the local-clone script entirely
offline. A pass means those evidence files agree; it cannot create a compiled
registry row or a release identity.

## Cluster and deployed identities

Canonical devnet genesis hash observed:

```text
EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG
```

The program addresses agree with Pyth's upgraded Solana contract table and
the reviewed upstream source at commit
`f50a3faf9fc5a223a22889799b2f778900f186b3`:

- [Pyth upgraded contract addresses](https://docs.pyth.network/price-feeds/core/upgrade/contracts#solana)
- [Pyth Solana migration notes](https://docs.pyth.network/price-feeds/core/upgrade/preparing/solana)
- [Reviewed upstream commit](https://github.com/pyth-network/pyth-crosschain/commit/f50a3faf9fc5a223a22889799b2f778900f186b3)

| role | address | observed account facts |
| --- | --- | --- |
| upgraded receiver | `rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp` | executable, Upgradeable Loader owner, 36-byte Program account; full Program-account body SHA-256 `ef37dd1cee22d731902a8c04ed2e13136a2b8aa7068d9db3aff2ed1ec7b634e5` |
| receiver ProgramData | `3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX` | deployment slot `460336311`; authority `upg8KLALUN7ByDHiBu4wEbMDTC6UnSVFSYfTyGfXuzr`; 416,909 bytes; full account-body SHA-256 `7122abc6b5e78d30bf88c869cb5d8783adaf897369d04eca827d3af8ffe18e5d` |
| upgraded Wormhole/router | `HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL` | executable, Upgradeable Loader owner, 36-byte Program account; full Program-account body SHA-256 `1ee590ae23d5ecbf775aba910f06a993dee8f77bfd7028790dbd349651c8034b` |
| router ProgramData | `9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x` | deployment slot `460336290`; authority `upg8KLALUN7ByDHiBu4wEbMDTC6UnSVFSYfTyGfXuzr`; 656,005 bytes; full account-body SHA-256 `f26f4b53b0f980455886116f500fa74ba475e51b1acb7f486b18afa9d73d948f` |
| receiver Config | `H3R4M45f2gyqp6geVUruapzZdyxpgGZ96UnWkDM3ndye` | receiver-owned, non-executable, 370 bytes; full body SHA-256 `23a7a19cf60c1fda8f070323fb8f1013a32851b0921fb7b2ac085990cbfaa37a` |
| upgraded push oracle | `pyt2F414BA6dPttK6RddPZUdHfapoBN24GL5wbrPCou` | executable cloned dependency; not an admitted Clutch source identity |

The receiver and router ProgramData accounts remain upgradeable. These values
are therefore observations, not timeless constants. They must be captured
again over a named stability interval before any exact devnet profile is
compiled into a Clutch release.

## Observed post instruction

A bounded sample of the receiver's newest finalized transactions found the
receiver invoked as a CPI from the upgraded push-oracle program. The observed
receiver instruction logged `Instruction: PostUpdate`, used seven accounts in
this order, and began with bytes `855fcfaf0b4f762c`:

1. payer — writable signer at the transaction boundary;
2. encoded VAA;
3. receiver Config — read-only;
4. treasury — writable;
5. `PriceUpdateV2` — writable;
6. System Program — read-only;
7. write authority — signer for a direct call, or an `invoke_signed` PDA for
   the observed push-oracle CPI.

The discriminator is independently the first eight bytes of
`SHA-256("global:post_update")`, and the account order agrees with the reviewed
upstream `PostUpdate` Anchor context.

This exposes two distinct integration shapes:

- the Clutch direct-pull join can authenticate a top-level `post_update`
  immediately followed by `AppendSourceArchiveV2`;
- a push-oracle CPI is not visible as a separate instruction in the
  Instructions sysvar and must not be mislabeled as satisfying that adjacency
  contract.

The encoded-VAA account named by the sampled transaction had already been
closed when re-read, which is normal for an ephemeral proof account. Two
receiver-owned 134-byte update accounts remained available and decoded under
the repository's reviewed layout:

| update account | feed id | price | confidence | exponent | publish time | posted slot | body SHA-256 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `6HAuqASbHEh4w4REJEUUUCginTLfj1kwCh215ZLtMkrT` | `eaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a` | 99,995,000 | 6,357 | -8 | 1,787,431,676 | 486,727,682 | `fe665d8884fca3428c17a1a1ab5a8dbd4f35717900ebc1af182e8a5e93f69d16` |
| `9qYStiZz65fSGkpTsk1niVYifjK34d79hMbpNNcwDxTd` | `add6499a420f809bbebc0b22fbf68acb8c119023897f6ea801688e0d6e391af4` | 116,779,625 | 1 | -8 | 1,787,431,676 | 486,727,664 | `25978e31019ba5e784ebb923cebd955d6fc97917c7e4f836b88242bb77bbc02e` |

Both carry `VerificationLevel::Full` and canonical zero padding. The first is
direct evidence that a normal real update has non-zero confidence; a source
path that only succeeds when confidence is zero is therefore not a real-source
integration.

## Local clone

A local validator was booted from the canonical devnet endpoint with the
devnet feature set and cloned upgradeable receiver, router, push-oracle, and
Config accounts. At the end of the snapshot it was healthy at:

```text
http://127.0.0.1:9147
```

The four named accounts reloaded locally with the expected owners,
executability, and sizes. This establishes a real-binary/account substrate for
local integration work. It does **not** yet establish a successful real
`post_update -> AppendSourceArchiveV2` transaction: that requires a verified
encoded VAA or atomic proof payload plus the Clutch seam repairs identified by
the architecture review.

## Required before release use

Post-snapshot implementation status: items 1–4 below are repaired in the
current unsealed working source. The fixture adapter release advanced from v2
to v3; host tests and the laboratory SVM campaign cover exact ABI admission,
receiver-written evidence, and rollback. Those results do not promote this
snapshot or close items 5–7.

1. **Repaired in-flight:** require the update account writable in the append
   transaction.
2. **Repaired in-flight:** authenticate the exact post discriminator, account
   count, order, and flags.
3. **Repaired in-flight:** authenticate Clock's canonical sysvar owner.
4. **Repaired in-flight:** replace the no-op laboratory success path with a
   receiver that actually writes the 134-byte update and prove atomic rollback
   on append failure.
5. Decide whether admission binds only a reviewed adapter release or also an
   exact cluster/Profile digest. The first public profile should bind both.
6. Re-capture ProgramData and Config after the announced upgrade boundary and
   over a named stability interval.
7. Run the actual upstream receiver locally with a fully verified payload,
   then rebuild, measure, reseal, and independently reproduce the Clutch ELF.
