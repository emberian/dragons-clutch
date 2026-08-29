# Credential-free devnet Pyth market open

The credential-free Pyth route reads the sponsored SOL/USD account already on
Solana devnet. It does not call Hermes or Pyth Price Service, does not need a
Pyth API key, and never posts a Receiver update. Hermes now needs an API key;
the exact boundary and release pins are recorded in
[`PYTH_CREDENTIAL_FREE_DEVNET_2026_08_29.md`](../evidence/PYTH_CREDENTIAL_FREE_DEVNET_2026_08_29.md).

This guide prepares and preflights an Open Market. It does not authorize a
devnet write, signing, funding, deployment, or publication. Add `--execute`
only under a separate authorization after checking the produced report.

## Inputs

The external campaign requires absolute paths to a current checked `PLAN`, a
funded `core-upgrade-authority` keypair, and the six other named campaign
keypairs: collateral mint/wallet, founding beneficiary/founder, projection
witness, and source funder. It reads only those explicit paths; it never finds
or reads a default wallet.

The market compiler also requires an explicit Direct fee policy and recipient.
These are immutable market facts, not command defaults.

## Read-only preparation

```sh
BOOT=tools/local-validator/bootstrap/successor
RPC=https://api.devnet.solana.com
GENESIS=EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG
WORK=/absolute/operator-work-dir

# Two bounded RPC reads: devnet genesis, then the fixed sponsored account.
tools/release/devnet-price-update.sh --url "$RPC" --out "$WORK/sol-usd.update"

# The generated MarketRunInput contains the sponsored Source graph. The 1,800 s
# default resolution window is above the enforced 1,252 s cadence floor.
cargo run --manifest-path "$BOOT/Cargo.toml" -- devnet-sponsored-market \
  --registry-program-id "$REGISTRY_PROGRAM" --plan "$PLAN" \
  --rpc-url "$RPC" --i-mean-devnet "$GENESIS" \
  --direct-fee-basis-points "$DIRECT_FEE_BPS" \
  --direct-fee-recipient "$DIRECT_FEE_RECIPIENT" \
  --price-update "$WORK/sol-usd.update" --window-start "$(date +%s)" \
  > "$WORK/sponsored-market.json"

# Reads only by default. It authenticates the cluster, program slot pins,
# sponsored Pyth release, payer capacity, and prospective market coordinates.
cargo run --manifest-path "$BOOT/Cargo.toml" -- campaign \
  --rpc-url "$RPC" --i-mean-devnet "$GENESIS" --plan "$PLAN" \
  --market "$WORK/sponsored-market.json" \
  --keypair-core-upgrade-authority "$CORE_KEYPAIR" \
  --keypair-collateral-mint "$MINT_KEYPAIR" \
  --keypair-collateral-wallet "$WALLET_KEYPAIR" \
  --keypair-founding-beneficiary "$BENEFICIARY_KEYPAIR" \
  --keypair-founding-founder "$FOUNDER_KEYPAIR" \
  --keypair-founding-projection-witness "$WITNESS_KEYPAIR" \
  --keypair-founding-source-funder "$SOURCE_FUNDER_KEYPAIR" \
  --evidence "$WORK/open-preflight.json"
```

When separately authorized, repeat the campaign command unchanged with
`--execute`. The driver resumes only from authenticated chain state; it never
deploys programs. A partial founding is deliberately refused rather than being
overwritten because it carries real collateral.

## After Open

The current exterior callers are progressive and each executes exactly one
durable action per invocation:

- `devnet-direct-trade-v1` materializes replay/token prestate, an exact frozen
  ALT, capability seal, then the Direct Hot v0 transaction. Its checked route
  is 61 message accounts and 1,159 bytes.
- `devnet-sponsored-push-v1` captures the sponsored account into an immutable
  candidate, advances the canonical head, settles the best valid submitted
  candidate, and closes candidate/head accounts only after terminal state.
- `flagship-resolution-v1` performs the authenticated provider lifecycle; its
  durable checkpoint resumes a submitted signature by polling, never by
  resending.
- `devnet-terminal-sequence-v1` executes the ordered terminal/retirement v0
  mutations with the same signed-before-send journal rule.

Each command has a key-free, read-only preflight. Treat local-validator runs
and focused native tests as local evidence only; they are not devnet execution
evidence.
