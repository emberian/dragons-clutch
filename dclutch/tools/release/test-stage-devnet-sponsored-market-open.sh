#!/usr/bin/env bash
# Refusal tests for the sponsored-market-open stager and the wrapper it emits.
#
# Two irreversible founding parameters are covered, both of which killed a live
# devnet market: the Direct fee rate and the founder identity (an identity whose
# secret nobody holds strands the collateral and blocks retirement forever --
# decision 0015 section 8).
#
# THE FEE RATE'S CAUSE IS NOT WHAT THIS HEADER USED TO SAY. It said "a nonzero
# rate founds a market that cannot trade", pointing at the 115,003 CU a
# fee-bearing fill is over the ceiling. That is a real bound and it is not this
# one, and reading it that way is how a market got founded at 0 and another at
# 30. The permanent killer is inequality with FIFTY:
# `direct_token_setup_v1` creates the seller's and venue's Direct token accounts
# before any fill and refuses unless the finalized Direct config reads exactly
# DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1 = 50, so 0 is as fatal as 30 and both
# are fatal FOREVER -- the config is a finalized Registry record. The CU ceiling
# is then a bound on the fill SIZE at 50, not on the rate: gross collateral of
# 1..=199 atoms floors the fee to zero and takes the one-CPI branch.
#
# Every case below runs before the price reader and before cargo: no network,
# no build, no key. The wrapper cases extract the emitted script and run it with
# `cargo` and `solana-keygen` stubbed, so the guards execute rather than being
# grepped for.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
STAGER="$ROOT/tools/release/stage-devnet-sponsored-market-open.sh"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/bin"
printf '%s\n' '#!/usr/bin/env bash' 'echo "cargo stub: $*"' > "$WORK/bin/cargo"
printf '%s\n' '#!/usr/bin/env bash' '[ "$1" = pubkey ] || { echo "unexpected solana-keygen $*" >&2; exit 3; }' 'head -n 1 "$2"' > "$WORK/bin/solana-keygen"
chmod +x "$WORK/bin"/*
printf '{}\n' > "$WORK/plan.json"
# THE DRIVER IS STUBBED TOO, and until 2026-09-03 it was not -- so case 10
# below, "the held identity is what reaches the driver", could not reach it.
# The wrapper invokes the driver by path, so no PATH stub substitutes for it:
# the case exited 127 on a missing file, which is a test of nothing. It was
# invisible because the RPC origin defect above fired four cases earlier.
#
# It now lives in `bin/` BESIDE the wrapper rather than in a build scratch the
# wrapper reached into, which is the whole of case 14.
printf '%s\n' '#!/usr/bin/env bash' 'echo "driver stub: $*"' \
  > "$WORK/bin/dclutch-local-successor-bootstrap"
chmod +x "$WORK/bin/dclutch-local-successor-bootstrap"
FOUNDER=FounderPub1111111111111111111111111111111111
OTHER=OtherPub111111111111111111111111111111111111
printf '%s\n' "$FOUNDER" > "$WORK/founder.json"

fail() { echo "FAIL: $1" >&2; exit 1; }

# `run_stager <expect-status> <expect-substring> -- args...` -- the stager's own
# argument gate. Every invocation names a RELATIVE --work so that a run which
# gets past the fee gate stops at "--work must be absolute" instead of doing
# work: that message is the marker for "the fee gate let this through".
run_stager() {
    local want_status="$1" want_text="$2"; shift 3
    local out status=0
    out="$(PATH="$WORK/bin:$PATH" "$STAGER" "$@" 2>&1)" || status=$?
    [ "$status" = "$want_status" ] || fail "expected exit $want_status, got $status, for: $* (output: $out)"
    case "$out" in *"$want_text"*) ;; *) fail "expected '$want_text' in output of: $* (output: $out)" ;; esac
}

COMMON=(--work relative-dir --plan "$WORK/plan.json" --registry-program-id R --direct-fee-recipient F --window-start 1800000000)

# 1. The rate must be stated. This is the new refusal.
run_stager 2 '--direct-fee-basis-points is required' -- "${COMMON[@]}"
# 2. and 3. It must be a plain decimal within decision 0014 D2's band.
run_stager 2 '--direct-fee-basis-points must be a plain decimal count' -- "${COMMON[@]}" --direct-fee-basis-points 5x
run_stager 2 'exceeds MAX_FEE_BPS=500' -- "${COMMON[@]}" --direct-fee-basis-points 501
# 4. 50 is the ONLY rate the deployed setup release can fill, so it is the only
#    one that passes the gate on its own. `direct_token_setup_v1` creates the
#    seller's and venue's Direct token accounts before any fill and refuses
#    anything else; the config is a finalized Registry record, so the rate is
#    sealed at founding forever.
run_stager 2 '--work must be absolute' -- "${COMMON[@]}" --direct-fee-basis-points 50
# 5. Every other rate in the band is now a REFUSAL, not a pass. These two lines
#    asserted the opposite until 2026-09-02, which is how four devnet markets
#    were founded permanently unfillable -- market19 6WZXJ7jB at 0, and
#    cohort-11's SOL/USD at 30 the day after the prose warning was written.
run_stager 2 'PERMANENTLY UNFILLABLE' -- "${COMMON[@]}" --direct-fee-basis-points 0
run_stager 2 'PERMANENTLY UNFILLABLE' -- "${COMMON[@]}" --direct-fee-basis-points 30
run_stager 2 'PERMANENTLY UNFILLABLE' -- "${COMMON[@]}" --direct-fee-basis-points 500
# 6. The refusal names the remedy, both halves of it.
run_stager 2 '--direct-fee-basis-points 50' -- "${COMMON[@]}" --direct-fee-basis-points 30
run_stager 2 '--i-mean-unfillable' -- "${COMMON[@]}" --direct-fee-basis-points 30
# 7. An unfillable market may still be drawn on purpose, said out loud. The
#    world is allowed to contain markets this release cannot fill.
run_stager 2 '--work must be absolute' -- "${COMMON[@]}" --direct-fee-basis-points 30 --i-mean-unfillable
run_stager 2 '--work must be absolute' -- "${COMMON[@]}" --direct-fee-basis-points 0 --i-mean-unfillable

# The red controls below run the LAST revision of this stager that carried
# neither guard, not simply HEAD -- a control pinned to HEAD stops discriminating
# the moment the fix lands, which is the shape of a gate that cannot fail.
# `git log -S` finds that revision by content, so a rebase cannot stale it.
find_pre_guard_stager() {
    local rel=tools/release/stage-devnet-sponsored-market-open.sh out=$1 rev
    git -C "$ROOT" rev-parse --verify --quiet HEAD >/dev/null || return 1
    for rev in $(git -C "$ROOT" log --format=%H -n 40 -- "$rel" 2>/dev/null); do
        git -C "$ROOT" show "$rev:$rel" > "$out" 2>/dev/null || continue
        if ! grep -q 'DCLUTCH_FOUNDING_FOUNDER_KEYPAIR' "$out" \
           && ! grep -q 'direct-fee-basis-points is required' "$out"; then
            echo "$rev"; return 0
        fi
    done
    rm -f "$out"; return 1
}

# 5. THE FEE GATE IS PROVEN RED there: the same argument vector, with no rate,
#    reached the --work check. If this stops firing, case 1 tests nothing.
if PRE_GUARD_REV="$(find_pre_guard_stager "$WORK/prefix-stager.sh")"; then
    chmod +x "$WORK/prefix-stager.sh"
    prior="$(PATH="$WORK/bin:$PATH" bash "$WORK/prefix-stager.sh" "${COMMON[@]}" 2>&1 || true)"
    case "$prior" in
        *'--work must be absolute'*)
            echo "red control: ${PRE_GUARD_REV:0:8} accepted an unstated fee rate" ;;
        *) fail "fee red control did not reproduce at ${PRE_GUARD_REV:0:8}: $prior" ;;
    esac
else
    echo 'note: no pre-guard revision of the stager is reachable; red controls skipped'
fi

# THE WRAPPER IS EMITTED BY THE STAGER'S OWN EMITTER, not by a second reading of
# it. This used to `sed` the heredoc body out and unescape one level of quoting,
# which is a second interpreter of the stager's escaping -- and it broke the
# moment the stager gained a line the heredoc INTERPOLATES rather than escapes:
# `$RPC_ORIGIN_LINE`, chosen by the `case` above the heredoc so a credential-
# bearing endpoint is never written to disk. The unescape cannot evaluate a
# choice, so it produced a wrapper whose line 3 was the literal text
# `$RPC_ORIGIN_LINE` -- unbound under the wrapper's own `set -u`, so every guard
# below reported that instead of what it tests. Running the stager's own two
# blocks costs nothing and has no second author.
export BOOT="$WORK" PLAN="$WORK/plan.json" DEVNET_RPC=https://example.invalid DEVNET_GENESIS=G
{
    sed -n '/^# THE ENDPOINT IS NOT WRITTEN TO DISK/,/^esac$/p' "$STAGER"
    sed -n '/^cat > "\$WORK\/open-market.execute.sh" <<EOF$/,/^EOF$/p' "$STAGER"
    sed -n '/^# THE SECOND VALUE TEST/,/^fi$/p' "$STAGER"
} > "$WORK/emit-wrapper.sh"
grep -q '^ *RPC_ORIGIN_LINE=' "$WORK/emit-wrapper.sh" || fail 'wrapper emitter extraction found no RPC origin choice'
grep -q 'DCLUTCH_FOUNDING_FOUNDER_KEYPAIR' "$WORK/emit-wrapper.sh" || fail 'wrapper emitter extraction found no founder guard'
WORK="$WORK" bash "$WORK/emit-wrapper.sh" || fail 'the stager emitter did not run'
mv "$WORK/open-market.execute.sh" "$WORK/wrapper.sh"
bash -n "$WORK/wrapper.sh" || fail 'emitted wrapper is not valid bash'
grep -q 'DCLUTCH_FOUNDING_FOUNDER_KEYPAIR' "$WORK/wrapper.sh" || fail 'emitted wrapper carries no founder guard'

run_wrapper() {
    local want_status="$1" want_text="$2"; shift 3
    local out status=0
    out="$(env "$@" PATH="$WORK/bin:$PATH" bash "$WORK/wrapper.sh" 2>&1)" || status=$?
    [ "$status" = "$want_status" ] || fail "wrapper: expected exit $want_status, got $status (output: $out)"
    case "$out" in *"$want_text"*) ;; *) fail "wrapper: expected '$want_text' (output: $out)" ;; esac
}

KEYS=(DCLUTCH_AUTHORIZE_MARKET_OPEN=YES
      DCLUTCH_CAMPAIGN_PAYER_KEYPAIR=/k/a DCLUTCH_COLLATERAL_MINT_KEYPAIR=/k/b
      DCLUTCH_COLLATERAL_WALLET_KEYPAIR=/k/c DCLUTCH_FOUNDING_BENEFICIARY_KEYPAIR=/k/d
      DCLUTCH_FOUNDING_PROJECTION_WITNESS_KEYPAIR=/k/e DCLUTCH_FOUNDING_SOURCE_FUNDER_KEYPAIR=/k/f)

# 6. A bare public founder is no longer accepted at all.
run_wrapper 1 'absolute founder keypair path required' -- "${KEYS[@]}" "DCLUTCH_FOUNDING_FOUNDER=$FOUNDER" "DCLUTCH_SUBSTITUTED_FOUNDER=$OTHER"
# 7. The path must be absolute and must exist.
run_wrapper 2 'must be absolute' -- "${KEYS[@]}" DCLUTCH_FOUNDING_FOUNDER_KEYPAIR=founder.json "DCLUTCH_SUBSTITUTED_FOUNDER=$OTHER"
run_wrapper 2 'existing regular non-symlink file' -- "${KEYS[@]}" "DCLUTCH_FOUNDING_FOUNDER_KEYPAIR=$WORK/absent.json" "DCLUTCH_SUBSTITUTED_FOUNDER=$OTHER"
# 8. A stated identity that disagrees with the file is named on both sides.
run_wrapper 2 "names $OTHER but the keypair file holds $FOUNDER" -- "${KEYS[@]}" \
    "DCLUTCH_FOUNDING_FOUNDER_KEYPAIR=$WORK/founder.json" "DCLUTCH_FOUNDING_FOUNDER=$OTHER" "DCLUTCH_SUBSTITUTED_FOUNDER=$OTHER"
# 9. The hostile probe identity must stay distinct from the founder.
run_wrapper 2 'must be a DISTINCT identity' -- "${KEYS[@]}" \
    "DCLUTCH_FOUNDING_FOUNDER_KEYPAIR=$WORK/founder.json" "DCLUTCH_SUBSTITUTED_FOUNDER=$FOUNDER"
# 10. The held identity is what reaches the driver -- the file, not the operator's word.
run_wrapper 0 "--founding-founder $FOUNDER" -- "${KEYS[@]}" \
    "DCLUTCH_FOUNDING_FOUNDER_KEYPAIR=$WORK/founder.json" "DCLUTCH_SUBSTITUTED_FOUNDER=$OTHER"
# 11. Without solana-keygen the wrapper refuses rather than guessing.
out=0
env "${KEYS[@]}" "DCLUTCH_FOUNDING_FOUNDER_KEYPAIR=$WORK/founder.json" \
    "DCLUTCH_SUBSTITUTED_FOUNDER=$OTHER" PATH="/usr/bin:/bin" \
    bash "$WORK/wrapper.sh" >/dev/null 2>&1 || out=$?
[ "$out" = 2 ] || fail "expected exit 2 with no solana-keygen on PATH, got $out"

# 12. THE FOUNDER GUARD IS PROVEN RED at the same pinned revision: with no
#     founder keypair at all, its wrapper reached the driver and would have
#     founded against an identity nobody holds.
if [ -f "$WORK/prefix-stager.sh" ]; then
    sed -n '/^cat > "\$WORK\/open-market.execute.sh" <<EOF$/,/^EOF$/p' "$WORK/prefix-stager.sh" \
      | sed '1d;$d' | sed -e 's/\\\$/$/g' -e 's/\\\\$/\\/' > "$WORK/prior-wrapper.sh"
    prior=0
    prior_out="$(env "${KEYS[@]}" "DCLUTCH_FOUNDING_FOUNDER=$FOUNDER" "DCLUTCH_SUBSTITUTED_FOUNDER=$OTHER" \
        PATH="$WORK/bin:$PATH" bash "$WORK/prior-wrapper.sh" 2>&1)" || prior=$?
    case "$prior_out" in
        *"--founding-founder $FOUNDER"*)
            [ "$prior" = 0 ] || fail "founder red control ran but exited $prior"
            echo "red control: ${PRE_GUARD_REV:0:8} founded against a bare public founder" ;;
        *) fail "founder red control did not reproduce at ${PRE_GUARD_REV:0:8}: $prior_out" ;;
    esac
fi

# 13. THE STAGING MANIFEST MUST NOT CARRY THE CREDENTIAL. A keyed endpoint puts
#     its secret in the query string, and this manifest is an artifact people
#     copy around. The raw key reached a real one on 2026-08-30.
cat > "$WORK/redaction-check.py" <<'CHECK'
import re, sys, pathlib
from urllib.parse import urlsplit
src = pathlib.Path(sys.argv[1]).read_text()
if "'rpcUrl'" in src:
    raise SystemExit("staging manifest still emits a raw rpcUrl field")
if "rpcOriginRedacted" not in src:
    raise SystemExit("staging manifest does not emit rpcOriginRedacted")
match = re.search(r"^def redact_origin\(url\):\n(?:[ \t].*\n|\n)+", src, re.M)
if match is None:
    raise SystemExit("stager has no redact_origin helper to exercise")
namespace = {"urlsplit": urlsplit}
exec(match.group(0), namespace)
redact = namespace["redact_origin"]
secret = "THISMUSTNOTAPPEAR"
for url in (
    "https://devnet.helius-rpc.com/?api-key=" + secret,
    "https://api.devnet.solana.com",
    "https://api.devnet.solana.com/",
    "http://127.0.0.1:8899/path",
    "garbage",
):
    out = redact(url)
    if secret in out:
        raise SystemExit("redact_origin leaked the credential for " + url)
    if not out:
        raise SystemExit("redact_origin returned nothing for " + url)
CHECK
python3 "$WORK/redaction-check.py" "$STAGER" || fail 'staging manifest redaction missing or leaking'
echo 'redaction: the staging manifest emits a redacted origin and no raw rpcUrl'

# 14. THE JOB DIRECTORY IS SELF-CONTAINED. The emitter above already ran the
#     stager's own value test -- if the wrapper had named an absolute path, no
#     wrapper would exist and every case since 6 would have failed on a missing
#     file. Say it directly anyway, because a guard that only fires through
#     another case's failure is a guard nobody can read.
grep -q '^HERE=' "$WORK/wrapper.sh" || fail 'emitted wrapper does not resolve its own directory'
grep -q '"\$HERE/bin/dclutch-local-successor-bootstrap"' "$WORK/wrapper.sh" \
    || fail 'emitted wrapper does not invoke the driver copied beside it'
python3 - "$WORK/wrapper.sh" <<'SELFCHECK' || fail 'emitted wrapper names an absolute path'
import pathlib, re, sys
lines = pathlib.Path(sys.argv[1]).read_text().splitlines()
bad = [
    (n, m.group(0))
    for n, line in enumerate(lines, 1)
    if not (n == 1 and line.startswith("#!"))
    for m in re.finditer(r"""(?<![\w$:/])/[A-Za-z0-9._/-]+""", line)
    if m.group(0) != "/dev/null"
]
if bad:
    raise SystemExit(f"absolute paths in the emitted wrapper: {bad}")
SELFCHECK
#     THE GUARD IS PROVEN RED against a wrapper that names one, so the check
#     above is not passing because the regex matches nothing.
sed 's#"\$HERE/bin/dclutch#"/private/tmp/scratch/bin/dclutch#' "$WORK/wrapper.sh" \
    > "$WORK/stranded-wrapper.sh"
if python3 - "$WORK/stranded-wrapper.sh" <<'SELFCHECK'
import pathlib, re, sys
lines = pathlib.Path(sys.argv[1]).read_text().splitlines()
bad = [
    (n, m.group(0))
    for n, line in enumerate(lines, 1)
    if not (n == 1 and line.startswith("#!"))
    for m in re.finditer(r"""(?<![\w$:/])/[A-Za-z0-9._/-]+""", line)
    if m.group(0) != "/dev/null"
]
if bad:
    raise SystemExit(f"absolute paths in the emitted wrapper: {bad}")
SELFCHECK
then
    fail 'the self-containment check does not fire on a scratch path'
fi
echo 'self-containment: the emitted wrapper names no absolute path, and the check fires when one is injected'

echo 'stage-devnet-sponsored-market-open refusals: PASS (21 cases)'
