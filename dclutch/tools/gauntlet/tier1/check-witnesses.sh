#!/usr/bin/env bash
# Evaluate a tier's asserted witnesses against its campaign evidence.
#
# A witness is a value whose provenance is INDEPENDENT of the code under test —
# a Solana runtime limit, an SPL layout, a Lean-emitted vector, a measurement
# with a date and a validator version. Reading a value back out of the code
# under test and asserting it equals itself is not a witness, and this script
# refuses any entry that does not state where its number came from.
#
# usage: check-witnesses.sh WITNESSES.json EVIDENCE.json PLAN.json
set -euo pipefail

WITNESSES="${1:?witness file}"
EVIDENCE="${2:?campaign evidence}"
PLAN="${3:?bootstrap plan}"

command -v jq >/dev/null 2>&1 || { echo "witnesses: jq not found" >&2; exit 1; }
for file in "$WITNESSES" "$EVIDENCE" "$PLAN"; do
    [ -f "$file" ] || { echo "witnesses: missing $file" >&2; exit 1; }
done

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
