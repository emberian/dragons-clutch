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
MEAN_UNFILLABLE=0
# The market's SHAPE. These three were hardcoded into the compiler invocation
# below until 2026-08-31, which meant every devnet market this script had ever
# founded asked the same question about the same two prices -- $120 and $180 --
# whatever SOL was actually worth on the day. Spot was $102.54 at cohort-8's
# founding, so both boundaries sat above the money and one outcome carried
# essentially the whole probability. A market whose answer is already known is
# not a market. The defaults below are EXACTLY the old hardcoded values, so a
# command line written without these flags stages the market it always did.
CUTS="12000,18000"
# The author's BELIEF about the outcome. Since `26179076` the gated product
# entrance measures a partition for degeneracy against a declared band, and a
# Pyth market without one refuses by name -- so a stager that could not pass one
# could not stage a Pyth market at all. All five or none, the same discipline
# the compiler enforces: a partial band is not a weaker belief, it is an
# unstated one.
BAND_ANCHOR=""
BAND_VOLATILITY_BPS=""
BAND_WINDOW_SLOTS=""
BAND_PLAUSIBLE_HALF_WIDTHS=""
BAND_MAX_CELL_SHARE_BPS=""
COEFFICIENTS="1,0,1,0"
CUT_DENOMINATOR="100"

usage() {
    cat <<'EOF'
Usage:
  stage-devnet-sponsored-market-open.sh --work ABSOLUTE_NEW_DIR \
    --plan ABSOLUTE_CHECKED_PLAN_JSON --registry-program-id PUBKEY \
    --direct-fee-recipient PUBKEY --direct-fee-basis-points N \
    --window-start UNIX_SECONDS [--rpc-url URL] [--i-mean-unfillable] \
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

--band-anchor, --band-volatility-bps, --band-window-slots,
--band-plausible-half-widths and --band-max-cell-share-bps state the author's
BELIEF and are required together or not at all.  The gated product entrance
measures the partition for degeneracy against that belief, so a Pyth market
without one refuses by name rather than founding a market whose answer is
already known.

--direct-fee-basis-points has no default and must be stated.  The rate is
sealed into the Market at founding and cannot be changed afterwards.

PASS 50.  Not 0.  THIS IS NOW A GATE, NOT ADVICE: any other rate refuses unless
you add --i-mean-unfillable to say that an unfillable market is what you meant.
The gate exists because this paragraph did not.  It used to say the opposite --
"pass 0 for a market that must trade" -- and that sentence founded three markets
that can never take a fill; cohort-11's SOL/USD market was then founded at 30 on
2026-09-01, the day AFTER this paragraph was corrected, by an operator reading
this very page.  A ZERO-FEE MARKET CANNOT BE SET UP AT ALL:
direct_token_setup_v1 is the sole creator of the seller's and the venue's
Direct token accounts, so it precedes every Hot fill, and it refuses unless
the Market's finalized Direct config reads exactly
DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1 -- documented in the codec as "the one
Direct fee rate admitted by this setup release", and equal to 50.  Devnet
market19 6WZXJ7jB was founded at 0 on 2026-08-30 and is permanently unfillable
for that reason alone.

The compute ceiling is real but it is a property of the FILL, not the rate.
The InlineOrdinary program (Lean-authored, emitted into
crates/dclutch-trading/src/generated_ordinary_v3.rs) computes
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
        --i-mean-unfillable) MEAN_UNFILLABLE=1; shift ;;
        --window-start) WINDOW_START="${2:?--window-start needs a value}"; shift 2 ;;
        --cuts) CUTS="${2:?--cuts needs a value}"; shift 2 ;;
        --coefficients) COEFFICIENTS="${2:?--coefficients needs a value}"; shift 2 ;;
        --cut-denominator) CUT_DENOMINATOR="${2:?--cut-denominator needs a value}"; shift 2 ;;
        --band-anchor) BAND_ANCHOR="${2:?--band-anchor needs a value}"; shift 2 ;;
        --band-volatility-bps) BAND_VOLATILITY_BPS="${2:?--band-volatility-bps needs a value}"; shift 2 ;;
        --band-window-slots) BAND_WINDOW_SLOTS="${2:?--band-window-slots needs a value}"; shift 2 ;;
        --band-plausible-half-widths) BAND_PLAUSIBLE_HALF_WIDTHS="${2:?--band-plausible-half-widths needs a value}"; shift 2 ;;
        --band-max-cell-share-bps) BAND_MAX_CELL_SHARE_BPS="${2:?--band-max-cell-share-bps needs a value}"; shift 2 ;;
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
# (crates/dclutch-trading/src/successor.rs) refuses the same rate at config
# construction, and the authored transition refuses it again as a relation, so a
# founding that avoids this script is bounded too. It stays because refusing at
# the operator's own console is cheaper than refusing after a staged build.
if [ "$FEE_BPS" -gt 500 ]; then
    echo "--direct-fee-basis-points exceeds MAX_FEE_BPS=500 (decision 0014 D2)" >&2
    exit 2
