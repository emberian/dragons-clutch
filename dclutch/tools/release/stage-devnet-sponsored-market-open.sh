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
FEE_BPS=""
# The market's SHAPE. These three were hardcoded into the compiler invocation
# below until 2026-08-31, which meant every devnet market this script had ever
# founded asked the same question about the same two prices -- $120 and $180 --
# whatever SOL was actually worth on the day. Spot was $102.54 at cohort-8's
# founding, so both boundaries sat above the money and one outcome carried
# essentially the whole probability. A market whose answer is already known is
# not a market. The defaults below are EXACTLY the old hardcoded values, so a
# command line written without these flags stages the market it always did.
CUTS="12000,18000"
COEFFICIENTS="1,0,1,0"
CUT_DENOMINATOR="100"

usage() {
    cat <<'EOF'
Usage:
  stage-devnet-sponsored-market-open.sh --work ABSOLUTE_NEW_DIR \
    --plan ABSOLUTE_CHECKED_PLAN_JSON --registry-program-id PUBKEY \
    --direct-fee-recipient PUBKEY --direct-fee-basis-points N \
    --window-start UNIX_SECONDS [--rpc-url URL] \
    [--cuts I128,..] [--coefficients U64,..] [--cut-denominator U64]

--cuts, --coefficients and --cut-denominator set the market's SHAPE and each
default to the value this script has always emitted, so a command line written
without them stages the market it always did.  --cuts sets the WIDTH: outcomes =
cuts + 2, the two open tails plus the explicit failure outcome, and
--coefficients must then carry exactly that many payouts.  Cuts are read in the
coordinate domain's own units -- for sol-usd that is USD CENTS over
--cut-denominator, so 10254 with denominator 100 is $102.54.

CENTRE THE CUTS ON SPOT.  Until 2026-08-31 these were hardcoded to 12000,18000
and every market this script founded asked about $120 and $180 no matter what
SOL cost that day; at cohort-8's founding spot was $102.54, which put both
boundaries above the money and left one outcome holding nearly all the
probability.  A market whose answer is already known teaches nobody anything.
Scale the width to realized volatility over the market's own window.

This stages the credential-free sponsored SOL/USD PriceUpdateV2 input and the
real `devnet-sponsored-market` MarketRunInput, then writes an execute-only
campaign wrapper that requires explicit environment variables and
DCLUTCH_AUTHORIZE_MARKET_OPEN=YES.  No key file is read and no transaction is
submitted by this command.

--direct-fee-basis-points has no default and must be stated.  The rate is
sealed into the Market at founding and cannot be changed afterwards.

PASS 50.  Not 0.  This paragraph used to say the opposite -- "pass 0 for a
market that must trade" -- and that sentence founded three markets that can
never take a fill.  A ZERO-FEE MARKET CANNOT BE SET UP AT ALL:
direct_token_setup_v1 is the sole creator of the seller's and the venue's
Direct token accounts, so it precedes every Hot fill, and it refuses unless
the Market's finalized Direct config reads exactly
DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1 -- documented in the codec as "the one
Direct fee rate admitted by this setup release", and equal to 50.  Devnet
market19 6WZXJ7jB was founded at 0 on 2026-08-30 and is permanently unfillable
for that reason alone.

The compute ceiling is real but it is a property of the FILL, not the rate.
crates/dclutch-direct-aot-v3-contract/src/lib.rs computes
fee = mul_div_floor(gross, policy_fee_bps, 10_000), so at 50 bps every trade
whose gross collateral is 1..=199 atoms has fee 0, sets seller_terminal,
clears the fee routes, and makes ONE Custody CPI -- the branch measured at
1,329,618..1,349,118 CU against the 1,400,000 ceiling.  The 1,515,003 figure
in docs/evidence/DIRECT_HOT_FEE_BEARING_CU_2026_08_30.md is the TWO-CPI branch,
taken only when the fee does not floor, and it is still over by 115,003 (and
by less since the CACHEREAD frame work); that branch stays blocked until the
second-transaction fee leg ships.

So: found at 50, and keep the first fills small enough that the fee floors.
Pass any other rate only when you mean to found a market that cannot trade,
or once the second-transaction fee leg has shipped.
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
        --direct-fee-basis-points) FEE_BPS="${2:?--direct-fee-basis-points needs a value}"; shift 2 ;;
        --window-start) WINDOW_START="${2:?--window-start needs a value}"; shift 2 ;;
        --cuts) CUTS="${2:?--cuts needs a value}"; shift 2 ;;
        --coefficients) COEFFICIENTS="${2:?--coefficients needs a value}"; shift 2 ;;
        --cut-denominator) CUT_DENOMINATOR="${2:?--cut-denominator needs a value}"; shift 2 ;;
        --rpc-url) DEVNET_RPC="${2:?--rpc-url needs a value}"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

