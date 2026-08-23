# Checked deployment manifests

This directory is the repository-owned location for reviewed public-cluster
deployment coordinate records. It intentionally contains no live deployment
manifest today.

Devnet records use
`dragons-clutch/devnet-deployment-manifest/v1` and are consumed only by:

```text
operatord compose-devnet-chain-config \
  --deployment-manifest /absolute/devnet-deployment.json \
  --capability-manifest /absolute/checked-profile.json \
  --built-elf /absolute/clutch_sbf.so
```

The parser requires canonical compact ASCII JSON plus one newline, the exact
Solana devnet genesis and public HTTP/WebSocket endpoints, finalized
Program/ProgramData/deployment-slot coordinates, the checked capability
manifest/profile/source/ELF tuple, compiler release, neutral sink, and explicit
`not-exposed` signing/submission/deployment fields.

The devnet deployment slot must be a canonical positive decimal observed from
the finalized ProgramData account. Local `--bpf-program` releases instead use
the synthesized slot zero and are owned only by the v6 local session seal; the
two coordinate types are deliberately not interchangeable.

A devnet record must never be copied from or encoded as the
`dragons-clutch/local-validator-public-manifest/v6` session seal. The composer
performs no RPC call, wallet read, signing, submission, faucet request, or
deployment. See
[`REAL_INFRA_CHAIN_LAUNCH.md`](../../../docs/implementation/REAL_INFRA_CHAIN_LAUNCH.md)
for the exact field order and invocation.