fi
# A WALL, because this one has been a landmine four times.
#
# `direct_token_setup_v1` is the sole creator of the seller's and the venue's
# Direct token accounts, so it precedes every fill, and it refuses unless the
# Market's finalized Direct config reads exactly
# DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1 = 50
# (programs/dclutch-trading-sbf/src/direct_token_setup_v1.rs:477,
#  crates/dclutch-trading/src/token_setup_v1.rs:25). The config is a
# finalized Registry record, so the rate is sealed at founding and no later
# artifact -- no release, no redeploy, no upgrade -- can change it.
#
# The paragraph above this gate has said PASS 50 since 2026-08-31 and prose does
# not refuse. Devnet market19 6WZXJ7jB was founded at 0, two more followed, and
# cohort-11's SOL/USD market was founded at 30 on 2026-09-01 -- the day AFTER
# the warning was written -- and every stage before the fill is indifferent to
# the rate, so all four founded, opened, activated and admitted successfully and
# are permanently unfillable.
#
# Drawing such a market on purpose stays possible, because the world is allowed
# to contain markets this release cannot fill; it just has to be said out loud.
if [ "$FEE_BPS" != "50" ] && [ "$MEAN_UNFILLABLE" != "1" ]; then
    cat >&2 <<UNFILLABLE
--direct-fee-basis-points $FEE_BPS founds a PERMANENTLY UNFILLABLE market.
direct_token_setup_v1 creates the seller's and venue's Direct token accounts
before any fill and refuses unless the Market's finalized Direct config reads
exactly 50; that config is a finalized Registry record, so the rate is sealed
at founding and no release, redeploy or upgrade can change it. Four devnet
markets have already been founded this way and can never take a trade.
Pass --direct-fee-basis-points 50, or add --i-mean-unfillable to state that an
unfillable market is what you meant.
UNFILLABLE
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

# All five band parts or none. The compiler refuses a partial band by name; this
# refuses it before a socket is opened, naming the same five.
BAND_SET=0
for part in "$BAND_ANCHOR" "$BAND_VOLATILITY_BPS" "$BAND_WINDOW_SLOTS" \
            "$BAND_PLAUSIBLE_HALF_WIDTHS" "$BAND_MAX_CELL_SHARE_BPS"; do
    [ -n "$part" ] && BAND_SET=$((BAND_SET + 1))
done
BAND_FLAGS=""
if [ "$BAND_SET" = 5 ]; then
    BAND_FLAGS="--band-anchor $BAND_ANCHOR --band-volatility-bps $BAND_VOLATILITY_BPS --band-window-slots $BAND_WINDOW_SLOTS --band-plausible-half-widths $BAND_PLAUSIBLE_HALF_WIDTHS --band-max-cell-share-bps $BAND_MAX_CELL_SHARE_BPS"
elif [ "$BAND_SET" != 0 ]; then
    echo "an incomplete founding band was stated: --band-anchor, --band-volatility-bps, --band-window-slots, --band-plausible-half-widths and --band-max-cell-share-bps are required together, because the band is the author's belief about the outcome and no part of it has a default" >&2
    exit 2
fi

# The price reader makes exactly the bounded public reads it documents and
# writes a fresh 134-byte account body. It never contacts Hermes/Price Service.
"$PRICE_READER" --url "$DEVNET_RPC" --out "$WORK/sol-usd.price-update-v2"

# THE BINARY IS BUILT AND THEN CALLED, NEVER `cargo run`. `cargo run` echoes the
# command line it is about to exec -- `Running \`target/debug/... --rpc-url
# https://.../?api-key=<KEY> ...\`` -- so a keyed endpoint reaches every log an
# operator tees, which is the same credential leak `674a7873e` closed for the
# file at rest and did not close for the run. Measured 2026-09-03 by COHORT-14C:
# staging market C put the live Helius key into its own log on line one. The
# build carries no credential on its command line, so this split is the fix.
# BUILT FROM $REPO, NOT FROM WHEREVER THE CALLER STOOD. `--manifest-path` does
# not move cargo: rustup resolves `rust-toolchain.toml` from the CURRENT
# WORKING DIRECTORY, so a caller whose shell sat in another repository builds
# this tree's driver with that repository's toolchain -- silently, and with a
# cold target directory to prove it. Cohort-17 measured it: the Direct founding
# was launched from a sibling checkout and spent forty minutes recompiling the
# whole successor package under `nightly` while this tree pins 1.97.1. The
# toolchain is a property of the repository being built.
( cd "$REPO" && cargo build --locked --manifest-path "$BOOT/Cargo.toml" ) >&2
BOOT_BIN="${CARGO_TARGET_DIR:-$REPO/target}/debug/dclutch-local-successor-bootstrap"
[ -x "$BOOT_BIN" ] || { echo "the successor bootstrap binary is missing at $BOOT_BIN" >&2; exit 2; }

