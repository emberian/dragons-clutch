#!/usr/bin/env bash
# Prepare, but never submit, the permanent-devnet sponsored-Pyth flagship open.
#
# This is intentionally a thin wrapper over the real input producer and the
# external campaign driver.  It neither reads a keypair nor sends a transaction.
set -euo pipefail

DEVNET_RPC="https://api.devnet.solana.com"
DEVNET_GENESIS="EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"
PRICE_ACCOUNT="7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE"
WORK=""
PLAN=""
REGISTRY=""
FEE_RECIPIENT=""
WINDOW_START=""

usage() {
    cat <<'EOF'
Usage:
  stage-devnet-sponsored-market-open.sh --work ABSOLUTE_NEW_DIR \
    --plan ABSOLUTE_CHECKED_PLAN_JSON --registry-program-id PUBKEY \
    --direct-fee-recipient PUBKEY --window-start UNIX_SECONDS [--rpc-url URL]

This stages the credential-free sponsored SOL/USD PriceUpdateV2 input and the
real `devnet-sponsored-market` MarketRunInput. It always fixes Direct fees at
50 basis points per side and writes an execute-only campaign wrapper that
requires explicit environment variables and DCLUTCH_AUTHORIZE_MARKET_OPEN=YES.
No key file is read and no transaction is submitted by this command.
EOF
}

