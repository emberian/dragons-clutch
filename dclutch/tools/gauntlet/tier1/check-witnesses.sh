#!/usr/bin/env bash
# Evaluate a tier's asserted witnesses against its campaign evidence.
#
# A witness is a value whose provenance is INDEPENDENT of the code under test —
# a Solana runtime limit, an SPL layout, a Lean-emitted vector, a measurement
# with a date and a validator version. Reading a value back out of the code
# under test and asserting it equals itself is not a witness, and this script
# refuses any entry that does not state where its number came from.
#
# This evaluator is SHARED by every tier. Do not fork it; TIERS.md says why.
#
# usage: check-witnesses.sh WITNESSES.json EVIDENCE.json CONTEXT.json
set -euo pipefail

WITNESSES="${1:?witness file}"
EVIDENCE="${2:?campaign evidence}"
PLAN="${3:?bootstrap plan}"

command -v jq >/dev/null 2>&1 || { echo "witnesses: jq not found" >&2; exit 1; }
for file in "$WITNESSES" "$EVIDENCE" "$PLAN"; do
    [ -f "$file" ] || { echo "witnesses: missing $file" >&2; exit 1; }
done

# ------------------------------------------------------------- CU budgets
# One file, one owner. A `cu-budget` witness names its campaign and nothing
# else; every number lives in tools/gauntlet/CU_BUDGETS.json so that a tier
# cannot carry a second, drifting copy of a budget.
GAUNTLET_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
CU_BUDGETS_CANONICAL="$GAUNTLET_ROOT/CU_BUDGETS.json"
CU_BUDGETS="${DCLUTCH_CU_BUDGETS_OVERRIDE:-$CU_BUDGETS_CANONICAL}"
if [ -n "${DCLUTCH_CU_BUDGETS_OVERRIDE:-}" ]; then
    {
        echo "witnesses: !! CU BUDGETS OVERRIDDEN !!"
        echo "witnesses: reading $CU_BUDGETS instead of $CU_BUDGETS_CANONICAL."
        echo "witnesses: this run is a DEMONSTRATION, not a gate. Its budget rows prove nothing."
    } >&2
fi

