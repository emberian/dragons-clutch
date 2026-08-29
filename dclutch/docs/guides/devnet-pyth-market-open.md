# Devnet Pyth market open: executable boundary

This is the shortest current route to an **Open** devnet SOL/USD market. It
does not claim a devnet trade, provider resolution, or wallet payout: those
callers are not yet constructible for a founded market. Use the local journey
for that coverage until the Trading Hot-gate work lands.

The Pyth path is sponsored in the useful sense: it reads the already-posted,
fully verified `PriceUpdateV2` account on Solana devnet. It does not post an
update, use Hermes / Pyth Price Service, or require an API key. The committed
receiver Config has `single_update_fee = 0`; the third-party Pyth pusher remains
a liveness dependency, so the market still has its disclosed failure path.
Unauthenticated Hermes endpoints are recorded as HTTP 401 in
`docs/design/MAINNET_STATE_RELAY.md` §8 and are not a fallback for this run.

## Inputs

Have these operator-provided, absolute paths before any write:

- `PLAN`: a current `dclutch-successor-plan-v2` made from the checked seven-role
  deployment observations. Its slot pins must match the deployed mutable
  substrate.
- Seven Solana CLI JSON keypair files: `core-upgrade-authority`,
  `collateral-mint`, `collateral-wallet`, `founding-beneficiary`,
  `founding-founder`, `founding-projection-witness`, and
  `founding-source-funder`. The driver reads only the explicitly named files;
  it never searches for a default wallet.
- `REGISTRY_PROGRAM`: the Registry program id from that plan/deployment.
- A funded core-upgrade-authority address. The driver preflight reports the
  exact shortfall and never requests an airdrop on devnet.

Use a checked release/plan for the current deployed bytes. A moved Loader
ProgramData slot is a fail-closed conflict: prepare a new plan/release before
founding, rather than attempting to reuse the old one.

## Open sequence

From the repository root, set only paths and public identifiers; do not put a
private key or API credential in the command line.

```sh
BOOT=tools/local-validator/bootstrap/successor
RPC=https://api.devnet.solana.com
GENESIS=EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG
WORK=/absolute/operator-work-dir

# Bounded reads only: validates the Pyth deployment and extracts the current
# finalized 134-byte receiver-owned update. It signs nothing.
tools/release/devnet-observe.sh --url "$RPC" --out "$WORK/pyth-observation"
tools/release/devnet-price-update.sh --url "$RPC" --out "$WORK/sol-usd.update"

# Generate the MarketRunInput. Pick a start shortly after the fresh read;
# 1,800 s is the default and exceeds the enforced 1,252 s cadence floor.
cargo run --manifest-path "$BOOT/Cargo.toml" -- devnet-market \
  --registry-program-id "$REGISTRY_PROGRAM" \
  --price-update "$WORK/sol-usd.update" \
  --window-start "$(date +%s)" > "$WORK/sol-usd-market.json"

# Read-only, enforced preflight. It authenticates the devnet Pyth release row
# and prints every stage, slot pin, payer shortfall, and planned market PDA.
cargo run --manifest-path "$BOOT/Cargo.toml" -- campaign \
  --rpc-url "$RPC" --i-mean-devnet "$GENESIS" --plan "$PLAN" \
  --market "$WORK/sol-usd-market.json" \
  --keypair-core-upgrade-authority "$CORE_KEYPAIR" \
  --keypair-collateral-mint "$MINT_KEYPAIR" \
  --keypair-collateral-wallet "$WALLET_KEYPAIR" \
  --keypair-founding-beneficiary "$BENEFICIARY_KEYPAIR" \
  --keypair-founding-founder "$FOUNDER_KEYPAIR" \
  --keypair-founding-projection-witness "$WITNESS_KEYPAIR" \
  --keypair-founding-source-funder "$SOURCE_FUNDER_KEYPAIR" \
  --evidence "$WORK/open-preflight.json"
```

Only after inspecting that report and receiving separate authorization to make
devnet writes, repeat the last command with `--execute`. The driver resumes
from chain state and does not deploy programs; it publishes/initializes/
activates any incomplete infrastructure stages and atomically founds through
Open. Never rerun a `partial` founding with fresh assumptions: the driver
refuses it because collateral is involved.

## What stops here

`dclutch buy` / `sell` can compile an inline Direct transaction, but a founded
market has none of the Direct Hot prestate (extra Positions and replay roots),
its shipped Direct artifact is the wrong four-outcome geometry, and adding the
required compute-budget instruction exceeds the legacy packet calculation.
No devnet trade command is therefore honest today.

The Pyth provider resolver and terminal settlement are exercised by the
loopback journey. They are not externally callable because their Claims
mutations require a caller-authority PDA signed by an activated program; a
wallet cannot sign that PDA. `dclutch redeem` can create the Claims Custody
replay, then deliberately reports the payout block rather than falsely
submitting it. See `tools/gauntlet/journey/src/journey.rs`'s named gaps for the
current owners and exact route boundaries.

For an offline/native lifecycle check, run:

```sh
tools/gauntlet/journey/run-journey.sh
```

This is local-validator evidence, not devnet evidence.