absolute_existing() {
    case "$2" in /*) ;; *) echo "$1 must be absolute" >&2; exit 2 ;; esac
    if [ ! -f "$2" ] || [ -L "$2" ]; then
        echo "$1 must be an existing regular non-symlink file" >&2
        exit 2
    fi
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --work) WORK="${2:?--work needs a value}"; shift 2 ;;
        --plan) PLAN="${2:?--plan needs a value}"; shift 2 ;;
        --registry-program-id) REGISTRY="${2:?--registry-program-id needs a value}"; shift 2 ;;
        --direct-fee-recipient) FEE_RECIPIENT="${2:?--direct-fee-recipient needs a value}"; shift 2 ;;
        --window-start) WINDOW_START="${2:?--window-start needs a value}"; shift 2 ;;
        --rpc-url) DEVNET_RPC="${2:?--rpc-url needs a value}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

for required in WORK PLAN REGISTRY FEE_RECIPIENT WINDOW_START; do
    if [ -z "${!required}" ]; then
        case "$required" in
            WORK) flag=--work ;;
            PLAN) flag=--plan ;;
            REGISTRY) flag=--registry-program-id ;;
            FEE_RECIPIENT) flag=--direct-fee-recipient ;;
            WINDOW_START) flag=--window-start ;;
        esac
        echo "$flag is required" >&2
        exit 2
    fi
done
case "$WORK" in /*) ;; *) echo "--work must be absolute" >&2; exit 2 ;; esac
case "$WINDOW_START" in ''|*[!0-9-]*) echo "--window-start must be decimal Unix seconds" >&2; exit 2 ;; esac
absolute_existing --plan "$PLAN"
if [ -e "$WORK" ] || [ -L "$WORK" ]; then
    echo "--work must name a fresh directory; refusing to overwrite $WORK" >&2
    exit 2
fi
PARENT="$(dirname "$WORK")"
if [ ! -d "$PARENT" ] || [ -L "$PARENT" ]; then
    echo "--work parent must be an existing non-symlink directory" >&2
    exit 2
fi

REPO="$(cd "$(dirname "$0")/../.." && pwd -P)"
BOOT="$REPO/tools/local-validator/bootstrap/successor"
PRICE_READER="$REPO/tools/release/devnet-price-update.sh"
mkdir -m 700 "$WORK"
trap 'rm -rf "$WORK"' ERR INT TERM

# The price reader makes exactly the bounded public reads it documents and
# writes a fresh 134-byte account body. It never contacts Hermes/Price Service.
"$PRICE_READER" --url "$DEVNET_RPC" --out "$WORK/sol-usd.price-update-v2"

# The compiler is the semantic owner of the sponsored provider release, four
# outcomes, range partition, permanent program-plan checks, and Direct graph.
cargo run --locked --manifest-path "$BOOT/Cargo.toml" -- devnet-sponsored-market \
    --registry-program-id "$REGISTRY" \
    --plan "$PLAN" \
    --rpc-url "$DEVNET_RPC" \
    --i-mean-devnet "$DEVNET_GENESIS" \
    --direct-fee-basis-points 50 \
    --direct-fee-recipient "$FEE_RECIPIENT" \
    --price-update "$WORK/sol-usd.price-update-v2" \
    --window-start "$WINDOW_START" \
    --product product/sol-usd-sponsored-range-protection \
    --coordinate-domain coordinate-domain/usd-cents-per-sol \
    --feed sol-usd-sponsored \
    --cuts 12000,18000 \
    --coefficients 1,0,1,0 \
    > "$WORK/market.json"

# This only makes the remaining authority explicit. It invokes the existing
# campaign driver after the operator supplies paths/public identities; staging
# never opens a key file or invokes this wrapper.
cat > "$WORK/open-market.execute.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
: "\${DCLUTCH_AUTHORIZE_MARKET_OPEN:?set to YES under a separate authorization}"
[ "\$DCLUTCH_AUTHORIZE_MARKET_OPEN" = YES ] || { echo 'authorization not granted' >&2; exit 2; }
: "\${DCLUTCH_CAMPAIGN_PAYER_KEYPAIR:?absolute keypair path required}"
: "\${DCLUTCH_COLLATERAL_MINT_KEYPAIR:?absolute keypair path required}"
: "\${DCLUTCH_COLLATERAL_WALLET_KEYPAIR:?absolute keypair path required}"
: "\${DCLUTCH_FOUNDING_BENEFICIARY_KEYPAIR:?absolute keypair path required}"
: "\${DCLUTCH_FOUNDING_PROJECTION_WITNESS_KEYPAIR:?absolute keypair path required}"
: "\${DCLUTCH_FOUNDING_SOURCE_FUNDER_KEYPAIR:?absolute keypair path required}"
: "\${DCLUTCH_FOUNDING_FOUNDER:?public founder Pubkey required}"
: "\${DCLUTCH_SUBSTITUTED_FOUNDER:?distinct public substituted-founder Pubkey required}"
cargo run --locked --manifest-path '$BOOT/Cargo.toml' -- campaign --founding-only \\
  --rpc-url '$DEVNET_RPC' --i-mean-devnet '$DEVNET_GENESIS' \\
  --plan '$PLAN' --market '$WORK/market.json' --evidence '$WORK/campaign-open.json' \\
  --keypair-campaign-payer "\$DCLUTCH_CAMPAIGN_PAYER_KEYPAIR" \\
  --keypair-collateral-mint "\$DCLUTCH_COLLATERAL_MINT_KEYPAIR" \\
  --keypair-collateral-wallet "\$DCLUTCH_COLLATERAL_WALLET_KEYPAIR" \\
  --keypair-founding-beneficiary "\$DCLUTCH_FOUNDING_BENEFICIARY_KEYPAIR" \\
  --keypair-founding-projection-witness "\$DCLUTCH_FOUNDING_PROJECTION_WITNESS_KEYPAIR" \\
  --keypair-founding-source-funder "\$DCLUTCH_FOUNDING_SOURCE_FUNDER_KEYPAIR" \\
  --founding-founder "\$DCLUTCH_FOUNDING_FOUNDER" \\
  --substituted-founder "\$DCLUTCH_SUBSTITUTED_FOUNDER" --execute
EOF
chmod 700 "$WORK/open-market.execute.sh"

python3 - "$WORK/market-open-staging.json" "$WORK/market.json" "$PLAN" "$REGISTRY" "$FEE_RECIPIENT" "$DEVNET_RPC" "$DEVNET_GENESIS" "$PRICE_ACCOUNT" <<'PY'
import json, sys
out, market_path, plan, registry, recipient, rpc, genesis, price_account = sys.argv[1:]
market = json.load(open(market_path, encoding='utf-8'))
if market.get('direct_capability') is None:
    raise SystemExit('compiler omitted the permanent Direct capability')
document = {
  'schema': 'dclutch-devnet-sponsored-market-open-staging-v1',
  'cluster': {'rpcUrl': rpc, 'genesisHash': genesis},
  'plan': plan,
  'permanentProgramAuthority': {'registryProgramId': registry, 'programPinsSource': plan},
  'sponsoredPyth': {
    'priceUpdateV2Account': price_account,
    'bodyPath': str(market_path).replace('market.json', 'sol-usd.price-update-v2'),
    'credentialFree': True,
    'hermesOrPriceServiceCredentials': 'not used',
  },
  'flagship': {
    'product': 'product/sol-usd-sponsored-range-protection',
    'outcomes': 4,
    'cuts': ['12000', '18000'],
    'directFeeBasisPointsPerSide': 50,
    'directFeeRecipient': recipient,
    'marketInputPath': market_path,
  },
  'execution': {
    'driver': 'campaign --founding-only',
    'executeWrapper': str(market_path).replace('market.json', 'open-market.execute.sh'),
    'postOpenEvidencePath': str(market_path).replace('market.json', 'campaign-open.json'),
    'postOpenCapture': ['campaign-open.json accounts map', 'founding_custody_context', 'direct_selected_manifest_entry_index', 'finalized founding transaction signatures and slots'],
    'remainingRuntimeInputs': [
      'six explicit founding keypair paths: campaign-payer, collateral-mint, collateral-wallet, founding-beneficiary, founding-projection-witness, founding-source-funder',
      'two distinct public identities: founding-founder and substituted-founder',
      'separate authorization: DCLUTCH_AUTHORIZE_MARKET_OPEN=YES',
    ],
  },
}
with open(out, 'x', encoding='utf-8') as handle:
    json.dump(document, handle, indent=2, sort_keys=True)
    handle.write('\n')
PY

printf 'staged sponsored devnet flagship MarketRunInput at %s/market.json\n' "$WORK"
printf 'no transaction was submitted; the canonical post-open capture will be %s/campaign-open.json\n' "$WORK"