# Evaluate every budget entry naming one campaign, against this campaign's own
# evidence. Prints one row per entry; returns non-zero if any row is red.
#
# A `stage` budget reads the chain's own inner accounting lines rather than a
# number the campaign asserts: the walker tracks `Program X invoke [n]` /
# `success` / `failed` to recover each `consumed` line's depth, and a stage is
# the n-th depth-2 invocation. That keeps stage budgets free of any program
# address, so they survive a run whose gauntlet-local addresses move.
evaluate_cu_budgets() {
    local campaign="$1" witness_id="$2" rows row verdict entry_id observed budget margin what
    local budget_failures=0

    if [ ! -f "$CU_BUDGETS" ]; then
        echo "witness $witness_id: FAILED — no budgets file at $CU_BUDGETS" >&2
        return 1
    fi

    rows="$(jq -r --arg campaign "$campaign" --slurpfile budgets "$CU_BUDGETS" '
        def cu_stages(lbl):
          [ .transactions[] | select(.label == lbl) | .logs[]? ]
          | reduce .[] as $line ({stack: [], out: []};
              if ($line | test("^Program [1-9A-HJ-NP-Za-km-z]+ invoke \\[[0-9]+\\]$")) then
                .stack += [ ($line
                  | capture("^Program (?<p>[1-9A-HJ-NP-Za-km-z]+) invoke \\[(?<d>[0-9]+)\\]$")
                  | {program: .p, depth: (.d | tonumber)}) ]
              elif ($line | test("^Program [1-9A-HJ-NP-Za-km-z]+ consumed [0-9]+ of [0-9]+ compute units$")) then
                ((.stack | length) as $n
                 | if $n > 0
                   then .out += [ (.stack[$n - 1]
                        + {consumed: ($line | capture("consumed (?<c>[0-9]+) of") | .c | tonumber)}) ]
                   else . end)
              elif ($line | test("^Program [1-9A-HJ-NP-Za-km-z]+ (success|failed)")) then
                .stack |= .[0:-1]
              else . end)
          | .out | map(select(.depth == 2));

        . as $evidence
        | $budgets[0] as $doc
        | ($doc.ceiling.compute_units) as $ceiling
        | [ $doc.budgets[] | select(.campaign == $campaign) ] as $entries
        | if ($entries | length) == 0 then
            ["NOCAMPAIGN", $campaign, "-", "-", "-", "no budget entry names this campaign"] | @tsv
          else
            $entries[] as $b
            | ($b.transaction
               + (if $b.scope == "stage"
                  then "  ::  stage \($b.stage.index) \($b.stage.name)"
                  else "" end)) as $what
            | if ($b.enforced | not) then
                ["RECORDED", $b.id, "-", "-", "-", $what] | @tsv
              else
                ( if $b.scope == "stage"
                  then ($evidence | cu_stages($b.transaction))
                       | (if (length >= $b.stage.index)
                          then [ .[$b.stage.index - 1].consumed ] else [] end)
                  elif $b.scope == "transaction"
                  then [ $evidence.transactions[]
                         | select(.label == $b.transaction)
                         | .compute_units_consumed ]
                  else null
                  end ) as $hits
                | if $b.budget != ($b.measured + $b.tolerance) then
                    ["SCHEMA", $b.id, "\($b.measured)", "\($b.budget)", "-",
                     "budget is not measured+tolerance (\($b.measured)+\($b.tolerance)=\($b.measured + $b.tolerance))"] | @tsv
                  elif $hits == null then
                    ["SCHEMA", $b.id, "-", "-", "-", "an enforced budget needs scope transaction or stage, not \($b.scope)"] | @tsv
                  elif $b.budget > $ceiling then
                    ["CEILING", $b.id, "\($b.measured)", "\($b.budget)", "\($ceiling - $b.budget)",
                     "the budget is ABOVE the \($ceiling) ceiling: this transaction has stopped fitting and no tolerance can be written for it"] | @tsv
                  elif ($hits | length) == 0 then
                    ["MISSING", $b.id, "-", "\($b.budget)", "-",
                     "the campaign submitted nothing matching \($what) — a budget that matches nothing overstates coverage"] | @tsv
                  elif ($hits | length) > 1 then
                    ["AMBIGUOUS", $b.id, "-", "\($b.budget)", "-",
                     "\($hits | length) transactions carry this label; a budget must name exactly one"] | @tsv
                  elif $hits[0] == null then
                    ["NOCU", $b.id, "-", "\($b.budget)", "-",
                     "the campaign recorded no compute_units_consumed for \($what)"] | @tsv
                  elif $hits[0] > $b.budget then
                    ["OVER", $b.id, "\($hits[0])", "\($b.budget)", "+\($hits[0] - $b.budget)",
                     "OVER BUDGET by \($hits[0] - $b.budget) CU: \($what)"] | @tsv
                  else
                    ["OK", $b.id, "\($hits[0])", "\($b.budget)", "\($ceiling - $hits[0])", $what] | @tsv
                  end
              end
          end' "$EVIDENCE")" || {
        echo "witness $witness_id: FAILED — the budget evaluator could not read the evidence or the budgets file" >&2
        return 1
    }

    printf '  %-9s %-48s %10s %10s %11s\n' VERDICT BUDGET-ID OBSERVED BUDGET MARGIN
    printf '  (MARGIN: for OK, compute units left to the ceiling; for OVER, how far over budget.)\n'
    while IFS=$'\t' read -r verdict entry_id observed budget margin what; do
        [ -n "$verdict" ] || continue
        case "$verdict" in
            OK|RECORDED)
                printf '  %-9s %-48s %10s %10s %11s\n' \
                    "$verdict" "$entry_id" "$observed" "$budget" "$margin"
                ;;
            *)
                budget_failures=$((budget_failures + 1))
                printf '  %-9s %-48s %10s %10s %11s\n' \
                    "$verdict" "$entry_id" "$observed" "$budget" "$margin" >&2
                printf '            %s\n' "$what" >&2
                ;;
        esac
    done <<< "$rows"

    if [ "$budget_failures" -ne 0 ]; then
        echo "  $budget_failures CU budget row(s) RED for campaign $campaign; see $CU_BUDGETS" >&2
        return 1
    fi
    return 0
}

