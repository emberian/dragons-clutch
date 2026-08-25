# Synthetic-local Pyth release V1

This manifest derives the feature-gated dClutch lab release from
`fixtures/pyth/local-upgraded-2026-08-22/`. It is local execution evidence,
not a production catalog row, provider-availability claim, or authenticated
Solana cluster release.

## Measured identifiers

- Provider source commit:
  `f50a3faf9fc5a223a22889799b2f778900f186b3`.
- `pyth-solana-receiver-sdk-2.0.0.crate` SHA-256:
  `245b1b03dd2177402018b6072fcbb7bea5b3d280427b1954796bf1dc189be48b`.
- The archive's extracted `src/price_update.rs` SHA-256, used as the exact
  PriceUpdate codec identifier:
  `12d0ce8bc3907ae2949043397eaf3d5bd25deed98450c6969d957be402c807ae`.
- Receiver ABI identifier, equal to the captured `receiver.so` SHA-256:
  `c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64`.
- Router ABI identifier, equal to the captured `router.so` SHA-256:
  `f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb`.

The adapter authenticates the exact Program and ProgramData keys and their
deployment slots. It does not hash the combined 1,072,824 ELF bytes during
each resolution; the ProgramData generation is the onchain executable bind,
while the ELF digests make the fixture and ABI evidence reproducible.

## Domain-separated identifiers

The synthetic local release label/cluster identifier is SHA-256 of these exact
bytes:

```text
dclutch/synthetic-local-release/v1 || 00 || local-upgraded-2026-08-22
```

Its digest is
`4081d55d4031313fcf4b7c41313d547a9441c8f9c048741a7a951b3e035e22d9`.
This is deliberately not a devnet or mainnet genesis hash.

The dClutch adapter semantic identifier is SHA-256 of these exact byte strings,
separated by one `00` byte:

```text
dclutch/pyth-adapter/v1
resolve-categorical-pyth-v1
internal-post-update
inline-terminal-receipt
```

Its digest is
`3fdfc94589c69b133864468320976f8e790e7fe0f145897b6eabc22bd7c8711b`.

## Quorum distinction

The captured receiver Config has `minimum_signatures = 5`. That local receiver
policy is not the router's guardian-set cardinality or strict-majority fact.
The adapter binds the complete Config digest, while the authenticated router
generation and verified EncodedVaa path own full-VAA verification. The lab
release therefore must not compare the Config value five with the 19-guardian
strict-majority threshold ten.

The fixture feed ID `[0x2a; 32]` has no real asset meaning. A lab Market must
pair it with explicitly synthetic base and quote semantic IDs in its inline
feed profile, and that profile cannot seed a production Market.