for required in WORK PLAN REGISTRY FEE_RECIPIENT FEE_BPS WINDOW_START; do
    if [ -z "${!required}" ]; then
        case "$required" in
            WORK) flag=--work ;;
            PLAN) flag=--plan ;;
            REGISTRY) flag=--registry-program-id ;;
            FEE_RECIPIENT) flag=--direct-fee-recipient ;;
            FEE_BPS) flag=--direct-fee-basis-points ;;
            WINDOW_START) flag=--window-start ;;
        esac
        echo "$flag is required" >&2
        exit 2
    fi
done
# The rate is irreversible once founded, so it is checked before anything else
# the operator can still fix: refuse anything but a plain decimal, so no shell
# expansion or empty string can reach the compiler as a silent zero.
case "$FEE_BPS" in ''|*[!0-9]*) echo "--direct-fee-basis-points must be a plain decimal count" >&2; exit 2 ;; esac
# This guard is no longer the only one. `DIRECT_MAX_FEE_BASIS_POINTS_V1`
# (crates/dclutch-direct-codec/src/successor.rs) refuses the same rate at config
# construction, and the authored transition refuses it again as a relation, so a
# founding that avoids this script is bounded too. It stays because refusing at
# the operator's own console is cheaper than refusing after a staged build.
if [ "$FEE_BPS" -gt 500 ]; then
    echo "--direct-fee-basis-points exceeds MAX_FEE_BPS=500 (decision 0014 D2)" >&2
    exit 2
