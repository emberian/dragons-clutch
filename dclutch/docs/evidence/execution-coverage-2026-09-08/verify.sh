#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "$0")/../../.." && pwd)"
work="${1:-$(mktemp -d /private/tmp/dclutch-execution-coverage-20260908.XXXXXX)}"
revision="${2:-HEAD}"
resolved="$(git -C "$repo" rev-parse "${revision}^{commit}")"

[ ! -e "$work/out/ledger.json" ] || {
    echo "execution coverage: work directory already contains a ledger: $work" >&2
    exit 64
}
mkdir -p "$work"
"$repo/tools/gate" census --commit "$resolved" --work "$work" --no-tests

"$repo/tools/gate" census observe --work "$work" \
    --bindings "$repo/docs/evidence/census-route-binding-2026-09-08/fold-bindings.json" \
    --programs "$repo/docs/evidence/census-route-binding-2026-09-08/fold-programs.json" \
    --evidence "$repo/docs/evidence/census-route-binding-2026-09-08/normalized-evidence.json"

"$repo/tools/gate" census observe --work "$work" \
    --bindings "$repo/tools/gauntlet/scoring-dealer/bindings.json" \
    --programs "$repo/docs/evidence/execution-coverage-2026-09-08/dealer-programs.json" \
    --evidence "$repo/docs/evidence/execution-coverage-2026-09-08/dealer-evidence.json"

"$repo/tools/gate" census --commit "$resolved" --work "$work" --no-tests

jq -e '
  ([.observations[] | select(
      .outcome == "executed" and
      .evidence_level == "finalized-instruction" and
      (.signature | type == "string" and length > 0) and
      (.slot | type == "number")
    ) | .route] | unique) == [
      "claims/founding_v5::process",
      "trading/scoring_dealer_v1::fill::process_dealer_fill_v1",
      "trading/scoring_dealer_v1::found::process_dealer_found_v1",
      "trading/scoring_dealer_v1::quote::process_dealer_quote_v1",
      "trading/scoring_dealer_v1::withdraw::process_dealer_withdraw_v1"
    ] and
  ([.observations[] | select(.outcome != "executed")] | length) == 0 and
  (.observations | length) == 5
' "$work/out/ledger.json" >/dev/null

python3 "$repo/docs/evidence/execution-coverage-2026-09-08/classify.py" \
    "$repo" "$work/out/inventory.json" "$work/out/ledger.json" \
    "$work/out/HISTORICAL_BINDING_CLAIMS.json"
jq -e '
  .totals == {
    "inventory_routes": 164,
    "inventory_refusals": 456,
    "inventory_unclassified": 0,
    "exact_devnet_accepted_routes": 46,
    "exact_local_validator_accepted_routes": 5,
    "exact_historical_agave_union_routes": 50,
    "historical_success_claim_union_before_dealer": 119,
    "historical_success_claim_union": 123,
    "historical_refusal_only_claims": 0,
    "no_historical_success_claim": 41,
    "local_validator_claim_only_no_exact_checked_evidence": 32,
    "program_test_only_success_claim": 41
  }
' "$work/out/HISTORICAL_BINDING_CLAIMS.json" >/dev/null

printf 'repository=%s\nrevision=%s\nwork=%s\nledger=%s\nreport=%s\nclaims=%s\n' \
    "$repo" "$resolved" "$work" "$work/out/ledger.json" "$work/out/CENSUS.md" \
    "$work/out/HISTORICAL_BINDING_CLAIMS.json"