total=0
failed=0

count="$(jq '.witnesses | length' "$WITNESSES")"
index=0
while [ "$index" -lt "$count" ]; do
    entry="$(jq -c ".witnesses[$index]" "$WITNESSES")"
    index=$((index + 1))
    total=$((total + 1))

    id="$(printf '%s' "$entry" | jq -r '.id')"
    kind="$(printf '%s' "$entry" | jq -r '.kind')"
    query="$(printf '%s' "$entry" | jq -r '.query')"
    expect="$(printf '%s' "$entry" | jq -r '.expect // ""')"
    expect_from="$(printf '%s' "$entry" | jq -r '.expect_from // ""')"
    provenance="$(printf '%s' "$entry" | jq -r '.provenance // ""')"

    if [ -z "$provenance" ] || [ "$provenance" = "null" ]; then
        echo "witness $id: REFUSED — no provenance. A number with no provenance is a mirror." >&2
        failed=$((failed + 1))
        continue
    fi

    # A budget witness carries no number of its own: it names a campaign, and
    # every figure lives in the single owned CU_BUDGETS.json. It expands to one
    # row per budget entry rather than to a single pass/fail, because "the
    # campaign got more expensive" is useless unless it says WHICH transaction.
    if [ "$kind" = "cu-budget" ]; then
        campaign="$(printf '%s' "$entry" | jq -r '.campaign // ""')"
        if [ -z "$campaign" ] || [ "$campaign" = "null" ]; then
            echo "witness $id: REFUSED — a cu-budget witness must name its campaign" >&2
            failed=$((failed + 1))
            continue
        fi
        printf 'witness %-42s CU BUDGETS for campaign %s\n' "$id" "$campaign"
        if evaluate_cu_budgets "$campaign" "$id"; then
            printf 'witness %-42s OK    every budgeted transaction is under budget\n' "$id"
        else
            printf 'witness %-42s FAIL  see the red rows above\n' "$id" >&2
            echo "         provenance: $provenance" >&2
            failed=$((failed + 1))
        fi
        continue
    fi

    case "$kind" in
        evidence-jq) target="$EVIDENCE"; other="$PLAN" ;;
        plan-jq)     target="$PLAN"; other="$EVIDENCE" ;;
        *)
            echo "witness $id: REFUSED — unknown kind '$kind'" >&2
            failed=$((failed + 1))
            continue
            ;;
    esac

    # A cross-check between two independent derivations: the host-side
    # prediction in the bootstrap plan against what the chain actually holds.
    # Neither side reads the other, which is what makes it a witness rather
    # than a mirror.
    if [ -n "$expect_from" ] && [ "$expect_from" != "null" ]; then
        if ! expect="$(jq -r "$expect_from" "$other" 2>&1)"; then
            echo "witness $id: FAILED — cross-file query error: $expect" >&2
            failed=$((failed + 1))
            continue
        fi
    fi

    if ! actual="$(jq -r "$query" "$target" 2>&1)"; then
        echo "witness $id: FAILED — query error: $actual" >&2
        failed=$((failed + 1))
        continue
    fi

    if [ "$actual" = "$expect" ]; then
        printf 'witness %-42s OK    %s\n' "$id" "$expect"
    else
        printf 'witness %-42s FAIL  expected %s, chain says %s\n' "$id" "$expect" "$actual" >&2
        echo "         provenance: $provenance" >&2
        failed=$((failed + 1))
    fi
done

printf '\nwitnesses: %d checked, %d failed\n' "$total" "$failed"
[ "$failed" -eq 0 ]