fi
case "$WORK" in /*) ;; *) echo "--work must be absolute" >&2; exit 2 ;; esac
case "$WINDOW_START" in ''|*[!0-9-]*) echo "--window-start must be decimal Unix seconds" >&2; exit 2 ;; esac

# The shape is checked HERE, where the operator can still fix it by retyping one
# argument, rather than 40 KB later inside the compiled document. The compiler
# re-checks all of this over the compiled input and would catch it -- but it
# speaks about a MarketRunInput, and someone who typed four cuts and five
# coefficients deserves to be told that, in those words, before anything is
# compiled. The width rule is the compiler's own: outcomes = cuts + 2, the two
# open tails plus the explicit failure outcome.
case "$CUT_DENOMINATOR" in ''|*[!0-9]*|0) echo "--cut-denominator must be a positive plain decimal" >&2; exit 2 ;; esac
case "$CUTS" in *,,*|,*|*,) echo "--cuts must be a comma-separated list with no empty entries" >&2; exit 2 ;; esac
case "$COEFFICIENTS" in *,,*|,*|*,) echo "--coefficients must be a comma-separated list with no empty entries" >&2; exit 2 ;; esac
cut_count=0
previous_cut=""
for cut in ${CUTS//,/ }; do
    case "$cut" in ''|-|*[!0-9-]*|*-*-*) echo "--cuts entry '$cut' is not a decimal integer" >&2; exit 2 ;; esac
    case "$cut" in ?*-*) echo "--cuts entry '$cut' is not a decimal integer" >&2; exit 2 ;; esac
    if [ -n "$previous_cut" ] && [ "$previous_cut" -ge "$cut" ]; then
        echo "--cuts must be STRICTLY increasing: '$previous_cut' then '$cut' describes a region of zero or negative width, which is an outcome no coordinate can land in" >&2
        exit 2
    fi
    previous_cut="$cut"
    cut_count=$((cut_count + 1))
done
coefficient_count=0
for coefficient in ${COEFFICIENTS//,/ }; do
    case "$coefficient" in ''|*[!0-9]*) echo "--coefficients entry '$coefficient' is not an unsigned decimal" >&2; exit 2 ;; esac
    coefficient_count=$((coefficient_count + 1))
done
if [ "$coefficient_count" -ne "$((cut_count + 2))" ]; then
    echo "$cut_count cuts describe a $((cut_count + 2))-outcome market (two tails plus the explicit failure outcome), so it needs $((cut_count + 2)) coefficients and $coefficient_count were given" >&2
    exit 2
fi
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
    --direct-fee-basis-points "$FEE_BPS" \
    --direct-fee-recipient "$FEE_RECIPIENT" \
    --price-update "$WORK/sol-usd.price-update-v2" \
    --window-start "$WINDOW_START" \
    --product product/sol-usd-sponsored-range-protection \
    --coordinate-domain coordinate-domain/usd-cents-per-sol \
    --feed sol-usd-sponsored \
    --cuts "$CUTS" \
    --coefficients "$COEFFICIENTS" \
    --cut-denominator "$CUT_DENOMINATOR" \
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
# The founder is the identity the founding mints the whole complete set to, and
# burning those claims is the only route to an empty aggregate, which is the
# only route to retirement and to the collateral. Terminal settlement binds the
# signer to the Position owner, so an identity whose key nobody holds strands
# the market's principal permanently: on 2026-08-30 all three live devnet
# markets were found to share one such founder and none of them can ever be
# retired (decision 0015 section 8). The driver still takes only a public key --
# the founder never signs at founding -- so the obligation to HOLD it has to be
# proved here or nowhere.
: "\${DCLUTCH_FOUNDING_FOUNDER_KEYPAIR:?absolute founder keypair path required: found only against an identity you hold}"
case "\$DCLUTCH_FOUNDING_FOUNDER_KEYPAIR" in /*) ;; *) echo 'DCLUTCH_FOUNDING_FOUNDER_KEYPAIR must be absolute' >&2; exit 2 ;; esac
if [ ! -f "\$DCLUTCH_FOUNDING_FOUNDER_KEYPAIR" ] || [ -L "\$DCLUTCH_FOUNDING_FOUNDER_KEYPAIR" ]; then
    echo 'DCLUTCH_FOUNDING_FOUNDER_KEYPAIR must be an existing regular non-symlink file' >&2
    exit 2
fi
command -v solana-keygen >/dev/null || { echo 'solana-keygen is required to prove founder key custody' >&2; exit 2; }
DCLUTCH_FOUNDING_FOUNDER_DERIVED="\$(solana-keygen pubkey "\$DCLUTCH_FOUNDING_FOUNDER_KEYPAIR")"
if [ -n "\${DCLUTCH_FOUNDING_FOUNDER:-}" ] && [ "\$DCLUTCH_FOUNDING_FOUNDER" != "\$DCLUTCH_FOUNDING_FOUNDER_DERIVED" ]; then
    echo "DCLUTCH_FOUNDING_FOUNDER names \$DCLUTCH_FOUNDING_FOUNDER but the keypair file holds \$DCLUTCH_FOUNDING_FOUNDER_DERIVED" >&2
    exit 2
fi
: "\${DCLUTCH_SUBSTITUTED_FOUNDER:?distinct public substituted-founder Pubkey required}"
# The substituted founder is the hostile cross-request probe's identity. It
# never signs and is never funded, so a bare public key is correct for it.
if [ "\$DCLUTCH_SUBSTITUTED_FOUNDER" = "\$DCLUTCH_FOUNDING_FOUNDER_DERIVED" ]; then
    echo 'the substituted founder must be a DISTINCT identity from the founder' >&2
    exit 2
fi
cargo run --locked --manifest-path '$BOOT/Cargo.toml' -- campaign --founding-only \\
  --rpc-url '$DEVNET_RPC' --i-mean-devnet '$DEVNET_GENESIS' \\
  --plan '$PLAN' --market '$WORK/market.json' --evidence '$WORK/campaign-open.json' \\
  --keypair-campaign-payer "\$DCLUTCH_CAMPAIGN_PAYER_KEYPAIR" \\
  --keypair-collateral-mint "\$DCLUTCH_COLLATERAL_MINT_KEYPAIR" \\
  --keypair-collateral-wallet "\$DCLUTCH_COLLATERAL_WALLET_KEYPAIR" \\
  --keypair-founding-beneficiary "\$DCLUTCH_FOUNDING_BENEFICIARY_KEYPAIR" \\
  --keypair-founding-projection-witness "\$DCLUTCH_FOUNDING_PROJECTION_WITNESS_KEYPAIR" \\
  --keypair-founding-source-funder "\$DCLUTCH_FOUNDING_SOURCE_FUNDER_KEYPAIR" \\
  --founding-founder "\$DCLUTCH_FOUNDING_FOUNDER_DERIVED" \\
  --substituted-founder "\$DCLUTCH_SUBSTITUTED_FOUNDER" --execute
EOF
chmod 700 "$WORK/open-market.execute.sh"

python3 - "$WORK/market-open-staging.json" "$WORK/market.json" "$PLAN" "$REGISTRY" "$FEE_RECIPIENT" "$DEVNET_RPC" "$DEVNET_GENESIS" "$PRICE_ACCOUNT" "$FEE_BPS" <<'PY'
import json, sys
out, market_path, plan, registry, recipient, rpc, genesis, price_account, fee_bps = sys.argv[1:]


def redact_origin(url):
    """Scheme and host only. A keyed endpoint carries its credential in the
    query string, and this manifest is an artifact people copy around, so the
    credential must never reach it -- the drivers redact their own
    `rpc_origin_redacted` for exactly this reason and this file did not.
    Written after the raw key landed in a real staging manifest on 2026-08-30."""
    from urllib.parse import urlsplit
    parts = urlsplit(url)
    if not parts.scheme or not parts.netloc:
        return '<redacted>'
    tail = '/<redacted>' if (parts.query or parts.path not in ('', '/')) else ''
    return f'{parts.scheme}://{parts.netloc}{tail}'
market = json.load(open(market_path, encoding='utf-8'))
if market.get('direct_capability') is None:
    raise SystemExit('compiler omitted the permanent Direct capability')
document = {
  'schema': 'dclutch-devnet-sponsored-market-open-staging-v1',
  'cluster': {'rpcOriginRedacted': redact_origin(rpc), 'genesisHash': genesis},
  'plan': plan,
  'permanentProgramAuthority': {'registryProgramId': registry, 'programPinsSource': plan},
  'sponsoredPyth': {
    'priceUpdateV2Account': price_account,
    'bodyPath': str(market_path).replace('market.json', 'sol-usd.price-update-v2'),
    'credentialFree': True,
    'hermesOrPriceServiceCredentials': 'not used',
  },
  # Read the SHAPE off the compiled input rather than restating the defaults.
  # These three were hardcoded to 4 outcomes and cuts 12000/18000, so any
  # market founded with different --cuts got a staging record that described a
  # market other than the one it staged.
  'flagship': {
    'product': 'product/sol-usd-sponsored-range-protection',
    'outcomes': len(market['coefficients']),
    'cuts': [str(cut) for cut in market['cuts']],
    'directFeeBasisPointsPerSide': int(fee_bps),
    'directFeeRecipient': recipient,
    'marketInputPath': market_path,
    'feeRateIsIrreversible': True,
    # Whether a fill can be SET UP at all, which is prior to whether it fits.
    # direct_token_setup_v1 admits exactly one rate; see --direct-fee-basis-points.
    'directTokenSetupAdmitsThisRate': int(fee_bps) == 50,
    # The ceiling is a property of the FILL. At 50 bps a trade whose gross
    # collateral is 1..=199 atoms floors its fee to zero, takes the one-CPI
    # branch, and fits; a larger fill takes the two-CPI branch and does not.
    'maximumGrossCollateralAtomsWhoseFeeFloorsToZero':
        (10_000 // int(fee_bps) - 1) if int(fee_bps) else None,
  },
  'execution': {
    'driver': 'campaign --founding-only',
    'executeWrapper': str(market_path).replace('market.json', 'open-market.execute.sh'),
    'postOpenEvidencePath': str(market_path).replace('market.json', 'campaign-open.json'),
    'postOpenCapture': ['campaign-open.json accounts map', 'founding_custody_context', 'direct_selected_manifest_entry_index', 'finalized founding transaction signatures and slots'],
    'remainingRuntimeInputs': [
      'six explicit founding keypair paths: campaign-payer, collateral-mint, collateral-wallet, founding-beneficiary, founding-projection-witness, founding-source-funder',
      'a SEVENTH keypair path, DCLUTCH_FOUNDING_FOUNDER_KEYPAIR: the driver needs only the founder public key, but nobody can ever retire this market or recover its collateral without that secret, so the wrapper derives the identity from a file you hold',
      'one public identity: substituted-founder, which never signs and is never funded',
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