# The compiler is the semantic owner of the sponsored provider release, four
# outcomes, range partition, permanent program-plan checks, and Direct graph.
#
# ITS STDERR IS CAPTURED AND THEN SHOWN, never swallowed. The compiler prints
# the founding-terms line -- the reserve it committed, the budget, the derived
# payout scale and the exact complete-set count -- and the staging manifest
# below quotes that line VERBATIM rather than restating the derivation, so the
# scale rule keeps the one author it has in the compiler. A failed compile
# still reaches the operator's terminal because the capture is replayed before
# the status is honoured.
set +e
"$BOOT_BIN" devnet-sponsored-market \
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
    $BAND_FLAGS \
    > "$WORK/market.json" 2> "$WORK/compile.stderr"
COMPILE_STATUS=$?
set -e
cat "$WORK/compile.stderr" >&2
[ "$COMPILE_STATUS" -eq 0 ] || exit "$COMPILE_STATUS"

# The founding-terms line, selected by its own marker rather than by position.
# Its absence is a refusal: a market staged by a compiler that did not state
# what reserve it committed is a market whose terms surface would be silent
# about the one number decision 0025's rounding boundary is about.
FOUNDING_TERMS="$(grep -m1 -e 'founding-reserve-terms-v1:' "$WORK/compile.stderr" || true)"
if [ -z "$FOUNDING_TERMS" ]; then
    echo 'the market compiler stated no founding-reserve-terms-v1 line; refusing to stage a founding whose reserve nothing discloses' >&2
    exit 2
fi

# THE ENDPOINT IS NOT WRITTEN TO DISK WHEN IT CARRIES A CREDENTIAL. A keyed RPC
# endpoint holds its credential in the query string (or in userinfo), and this
# generator used to bake `--rpc-url` into the script it writes -- so a job
# directory ended up holding the key at rest, and cohort-14 hand-edited its copy
# afterwards, which is how its HOLD_STATE came to say the file never held it.
# The staging manifest below learned this on 2026-08-30 and redacts its origin;
# the executable did not. Same lesson, second file.
case "$DEVNET_RPC" in
    *\?*|*@*)
        RPC_ORIGIN_LINE=': "${DCLUTCH_RPC_URL:?the staged endpoint carries a credential and was deliberately NOT written to this file; export DCLUTCH_RPC_URL}"'
        ;;
    *)
        RPC_ORIGIN_LINE="DCLUTCH_RPC_URL=\"\${DCLUTCH_RPC_URL:-$DEVNET_RPC}\""
        ;;
esac

# THE JOB DIRECTORY IS THE UNIT, AND A PATH INTO A BUILD SCRATCH IS NOT IN IT.
#
# This generator used to resolve the driver as `<successor>/target/debug/...` from its
# own location and freeze that absolute path into the script it wrote. When the
# generator was invoked from a detached worktree under /private/tmp -- which is
# how every cohort has run it -- the emitted job directory named a scratch it
# did not own, and deleting that scratch stranded the cohort. Cohort-15 has
# sixteen such references.
#
# So the driver is BUILT ONCE HERE and COPIED IN, and the wrapper resolves it
# from its own location. The job directory is then self-contained and, because
# nothing in the wrapper is absolute, relocatable as one tree.
mkdir -m 700 "$WORK/bin"
( cd "$REPO" && cargo build --locked --manifest-path "$BOOT/Cargo.toml" ) >&2
DRIVER_BUILD="${CARGO_TARGET_DIR:-$REPO/target}/debug/dclutch-local-successor-bootstrap"
cp "$DRIVER_BUILD" \
    "$WORK/bin/dclutch-local-successor-bootstrap"
cmp -s "$DRIVER_BUILD" \
    "$WORK/bin/dclutch-local-successor-bootstrap" \
    || { echo "copied driver differs from the build it was taken from" >&2; exit 1; }
chmod 700 "$WORK/bin/dclutch-local-successor-bootstrap"
shasum -a 256 "$WORK/bin/dclutch-local-successor-bootstrap" \
    | cut -d' ' -f1 > "$WORK/bin/dclutch-local-successor-bootstrap.sha256"
