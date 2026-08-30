# Credential-free devnet Pyth market open

The credential-free Pyth route reads the sponsored SOL/USD account already on
Solana devnet. It does not call Hermes or Pyth Price Service, does not need a
Pyth API key, and never posts a Receiver update. Hermes now needs an API key;
the exact boundary and release pins are recorded in
[`PYTH_CREDENTIAL_FREE_DEVNET_2026_08_29.md`](../evidence/PYTH_CREDENTIAL_FREE_DEVNET_2026_08_29.md).

This guide prepares and preflights an Open Market. It does not authorize a
devnet write, signing, funding, deployment, or publication. Add `--execute`
only under a separate authorization after checking the produced report.

## Stage the executable bundle

Use the staging wrapper rather than hand-assembling a Market input. It calls the
production `devnet-sponsored-market` compiler and writes an execute-only wrapper
for the production `campaign --founding-only` caller. Staging performs bounded
public RPC reads but never opens a key file or submits a transaction.

```sh
tools/release/stage-devnet-sponsored-market-open.sh \
  --work /absolute/new-market-open-dir \
  --plan /absolute/checked-devnet-plan.json \
  --registry-program-id "$REGISTRY_PROGRAM" \
  --direct-fee-recipient "$DIRECT_FEE_RECIPIENT" \
  --window-start "$(date +%s)"
```

The wrapper fixes Direct at **50 basis points per side** and compiles the
four-outcome flagship SOL/USD range product with cuts `12000,18000`. The checked
plan is the source of the permanent Registry, Core, Claims, Trading, Resolution,
Custody, and Rent-Credit program pins; no ID is copied into the staging tool.

The resulting directory contains `market.json`, the 134-byte sponsored
`sol-usd.price-update-v2`, `market-open-staging.json`, and an
`open-market.execute.sh` command which remains inert until separately
authorized. The canonical post-open address capture is its
`campaign-open.json` evidence: its account map, founding custody context,
selected Direct manifest entry, and finalized founding transaction receipts are
the caller artifacts for Direct and terminal stages.

## Remaining execution inputs

The execute wrapper requires only explicit inputs: six keypair paths
(`campaign-payer`, `collateral-mint`, `collateral-wallet`, `founding-beneficiary`,
`founding-projection-witness`, and `founding-source-funder`), two distinct public
identities (`founding-founder` and `substituted-founder`), and a separate
`DCLUTCH_AUTHORIZE_MARKET_OPEN=YES` authorization. It never falls back to a
default wallet.

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
  --direct-fee-basis-points 50 \
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
  is 61 message accounts and 1,167 bytes.
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