# The plan is the other input the wrapper reads, and it is equally not the
# generator's to leave outside.
cp "$PLAN" "$WORK/plan.json"
cmp -s "$PLAN" "$WORK/plan.json" \
    || { echo "copied plan differs from its admitted input" >&2; exit 1; }

# This only makes the remaining authority explicit. It invokes the existing
# campaign driver after the operator supplies paths/public identities; staging
# never opens a key file or invokes this wrapper.
cat > "$WORK/open-market.execute.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
HERE="\$(cd "\$(dirname "\$0")" && pwd -P)"
$RPC_ORIGIN_LINE
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
# The driver was built and copied in at staging time, so this file neither
# builds nor names anything outside its own directory. It used to \`cargo build\`
# here -- which was itself a fix, because \`cargo run\` echoes its exec line and
# \$DCLUTCH_RPC_URL is expanded by the shell BEFORE cargo sees it, so the
# credential this file is careful not to hold would have been printed by every
# run of it. Building at staging keeps that property and adds the one this file
# lacked: it does not depend on a source tree still being there.
"\$HERE/bin/dclutch-local-successor-bootstrap" campaign --founding-only \\
  --rpc-url "\$DCLUTCH_RPC_URL" --i-mean-devnet '$DEVNET_GENESIS' \\
  --plan "\$HERE/plan.json" --market "\$HERE/market.json" \\
  --evidence "\$HERE/campaign-open.json" \\
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

# The VALUE TEST, run every time rather than trusted: the emitted script must
# not contain a credential-bearing endpoint. A generator that merely intends not
# to write a secret and a generator that checks log identically.
case "$DEVNET_RPC" in
    *\?*|*@*)
        if grep -qF -- "$DEVNET_RPC" "$WORK/open-market.execute.sh"; then
            echo 'the generated open-market.execute.sh holds the endpoint credential; refusing to leave it on disk' >&2
            rm -f "$WORK/open-market.execute.sh"
            exit 2
        fi
        ;;
esac

# THE SECOND VALUE TEST, and the one cohort-15 needed: the emitted script must
# name no absolute path at all. Every input it reads is beside it, resolved from
# its own location, so the job directory can be moved, archived or handed over
# and still run -- and no build scratch can strand it by being deleted.
if ! python3 - "$WORK/open-market.execute.sh" <<'SELFCONTAINED'
import pathlib
import re
import sys

script = pathlib.Path(sys.argv[1])
# `/dev/null` is a kernel device, not an input this job directory has to carry.
# Nothing else is exempt: a real filesystem path is exactly what strands a
# cohort when the tree that held it is deleted.
ALLOWED = {"/dev/null"}
# `://` is a URL authority, not a filesystem root; the endpoint is separately
# refused by the credential check above.
offenders = []
for number, line in enumerate(script.read_text().splitlines(), 1):
    if number == 1 and line.startswith("#!"):
        continue
    for match in re.finditer(r"""(?<![\w$:/])/[A-Za-z0-9._/-]+""", line):
        if match.group(0) in ALLOWED:
            continue
        offenders.append(f"  line {number}: {match.group(0)}")
if offenders:
    print("the generated wrapper names absolute paths:", file=sys.stderr)
    print("\n".join(offenders), file=sys.stderr)
    raise SystemExit(1)
SELFCONTAINED
then
    echo 'refusing to leave a job directory whose script depends on a path outside it' >&2
    rm -f "$WORK/open-market.execute.sh"
    exit 2
fi

python3 - "$WORK/market-open-staging.json" "$WORK/market.json" "$PLAN" "$REGISTRY" "$FEE_RECIPIENT" "$DEVNET_RPC" "$DEVNET_GENESIS" "$PRICE_ACCOUNT" "$FEE_BPS" "$FOUNDING_TERMS" <<'PY'
import json, sys
out, market_path, plan, registry, recipient, rpc, genesis, price_account, fee_bps, founding_terms = sys.argv[1:]


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
    # THE FOUNDING RESERVE, read off the compiled input, and the compiler's own
    # sentence about where that number came from quoted verbatim. Cohort-16's
    # first founding refused after 140 published transactions because the
    # reserve's lower half was not an exact multiple of the market's derived
    # payout scale, and no surface an operator read before authorizing said
    # what either number was. The rule stays authored once, in the compiler;
    # this only shows it.
    'foundingCollateralReserveAtoms': int(market['initial_collateral_atoms']),
    'foundingReserveTerms': founding_terms,
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

printf '%s\n' "$FOUNDING_TERMS"
printf 'staged sponsored devnet flagship MarketRunInput at %s/market.json\n' "$WORK"
printf 'no transaction was submitted; the canonical post-open capture will be %s/campaign-open.json\n' "$WORK"
